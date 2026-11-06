// SPDX-License-Identifier: MulanPSL-2.0
//! Append-only layer chain + signatures (T4 / §21.1 P2).
//! 只追加的层链与签名（T4 / §21.1 P2）。
//!
//! # 三条已定的规则
//!
//! 1. **`prev_hash = H(前一层记录的 canonical 字节)`**（P2-a）—— 不覆盖整个文件前缀，
//!    追加保持 **O(1)**；
//! 2. **层记录复用 `VALUE` 编码**，签名对象 = `canonical_text(layer)`（P2-b）；
//! 3. **`delegation` 放在层记录里**（P2-c）⇒ 天然被前一层签名覆盖，正是 D16 要的。
//!
//! # 文件尾的样子
//!
//! ```text
//! … 段数据 | L0 | T0 | L1 | T1 …
//!                        ↑ 读者只看 EOF 往前 64 B 的 Trailer
//! ```
//!
//! 每层记录 = `len:u32 ‖ prev_off:u64 ‖ reserved:u32 ‖ body`（16 B 头 + 体），
//! 体 = 一个 `Value` 的 L0 字节。Trailer（64 B）指向**最后一层**，层记录自己带
//! `prev_off` 组成**反向链**。
//!
//! 为什么是反向链而不是正向：追加只能往 EOF 写，正向链要改上一层的 `next` 指针
//! （= 改旧字节）。反向链让「追加不改旧字节」这条不变量在**字节层面**也成立，
//! 不只是「签名仍然有效」。
//! ｜ Reverse links so an append touches nothing but EOF (invariant 3 holds byte-wise).
//!
//! ⚠️ **偏移不进签名**：`prev_off` / Trailer 都在签名对象之外 —— 与 §18.3「位置不是语义」
//! 同口径。所以「同一份数据在不同位置被追加」签出来的东西是一样的。
//!
//! # 签名对象的一处澄清（实现即文档）
//!
//! P2-b 写「签名对象 = `canonical_text(layer)`」。可 `sig` 就在 layer 里，直接签它就循环了。
//! 故落地口径（已回写 §21.1）：**签名对象 = 去掉 `sig` 字段后的 `canonical_text`**，
//! 与 §18.4「排除 `SIG` 段自身」同一个道理。

use sml_value::{Map, Path, Value};

use crate::hash::{self, HashAlgo};
use crate::manifest::ManifestError;

/// Trailer magic at EOF / 文件尾 Trailer 的魔数。
pub const TRAILER_MAGIC: [u8; 8] = *b"SMLTRLR1";
/// Fixed trailer size / Trailer 固定长度。
pub const TRAILER_LEN: usize = 64;
/// Layer record header: `len:u32 ‖ prev_off:u64 ‖ reserved:u32`.
/// 层记录头：长度、上一层偏移、保留位。
pub const LAYER_HEADER_LEN: usize = 16;

/// The trailer at EOF: which layer is the head, and where it sits.
/// 文件尾的 Trailer：头是哪一层、它在哪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trailer {
    /// Number of layers in the chain ｜ 层的个数。
    pub layer_count: u32,
    /// Offset of the **last** layer record ｜ **最后一层**记录的偏移。
    pub last_layer_off: u64,
    /// `H(canonical bytes of the last layer)` ｜ 最后一层的哈希。
    pub head_hash: [u8; 32],
}

impl Trailer {
    /// Serialize to the fixed 64 B form. ｜ 落成固定 64 B。
    pub fn to_bytes(&self) -> [u8; TRAILER_LEN] {
        let mut out = [0u8; TRAILER_LEN];
        out[0..8].copy_from_slice(&TRAILER_MAGIC);
        out[8..12].copy_from_slice(&self.layer_count.to_le_bytes());
        out[12..20].copy_from_slice(&self.last_layer_off.to_le_bytes());
        out[20..52].copy_from_slice(&self.head_hash);
        out
    }

    /// Parse the last 64 bytes of a file; `None` when the magic is absent (no layers).
    /// 解析文件最后 64 B；没有魔数就当「没层」。
    pub fn parse(bytes: &[u8]) -> Option<Trailer> {
        if bytes.len() < TRAILER_LEN {
            return None;
        }
        let t = &bytes[bytes.len() - TRAILER_LEN..];
        if t[0..8] != TRAILER_MAGIC {
            return None;
        }
        let mut head_hash = [0u8; 32];
        head_hash.copy_from_slice(&t[20..52]);
        Some(Trailer {
            layer_count: u32::from_le_bytes(t[8..12].try_into().expect("4 bytes")),
            last_layer_off: u64::from_le_bytes(t[12..20].try_into().expect("8 bytes")),
            head_hash,
        })
    }
}

/// One signature over a layer (D36: an array, **all** must pass).
/// 层上的一条签名（D36：数组，**全部**必须通过）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigEntry {
    /// §20 algorithm id (`0x40` Ed25519) ｜ 算法编号。
    pub algo: u8,
    /// Raw signature bytes (hex in the value form) ｜ 签名字节（值形态里是十六进制）。
    pub bytes: Vec<u8>,
}

/// Authorisation to change the signing key (D16: 转授权链).
/// 换钥授权（D16 的转授权链）。
///
/// 由**前一个作者**签，覆盖 `(from, to, max_index)`；只有 `j ≤ max_index` 的层才被授权。
/// Signed by the previous author over `(from, to, max_index)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delegation {
    /// Previous author's public key ｜ 授权方公钥。
    pub from: [u8; 32],
    /// Highest layer index this delegation covers ｜ 覆盖到的最大层号。
    pub max_index: u32,
    /// Ed25519 signature over the canonical delegation text ｜ 对授权文本的签名。
    pub sig: Vec<u8>,
}

/// A layer record's signed content (everything except `sig`).
/// 一层记录的可签内容（除 `sig` 以外的全部）。
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// 0-based index, must be continuous ｜ 从 0 开始、必须连续。
    pub index: u32,
    /// `H(previous layer's canonical bytes)`; all zeros for the first layer
    /// 上一层的哈希；首层为全 0。
    pub prev_hash: [u8; 32],
    /// Author's Ed25519 public key (32 B) ｜ 作者公钥。
    pub author: [u8; 32],
    /// `H(all bytes before the first layer record)` — binds the **base document** into the
    /// signed content (§21.1b E, 用户 2026-11-05 裁决「甲」).
    /// 「第一条层记录之前的全部字节」的摘要 —— 把**基文档**绑进签名范围。
    ///
    /// 没有它，链头只由层记录决定：换掉基文档、重算段摘要，签名仍然通过。
    /// ｜ Without it the chain says nothing about the document it is attached to.
    pub base_hash: [u8; 32],
    /// Optional patch merged into the value (D22: arrays replace wholesale)
    /// 可选补丁，合并进当前值（D22：数组整体替换）。
    pub patch: Option<Value>,
    /// Optional key-change authorisation (D16) ｜ 可选换钥授权。
    pub delegation: Option<Delegation>,
    /// Signatures, all of which must verify (D36) ｜ 签名数组，全部必须通过。
    pub sigs: Vec<SigEntry>,
}

impl Layer {
    /// A fresh, unsigned layer pointing at `prev_hash`.
    /// 新建一个未签名、指向 `prev_hash` 的层。
    pub fn new(
        index: u32,
        prev_hash: [u8; 32],
        author: [u8; 32],
        base_hash: [u8; 32],
    ) -> Layer {
        Layer {
            index,
            prev_hash,
            author,
            base_hash,
            patch: None,
            delegation: None,
            sigs: Vec::new(),
        }
    }

    /// Encode as a `Value`; `with_sigs = false` drops the `sig` field (the signable form).
    /// 编成 `Value`；`with_sigs = false` 时去掉 `sig`（可签形态）。
    pub fn to_value(&self, with_sigs: bool) -> Value {
        let mut m = Map::ordered();
        m.insert("index".to_string(), Value::Int(self.index as i64));
        m.insert("prev_hash".to_string(), Value::Str(hex(&self.prev_hash)));
        m.insert("author".to_string(), Value::Str(hex(&self.author)));
        m.insert("base_hash".to_string(), Value::Str(hex(&self.base_hash)));
        if let Some(p) = &self.patch {
            m.insert("patch".to_string(), p.clone());
        }
        if let Some(d) = &self.delegation {
            let mut dm = Map::ordered();
            dm.insert("from".to_string(), Value::Str(hex(&d.from)));
            dm.insert("max_index".to_string(), Value::Int(d.max_index as i64));
            dm.insert("sig".to_string(), Value::Str(hex_slice(&d.sig)));
            m.insert("delegation".to_string(), Value::Object(dm));
        }
        if with_sigs {
            let sigs: Vec<Value> = self
                .sigs
                .iter()
                .map(|s| {
                    let mut sm = Map::ordered();
                    sm.insert("algo".to_string(), Value::Int(s.algo as i64));
                    sm.insert("bytes".to_string(), Value::Str(hex_slice(&s.bytes)));
                    Value::Object(sm)
                })
                .collect();
            m.insert("sig".to_string(), Value::Array(sigs));
        }
        Value::Object(m)
    }

    /// The exact bytes a signature covers (P2-b + the `sig`-exclusion above).
    /// 签名真正覆盖的字节（P2-b + 上面那条「排除 sig」）。
    pub fn signed_text(&self) -> Result<String, crate::CanonicalError> {
        self.to_value(false).canonical_text()
    }

    /// Canonical bytes of the whole layer **with** signatures — what `prev_hash` hashes.
    /// 含签名的整层 canonical 字节 —— `prev_hash` 算的就是它。
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, crate::CanonicalError> {
        self.to_value(true).canonical_bytes()
    }

    /// Decode from a `Value`. ｜ 从 `Value` 解码。
    pub fn from_value(v: &Value) -> Result<Layer, ManifestError> {
        let m = match v {
            Value::Object(m) => m,
            _ => return Err(ManifestError::NotAnObject),
        };
        let index = match m.get("index") {
            Some(Value::Int(n)) => *n as u32,
            _ => return Err(ManifestError::Missing("index")),
        };
        let prev_hash = field_hash(m, "prev_hash")?;
        let author = field_hash(m, "author")?;
        // 缺 base_hash 一律当**不合规**（不静默降级成旧格式）：本项目 publish=false，
        // 没有需要兼容的历史签名文件；放行等于让「基文档被换」重新变成盲区。
        let base_hash = field_hash(m, "base_hash")?;
        let patch = m.get("patch").cloned();
        let delegation = match m.get("delegation") {
            Some(Value::Object(d)) => Some(Delegation {
                from: field_hash(d, "from")?,
                max_index: match d.get("max_index") {
                    Some(Value::Int(n)) => *n as u32,
                    _ => return Err(ManifestError::Missing("delegation.max_index")),
                },
                sig: match d.get("sig") {
                    Some(Value::Str(s)) => from_hex(s)
                        .ok_or(ManifestError::BadShape("delegation.sig（需十六进制）"))?,
                    _ => return Err(ManifestError::Missing("delegation.sig")),
                },
            }),
            Some(_) => return Err(ManifestError::BadShape("delegation")),
            None => None,
        };
        let mut sigs = Vec::new();
        match m.get("sig") {
            Some(Value::Array(a)) => {
                for e in a {
                    let sm = match e {
                        Value::Object(sm) => sm,
                        _ => return Err(ManifestError::BadShape("sig[]")),
                    };
                    let algo = match sm.get("algo") {
                        Some(Value::Int(n)) => *n as u8,
                        _ => return Err(ManifestError::Missing("sig[].algo")),
                    };
                    let bytes = match sm.get("bytes") {
                        Some(Value::Str(s)) => {
                            from_hex(s).ok_or(ManifestError::BadShape("sig[].bytes（需十六进制）"))?
                        }
                        _ => return Err(ManifestError::Missing("sig[].bytes")),
                    };
                    sigs.push(SigEntry { algo, bytes });
                }
            }
            Some(_) => return Err(ManifestError::BadShape("sig")),
            None => {}
        }
        Ok(Layer {
            index,
            prev_hash,
            author,
            base_hash,
            patch,
            delegation,
            sigs,
        })
    }
}

/// The text a [`Delegation`] signature covers. ｜ 换钥授权签名覆盖的文本。
///
/// 独立于层记录：授权是「从 A 到 B 直到第 N 层」这件事本身，与它被放进哪一层无关。
/// Independent of which layer carries it — it authorises a key change, not a layer.
pub fn delegation_text(from: &[u8; 32], to: &[u8; 32], max_index: u32) -> String {
    format!(
        "smlb/delegate/v1\nfrom:{}\nto:{}\nmax_index:{max_index}\n",
        hex(from),
        hex(to)
    )
}

fn field_hash(m: &Map, key: &str) -> Result<[u8; 32], ManifestError> {
    match m.get(key) {
        Some(Value::Str(s)) => {
            let raw = from_hex(s).ok_or(ManifestError::BadShape("需要 64 位十六进制"))?;
            let arr: [u8; 32] = raw
                .as_slice()
                .try_into()
                .map_err(|_| ManifestError::BadShape("需要 64 位十六进制"))?;
            Ok(arr)
        }
        _ => Err(ManifestError::Missing("prev_hash/author")),
    }
}

fn hex(d: &[u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_slice(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        out.push(u8::from_str_radix(&s[i..i + 2], 16).ok()?);
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// 读写层链 / reading the chain
// ---------------------------------------------------------------------------

/// All layers, oldest first (decoded from the reverse chain).
/// 全部层，从旧到新（由反向链解出）。
/// Where the **base document** ends: offset of the first (oldest) layer record.
/// 基文档在哪结束：最旧那条层记录的偏移。
///
/// 反向链沿 `prev_off` 回溯到 0 即最旧记录。没有 Trailer 时整个文件都是基文档。
/// ｜ Walk the reverse chain to `prev_off == 0`; without a trailer, the whole file is the base.
pub fn base_prefix_len(bytes: &[u8]) -> Result<usize, LayerError> {
    let Some(t) = Trailer::parse(bytes) else {
        return Ok(bytes.len());
    };
    let mut off = t.last_layer_off;
    // 步数必须封顶：`prev_off` 是**文件里的数字**，可以被写成环（自己指自己 / 两点互指）。
    // 不封顶就是死循环（fuzz 的「不卡死」判据）。上限用「文件最多放得下几条记录」，
    // 而不是 `layer_count` —— 后者也是可伪造的 u32，最大 4G 步。
    let max_steps = bytes.len() / LAYER_HEADER_LEN + 1;
    for _ in 0..max_steps {
        let (_body, prev_off) = read_record(bytes, off)?;
        if prev_off == 0 {
            return Ok(off as usize);
        }
        off = prev_off;
    }
    Err(LayerError::Truncated { at: off })
}

/// Hash of the base document (everything before the first layer record).
/// 基文档的摘要（第一条层记录之前的全部字节）。
pub fn base_hash(bytes: &[u8], algo: HashAlgo) -> Result<[u8; 32], LayerError> {
    let n = base_prefix_len(bytes)?;
    hash::digest(algo, &bytes[..n]).map_err(|_| LayerError::Truncated { at: n as u64 })
}

pub fn layers(bytes: &[u8]) -> Result<Vec<Layer>, LayerError> {
    let Some(t) = Trailer::parse(bytes) else {
        return Ok(Vec::new());
    };
    // ⚠️ 三条防御都针对**伪造的 Trailer**（fuzz 撞出来的，别删）：
    // 1. 不按 `layer_count` 预分配 —— 它是 u32，写 0xFFFF_FFFF 就是当场要 4G 项的分配；
    // 2. `prev_off` 可以成环（含自指），步数必须封顶，否则死循环 + 无界增长；
    // 3. 偏移检查交给 `read_record`（那里走 `checked_add`），这里不再自己加。
    let declared = t.layer_count as usize;
    let mut out: Vec<Layer> = Vec::new();
    let mut off = t.last_layer_off;
    let max_steps = declared.min(bytes.len() / LAYER_HEADER_LEN) + 1;
    for _ in 0..max_steps {
        let (body, prev_off) = read_record(bytes, off)?;
        let v = Value::from_canonical_bytes(body).map_err(LayerError::Canonical)?;
        let layer = Layer::from_value(&v).map_err(LayerError::Manifest)?;
        out.push(layer);
        if prev_off == 0 {
            break;
        }
        off = prev_off;
    }
    out.reverse();
    if out.len() != declared {
        return Err(LayerError::CountMismatch {
            declared,
            found: out.len(),
        });
    }
    Ok(out)
}

fn read_record(bytes: &[u8], off: u64) -> Result<(&[u8], u64), LayerError> {
    let start = off as usize;
    if start
        .checked_add(LAYER_HEADER_LEN)
        .map_or(true, |e| e > bytes.len())
    {
        return Err(LayerError::Truncated { at: off });
    }
    let len = u32::from_le_bytes(bytes[start..start + 4].try_into().expect("4 bytes")) as usize;
    let prev_off = u64::from_le_bytes(bytes[start + 4..start + 12].try_into().expect("8 bytes"));
    let body_start = start + LAYER_HEADER_LEN;
    let body_end = body_start
        .checked_add(len)
        .ok_or(LayerError::Truncated { at: off })?;
    if body_end > bytes.len() {
        return Err(LayerError::Truncated { at: off });
    }
    Ok((&bytes[body_start..body_end], prev_off))
}

/// Why the layer chain could not be read. ｜ 层链读不出来的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LayerError {
    /// A layer record points outside the file ｜ 层记录指向文件外。
    Truncated {
        /// Offset ｜ 偏移。
        at: u64,
    },
    /// The layer body is not a valid canonical value ｜ 层体不是合法 canonical。
    Canonical(crate::CanonicalError),
    /// A required field is missing/bad ｜ 必填字段缺失或形状不对。
    Manifest(ManifestError),
    /// The trailer's `layer_count` disagrees with the chain length ｜ 层数对不上。
    CountMismatch {
        /// Declared in the trailer ｜ Trailer 里写的。
        declared: usize,
        /// Actually walked ｜ 实际走出来的。
        found: usize,
    },
}

impl std::fmt::Display for LayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerError::Truncated { at } => write!(f, "层记录在偏移 {at} 处越出文件"),
            LayerError::Canonical(e) => write!(f, "层体不是合法 canonical：{e}"),
            LayerError::Manifest(ManifestError::Missing(k)) => {
                write!(f, "层体缺必填字段 `{k}`")
            }
            LayerError::Manifest(ManifestError::BadShape(k)) => {
                write!(f, "层体字段 `{k}` 形状不对")
            }
            LayerError::Manifest(e) => write!(f, "层体字段不对：{e}"),
            LayerError::CountMismatch { declared, found } => write!(
                f,
                "Trailer 说 {declared} 层，实际走出 {found} 层（链断了或被截断）"
            ),
        }
    }
}

impl std::error::Error for LayerError {}

/// Merge every layer's patch into `base`, in order.
/// 按顺序把每层的 patch 合并进 `base`。
///
/// 合并规则用既有的 [`sml_value::insert_or_merge`]：键已存在 ⇒ **原位覆盖**（位置不变），
/// 不存在 ⇒ 追加到尾部；**数组整体替换**（D22）。
/// Uses the existing merge helper: in-place override, append at tail, arrays replace wholesale.
pub fn apply_patches(
    base: &Value,
    layers: &[Layer],
) -> Result<Value, crate::CanonicalError> {
    let mut cur = base.clone();
    for l in layers {
        let Some(patch) = &l.patch else { continue };
        match (&mut cur, patch) {
            (Value::Object(base_map), Value::Object(patch_map)) => {
                for (k, v) in patch_map.iter() {
                    sml_value::insert_or_merge(base_map, k, v.clone(), |old, new| *old = new);
                }
            }
            // 非对象的补丁：整值替换（D22 的「数组整体替换」同理）
            (slot, other) => *slot = other.clone(),
        }
    }
    Ok(cur)
}

/// The value **as of** layer `upto` (inclusive) — history replay.
/// 「到第 `upto` 层为止（含）」的值 —— 历史回放。
///
/// 口径：`upto` 是**层号**（0 起），不是层数；超过链尾按链尾算。
/// ｜ `upto` is a layer **index** (0-based), not a count; clamped at the chain tail.
pub fn value_at(bytes: &[u8], base: &Value, upto: usize) -> Result<Value, LayerError> {
    let all = layers(bytes)?;
    let n = upto.saturating_add(1).min(all.len());
    apply_patches(base, &all[..n]).map_err(LayerError::Canonical)
}

/// Convenience: `Value::get_path` wrapper used by the CLI (`smlb cat --at N`).
/// 便捷函数：带层号的路径取值（给 `smlb cat --at N` 用）。
pub fn history_value_at(
    bytes: &[u8],
    base: &Value,
    upto: usize,
    path: &str,
) -> Result<Value, LayerError> {
    let v = value_at(bytes, base, upto)?;
    let p = Path::parse(path).map_err(|_| LayerError::Manifest(ManifestError::BadShape("path")))?;
    v.get_path(&p)
        .map(|x| x.clone())
        .map_err(|_| LayerError::Manifest(ManifestError::BadShape("path")))
}

/// Chain-only report (no cryptography): what a build **without** the `sign` feature can still say.
/// 只查链结构的报告（不碰密码学）—— 没有 `sign` feature 的构建也能给出这些结论。
#[derive(Debug, Clone)]
pub struct ChainReport {
    /// Layer count ｜ 层数。
    pub layers: usize,
    /// `true` only when every structural check passed ｜ 结构全对才为 `true`。
    pub ok: bool,
    /// One line per finding ｜ 逐条结论。
    pub lines: Vec<String>,
}

/// Check what needs no keys: trailer sanity, index continuity, `prev_hash` links, head hash.
/// 检查不需要密钥的那部分：Trailer、序号连续、`prev_hash` 衔接、head 哈希。
///
/// 这一层的价值：**没有 `sign` feature 的构建也能回答「文件被改过没有」**（改内容 ⇒ 段摘要不符；
/// 删/改层 ⇒ 链断或 head 不符），只是不能回答「是谁签的」。
/// ｜ Structural integrity without keys: “was it changed?”, just not “by whom?”.
/// The chain's **head**: hash of the last layer's canonical bytes.
/// 层链的**头**：最后一层规范字节的摘要。
///
/// `None` 表示文件里**没有层** —— 这是「未签名」与「链被整个剥离」在字节层面**唯一**的区别，
/// 也正是为什么需要一个外部锚点（`--trust`）才能把这两件事分开（D18）。
/// ｜ `None` means “no layers” — indistinguishable from “chain stripped”, hence an external anchor.
pub fn chain_head(bytes: &[u8], algo: HashAlgo) -> Result<Option<[u8; 32]>, LayerError> {
    let all = layers(bytes)?;
    let Some(last) = all.last() else {
        return Ok(None);
    };
    let body = last.canonical_bytes().map_err(LayerError::Canonical)?;
    let head = hash::digest(algo, &body).map_err(|_| LayerError::Truncated { at: 0 })?;
    Ok(Some(head))
}

pub fn verify_chain(bytes: &[u8], algo: HashAlgo) -> Result<ChainReport, LayerError> {
    let all = layers(bytes)?;
    let mut lines = Vec::new();
    let mut ok = true;
    if all.is_empty() {
        lines.push("层链   没有层（未签名文件）".to_string());
        return Ok(ChainReport {
            layers: 0,
            ok: true,
            lines,
        });
    }

    let mut prev_head = [0u8; 32];
    for (i, l) in all.iter().enumerate() {
        if l.index as usize != i {
            ok = false;
            lines.push(format!("层#{i}   序号是 {}，应为 {i}", l.index));
        }
        if l.prev_hash != prev_head {
            ok = false;
            lines.push(format!("层#{i}   prev_hash 与上一层不符（链断了）"));
        }
        let body = l.canonical_bytes().map_err(LayerError::Canonical)?;
        prev_head = hash::digest(algo, &body).map_err(|_| LayerError::Truncated { at: 0 })?;
    }

    // 基文档绑定（§21.1b E）：每层记的 base_hash 必须与**当前文件**的基文档一致。
    let want_base = base_hash(bytes, algo)?;
    for (i, l) in all.iter().enumerate() {
        if l.base_hash != want_base {
            ok = false;
            lines.push(format!("层#{i}   base_hash 与当前基文档不符 —— 基文档被换过"));
        }
    }

    if let Some(t) = Trailer::parse(bytes) {
        if t.head_hash != prev_head {
            ok = false;
            lines.push("Trailer   head_hash 与最后一层不符".to_string());
        } else {
            lines.push(format!("Trailer   head_hash OK（{} 层）", all.len()));
        }
    }
    lines.push(format!(
        "结论     {}（{} 层，仅结构）",
        if ok { "结构对得上" } else { "结构对不上" },
        all.len()
    ));
    Ok(ChainReport {
        layers: all.len(),
        ok,
        lines,
    })
}

