// SPDX-License-Identifier: MulanPSL-2.0
//! Random-access reader (T3): open a `.ambr` **without decoding the tree**, then fetch one
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

use std::collections::HashMap;

use sml_value::{
    parse_strtab_table, subtree_at_with_table, subtree_range, value_from_subtree_with_table,
    CanonicalError, Path, TreeCursor, Value,
};

use crate::container::{self, ContainerError, FileInfo, FrameEntry, Segment};
use crate::layer::{self, Layer};
use crate::manifest::{Manifest, ManifestPayload};

/// A zero-copy, random-access view over a `.ambr` file. ｜ `.ambr` 文件的零拷贝随机访问视图。
///
/// Borrows the caller's buffer: no `Value` is built at `open`, and no big copy is made.
/// 借用调用方的缓冲区：`open` 时不构造 `Value`，也不复制大块数据。
#[derive(Debug)]
pub struct Store<'a> {
    bytes: &'a [u8],
    info: FileInfo,
    /// Plain `VALUE` body (empty when the tree is framed). ｜ 明文节点树（分帧时为空）。
    value: &'a [u8],
    /// `FRAMES` body when the tree is framed (T5). ｜ 分帧时的 `FRAMES` 段体。
    frames: Option<&'a [u8]>,
    strtab: &'a [u8],
    /// `STRTAB` parsed **on first use**, then cached. ｜ 首次使用时解析、之后缓存的 `STRTAB` 表。
    ///
    /// Why lazy: `open` promises O(1) heap (the T3 RSS criterion); parsing the table eagerly
    /// would allocate O(keys) for callers that never call `get`.
    /// Why cache: `get` used to re-walk the whole table on every call (twice on the framed path);
    /// the table is a pure function of the segment, so per-call O(keys) is pure waste.
    /// ｜ 惰性保住 `open` 的 O(1) 内存；缓存去掉每次 `get` 的 O(键数) 重扫。
    strtab_table: std::sync::OnceLock<StrTab<'a>>,
    manifest: Option<Manifest>,
    layers: Vec<Layer>,
    /// Optional `DIR` body (N05); parsed on first use. ｜ 可选的 `DIR` 段体，首次使用时解析。
    dir: Option<&'a [u8]>,
    dir_index: std::sync::OnceLock<Option<crate::dir::Dir>>,
    /// `key → STRTAB id`, built on the first `DIR` lookup ｜ 首次用 `DIR` 时建的键→id 表。
    key_ids: std::sync::OnceLock<HashMap<&'a str, u32>>,
    /// Optional `TYPED` body (N04) ｜ 可选的 `TYPED` 段体。
    typed: Option<&'a [u8]>,
}

/// Parsed `STRTAB`, cached. Custom `Debug` keeps `Store`'s debug output short.
/// 已解析的 `STRTAB` 缓存；自定义 `Debug` 避免打印整张表。
struct StrTab<'a>(Vec<&'a str>);

impl std::fmt::Debug for StrTab<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StrTab({} entries)", self.0.len())
    }
}

/// A cursor over a `FRAMES` body: **skips without inflating**, inflates only what is read.
/// `FRAMES` 段体上的游标：**跳过不解压**，只解真正读到的字节。
///
/// 一帧缓存（`cache`）：节点树是顺序读的，跨帧的读很少，缓存一帧足够；换帧才解压一次。
/// ｜ A one-frame cache: navigation is sequential, so switching frames is rare.
struct FrameCursor<'a> {
    body: &'a [u8],
    entries: Vec<FrameEntry>,
    len: usize,
    pos: usize,
    cache: Option<(usize, Vec<u8>)>,
}

impl<'a> FrameCursor<'a> {
    fn new(body: &'a [u8]) -> Result<Self, ContainerError> {
        let entries = container::parse_frame_index(body)?;
        let len = entries
            .last()
            .map(|e| (e.uncomp_off + e.uncomp_len) as usize)
            .unwrap_or(0);
        Ok(FrameCursor {
            body,
            entries,
            len,
            pos: 0,
            cache: None,
        })
    }

    /// Make frame `idx` the cached one. ｜ 把第 `idx` 帧设为缓存。
    fn load(&mut self, idx: usize) -> Result<(), ContainerError> {
        let e = self.entries[idx];
        let start = e.comp_off as usize;
        let end = start + e.comp_len as usize;
        let data = container::inflate_frame(&self.body[start..end], e.uncomp_len as usize)?;
        self.cache = Some((idx, data));
        Ok(())
    }

    /// Materialise `[start, end)` — decompressing each overlapping frame **once**.
    /// 物化 `[start, end)`：覆盖该区间的每一帧只解一次。
    fn read_range(&mut self, start: usize, end: usize) -> Result<Vec<u8>, ContainerError> {
        let mut out = Vec::with_capacity(end.saturating_sub(start));
        for i in 0..self.entries.len() {
            let e = self.entries[i];
            let f_start = e.uncomp_off as usize;
            let f_end = f_start + e.uncomp_len as usize;
            if f_end <= start || f_start >= end {
                continue;
            }
            if self.cache.as_ref().map(|(ci, _)| *ci) != Some(i) {
                self.load(i)?;
            }
            let buf = &self.cache.as_ref().expect("just loaded").1;
            let s = start.max(f_start) - f_start;
            let e2 = end.min(f_end) - f_start;
            out.extend_from_slice(&buf[s..e2]);
        }
        Ok(out)
    }

    fn byte_at(&mut self, off: usize) -> Result<u8, CanonicalError> {
        let idx = self
            .entries
            .iter()
            .position(|e| {
                off >= e.uncomp_off as usize && off < (e.uncomp_off + e.uncomp_len) as usize
            })
            .ok_or_else(|| CanonicalError::Source(format!("偏移 {off} 不在任何帧里")))?;
        if self.cache.as_ref().map(|(ci, _)| *ci) != Some(idx) {
            self.load(idx)
                .map_err(|e| CanonicalError::Source(format!("{e}")))?;
        }
        let base = self.entries[idx].uncomp_off as usize;
        let buf = &self.cache.as_ref().expect("loaded").1;
        buf.get(off - base)
            .copied()
            .ok_or_else(|| CanonicalError::Source("帧内偏移越界".to_string()))
    }
}

impl<'a> TreeCursor for FrameCursor<'a> {
    fn pos(&self) -> usize {
        self.pos
    }

    fn remaining(&self) -> usize {
        self.len - self.pos.min(self.len)
    }

    fn read_u8(&mut self) -> Result<u8, CanonicalError> {
        let b = self.byte_at(self.pos)?;
        self.pos += 1;
        Ok(b)
    }

    fn read_u32le(&mut self) -> Result<u32, CanonicalError> {
        let mut raw = [0u8; 4];
        for (i, slot) in raw.iter_mut().enumerate() {
            *slot = self.byte_at(self.pos + i)?;
        }
        self.pos += 4;
        Ok(u32::from_le_bytes(raw))
    }

    fn read_uvarint(&mut self) -> Result<u64, CanonicalError> {
        let start = self.pos;
        let mut result: u64 = 0;
        let mut shift = 0;
        loop {
            if shift > 63 {
                return Err(CanonicalError::VarintTooLong { at: start });
            }
            let byte = self.read_u8()?;
            result |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
    }

    fn skip(&mut self, n: usize) -> Result<(), CanonicalError> {
        if n > self.remaining() {
            return Err(CanonicalError::Truncated { at: self.pos });
        }
        self.pos += n;
        Ok(())
    }
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
            value: match parts.value {
                container::ValueSource::Plain(b) => b,
                container::ValueSource::Framed(_) => &[],
            },
            frames: match parts.value {
                container::ValueSource::Plain(_) => None,
                container::ValueSource::Framed(b) => Some(b),
            },
            strtab: parts.strtab,
            strtab_table: std::sync::OnceLock::new(),
            manifest: parts.manifest,
            layers: parts.layers,
            dir: parts.dir,
            dir_index: std::sync::OnceLock::new(),
            key_ids: std::sync::OnceLock::new(),
            typed: parts.typed,
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

    /// Parsed `STRTAB` table, built **on first use** and then reused.
    /// 已解析的 `STRTAB` 表：**首次使用时**才建，之后复用。
    ///
    /// Cost: one O(keys) pass the first time, O(1) afterwards. Keeping it lazy is what
    /// preserves `Store::open`'s O(1) heap (the T3 RSS criterion).
    /// ｜ 一次 O(键数) 换之后每次 O(1)；惰性是为了不破坏 `open` 的 O(1) 堆。
    fn strtab_table(&self) -> Result<&[&'a str], ContainerError> {
        if let Some(t) = self.strtab_table.get() {
            return Ok(&t.0);
        }
        let parsed = StrTab(parse_strtab_table(self.strtab)?);
        let _ = self.strtab_table.set(parsed); // 并发输了也没事：别人已放进去
        Ok(&self
            .strtab_table
            .get()
            .expect("table was just set, or a concurrent caller set it")
            .0)
    }

    /// Parsed `DIR`, when the file has a usable one ｜ 文件里有可用的 `DIR` 时返回它。
    fn dir_view(&self) -> Option<&crate::dir::Dir> {
        let body = self.dir?;
        self.dir_index
            .get_or_init(|| crate::dir::parse(body).ok())
            .as_ref()
    }

    /// `key → STRTAB id` map, built once ｜ 键 → 串 id 表，只建一次。
    fn key_ids(&self) -> Result<&HashMap<&'a str, u32>, ContainerError> {
        if let Some(m) = self.key_ids.get() {
            return Ok(m);
        }
        let table = self.strtab_table()?;
        let mut m = HashMap::with_capacity(table.len());
        for (i, k) in table.iter().enumerate() {
            m.entry(*k).or_insert(i as u32);
        }
        let _ = self.key_ids.set(m);
        Ok(self.key_ids.get().expect("map was just set"))
    }

    /// Resolve a path through `DIR`: binary search per indexed level.
    /// 通过 `DIR` 解路径：每个已索引的层做一次二分。
    ///
    /// Returns `None` whenever the index cannot answer (absent, stale, path deeper than
    /// `dir_depth`, path goes through an array, or the offset is out of range) — the caller
    /// then falls back to the linear walk. **Never** returns a wrong value.
    /// ｜ 索引答不了就返回 `None`，由调用方回退线性走；**绝不给错值**。
    fn lookup_dir(&self, p: &Path) -> Result<Option<Value>, ContainerError> {
        if self.frames.is_some() {
            return Ok(None);
        }
        let Some(dir) = self.dir_view() else {
            return Ok(None);
        };
        // 追加过层 ⇒ 索引已过期（§19.3 的 head_layer 语义），整段忽略。
        if dir.head_layer != self.layers.len() as u32 {
            return Ok(None);
        }
        let ids = self.key_ids()?;
        let mut cur: u32 = 0;
        for seg in p.segments() {
            let sml_value::Seg::Key(k) = seg else {
                return Ok(None);
            };
            let Some(&id) = ids.get(k.as_str()) else {
                return Ok(None);
            };
            let Some(child) = dir.find(cur, id) else {
                return Ok(None);
            };
            cur = child;
        }

        let off = cur as usize;
        if off + 6 > self.value.len() {
            return Ok(None);
        }
        let sub = u32::from_le_bytes([
            self.value[off + 2],
            self.value[off + 3],
            self.value[off + 4],
            self.value[off + 5],
        ]) as usize;
        let end = off
            .checked_add(6)
            .and_then(|x| x.checked_add(sub))
            .ok_or(CanonicalError::SubtreeLenOverflow { at: off, declared: sub })?;
        if end > self.value.len() {
            return Ok(None);
        }
        let table = self.strtab_table()?;
        Ok(Some(value_from_subtree_with_table(
            table,
            &self.value[off..end],
        )?))
    }

    /// Raw `DIR` body, when present ｜ `DIR` 段体（若有）。
    pub fn dir_body(&self) -> Option<&'a [u8]> {
        self.dir
    }

    /// Raw `TYPED` body, when present ｜ `TYPED` 段体（若有）。
    pub fn typed_body(&self) -> Option<&'a [u8]> {
        self.typed
    }

    /// Decode typed variable `id` as `T` (N04 / §25).
    /// 按 `T` 解强类型变量 `id`（N04 / §25）。
    pub fn var<T: crate::typed::Decode>(&self, id: crate::typed::VarId) -> Result<T, ContainerError> {
        let seg = self.typed.ok_or_else(|| {
            ContainerError::Typed("本文件没有 TYPED 段（0x70）".to_string())
        })?;
        crate::typed::read_var(seg, id).map_err(ContainerError::from)
    }

    /// The raw bytes of one subtree — **zero copy**, header included.
    /// 一棵子树的原始字节 —— **零拷贝**，含节点头。
    ///
    /// Siblings are skipped via `subtree_len`; nothing is allocated for them.
    /// 兄弟节点靠 `subtree_len` 跳过，不为它们分配任何东西。
    pub fn subtree_bytes(&self, path: &str) -> Result<&'a [u8], ContainerError> {
        if self.frames.is_some() {
            return Err(ContainerError::FramedNotZeroCopy);
        }
        let p = Path::parse(path).map_err(ContainerError::Path)?;
        let table = self.strtab_table()?;
        let sub = subtree_at_with_table(table, self.value, &p)?;
        Ok(sub.bytes)
    }

    /// Same as [`Store::subtree_bytes`], but **materialised** so framed files work too.
    /// 同 [`Store::subtree_bytes`]，但**物化**返回，分帧文件也能用。
    ///
    /// 分帧时只解「覆盖目标区间」的帧 —— 这正是把导航器接到帧上的意义。
    /// ｜ For framed files only the frames overlapping the target are inflated.
    pub fn subtree_vec(&self, path: &str) -> Result<Vec<u8>, ContainerError> {
        let p = Path::parse(path).map_err(ContainerError::Path)?;
        match self.frames {
            None => {
                let table = self.strtab_table()?;
                let sub = subtree_at_with_table(table, self.value, &p)?;
                Ok(sub.bytes.to_vec())
            }
            Some(body) => {
                let table = self.strtab_table()?;
                let mut cur = FrameCursor::new(body)?;
                let (s, e, _) = subtree_range(&mut cur, table, p.segments(), 0)?;
                cur.read_range(s, e)
            }
        }
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

        // 2) DIR 快路径（N05）：索引有效且路径不深于索引深度时，逐层二分
        if let Some(v) = self.lookup_dir(&p)? {
            return Ok((v, 1));
        }

        // 3) 导航：明文零拷贝切片；分帧只解覆盖目标的那几帧
        match self.frames {
            None => {
                let table = self.strtab_table()?;
                let sub = subtree_at_with_table(table, self.value, &p)?;
                let v = value_from_subtree_with_table(table, sub.bytes)?;
                Ok((v, sub.nodes_visited))
            }
            Some(body) => {
                let table = self.strtab_table()?;
                let mut cur = FrameCursor::new(body)?;
                let (s, e, visited) = subtree_range(&mut cur, table, p.segments(), 0)?;
                let bytes = cur.read_range(s, e)?;
                let v = value_from_subtree_with_table(table, &bytes)?;
                Ok((v, visited))
            }
        }
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
        let base = Value::from_canonical_blocks(self.strtab, &self.logical_value()?)?;
        if self.layers.is_empty() {
            Ok(base)
        } else {
            Ok(layer::apply_patches(&base, &self.layers)?)
        }
    }

    /// The value **as of** layer `upto` (inclusive); see [`layer::value_at`].
    /// 「到第 `upto` 层为止（含）」的值，见 [`layer::value_at`]。
    pub fn history_at(&self, upto: usize) -> Result<Value, ContainerError> {
        let base = Value::from_canonical_blocks(self.strtab, &self.logical_value()?)?;
        layer::value_at(self.bytes, &base, upto).map_err(ContainerError::Layer)
    }

    /// Payload names declared by the manifest. ｜ manifest 声明的载荷清单。
    pub fn payloads(&self) -> &[ManifestPayload] {
        self.manifest
            .as_ref()
            .map(|m| m.payloads.as_slice())
            .unwrap_or(&[])
    }

    /// Extract one payload by name (zero copy). ｜ 按名字取一段载荷（零拷贝）。
    pub fn payload(&self, name: &str) -> Result<std::borrow::Cow<'a, [u8]>, ContainerError> {
        container::payload(self.bytes, name)
    }

    /// The whole node tree as one buffer: plain → borrow (copy is avoided by `Cow::Borrowed`),
    /// framed → inflate every frame. ｜ 整棵节点树：明文借用，分帧全解。
    fn logical_value(&self) -> Result<std::borrow::Cow<'a, [u8]>, ContainerError> {
        match self.frames {
            None => Ok(std::borrow::Cow::Borrowed(self.value)),
            Some(body) => Ok(std::borrow::Cow::Owned(
                container::inflate_frames_all(body)?,
            )),
        }
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
