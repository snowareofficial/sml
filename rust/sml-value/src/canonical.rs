// SPDX-License-Identifier: MulanPSL-2.0
//! **Canonical form**: the same data must yield the same text and the same bytes, no matter
//! whether it came from `.sml` text, from program construction, or from another `.smlb`.
//! **canonical 形态**：让「同一份数据」无论来自 `.sml` 文本、程序构造，还是另一个
//! `.smlb`，都产出**同一串文本**与**同一串字节**。
//!
//! Rules live in `docs/smlb-smldb-spec.md` §18 (C1–C13) and §19 (`VALUE` segment layout).
//! 依据 `docs/smlb-smldb-spec.md` §18（C1–C13）与 §19（`VALUE` 段布局）。两张面孔：
//!
//! | Form / 形态 | Entry / 入口 | Use / 用途 | Affected by `fmt` / 受 `fmt` 影响 |
//! |---|---|---|---|
//! | Text / 文本 | [`Value::canonical_text`] | **signing target**, cross-end vectors, human diff ｜ **签名对象**、跨端向量、人类比对 | No (decoupled from bytes) ｜ 不受 |
//! | Bytes / 字节 | [`Value::canonical_bytes`] | what `dump()` writes, for `load()` ｜ `dump()` 的输出，供 `load()` 往返 | Yes ｜ 受 |
//!
//! Sign the text, never the bytes (§18.5 P5-a): otherwise one `fmt` bump invalidates every
//! historical signature. ｜ 签名签文本不签字节：否则 `fmt` 一升级，历史签名全部失效。
//!
//! # Three hard rules / 三条硬规矩
//!
//! 1. **Objects must keep source order** (C2). Without the `preserve_order` feature `Map` is
//!    a `BTreeMap` and “source order” **does not exist in the type at all**; this module then
//!    returns [`CanonicalError::KeyOrderUnavailable`] instead of degrading to “write sorted”.
//!    Silent sorting would make “the same document signs differently on two machines” a fact.
//! 2. **Non-finite floats are an error** (C13). NaN / ±Inf have no SML literal — canonical is
//!    static text, so it refuses rather than faking `"nan"` (which would change the type).
//! 3. **No opportunistic pretty-printing** (C12). Every layout rule is a fixed value
//!    (indent, when to stay on one line, separator) — never a function of line width.
//!
//! 1. **对象必须保序**（C2）：`Map` 显式携带 `Sorted` / `Source` 两态；遇到 `Sorted`
//!    一律 [`CanonicalError::KeyOrderUnavailable`]，绝不退化成「按排序写」。
//!    这不再取决于编译开关，而取决于**你手里这个对象**是哪一态。
//! 2. **非有限浮点报错**（C13）：NaN / ±Inf 没有字面量，不许伪造成 `"nan"` 字符串（会改类型）。
//! 3. **不许顺手美化**（C12）：排版规则全部是定值，不按行宽、不按「看起来更好」调整。
//!
//! # How it differs from `to_sml` (they are **not** interchangeable) / 与 `to_sml` 的差别
//!
//! | Item / 项 | `to_sml` | `canonical_text` |
//! |---|---|---|
//! | Empty object / array ｜ 空容器 | `{}` / `[]` | `{ }` / `[ ]`（C9） |
//! | Metadata key position ｜ 元数据键位置 | as-is ｜ 原样 | `__type`→`__name`→`__args`→`__featags` first ｜ 固定最前（C10） |
//! | `Float` raw | kept if valid ｜ 有就原样 | **ignored** ｜ **忽略**（C5/C12） |
//! | Non-finite float ｜ 非有限浮点 | silently `"nan"` ｜ 静默写成字符串 | error ｜ 报错（C13） |
//! | Depth overflow ｜ 深度超限 | placeholder text ｜ 静默写占位 | error ｜ 报错（`E-LIMIT-004`） |
//! | Missing key order ｜ 键序缺失 | unknown concept ｜ 无此概念 | error ｜ 报错（C2） |
//!
//! # Example / 例子
//!
//! ```
//! # {
//! use sml_value::{Map, Value};
//!
//! let mut m = Map::ordered();          // 显式保源序
//! m.insert("z".to_string(), Value::Int(1));
//! m.insert("a".to_string(), Value::Int(2));
//! let v = Value::Object(m);
//! // source order in, source order out (**not** `a` first) ｜ 源序进、源序出
//! assert_eq!(v.canonical_text().unwrap(), "z: 1\na: 2\n");
//! # }
//! ```

use std::collections::HashMap;

use crate::text_rules::{format_float, quote_if_needed};
use crate::path::{Path, Seg};
use crate::{Map, Value, MAX_VALUE_DEPTH};

/// Fixed output order of metadata keys (C10 / P5-c) — **the order is the definition**.
/// 元数据键的固定输出序（C10 / P5-c）。**顺序即定义**，不跟源序走。
const META_KEYS: [&str; 4] = ["__type", "__name", "__args", "__featags"];

/// Node type tags (§19.2). Once published, a value is never reassigned.
/// 节点类型标签（§19.2）。数值一经发布不再改。
const TAG_NULL: u8 = 0;
const TAG_FALSE: u8 = 1;
const TAG_TRUE: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_FLOAT: u8 = 4;
const TAG_STR: u8 = 5;
const TAG_ARR: u8 = 6;
const TAG_OBJ: u8 = 7;

/// `flags` bit 0: this object contains `__type` (saves one lookup, §19.2).
/// `flags` 位 0：本对象含 `__type`（省一次查表）。
const FLAG_HAS_TYPE: u8 = 0b0000_0001;

/// `subtree_len` is 4 B, so one node's payload tops out at 4 GiB (§19.2).
/// 单节点 `subtree_len` 是 4 B ⇒ 子树载荷上限 4 GiB。
const MAX_SUBTREE_LEN: usize = u32::MAX as usize;

/// Why canonical failed / canonical 失败原因。
///
/// Why an enum and not a `String` / 为什么是**独立的枚举**：the three failure families are of
/// different nature — “built wrong” (C2), “data inexpressible” (C13), “bytes are corrupt”
/// (decode) — and callers must handle them differently (e.g. `--check` maps them to distinct
/// exit codes). The code table has no `E-SMLB-*` yet (that surface is fixed in T2), so this
/// exposes a **stable `kind()` name** plus descriptive text; depth keeps `E-LIMIT-004`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CanonicalError {
    /// An object carries `Sorted` keys, so source order is not available (C2).
    /// 对象是显式无序态（`Map::Unordered`），没有源序可写（C2）。
    ///
    /// This is a **data-level** fact: `parse_ordered()` / `Map::ordered()` produces an
    /// object that encodes. No compiler flag changes the answer.
    /// 这是**数据层**事实：用 `parse_ordered()` / `Map::ordered()` 构造的对象就能编码；
    /// 换任何编译开关都不会改变结论。
    KeyOrderUnavailable,
    /// A `Float` is NaN / ±Inf, which has no SML literal (C13).
    /// `Float` 是 NaN / ±Inf，没有 SML 字面量（C13）。
    NonFiniteFloat,
    /// Nesting deeper than [`MAX_VALUE_DEPTH`] (`E-LIMIT-004`).
    /// 嵌套深度超过 [`MAX_VALUE_DEPTH`]（`E-LIMIT-004`）。
    DepthExceeded {
        /// Depth at the offending node (root = 0) ｜ 触发处所在深度（根 = 0）。
        depth: usize,
    },
    /// One node's payload exceeds 4 GiB, which `subtree_len:u32` cannot hold (§19.2).
    /// 单节点载荷超过 4 GiB，`subtree_len` 放不下（§19.2）。
    SubtreeTooLarge {
        /// Actual payload size in bytes ｜ 实际载荷字节数。
        len: usize,
    },

    /// Decode: the byte string ended early.
    /// 解码：字节在某个偏移处提前结束。
    Truncated {
        /// Offset where data was expected ｜ 期望读到数据处的偏移。
        at: usize,
    },
    /// Decode: a LEB128 integer ran past 10 bytes.
    /// 解码：LEB128 变长整数超过 10 字节。
    VarintTooLong {
        /// Start offset ｜ 起始偏移。
        at: usize,
    },
    /// Decode: unknown node tag.
    /// 解码：未知节点 tag。
    UnknownTag {
        /// The unknown tag value ｜ 未知 tag 值。
        tag: u8,
        /// Offset ｜ 偏移。
        at: usize,
    },
    /// Decode: a `STRTAB` entry is not valid UTF-8.
    /// 解码：`STRTAB` 里的字节不是合法 UTF-8。
    BadStrTable {
        /// Index of the offending entry ｜ 该表项的序号。
        index: usize,
    },
    /// Decode: a node points at a `STRTAB` entry that does not exist.
    /// 解码：节点引用了不存在的 `STRTAB` 项。
    StrIdOutOfRange {
        /// The out-of-range id ｜ 越界的 id。
        id: u64,
    },
    /// Decode: duplicate key inside one object (`D15`: no implicit last-wins).
    /// 解码：同一个对象里出现重复键（`D15`：不做隐式 last-wins）。
    DuplicateKey {
        /// The duplicated key ｜ 重复的键。
        key: String,
    },
    /// Decode: the node header's `subtree_len` disagrees with the bytes actually consumed.
    /// 解码：节点头声明 `subtree_len` 与实际消费字节数不一致。
    SubtreeLenMismatch {
        /// Payload start offset ｜ 节点载荷起始偏移。
        at: usize,
        /// Length declared in the header ｜ 头里声明的长度。
        declared: usize,
        /// Length actually consumed ｜ 实际消费的长度。
        actual: usize,
    },
    /// Decode: `subtree_len` exceeds the remaining bytes.
    /// 解码：`subtree_len` 超出剩余字节。
    SubtreeLenOverflow {
        /// Payload start offset ｜ 节点载荷起始偏移。
        at: usize,
        /// Length declared in the header ｜ 头里声明的长度。
        declared: usize,
    },
    /// Decode: bytes are left over after the whole tree was read.
    /// 解码：整棵树读完后还有剩余字节（多余尾巴）。
    TrailingBytes {
        /// Start offset of the leftovers ｜ 剩余字节起始偏移。
        at: usize,
    },
    /// Navigation: the path names a key/index that the tree does not have.
    /// 导航：路径里的键/下标在树里不存在。
    ///
    /// Note this is about **bytes**, not about a `Value` that happens to be missing: the
    /// navigator never builds the surrounding objects, so it reports the failing segment.
    /// 注意这是「字节里找不到」，不是「某个 `Value` 缺键」：导航器不构造外围对象，
    /// 故只报出错的那一段。
    PathNotFound {
        /// The segment that could not be resolved (`key` or `[index]`) ｜ 解不开的那一段。
        seg: String,
    },
    /// The byte source could not supply bytes (e.g. a corrupt compression frame).
    /// 字节源拿不出字节（例如压缩帧损坏）。
    ///
    /// 值层只认识「字节」，不知道帧/压缩；这条变体的用途是把**源**的失败如实传上去，
    /// 而不是伪装成「截断」。
    /// ｜ The value layer knows bytes, not frames; this carries the source's failure verbatim.
    Source(String),
    /// Navigation: the path descends into a scalar, or indexes a non-array.
    /// 导航：路径往标量里钻，或对非数组取下标。
    PathTypeMismatch {
        /// The offending segment ｜ 出错的那一段。
        seg: String,
    },
}

impl CanonicalError {
    /// Stable machine-readable class name — a temporary anchor until the code table is fixed in T2
    /// (then these become `E-SMLB-*`). ｜ 稳定的机器可读分类名（T2 定稿后换成 `E-SMLB-*`）。
    pub fn kind(&self) -> &'static str {
        match self {
            CanonicalError::KeyOrderUnavailable => "key-order-unavailable",
            CanonicalError::NonFiniteFloat => "non-finite-float",
            CanonicalError::DepthExceeded { .. } => "depth-exceeded",
            CanonicalError::SubtreeTooLarge { .. } => "subtree-too-large",
            CanonicalError::Truncated { .. } => "truncated",
            CanonicalError::VarintTooLong { .. } => "varint-too-long",
            CanonicalError::UnknownTag { .. } => "unknown-tag",
            CanonicalError::BadStrTable { .. } => "bad-strtab",
            CanonicalError::StrIdOutOfRange { .. } => "str-id-out-of-range",
            CanonicalError::DuplicateKey { .. } => "duplicate-key",
            CanonicalError::SubtreeLenMismatch { .. } => "subtree-len-mismatch",
            CanonicalError::SubtreeLenOverflow { .. } => "subtree-len-overflow",
            CanonicalError::TrailingBytes { .. } => "trailing-bytes",
            CanonicalError::PathNotFound { .. } => "path-not-found",
            CanonicalError::Source(_) => "source-unavailable",
            CanonicalError::PathTypeMismatch { .. } => "path-type-mismatch",
        }
    }
}

impl std::fmt::Display for CanonicalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanonicalError::KeyOrderUnavailable => write!(
                f,
                "canonical 需要对象保序（spec §18 C2）：这个对象是 `Map::Unordered`（按字节序排序），\
                 拒绝退化成「按排序写」。请显式用 `parse_ordered()` / `Map::ordered()` / \
                 `OrderedMap` 构造有序对象后重试 —— 开 feature 不解决问题"
            ),
            CanonicalError::NonFiniteFloat => write!(
                f,
                "非有限浮点（NaN / ±Inf）没有 SML 字面量，canonical 拒绝伪造（spec §18 C13）；\
                 确需保存请显式转成字符串（\"nan\" / \"inf\"）"
            ),
            CanonicalError::DepthExceeded { depth } => write!(
                f,
                "canonical 深度 {depth} 超过上限 {MAX_VALUE_DEPTH}，拒绝输出 [E-LIMIT-004]"
            ),
            CanonicalError::SubtreeTooLarge { len } => write!(
                f,
                "单节点载荷 {len} 字节超过 4 GiB，subtree_len:u32 放不下（spec §19.2）"
            ),
            CanonicalError::Truncated { at } => {
                write!(f, "canonical 字节在偏移 {at} 处提前结束（截断）")
            }
            CanonicalError::VarintTooLong { at } => {
                write!(f, "canonical 字节在偏移 {at} 处的 LEB128 超过 10 字节")
            }
            CanonicalError::UnknownTag { tag, at } => {
                write!(f, "canonical 字节在偏移 {at} 处的节点 tag {tag} 未定义")
            }
            CanonicalError::BadStrTable { index } => {
                write!(f, "STRTAB 第 {index} 项不是合法 UTF-8")
            }
            CanonicalError::StrIdOutOfRange { id } => {
                write!(f, "节点引用了不存在的 STRTAB id {id}")
            }
            CanonicalError::DuplicateKey { key } => {
                write!(f, "对象里出现重复键 {key:?}（D15：不做隐式 last-wins）")
            }
            CanonicalError::SubtreeLenMismatch {
                at,
                declared,
                actual,
            } => write!(
                f,
                "偏移 {at} 处 subtree_len 声明 {declared}，实际消费 {actual}"
            ),
            CanonicalError::SubtreeLenOverflow { at, declared } => write!(
                f,
                "偏移 {at} 处 subtree_len 声明 {declared}，超出剩余字节"
            ),
            CanonicalError::Source(why) => write!(f, "字节源取不到数据：{why}"),
            CanonicalError::PathNotFound { seg } => {
                write!(f, "路径段 `{seg}` 在节点树里不存在")
            }
            CanonicalError::PathTypeMismatch { seg } => {
                write!(f, "路径段 `{seg}` 落在了标量上，或对非数组取了下标")
            }
            CanonicalError::TrailingBytes { at } => {
                write!(f, "树读完后在偏移 {at} 处还有多余字节")
            }
        }
    }
}

impl std::error::Error for CanonicalError {}

// ---------------------------------------------------------------------------
// 文本 canonical（C1–C13）
// ---------------------------------------------------------------------------

/// 判定移到**值层**：树里每个对象都必须显式是 `Map::Ordered`。
/// `preserve_order` 编译开关不再参与判定 —— 「能不能签」由你手里这个对象决定。
/// A [`Value`] **proven** to have source-ordered objects at every level.
/// 已证明「每个对象都是保序态」的值 —— 把 C2 的检查从**运行期**挪到**类型层**。
///
/// 为什么需要它：`parse()` 默认产出 `Map::Unordered`（历史字节序），而 `dump` 要求源序，
/// 于是「最自然的 `parse → dump`」会在运行期失败。有了这个 newtype，同样的错误在
/// **编译期**就暴露：`dump` 只收 `&CanonicalValue`，而它只能由 `TryFrom<Value>` 得到。
///
/// ```ignore
/// let cv = smlb::CanonicalValue::try_from(sml::parse_ordered(text)?)?;
/// let bytes = smlb::dump(&cv)?;          // 编译期就保证了有序
/// ```
///
/// 注意 `TryFrom` 的错误是 [`CanonicalError::KeyOrderUnavailable`] —— 转换点上必须
/// **显式**处理，这正是「少隐式」：不允许悄悄排序，也不允许悄悄通过。
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalValue(Value);

impl CanonicalValue {
    /// Borrow the inner value. ｜ 借用内部值。
    pub fn as_value(&self) -> &Value {
        &self.0
    }

    /// Take the inner value back. ｜ 取回内部值。
    pub fn into_value(self) -> Value {
        self.0
    }

    /// Canonical text — cannot fail with `KeyOrderUnavailable` by construction.
    /// canonical 文本；由构造保证**不会**再报 `KeyOrderUnavailable`。
    pub fn canonical_text(&self) -> Result<String, CanonicalError> {
        self.0.canonical_text()
    }

    /// Canonical bytes — same guarantee. ｜ canonical 字节，同样的保证。
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CanonicalError> {
        self.0.canonical_bytes()
    }

    /// Canonical segment blocks — same guarantee. ｜ canonical 段块，同样的保证。
    pub fn canonical_blocks(&self) -> Result<CanonicalBlocks, CanonicalError> {
        self.0.canonical_blocks()
    }

    /// Order-sensitive equality. ｜ 顺序敏感相等。
    pub fn eq_ordered(&self, other: &CanonicalValue) -> bool {
        self.0.eq_ordered(&other.0)
    }
}

impl TryFrom<Value> for CanonicalValue {
    type Error = CanonicalError;

    /// The single conversion point: every object must already be `Map::Ordered`.
    /// 唯一转换点：每个对象必须已经是 `Map::Ordered`。
    fn try_from(v: Value) -> Result<Self, Self::Error> {
        if v.all_objects_ordered() {
            Ok(CanonicalValue(v))
        } else {
            Err(CanonicalError::KeyOrderUnavailable)
        }
    }
}

impl TryFrom<&Value> for CanonicalValue {
    type Error = CanonicalError;

    /// Check first, clone only on success（避免为一次失败转换白拷整棵树）。
    fn try_from(v: &Value) -> Result<Self, Self::Error> {
        if v.all_objects_ordered() {
            Ok(CanonicalValue(v.clone()))
        } else {
            Err(CanonicalError::KeyOrderUnavailable)
        }
    }
}

impl Value {
    /// Every object in this tree explicitly carries source order.
    /// 树里每个对象都显式是保序态（判定看**值**，不看 feature）。
    fn all_objects_ordered(&self) -> bool {
        match self {
            Value::Object(m) => m.is_ordered() && m.values().all(|v| v.all_objects_ordered()),
            Value::Array(a) => a.iter().all(|v| v.all_objects_ordered()),
            _ => true,
        }
    }
}

impl Value {
    /// Canonical text — the **signed** object (§18.2 C1–C13). ｜ canonical 文本 —— **签名对象**（§18.2 C1–C13）。
    ///
    /// Always ends with exactly **one** `\n` (C3), never contains comments (C11), and always
    /// is legal SML text (P5-d) — otherwise `diff` / code review would have no foothold and
    /// canonical would become a third syntax of its own.
    /// 输出一定以**单个** `\n` 结尾（C3）、一定不含注释（C11）、一定是合法 SML 文本（P5-d）。
    ///
    /// ⚠️ “Legal SML” stops at a **container root**: a bare top-level scalar is rejected by the
    /// parser (`E-PARSE-008`), so `canonical_text` still emits it (the signing bytes are unique)
    /// but it cannot be read back as a document. Recorded in §18.2 as an implementation note.
    /// ⚠️ 「合法 SML」的前提是**顶层为容器**：裸的顶层标量会被 parser 拒（`E-PARSE-008`），
    /// 故标量仍产出文本（签名字节唯一）但不可作为文档回读。已回写进 §18.2。
    ///
    /// # Errors / 错误
    ///
    /// - [`CanonicalError::KeyOrderUnavailable`] — an object is `Map::Unordered` (C2);
    /// - [`CanonicalError::NonFiniteFloat`] — NaN / ±Inf (C13);
    /// - [`CanonicalError::DepthExceeded`] — deeper than [`MAX_VALUE_DEPTH`] (`E-LIMIT-004`).
    ///
    /// # Example / 例子
    ///
    /// ```
    /// # {
    /// use sml_value::Value;
    ///
    /// assert_eq!(Value::Int(1).canonical_text().unwrap(), "1\n");
    /// // 1 and 1.0 must differ, else load turns the float back into an Int
    /// // 1 与 1.0 必须是两串不同文本，否则 load 回来会变成 Int
    /// assert_eq!(Value::float(1.0).canonical_text().unwrap(), "1.0\n");
    /// assert_eq!(Value::Null.canonical_text().unwrap(), "null\n");
    /// # }
    /// ```
    pub fn canonical_text(&self) -> Result<String, CanonicalError> {
        if !self.all_objects_ordered() {
            return Err(CanonicalError::KeyOrderUnavailable);
        }
        let mut out = String::new();
        match self {
            // 顶层对象写「字段列表」（文档形态，不套 `{ }`）；空对象仍按 C9 写 `{ }`。
            Value::Object(m) if !m.is_empty() => write_fields(m, 0, 0, &mut out)?,
            other => write_value(other, 0, 0, &mut out)?,
        }
        // C3：文件以单个 `\n` 结尾（上面的分支都不留尾随换行）
        out.push('\n');
        Ok(out)
    }
}

/// Object body (caller writes `{`/`}` when nested): one `key: value` per line, `\n` between
/// lines, never trailing whitespace. ｜ 对象体：每条 `键: 值`，行间 `\n`，行尾不留空白。
fn write_fields(
    m: &Map,
    indent: usize,
    depth: usize,
    out: &mut String,
) -> Result<(), CanonicalError> {
    let mut first = true;
    let mut err = None;
    for_each_ordered(m, |k, v| {
        if err.is_some() {
            return;
        }
        if !first {
            out.push('\n');
        }
        first = false;
        push_indent(out, indent);
        out.push_str(&quote_if_needed(k));
        out.push(':');
        // 标量与单行容器写在同一行；多行容器的 `{` / `[` 也留在同一行（`键: {`），
        // 让「键与它的值永远相邻」，这是本节唯一的排版自由度，必须是定值。
        out.push(' ');
        if let Err(e) = write_value(v, indent, depth + 1, out) {
            err = Some(e);
        }
    });
    match err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Write one value (scalar or container); a multi-line container's closing bracket lands on
/// column `indent`. ｜ 写一个值（标量 / 容器），多行容器的收尾括号落在 `indent` 列。
fn write_value(
    v: &Value,
    indent: usize,
    depth: usize,
    out: &mut String,
) -> Result<(), CanonicalError> {
    if depth > MAX_VALUE_DEPTH {
        return Err(CanonicalError::DepthExceeded { depth });
    }
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Int(i) => {
            // C4：十进制 ASCII，无前导零、无 `+`；i64 里没有 -0，故无需归一。
            out.push_str(&i.to_string());
        }
        Value::Float(f, _raw) => {
            // C5 + C12：**忽略 raw**。raw 是解析期的书写痕迹（`1e10` vs `10000000000.0`），
            // 不参与相等比较，若让它进 canonical，「同一逻辑数据同字节」立刻不成立 ——
            // 同一份数据从文本读入与从 `.smlb` 读入会得到两串不同文本，签名随之分裂。
            if !f.is_finite() {
                return Err(CanonicalError::NonFiniteFloat);
            }
            out.push_str(&format_float(*f, None));
        }
        Value::Str(s) => out.push_str(&quote_if_needed(s)),
        Value::Array(a) if a.is_empty() => out.push_str("[ ]"),
        Value::Object(m) if m.is_empty() => out.push_str("{ }"),
        Value::Array(a) if is_flat(a.iter()) => {
            out.push('[');
            for e in a.iter() {
                // C8：元素之间用**空格**分隔（不用逗号），首尾各留一个空格
                out.push(' ');
                write_value(e, indent, depth + 1, out)?;
            }
            out.push_str(" ]");
        }
        Value::Object(m) if is_flat(m.values()) => {
            out.push('{');
            let mut first = true;
            let mut err = None;
            for_each_ordered(m, |k, val| {
                if err.is_some() {
                    return;
                }
                if !first {
                    out.push(',');
                }
                first = false;
                out.push(' ');
                out.push_str(&quote_if_needed(k));
                out.push_str(": ");
                if let Err(e) = write_value(val, indent, depth + 1, out) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
            out.push_str(" }");
        }
        Value::Array(a) => {
            out.push('[');
            for e in a.iter() {
                out.push('\n');
                push_indent(out, indent + 1);
                write_value(e, indent + 1, depth + 1, out)?;
            }
            out.push('\n');
            push_indent(out, indent);
            out.push(']');
        }
        Value::Object(m) => {
            out.push('{');
            out.push('\n');
            // 字段行的缩进由 `write_fields` 负责（它写 indent + 1 列）
            write_fields(m, indent + 1, depth, out)?;
            out.push('\n');
            push_indent(out, indent);
            out.push('}');
        }
    }
    Ok(())
}

/// Are all direct children scalars? Decides one-line vs multi-line.
/// 直接子项是否全为标量（决定「单行」还是「分行」）。
///
/// One level only, no recursion: a recursive version is vacuously true (any container's
/// descendants eventually reach scalars), i.e. it decides nothing — that exact trap was
/// hit in `dump.rs`. ｜ 只看一层、不递归（递归版判据恒真，等于没判）。
fn is_flat<'a, I>(mut it: I) -> bool
where
    I: Iterator<Item = &'a Value>,
{
    it.all(|v| !matches!(v, Value::Object(_) | Value::Array(_)))
}

/// Push `n` levels of 2-space indentation (C3: indentation is fixed at 2 spaces).
/// 压入 `n` 层 2 空格缩进（C3：缩进固定 2 空格）。
fn push_indent(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push_str("  ");
    }
}

/// Is this one of the metadata keys C10 hoists to the front?
/// 键是否属于 C10 的固定前置元数据键。
fn is_meta_key(k: &str) -> bool {
    META_KEYS.contains(&k)
}

/// Walk an object in C10 order: the four metadata keys first (in `META_KEYS` order), then
/// the rest in source order. ｜ 按 C10 的次序遍历对象：四个元数据键固定最前，其余按源序。
///
/// The explicit `&'a str / &'a Value` lifetimes are **required**: the byte encoder borrows
/// keys and string values into the STRTAB, and the elided version would tie them to
/// “lives only for the callback”, which cannot be lent out.
/// 生命周期写成 `&'a` 是**必须**的：编码器要把键与字符串值的 `&str` 借进 STRTAB，
/// 省略版会变成「只在回调内存活」的匿名生命周期，借不出去。
fn for_each_ordered<'a, F>(m: &'a Map, mut f: F)
where
    F: FnMut(&'a str, &'a Value),
{
    for k in META_KEYS {
        if let Some(v) = m.get(k) {
            f(k, v);
        }
    }
    for (k, v) in m.iter() {
        // 注意：`&String` / `&str` 两种载体都在这里走到 —— 交给参数位置的
        // deref 强制转换（`f(k, v)` / `is_meta_key(k)`），别写 `k.as_str()`。
        if !is_meta_key(k) {
            f(k, v);
        }
    }
}

// ---------------------------------------------------------------------------
// Byte canonical (§19) / 字节 canonical
// ---------------------------------------------------------------------------
//
// `canonical_bytes` is a **self-contained** “L0 encoding”: two blocks concatenated in a
// fixed order — `STRTAB` first, node tree second. §19.1 has them as two segments; T0 has no
// container yet, so they are concatenated in §19.2's in-segment format, with a `count:varint`
// added in front of the STRTAB to make the boundary locatable. (A standalone segment is
// delimited by its segment header's `len`; the count becomes unnecessary once containerised.)
// `canonical_bytes` 是**自包含**的「L0 编码」：`STRTAB` 在前、节点树在后（§19.1 里它们是
// 两个段；T0 没有容器，故按 §19.2 的段内格式原样串联，并在 STRTAB 前补一个 `count:varint`
// 以便定位边界：段独立成型时由段头 `len` 界定，容器化后可省）。
//
// ```text
// canonical_bytes = strtab ‖ node_tree
// strtab          = count:varint ‖ (len:varint ‖ utf8)*     // dedup, id = first-seen order
// node            = tag:u8 ‖ flags:u8 ‖ subtree_len:u32le ‖ payload
// payload(Null/False/True) = empty
// payload(Int)             = zigzag:varint
// payload(Float)           = f64 little-endian, 8 B
// payload(Str)             = str_id:varint
// payload(Arr)             = count:varint ‖ node*
// payload(Obj)             = count:varint ‖ (key_id:varint ‖ node)*   // source order
// ```
//
// `subtree_len` is the **payload** length (excluding the 6 B node header) ⇒ skipping a
// subtree is `pos + 4 + subtree_len` (§19.2). ｜ `subtree_len` = **载荷**字节数（不含 6 B
// 节点头）⇒ 跳过子树是 `pos + 4 + subtree_len`。

/// The two `.smlb` segment bodies produced by one encode: `STRTAB` and `VALUE`.
/// 一次编码产出的两个 `.smlb` 段体：`STRTAB` 与 `VALUE`。
///
/// Both are **segment bodies**, not segments: no 41 B segment header, no alignment, no
/// `count` prefix — those belong to the container (`smlb`, T1) and to the L0 form
/// (`canonical_bytes`, T0) respectively.
/// 两者都只是**段体**：没有 41 B 段头、不涉及对齐、不带 `count` 前缀 —— 那些分别属于
/// 容器层（`smlb`，T1）与 L0 形态（`canonical_bytes`，T0）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalBlocks {
    /// `(len:varint ‖ utf8)*` — **no** entry count (a segment header's `len` is the boundary).
    /// `(len:varint ‖ utf8)*`，**不带**表项个数（段头 `len` 就是边界）。
    pub strtab: Vec<u8>,
    /// Preorder node tree; identical to the tail of [`Value::canonical_bytes`].
    /// 前序节点树；与 [`Value::canonical_bytes`] 的尾部逐字节相同。
    pub value: Vec<u8>,
}

impl Value {
    /// Canonical bytes — the `VALUE` segment payload of `.smlb` (§19). ｜ canonical 字节 —— `.smlb` 的 `VALUE` 段载荷（§19）。
    ///
    /// Same logical data ⇒ same bytes (C12), whatever the origin. `Int(1)` and `Float(1.0)`
    /// are told apart by `tag` (not by spelling), and `subtree_len` lets a whole subtree be
    /// skipped in O(1). ｜ 同一逻辑数据 ⇒ 同一串字节（C12）：`Int(1)` 与 `Float(1.0)` 靠
    /// `tag` 区分（不靠字面量），`subtree_len` 让整棵子树 O(1) 跳过。
    ///
    /// # Errors / 错误
    ///
    /// Same as [`Value::canonical_text`] (C2 key order / C13 non-finite float / depth), plus
    /// [`CanonicalError::SubtreeTooLarge`] (one node over 4 GiB).
    ///
    /// # Example / 例子
    ///
    /// ```
    /// # {
    /// use sml_value::Value;
    ///
    /// let a = Value::Int(1).canonical_bytes().unwrap();
    /// let b = Value::float(1.0).canonical_bytes().unwrap();
    /// assert_ne!(a, b); // tag 3 vs tag 4
    ///
    /// // 往返：字节 → 值 → 字节，逐字节相同
    /// let v = Value::from_canonical_bytes(&a).unwrap();
    /// assert_eq!(v.canonical_bytes().unwrap(), a);
    /// # }
    /// ```
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CanonicalError> {
        if !self.all_objects_ordered() {
            return Err(CanonicalError::KeyOrderUnavailable);
        }
        let (count, strtab, nodes) = self.encode_blocks()?;
        let mut out = Vec::with_capacity(strtab.len() + nodes.len() + 8);
        // L0 形态：把「表项个数」写在最前，才能定位两张表的分界（§19.5 L3）
        write_uvarint(&mut out, count as u64);
        out.extend_from_slice(&strtab);
        out.extend_from_slice(&nodes);
        Ok(out)
    }

    /// The same bytes, **split into the two `.smlb` segments**: `(STRTAB, VALUE)`.
    /// 同一串字节，但**拆成两个 `.smlb` 段**（供容器层使用）。
    ///
    /// Why it exists / 为什么要有它：§19.1 把 `STRTAB` 与 `VALUE` 定为两个段，
    /// 而段头自带 `len`，段内就不需要「表项个数」了（L3）。容器层（`smlb`）拿到这两个
    /// 块即可直接落段，**不必自己解析 L0 前缀**，也不必复制一遍编码逻辑。
    ///
    /// `strtab` 的形态是 `(len:varint ‖ utf8)*`（不含个数）；`value` 是节点树。
    pub fn canonical_blocks(&self) -> Result<CanonicalBlocks, CanonicalError> {
        if !self.all_objects_ordered() {
            return Err(CanonicalError::KeyOrderUnavailable);
        }
        let (_count, strtab, value) = self.encode_blocks()?;
        Ok(CanonicalBlocks { strtab, value })
    }

    /// Inverse of [`Value::canonical_blocks`] — decode from the two segment bodies.
    /// [`Value::canonical_blocks`] 的逆运算：从两个段体解码。
    ///
    /// `strtab` 没有个数前缀 ⇒ 表项一直读到缓冲结尾（段头 `len` 就是个数边界）；
    /// `value` 必须**恰好**用完，多一个字节即 [`CanonicalError::TrailingBytes`]。
    pub fn from_canonical_blocks(strtab: &[u8], value: &[u8]) -> Result<Value, CanonicalError> {
        let mut sr = Reader::new(strtab);
        let mut table: Vec<&str> = Vec::new();
        while sr.pos < strtab.len() {
            let index = table.len();
            let len = sr.uvarint()?;
            let raw = sr.take(len)?;
            let s = std::str::from_utf8(raw).map_err(|_| CanonicalError::BadStrTable { index })?;
            table.push(s);
        }
        let mut vr = Reader::new(value);
        let v = decode_node(&mut vr, &table, 0)?;
        if vr.pos != value.len() {
            return Err(CanonicalError::TrailingBytes { at: vr.pos });
        }
        Ok(v)
    }

    /// Encode once, hand back the pieces: `(strtab_count, strtab_body, value_body)`.
    /// 编码一次，交出三块：表项个数、表体、节点体。
    fn encode_blocks(&self) -> Result<(usize, Vec<u8>, Vec<u8>), CanonicalError> {
        let mut tab = StrTab::default();
        let mut nodes = Vec::with_capacity(64);
        encode_node(self, &mut nodes, &mut tab, 0)?;
        let mut strtab = Vec::with_capacity(16);
        for s in &tab.table {
            write_uvarint(&mut strtab, s.len() as u64);
            strtab.extend_from_slice(s.as_bytes());
        }
        Ok((tab.table.len(), strtab, nodes))
    }

    /// Inverse of [`Value::canonical_bytes`] — the T0 “L0” decoder. ｜ canonical 字节 → 值。
    ///
    /// T1's `load()` will add the container (segment table / manifest / layer chain) on top;
    /// this function only accepts a **bare** `canonical_bytes` string and reports
    /// [`CanonicalError::TrailingBytes`] for a single extra byte — otherwise “appending never
    /// rewrites old bytes” could be broken silently.
    /// T1 的 `load()` 会在它之上加容器；本函数只认裸字节串，多一个字节都报错。
    pub fn from_canonical_bytes(bytes: &[u8]) -> Result<Value, CanonicalError> {
        let mut r = Reader::new(bytes);
        let count = r.uvarint()?;
        // 防御：每个表项至少 1 B（`len` 变长整数），不可能超过总字节数。
        if count > bytes.len() as u64 {
            return Err(CanonicalError::Truncated { at: r.pos });
        }
        let mut table: Vec<&str> = Vec::with_capacity(count as usize);
        for index in 0..count as usize {
            let len = r.uvarint()?;
            let raw = r.take(len)?;
            let s = std::str::from_utf8(raw).map_err(|_| CanonicalError::BadStrTable { index })?;
            table.push(s);
        }
        let v = decode_node(&mut r, &table, 0)?;
        if r.pos != bytes.len() {
            return Err(CanonicalError::TrailingBytes { at: r.pos });
        }
        Ok(v)
    }
}

/// Global string table: content → id, id = first-seen order (preorder + source order) ⇒ deterministic.
/// 全局字符串表：内容 → id，id = 首次出现序（前序 + 源序遍历）⇒ 确定性。
///
/// Keys and string values **share one table** (§19.2: keys go through `STRTAB` too), so
/// `"rule"` as a key and as a value costs one slot. Borrows `&'a str` instead of `String`:
/// the whole encoding is copy-free.
/// 键与字符串值**共用一张表**，故 `"rule"` 作为键和作为值只占一份；借用 `&'a str` 而非
/// `String`：编码全程零拷贝。
#[derive(Default)]
struct StrTab<'a> {
    ids: HashMap<&'a str, u32>,
    table: Vec<&'a str>,
}

impl<'a> StrTab<'a> {
    fn id_of(&mut self, s: &'a str) -> u32 {
        if let Some(&id) = self.ids.get(s) {
            return id;
        }
        let id = self.table.len() as u32;
        self.table.push(s);
        self.ids.insert(s, id);
        id
    }
}

/// Write one node: reserve `subtree_len`, backfill it after the children are written (§19.2).
/// 写一个节点：先占位 `subtree_len`，子节点写完后回填。
fn encode_node<'a>(
    v: &'a Value,
    out: &mut Vec<u8>,
    tab: &mut StrTab<'a>,
    depth: usize,
) -> Result<(), CanonicalError> {
    if depth > MAX_VALUE_DEPTH {
        return Err(CanonicalError::DepthExceeded { depth });
    }
    let (tag, flags) = match v {
        Value::Null => (TAG_NULL, 0),
        Value::Bool(false) => (TAG_FALSE, 0),
        Value::Bool(true) => (TAG_TRUE, 0),
        Value::Int(_) => (TAG_INT, 0),
        Value::Float(..) => (TAG_FLOAT, 0),
        Value::Str(_) => (TAG_STR, 0),
        Value::Array(_) => (TAG_ARR, 0),
        Value::Object(m) => (
            TAG_OBJ,
            if m.contains_key("__type") {
                FLAG_HAS_TYPE
            } else {
                0
            },
        ),
    };
    out.push(tag);
    out.push(flags);
    let len_at = out.len();
    out.extend_from_slice(&[0, 0, 0, 0]);
    let payload_at = out.len();

    match v {
        Value::Null | Value::Bool(_) => {}
        Value::Int(i) => {
            // zigzag：小值（|v| < 64）占 1 B
            write_uvarint(out, zigzag(*i));
        }
        Value::Float(f, _raw) => {
            // C13：NaN / ±Inf 在 canonical 里一律报错（文本与字节同一口径）。
            if !f.is_finite() {
                return Err(CanonicalError::NonFiniteFloat);
            }
            out.extend_from_slice(&f.to_le_bytes());
        }
        Value::Str(s) => {
            let id = tab.id_of(s);
            write_uvarint(out, id as u64);
        }
        Value::Array(a) => {
            write_uvarint(out, a.len() as u64);
            for e in a {
                encode_node(e, out, tab, depth + 1)?;
            }
        }
        Value::Object(m) => {
            // §19.2：元数据键**不特化**（就是普通键），按源序写 ——
            // 与 C10 的「文本里固定最前」是两件事，差异已在 §18.2/§19.2 登记。
            write_uvarint(out, m.len() as u64);
            let mut err = None;
            for_each_source_order(m, |k, val| {
                if err.is_some() {
                    return;
                }
                let id = tab.id_of(k);
                write_uvarint(out, id as u64);
                if let Err(e) = encode_node(val, out, tab, depth + 1) {
                    err = Some(e);
                }
            });
            if let Some(e) = err {
                return Err(e);
            }
        }
    }

    let payload_len = out.len() - payload_at;
    if payload_len > MAX_SUBTREE_LEN {
        return Err(CanonicalError::SubtreeTooLarge { len: payload_len });
    }
    out[len_at..len_at + 4].copy_from_slice(&(payload_len as u32).to_le_bytes());
    Ok(())
}

/// Pure source-order walk — the byte side does **not** hoist metadata keys (§19.2).
/// 纯源序遍历（字节侧不重排元数据键）。
fn for_each_source_order<'a, F>(m: &'a Map, mut f: F)
where
    F: FnMut(&'a str, &'a Value),
{
    for (k, v) in m.iter() {
        f(k, v);
    }
}

#[inline]
fn zigzag(i: i64) -> u64 {
    ((i << 1) ^ (i >> 63)) as u64
}

#[inline]
fn unzigzag(z: u64) -> i64 {
    ((z >> 1) as i64) ^ -((z & 1) as i64)
}

fn write_uvarint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn take(&mut self, n: u64) -> Result<&'a [u8], CanonicalError> {
        // 先按 u64 比，再转 usize：32 位目标上 `n as usize` 会截断，
        // 「要 4 GiB 却只给了 0」会变成「读到 0 字节还成功」。
        if n > self.remaining() as u64 {
            return Err(CanonicalError::Truncated { at: self.pos });
        }
        let n = n as usize;
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, CanonicalError> {
        let s = self.take(1)?;
        Ok(s[0])
    }

    fn u32le(&mut self) -> Result<u32, CanonicalError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn f64le(&mut self) -> Result<f64, CanonicalError> {
        let s = self.take(8)?;
        Ok(f64::from_le_bytes([
            s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
        ]))
    }

    fn uvarint(&mut self) -> Result<u64, CanonicalError> {
        let start = self.pos;
        let mut result: u64 = 0;
        let mut shift = 0;
        loop {
            if shift > 63 {
                return Err(CanonicalError::VarintTooLong { at: start });
            }
            let byte = self.u8()?;
            result |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(result);
            }
            shift += 7;
        }
    }
}

/// A forward-only cursor over the **node tree** bytes — the seam that lets a caller supply
/// bytes that are not in one contiguous buffer (e.g. zstd frames).
/// 节点树上的**只进游标** —— 这条接缝让调用方能提供「不在同一块内存里」的字节（如 zstd 帧）。
///
/// Why not `&[u8]`: a framed file must be able to **skip** a subtree without decompressing it.
/// Skipping is pure arithmetic; only the bytes actually read (node headers, key ids) and the
/// final range need materialising.
/// 为什么不是 `&[u8]`：分帧文件必须能**跳过**一棵子树而不解压它 —— 跳过是纯算术，
/// 只有真正读到的字节（节点头、key id）与最终目标区间才需要物化。
pub trait TreeCursor {
    /// Current logical offset. ｜ 当前逻辑偏移。
    fn pos(&self) -> usize;
    /// Bytes left. ｜ 剩余字节数。
    fn remaining(&self) -> usize;
    /// Read one byte. ｜ 读 1 字节。
    fn read_u8(&mut self) -> Result<u8, CanonicalError>;
    /// Read a little-endian u32. ｜ 读小端 u32。
    fn read_u32le(&mut self) -> Result<u32, CanonicalError>;
    /// Read a LEB128 u64. ｜ 读 LEB128 u64。
    fn read_uvarint(&mut self) -> Result<u64, CanonicalError>;
    /// Advance without reading (the whole point for framed bytes).
    /// 只前进不读取（对分帧字节来说，这正是重点）。
    fn skip(&mut self, n: usize) -> Result<(), CanonicalError>;
}

/// The plain-buffer cursor: zero copy, the bytes are already in memory.
/// 普通缓冲游标：零拷贝，字节本来就在内存里。
pub struct SliceCursor<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> SliceCursor<'a> {
    /// Wrap a `VALUE` body. ｜ 包一个 `VALUE` 段体。
    pub fn new(buf: &'a [u8]) -> Self {
        SliceCursor { buf, pos: 0 }
    }
}

impl<'a> TreeCursor for SliceCursor<'a> {
    fn pos(&self) -> usize {
        self.pos
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn read_u8(&mut self) -> Result<u8, CanonicalError> {
        if self.pos >= self.buf.len() {
            return Err(CanonicalError::Truncated { at: self.pos });
        }
        let b = self.buf[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_u32le(&mut self) -> Result<u32, CanonicalError> {
        if self.remaining() < 4 {
            return Err(CanonicalError::Truncated { at: self.pos });
        }
        let s = &self.buf[self.pos..self.pos + 4];
        self.pos += 4;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
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

/// A node's **raw byte range** inside the `VALUE` body — zero copy, no `Value` built.
/// 节点在 `VALUE` 段体里的**原始字节范围** —— 零拷贝，不构造 `Value`。
///
/// `bytes` includes the 6 B node header (`tag:u8 ‖ flags:u8 ‖ len:u32`), i.e. exactly the
/// slice one could hand back to [`Value::from_canonical_bytes`]-style decoding.
/// `bytes` 含 6 B 节点头，正好是可以直接再解码的那一段。
#[derive(Debug, Clone, Copy)]
pub struct Subtree<'a> {
    /// The node's bytes, header included ｜ 节点字节（含头）。
    pub bytes: &'a [u8],
    /// How many nodes the walk **looked at**: one per level on the path, plus every sibling it
    /// skipped by `subtree_len` (headers read, payloads never decoded).
    /// 走表时**看过**的节点数：路径上每层一个，加上每个按 `subtree_len` 跳过的兄弟
    /// （只读了头，载荷一个字节都没解码）。
    ///
    /// 口径的意义：这个数**只**反映「逐兄弟比较」的开销（`DIR` 将来要省的那部分），
    /// 而**解码量**恒等于目标子树的大小 —— 后者才是「不遍历全树」的判据。
    /// ｜ It measures the sibling scan only; the number of **decoded** nodes equals the target
    /// subtree, which is what “does not traverse the whole tree” asserts.
    pub nodes_visited: usize,
}

/// Find `path` in the node tree and return its raw byte range, **skipping siblings by
/// `subtree_len`** — no `Value` is built and no unrelated subtree is decoded.
/// 在节点树里按路径找到目标，返回它的原始字节范围；**兄弟节点靠 `subtree_len` 跳过**，
/// 不构造 `Value`、不解无关子树。
///
/// This is what makes “取单个子树不遍历全树” true at the byte level: descending costs
/// O(sum of sibling lengths skipped) in *pointer arithmetic*, and O(children) in comparisons —
/// never O(size of the document) in allocations.
/// 这就是「取单个子树不遍历全树」在字节层面的实现：下行只花指针算术与逐兄弟比较，
/// 分配量与文档大小无关。
///
/// - an empty path returns the root subtree ｜ 空路径返回根子树；
/// - `strtab` is the `STRTAB` segment body (same shape as [`Value::canonical_blocks`]'s);
///   对象键要靠它把 `key_id` 还原成字符串 ｜ 对象键要靠它还原成字符串。
pub fn subtree_at<'a>(
    strtab: &[u8],
    value: &'a [u8],
    path: &Path,
) -> Result<Subtree<'a>, CanonicalError> {
    let table = parse_strtab(strtab)?;
    subtree_at_with_table(&table, value, path)
}

/// Same as [`subtree_at`], but takes an **already parsed** `STRTAB` table.
/// 同 [`subtree_at`]，但复用**已经解析好**的 `STRTAB` 表。
pub fn subtree_at_with_table<'a>(
    table: &[&str],
    value: &'a [u8],
    path: &Path,
) -> Result<Subtree<'a>, CanonicalError> {
    let mut cur = SliceCursor::new(value);
    let (start, end, visited) = descend(&mut cur, table, path.segments(), 0)?;
    Ok(Subtree {
        bytes: &value[start..end],
        nodes_visited: visited,
    })
}

/// Locate one subtree over **any** cursor; returns its `[start, end)` byte range.
/// 在**任意**游标上定位一棵子树，返回它的 `[start, end)` 字节区间。
///
/// A range instead of bytes keeps both callers happy: a plain buffer slices it out for free, a
/// framed reader materialises exactly that range (⟹ only the frames it overlaps are inflated).
/// 返回区间而不是字节：切片路径白拿，分帧路径只解覆盖这段的那几帧。
pub fn subtree_range<C: TreeCursor>(
    cur: &mut C,
    table: &[&str],
    segs: &[Seg],
    depth: usize,
) -> Result<(usize, usize, usize), CanonicalError> {
    descend(cur, table, segs, depth)
}

/// Parse the `STRTAB` body into a lookup table (for [`subtree_range`] / [`value_from_subtree`]).
/// 把 `STRTAB` 段体解析成查找表（供 [`subtree_range`] / [`value_from_subtree`] 使用）。
pub fn parse_strtab_table(strtab: &[u8]) -> Result<Vec<&str>, CanonicalError> {
    parse_strtab(strtab)
}

/// Decode a **single** subtree produced by [`subtree_at`] into a `Value`.
/// 把 [`subtree_at`] 拿到的**单个子树**解成 `Value`。
///
/// `subtree` must be exactly one node: trailing bytes are rejected (same rule as
/// [`Value::from_canonical_bytes`], otherwise “append only” could be broken silently).
/// `subtree` 必须正好是一个节点，多一个字节就报 `TrailingBytes`。
pub fn value_from_subtree(strtab: &[u8], subtree: &[u8]) -> Result<Value, CanonicalError> {
    let table = parse_strtab(strtab)?;
    value_from_subtree_with_table(&table, subtree)
}

/// Same as [`value_from_subtree`], but takes an **already parsed** `STRTAB` table.
/// 同 [`value_from_subtree`]，但复用**已经解析好**的 `STRTAB` 表。
///
/// Why: `Store::get` used to re-walk the whole `STRTAB` body on every call (twice on the
/// framed path — once for navigation, once here). The table is a pure function of the
/// container's `STRTAB` segment, so it can be parsed once at `open` and reused.
/// ｜ The per-call O(key count) scan was the dominant cost of random access.
pub fn value_from_subtree_with_table(
    table: &[&str],
    subtree: &[u8],
) -> Result<Value, CanonicalError> {
    let mut r = Reader::new(subtree);
    let v = decode_node(&mut r, table, 0)?;
    if r.pos != subtree.len() {
        return Err(CanonicalError::TrailingBytes { at: r.pos });
    }
    Ok(v)
}

/// Parse the `STRTAB` body (`(len:varint ‖ utf8)*`, no count — the segment header is the boundary).
/// 解析 `STRTAB` 段体（`(len:varint ‖ utf8)*`，无个数 —— 段头 `len` 就是边界）。
fn parse_strtab(strtab: &[u8]) -> Result<Vec<&str>, CanonicalError> {
    let mut sr = Reader::new(strtab);
    let mut table: Vec<&str> = Vec::new();
    while sr.pos < strtab.len() {
        let index = table.len();
        let len = sr.uvarint()?;
        let raw = sr.take(len)?;
        let s = std::str::from_utf8(raw).map_err(|_| CanonicalError::BadStrTable { index })?;
        table.push(s);
    }
    Ok(table)
}

/// One node's header through any cursor: `(tag, payload_len)`, left at the payload.
/// 通过任意游标读一个节点头：返回 `(tag, 载荷长度)`，停在载荷起点。
fn read_header<C: TreeCursor>(cur: &mut C) -> Result<(u8, usize), CanonicalError> {
    let tag = cur.read_u8()?;
    let _flags = cur.read_u8()?;
    let len = cur.read_u32le()? as usize;
    if len > cur.remaining() {
        return Err(CanonicalError::SubtreeLenOverflow {
            at: cur.pos(),
            declared: len,
        });
    }
    Ok((tag, len))
}

/// Consume one whole node (header + payload) — **without reading the payload**.
/// 吃掉一整个节点（头 + 载荷）—— **不读载荷**。
fn skip_node<C: TreeCursor>(cur: &mut C) -> Result<(), CanonicalError> {
    let (_tag, len) = read_header(cur)?;
    cur.skip(len)
}

/// Walk `segs` from the node at the cursor; returns `[start, end)` and how many nodes were
/// looked at. The **target's payload is skipped over, never read**.
/// 从游标当前位置按 `segs` 下行，返回 `[start, end)` 与看过的节点数；**目标载荷只跳过、不读取**。
///
/// Skipping is why this is a range and not bytes: a framed reader can then materialise exactly
/// the frames that overlap `[start, end)` and leave the rest compressed.
/// 跳过正是「返回区间而不是字节」的理由：分帧读取器只需物化覆盖该区间的帧。
fn descend<C: TreeCursor>(
    cur: &mut C,
    table: &[&str],
    segs: &[Seg],
    depth: usize,
) -> Result<(usize, usize, usize), CanonicalError> {
    if depth > MAX_VALUE_DEPTH {
        return Err(CanonicalError::DepthExceeded { depth });
    }
    let start = cur.pos();
    let (tag, len) = read_header(cur)?;

    if segs.is_empty() {
        cur.skip(len)?;
        return Ok((start, cur.pos(), 1));
    }

    match (tag, &segs[0]) {
        (TAG_OBJ, Seg::Key(want)) => {
            let n = cur.read_uvarint()?;
            if n > (cur.remaining() / 2) as u64 {
                // 防御伪造的巨大 count（每项至少 key_id + 6 B 节点头 ⇒ 下界取 2 保守）
                return Err(CanonicalError::Truncated { at: cur.pos() });
            }
            let mut visited = 0usize;
            for _ in 0..n {
                let key_id = cur.read_uvarint()?;
                let key = table
                    .get(key_id as usize)
                    .ok_or(CanonicalError::StrIdOutOfRange { id: key_id })?;
                if key == want {
                    let (s, e, sub) = descend(cur, table, &segs[1..], depth + 1)?;
                    return Ok((s, e, sub + 1 + visited));
                }
                skip_node(cur)?;
                visited += 1;
            }
            Err(CanonicalError::PathNotFound {
                seg: want.clone(),
            })
        }
        (TAG_ARR, Seg::Index(want)) => {
            let n = cur.read_uvarint()?;
            if n > (cur.remaining() / 6) as u64 {
                return Err(CanonicalError::Truncated { at: cur.pos() });
            }
            if *want >= n as usize {
                return Err(CanonicalError::PathNotFound {
                    seg: format!("[{want}]"),
                });
            }
            let mut visited = 0usize;
            for idx in 0..n as usize {
                if idx == *want {
                    let (s, e, sub) = descend(cur, table, &segs[1..], depth + 1)?;
                    return Ok((s, e, sub + 1 + visited));
                }
                skip_node(cur)?;
                visited += 1;
            }
            Err(CanonicalError::PathNotFound {
                seg: format!("[{want}]"),
            })
        }
        (TAG_OBJ, other) | (TAG_ARR, other) => Err(CanonicalError::PathTypeMismatch {
            seg: match other {
                Seg::Key(k) => k.clone(),
                Seg::Index(i) => format!("[{i}]"),
            },
        }),
        _ => Err(CanonicalError::PathTypeMismatch {
            seg: match &segs[0] {
                Seg::Key(k) => k.clone(),
                Seg::Index(i) => format!("[{i}]"),
            },
        }),
    }
}

fn decode_node(
    r: &mut Reader<'_>,
    table: &[&str],
    depth: usize,
) -> Result<Value, CanonicalError> {
    if depth > MAX_VALUE_DEPTH {
        return Err(CanonicalError::DepthExceeded { depth });
    }
    let tag_at = r.pos;
    let tag = r.u8()?;
    let _flags = r.u8()?;
    let subtree_len = r.u32le()? as usize;
    let payload_at = r.pos;
    if subtree_len > r.remaining() {
        return Err(CanonicalError::SubtreeLenOverflow {
            at: payload_at,
            declared: subtree_len,
        });
    }
    let value = match tag {
        TAG_NULL => Value::Null,
        TAG_FALSE => Value::Bool(false),
        TAG_TRUE => Value::Bool(true),
        TAG_INT => Value::Int(unzigzag(r.uvarint()?)),
        TAG_FLOAT => {
            let f = r.f64le()?;
            if !f.is_finite() {
                return Err(CanonicalError::NonFiniteFloat);
            }
            Value::Float(f, None)
        }
        TAG_STR => {
            let id = r.uvarint()?;
            let s = table
                .get(id as usize)
                .ok_or(CanonicalError::StrIdOutOfRange { id })?;
            Value::Str((*s).to_string())
        }
        TAG_ARR => {
            let n = r.uvarint()?;
            // 防御：每节点至少 6 B（tag+flags+subtree_len），据此卡死伪造的巨大 count。
            if n > (r.remaining() / 6) as u64 {
                return Err(CanonicalError::Truncated { at: r.pos });
            }
            let mut a = Vec::with_capacity(n as usize);
            for _ in 0..n {
                a.push(decode_node(r, table, depth + 1)?);
            }
            Value::Array(a)
        }
        TAG_OBJ => {
            let n = r.uvarint()?;
            // 每条至少 7 B（key_id:varint + 6 B 节点头）
            if n > (r.remaining() / 7) as u64 {
                return Err(CanonicalError::Truncated { at: r.pos });
            }
            // 解码出来的对象**天然保序**：canonical 字节里的键序就是源序（§18 C2）。
            // 这不是「顺手选的」，而是让 `load()` 的结果可以直接再签一次 canonical。
            let mut m = Map::ordered();
            for _ in 0..n {
                let id = r.uvarint()?;
                let k = table
                    .get(id as usize)
                    .ok_or(CanonicalError::StrIdOutOfRange { id })?;
                if m.contains_key(*k) {
                    return Err(CanonicalError::DuplicateKey {
                        key: (*k).to_string(),
                    });
                }
                let v = decode_node(r, table, depth + 1)?;
                m.insert((*k).to_string(), v);
            }
            Value::Object(m)
        }
        other => {
            return Err(CanonicalError::UnknownTag {
                tag: other,
                at: tag_at,
            })
        }
    };
    let actual = r.pos - payload_at;
    if actual != subtree_len {
        return Err(CanonicalError::SubtreeLenMismatch {
            at: payload_at,
            declared: subtree_len,
            actual,
        });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(pairs: &[(&str, Value)]) -> Value {
        let mut m = Map::ordered();
        for (k, v) in pairs {
            m.insert((*k).to_string(), v.clone());
        }
        Value::Object(m)
    }

    fn text(v: &Value) -> String {
        v.canonical_text().expect("canonical_text")
    }

    #[test]
    fn scalars_follow_c4_to_c6() {
        assert_eq!(text(&Value::Int(0)), "0\n");
        assert_eq!(text(&Value::Int(-7)), "-7\n");
        assert_eq!(text(&Value::float(1.0)), "1.0\n");
        assert_eq!(text(&Value::float(-0.0)), "-0.0\n");
        assert_eq!(text(&Value::Bool(true)), "true\n");
        assert_eq!(text(&Value::Null), "null\n");
    }

    #[test]
    fn empty_containers_use_the_spaced_form_c9() {
        assert_eq!(text(&Value::Array(vec![])), "[ ]\n");
        assert_eq!(text(&obj(&[])), "{ }\n");
        assert_eq!(text(&obj(&[("a", Value::Array(vec![]))])), "a: [ ]\n");
    }

    #[test]
    fn key_order_is_source_order_c2() {
        let v = obj(&[("z", Value::Int(1)), ("a", Value::Int(2))]);
        assert_eq!(text(&v), "z: 1\na: 2\n");
    }

    #[test]
    fn metadata_keys_go_first_c10() {
        let v = obj(&[
            ("name", Value::Str("wea".into())),
            ("__featags", Value::Array(vec![])),
            ("__type", Value::Str("Rule".into())),
            ("__name", Value::Str("ssh".into())),
        ]);
        assert_eq!(
            text(&v),
            "__type: Rule\n__name: ssh\n__featags: [ ]\nname: wea\n"
        );
    }

    #[test]
    fn float_raw_is_ignored_c5_c12() {
        // 同一个值、两种书写痕迹 ⇒ 同一串 canonical 文本（否则签名会分裂）
        let with_raw = Value::float_with_raw(1e10, "1e10");
        let without = Value::float(1e10);
        assert_eq!(text(&with_raw), text(&without));
        assert_eq!(text(&without), "10000000000.0\n");
        assert_eq!(
            with_raw.canonical_bytes().unwrap(),
            without.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn non_finite_floats_error_c13() {
        for f in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let v = Value::float(f);
            assert_eq!(v.canonical_text(), Err(CanonicalError::NonFiniteFloat));
            assert_eq!(v.canonical_bytes(), Err(CanonicalError::NonFiniteFloat));
        }
    }

    #[test]
    fn int_and_float_are_distinguishable() {
        assert_ne!(
            Value::Int(1).canonical_bytes().unwrap(),
            Value::float(1.0).canonical_bytes().unwrap()
        );
        assert_ne!(text(&Value::Int(1)), text(&Value::float(1.0)));
    }

    #[test]
    fn bytes_differ_when_key_order_differs() {
        let a = obj(&[("a", Value::Int(1)), ("b", Value::Int(2))]);
        let b = obj(&[("b", Value::Int(2)), ("a", Value::Int(1))]);
        assert_ne!(a.canonical_bytes().unwrap(), b.canonical_bytes().unwrap());
    }

    #[test]
    fn same_value_same_bytes_regardless_of_build_path() {
        let a = obj(&[
            ("s", Value::Str("dup".into())),
            ("arr", Value::Array(vec![Value::Str("dup".into()), Value::Int(-3)])),
        ]);
        let mut m = Map::ordered();
        m.insert("s".to_string(), Value::Str("dup".into()));
        m.insert(
            "arr".to_string(),
            Value::Array(vec![Value::Str("dup".into()), Value::Int(-3)]),
        );
        assert_eq!(
            Value::Object(m).canonical_bytes().unwrap(),
            a.canonical_bytes().unwrap()
        );
    }

    #[test]
    fn strtab_dedups_strings_and_keys() {
        let v = obj(&[("k", Value::Str("k".into()))]);
        let bytes = v.canonical_bytes().unwrap();
        // count = 1（键与值同为 "k" ⇒ 只占一格）
        assert_eq!(bytes[0], 1);
        // 解码后仍是同一个键值对
        assert_eq!(Value::from_canonical_bytes(&bytes).unwrap(), v);
    }

    #[test]
    fn bytes_roundtrip_is_byte_identical() {
        let v = obj(&[
            ("a", Value::Int(-1)),
            ("b", Value::Array(vec![Value::Int(1), Value::float(2.5)])),
            ("c", obj(&[("d", Value::Null), ("e", Value::Bool(false))])),
            ("__type", Value::Str("T".into())),
            ("f", Value::Str("带引号\"的文字".into())),
        ]);
        let b1 = v.canonical_bytes().unwrap();
        let loaded = Value::from_canonical_bytes(&b1).unwrap();
        assert_eq!(loaded, v);
        assert!(loaded.eq_ordered(&v), "源序必须原样回来");
        assert_eq!(loaded.canonical_bytes().unwrap(), b1);
        assert_eq!(text(&loaded), text(&v));
    }

    #[test]
    fn depth_limit_is_explicit() {
        let mut v = Value::Int(1);
        for _ in 0..=MAX_VALUE_DEPTH {
            v = Value::Array(vec![v]);
        }
        assert!(matches!(
            v.canonical_text(),
            Err(CanonicalError::DepthExceeded { .. })
        ));
        assert!(matches!(
            v.canonical_bytes(),
            Err(CanonicalError::DepthExceeded { .. })
        ));
    }

    #[test]
    fn decoder_rejects_trailing_bytes() {
        let mut b = Value::Int(1).canonical_bytes().unwrap();
        b.push(0);
        assert!(matches!(
            Value::from_canonical_bytes(&b),
            Err(CanonicalError::TrailingBytes { .. })
        ));
    }

    #[test]
    fn decoder_rejects_unknown_tag() {
        // strtab count = 0，随后一个 tag=9 的节点
        let mut b = vec![0u8];
        b.extend_from_slice(&[9, 0, 0, 0, 0, 0]);
        assert!(matches!(
            Value::from_canonical_bytes(&b),
            Err(CanonicalError::UnknownTag { tag: 9, .. })
        ));
    }

    #[test]
    fn decoder_rejects_truncated_input() {
        let b = Value::Int(1).canonical_bytes().unwrap();
        for cut in 0..b.len() {
            assert!(Value::from_canonical_bytes(&b[..cut]).is_err(), "cut={cut}");
        }
    }

    #[test]
    fn flat_containers_stay_on_one_line_c8() {
        assert_eq!(text(&Value::Array(vec![Value::Int(1), Value::Int(2)])), "[ 1 2 ]\n");
        assert_eq!(
            text(&obj(&[("a", Value::Int(1)), ("b", Value::Int(2))])),
            "a: 1\nb: 2\n"
        );
        assert_eq!(
            text(&obj(&[("arr", Value::Array(vec![Value::Int(1), Value::Int(2)]))])),
            "arr: [ 1 2 ]\n"
        );
        // 元素里含容器 ⇒ 分行
        assert_eq!(
            text(&Value::Array(vec![Value::Array(vec![Value::Int(1)])])),
            "[\n  [ 1 ]\n]\n"
        );
    }
}
