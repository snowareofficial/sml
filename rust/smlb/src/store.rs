// SPDX-License-Identifier: MulanPSL-2.0
//! Random-access reader (T3): open a `.smlb` **without decoding the tree**, then fetch one
//! subtree by path. ｜ 随机访问读取（T3）：打开文件**不解整树**，再按路径取一棵子树。
//!
//! # 它解决的问题
//!
//! `load()` 是「整棵树解成 `Value`」：读 100 MB 文档只为一个字段，等于把 100 MB 的节点
//! 全部变成 `Value`（堆上几百 MB）。T3 的判据是「**取单个子树不遍历全树**」，做法是：
//!
//! 1. 打开时只解析头 + 段表 + manifest（`container::open_raw`），`VALUE` 段体保持 `&[u8]`；
//! 2. 按路径下行时，**兄弟节点靠 `subtree_len` 跳过**（[`sml_value::subtree_at`]），
//!    只对命中的那棵子树调用 `decode_node`。
//!
//! 代价是每层的「兄弟比较」仍是 O(兄弟数)（要比 key 字符串）—— 那正是 `DIR`（§19.3）将来
//! 要省掉的那部分；但**「分配量与文档大小无关」**这一条现在就已经成立，也是判据的核心。
//! ｜ Sibling *comparison* is still O(k) per level (that is what `DIR` will shave), but the
//! allocation cost is already independent of document size — which is what the criterion means.
//!
//! # 层链怎么办
//!
//! 有补丁时「当前值」不是 `VALUE` 段体本身。补丁只覆盖**顶层键**（D22），故 [`Store::get`]
//! 的做法是：先看有没有哪一层的补丁覆盖了路径首段 —— 有就在那个**已在内存里的补丁值**上
//! 继续走；没有才回到字节上零拷贝导航。非对象补丁（整值替换）按同样口径处理。
//! ｜ Patches override top-level keys only, so `get` first asks "did any patch touch this key?".
//!
//! # 与 `load()` 的分工
//!
//! | 场景 | 用哪个 |
//! |---|---|
//! | 要整棵树 / 要改它再写回 | [`crate::load`] |
//! | 只要一两个字段（大文件） | [`Store::get`] |
//! | 要逐层历史 | [`Store::history_at`] |
//! | 要段与载荷清单 | [`Store::segments`] / [`Store::payload`] |

use sml_value::{subtree_at, value_from_subtree, Path, Value};

use crate::container::{self, ContainerError, FileInfo, Segment};
use crate::layer::{self, Layer};
use crate::manifest::{Manifest, ManifestPayload};

/// A zero-copy, random-access view over a `.smlb` file. ｜ `.smlb` 文件的零拷贝随机访问视图。
///
/// Borrows the caller's buffer: no `Value` is built at `open`, and no big copy is made.
/// 借用调用方的缓冲区：`open` 时不构造 `Value`，也不复制大块数据。
#[derive(Debug)]
pub struct Store<'a> {
    bytes: &'a [u8],
    info: FileInfo,
    value: &'a [u8],
    strtab: &'a [u8],
    manifest: Option<Manifest>,
    layers: Vec<Layer>,
}

impl<'a> Store<'a> {
    /// Open and verify segment digests (default). ｜ 打开并校验段摘要（默认）。
    ///
    /// Verification costs one hash pass over the `VALUE`/`STRTAB` bodies: CPU O(n), RSS O(1)
    /// — the T3 RSS criterion is about memory, not time, so this stays cheap.
    /// 验摘要要过一遍段体的哈希：CPU O(n)、RSS O(1) —— T3 的判据是内存不是时间，故仍便宜。
    pub fn open(bytes: &'a [u8]) -> Result<Store<'a>, ContainerError> {
        Self::from_parts(container::open_raw(bytes, true)?, bytes)
    }

    /// Open without hashing — for callers that run [`crate::check`] anyway.
    /// 不验摘要地打开 —— 留给反正要跑 [`crate::check`] 的调用方。
    pub fn open_unverified(bytes: &'a [u8]) -> Result<Store<'a>, ContainerError> {
        Self::from_parts(container::open_raw(bytes, false)?, bytes)
    }

    fn from_parts(
        parts: container::RawParts<'a>,
        bytes: &'a [u8],
    ) -> Result<Store<'a>, ContainerError> {
        Ok(Store {
            bytes,
            info: parts.info,
            value: parts.value,
            strtab: parts.strtab,
            manifest: parts.manifest,
            layers: parts.layers,
        })
    }

    /// Parsed header + segment table. ｜ 已解析的头与段表。
    pub fn info(&self) -> &FileInfo {
        &self.info
    }

    /// Segments in table order. ｜ 按表序的段。
    pub fn segments(&self) -> &[Segment] {
        &self.info.segments
    }

    /// The manifest, when the file has one. ｜ manifest（可能没有）。
    pub fn manifest(&self) -> Option<&Manifest> {
        self.manifest.as_ref()
    }

    /// The append-only layer chain, oldest first. ｜ 层链（从旧到新）。
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Raw `VALUE` body (node tree, undecoded). ｜ 原始 `VALUE` 段体（节点树，未解码）。
    pub fn value_body(&self) -> &'a [u8] {
        self.value
    }

    /// Raw `STRTAB` body. ｜ 原始 `STRTAB` 段体。
    pub fn strtab_body(&self) -> &'a [u8] {
        self.strtab
    }

    /// The raw bytes of one subtree — **zero copy**, header included.
    /// 一棵子树的原始字节 —— **零拷贝**，含节点头。
    ///
    /// Siblings are skipped via `subtree_len`; nothing is allocated for them.
    /// 兄弟节点靠 `subtree_len` 跳过，不为它们分配任何东西。
    pub fn subtree_bytes(&self, path: &str) -> Result<&'a [u8], ContainerError> {
        let p = Path::parse(path).map_err(ContainerError::Path)?;
        let sub = subtree_at(self.strtab, self.value, &p)?;
        Ok(sub.bytes)
    }

    /// Fetch one subtree as a `Value`, returning how many nodes the walk looked at.
    /// 取一棵子树解成 `Value`，并返回导航时看过的节点数。
    ///
    /// The counter is the measurable form of “does not traverse the whole tree”: for a
    /// document with 10 000 siblings, fetching one of them must stay in single digits.
    /// 这个计数就是「不遍历全树」的可测形式：一万个兄弟里取一个，计数必须停在个位数。
    pub fn get_with_stats(&self, path: &str) -> Result<(Value, usize), ContainerError> {
        let p = Path::parse(path).map_err(ContainerError::Path)?;

        // 1) 顶层键被补丁覆盖？那就在补丁值上走（补丁本来就在内存里）
        if let Some(first) = p.segments().first() {
            if let sml_value::Seg::Key(k) = first {
                if let Some(v) = self.patch_override(k) {
                    let rest = Path::new(p.segments()[1..].to_vec());
                    let found = if rest.is_empty() {
                        v.clone()
                    } else {
                        v.get_path(&rest)
                            .map_err(ContainerError::Path)?
                            .clone()
                    };
                    return Ok((found, 0));
                }
            }
        }

        // 2) 字节上零拷贝导航，只解命中的那棵子树
        let sub = subtree_at(self.strtab, self.value, &p)?;
        let v = value_from_subtree(self.strtab, sub.bytes)?;
        Ok((v, sub.nodes_visited))
    }

    /// Fetch one subtree as a `Value`. ｜ 取一棵子树解成 `Value`。
    pub fn get(&self, path: &str) -> Result<Value, ContainerError> {
        self.get_with_stats(path).map(|(v, _)| v)
    }

    /// Canonical text of one subtree — the same bytes a signature would cover for that node.
    /// 一棵子树的 canonical 文本 —— 与签名覆盖该节点时的字节同形。
    pub fn text_of(&self, path: &str) -> Result<String, ContainerError> {
        Ok(self.get(path)?.canonical_text()?)
    }

    /// Canonical text of the **whole** document (decodes everything — use `text_of` for one node).
    /// **整份**文档的 canonical 文本（会解整树；只取一个节点请用 `text_of`）。
    pub fn text(&self) -> Result<String, ContainerError> {
        Ok(self.value()?.canonical_text()?)
    }

    /// The current value: base tree + every layer's patch, in order.
    /// 当前值：基础树 + 各层补丁（按顺序）。
    ///
    /// O(document): this is the “give me everything” path. ｜ O(文档)：这是「全都要」的那条路。
    pub fn value(&self) -> Result<Value, ContainerError> {
        let base = Value::from_canonical_blocks(self.strtab, self.value)?;
        if self.layers.is_empty() {
            Ok(base)
        } else {
            Ok(layer::apply_patches(&base, &self.layers)?)
        }
    }

    /// The value **as of** layer `upto` (inclusive); see [`layer::value_at`].
    /// 「到第 `upto` 层为止（含）」的值，见 [`layer::value_at`]。
    pub fn history_at(&self, upto: usize) -> Result<Value, ContainerError> {
        let base = Value::from_canonical_blocks(self.strtab, self.value)?;
        Ok(layer::value_at(self.bytes, &base, upto).map_err(ContainerError::Layer)?)
    }

    /// Payload names declared by the manifest. ｜ manifest 声明的载荷清单。
    pub fn payloads(&self) -> &[ManifestPayload] {
        self.manifest
            .as_ref()
            .map(|m| m.payloads.as_slice())
            .unwrap_or(&[])
    }

    /// Extract one payload by name (zero copy). ｜ 按名字取一段载荷（零拷贝）。
    pub fn payload(&self, name: &str) -> Result<&'a [u8], ContainerError> {
        container::payload(self.bytes, name)
    }

    /// The last layer whose patch overrides top-level key `key`.
    /// 最后一层里覆盖了顶层键 `key` 的补丁值。
    ///
    /// D22: an object patch overrides keys in place; a non-object patch replaces the whole value.
    /// D22：对象补丁原位覆盖键；非对象补丁替换整值。
    fn patch_override(&self, key: &str) -> Option<&Value> {
        for l in self.layers.iter().rev() {
            match &l.patch {
                Some(Value::Object(m)) => {
                    if let Some(v) = m.get(key) {
                        return Some(v);
                    }
                }
                Some(other) => return Some(other),
                None => {}
            }
        }
        None
    }
}
