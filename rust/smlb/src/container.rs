// SPDX-License-Identifier: MulanPSL-2.0
//! The `.smlb` **container** (T1): file header + segment table + segment data.
//! `.smlb` **容器**（T1）：文件头 + 段表 + 段数据。
//!
//! Scope / 范围：T1 只落「装壳」这一层 —— `dump` / `load` / `info` / `ls`。
//! **不做**签名与层链（T4）、压缩与加密（T5）、`DIR`（T2/T3）、`mmap`（T3）、
//! 外来载荷的语义（T2）、manifest 的内容（T2）。
//!
//! # File layout (v1) / 文件布局
//!
//! ```text
//! [0, 40)          header           见 `HEADER_LEN` 与 `dump`
//! [40, 4096)       zero padding
//! [4096, ...)      segment table    seg_count × 41 B（§19.1 的段头）
//! align_up(end)    segment data     每段 4 KiB 对齐（D28 块加密 / mmap）
//! ...
//! ```
//!
//! **段数据没有偏移字段**：它的位置由「表尾对齐，然后 `align_up(prev + prev.len)`」走
//! 出来。这样段表项保持 §19.1 的 41 B 不变，且 §18.3「manifest 不含 offset」的口径一致
//! —— 位置不是语义。代价是从第 k 段要走过前面 k 段（段数几十，纳秒级）。
//! ｜ No per-segment offset is stored: positions are walked (table end → align 4 KiB →
//! `align_up(prev + prev.len)`). Keeps the 41 B entry exactly as §19.1 defines it.
//!
//! # 三条不变量在本层的落点（§23.1）
//!
//! 1. **未知段可跳过** —— 表项自带 `len`，`load` 只认它需要的那几个 `seg_id`，
//!    其余一律跳过（不解析、不猜语义）。故「读前先跑解释器」不可能成为前置。
//! 2. **签名签 `canonical_text`** —— 本层不签任何东西（T4），但正因如此，
//!    段里的 `VALUE` 就是 T0 的字节，签名对象与容器无关。
//! 3. **追加不改旧字节** —— T1 只写「冻结文件」；`append` 属 T4，届时只追加段与
//!    Trailer（T4 的尾部表指针 `tail_off` 已在头里留好，v1 恒 0）。
//!
//! # 段摘要（T2 起）
//!
//! `sha256:32` 现在写**真摘要**（`crate::sha256`，零依赖自实现，FIPS 180-4 向量钉着）：
//! `load` 对**它用到的段**校验，`smlb check` 逐段全验并且**不提前抛错**（把每段结论都打出来）。
//! 全 0 摘要仍被接受 = 「这份文件没带摘要」，只是校验更少 —— 不是无效文件。
//! ｜ Digests are real from T2 on (`sha256.rs`); `load` verifies the segments it uses,
//! `check` verifies all of them. All-zero means “no digest”, not “invalid”.
//!
//! - **段对齐按 `flags` 位 0 声明**（§19.7 丙）：位 0 = 0 ⇒ 段体 8 字节对齐（明文，默认）；
//!   位 0 = 1 ⇒ 4 KiB 对齐（加密文件必须，T5 强制）。粒度**必须自描述**：走表靠它，
//!   猜错就整张表错位。
//! ｜ Segment alignment is declared by header `flags` bit 0 (8 B plaintext / 4 KiB encrypted).
//!
//! # manifest（T2 后半）
//!
//! `dump` 现在**总写** `0x04 MANIFEST` 段（[`crate::manifest`]）：`primary` / `hash_id` /
//! `segments[]` / `crypto` / `caps` / `patch` / `defaults` / `created` / `tool`。
//! `load` 以它为准找主内容（`primary`）、按 `hash_id` 验摘要、按 `caps` 决定「拒」还是「跳过」；
//! 没有 manifest 的旧文件退回 T1 的约定（有 `VALUE` 就是主内容，摘要按 SHA-256）。
//! ｜ `dump` always writes the manifest; `load` follows `primary` / `hash_id` / `caps`, and falls
//! back to the T1 convention for files that predate it.
//!
//! # 还没做的（不是忘了）
//!
//! `DIR`（T3）、签名与层链（T4）、压缩与加密（T5）、外来载荷的 `pack`/`unpack`（T2 余项）——
//! 后者要的段 id（`0x20 EMBED`）已登记但没人写。

use sml_value::{CanonicalError, Value};

use crate::hash::{self, HashAlgo};
use crate::manifest::{self, Manifest, ManifestError, ManifestPayload, ManifestSegment};

/// Magic, written once and never changed (`§12.3`'s existing convention).
/// 魔数，写一次永不变（沿用 §12.3 的既有约定）。
pub const MAGIC: [u8; 4] = *b"SWSB";

/// v1 header length. ｜ v1 文件头长度。
///
/// 字段排布（一律小端）：
///
/// ```text
/// off sz field           note
///  0  4  magic           "SWSB"
///  4  2  edition         低字节 = major，高字节 = minor（v4 ⇒ 04 00）
///  6  1  fmt             字节布局补丁位，常态 0（D5）
///  7  1  hdr_len         头部总长；旧读者据此跳过将来新增的字段
///  8  8  seg_table_off   u64（用户 2026-11-04：u64 现在就付）；v1 恒 = hdr_len
/// 16  4  seg_count
/// 20  2  flags           **位 0 = 段体对齐粒度**（0 = 8 B，1 = 4 KiB）；其余位忽略
/// 22  2  reserved        0
/// 24  8  tail_off        T4 尾部表预留，v1 恒 0
/// 32  8  reserved        0
/// ```
pub const HEADER_LEN: usize = 40;

/// `flags` bit 0: segment bodies are 4 KiB aligned (required once encrypted, §19.7 丙).
/// `flags` 位 0：段体 4 KiB 对齐（加密文件必须；见 §19.7 丙）。
pub const FLAG_ALIGN_BLOCK: u16 = 0b0000_0001;

/// Minimum alignment written when `flags` bit 0 is clear: 8 B (u64-friendly).
/// `flags` 位 0 为 0 时的对齐粒度：8 B。
pub const ALIGN_MIN: u64 = 8;

/// Alignment when `flags` bit 0 is set: 4 KiB (block encryption / mmap page, D28).
/// `flags` 位 0 为 1 时的对齐粒度：4 KiB。
pub const ALIGN_BLOCK: u64 = 4096;

/// Edition written by T1: `4.0` = `major 4, minor 0` ⇒ `0x0004`.
/// T1 写入的 edition：`4.0` ⇒ `0x0004`。
///
/// ⚠️ 单一事实来源是 `sml-feature` 的 `EDITIONS`（`Edition::CURRENT = V4`）。
/// 这里硬编码是因为 D3：`smlb` **只依赖值模型**，不拖解析/特性层。版本一旦升到 v5
/// （D1 说是跟 v5 一起开放），这里要**手动跟一手**。
pub const EDITION: u16 = 0x0004;

/// Byte-layout patch level (D5); `0` unless the layout itself changes.
/// 字节布局补丁位；除非布局本身变了，否则恒 0。
pub const FMT: u8 = 0;

/// `VALUE` segment id (§19.1). ｜ `VALUE` 段 id。
pub const SEG_VALUE: u8 = 0x01;
/// `STRTAB` segment id (§19.1). ｜ `STRTAB` 段 id。
pub const SEG_STRTAB: u8 = 0x02;
/// `DIR` segment id (§19.1) — index only; nothing writes it before T2/T3.
/// `DIR` 段 id（纯索引；T2/T3 之前没人写它）。
pub const SEG_DIR: u8 = 0x03;
/// `MANIFEST` segment id — **registered here in T1, written from T2**.
/// `MANIFEST` 段 id：T1 只登记，T2 才写内容。
pub const SEG_MANIFEST: u8 = 0x04;
/// `EMBED` segment id (§19.1) — foreign payload, stored verbatim (D23). Registered, unwritten
/// until `pack`/`unpack` lands.
/// `EMBED` 段 id（外来载荷，原样存取，D23）。已登记；`pack`/`unpack` 落地前没人写。
pub const SEG_EMBED: u8 = 0x20;
/// `PROVENANCE` segment id (§19.1) — T4 payload.
/// `PROVENANCE` 段 id（T4 才用）。
pub const SEG_PROVENANCE: u8 = 0x60;

/// Segment header size: `seg_id:u8 | comp:u8 | enc:u8 | flags:u16 | len:u32 | sha256:32`.
/// 段头大小（§19.1，41 B）。
pub const SEG_HEADER_LEN: usize = 41;

/// One segment header (§19.1), exactly as it sits on the wire.
/// 一个段头（§19.1），与线上字节一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentHeader {
    /// Segment kind ｜ 段种类（`SEG_*`）。
    pub seg_id: u8,
    /// Compression algorithm id (§20); `0` = none ｜ 压缩算法 id，0 = 未压缩。
    pub comp: u8,
    /// Encryption algorithm id (§20); `0` = plaintext ｜ 加密算法 id，0 = 明文。
    pub enc: u8,
    /// Segment flags; v1 writes `0` ｜ 段标志位，v1 写 0。
    pub flags: u16,
    /// Stored (post-compression/padding) payload length ｜ 落盘长度（压缩/填充之后）。
    pub len: u32,
    /// Digest whose algorithm is chosen by the manifest's `hash_id`; **T1 writes zeros**.
    /// 摘要（算法由 manifest 的 `hash_id` 定）；**T1 写全 0**。
    pub hash: [u8; 32],
}

/// A segment header plus the offset its body starts at (derived by walking, never stored).
/// 段头 + 段体起始偏移（走出来的，从不存储）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// The 41 B header ｜ 41 B 段头。
    pub header: SegmentHeader,
    /// Body offset in the file ｜ 段体在文件中的偏移。
    pub offset: u64,
}

/// Everything `info` reports about a file. ｜ `info` 能报出来的全部文件信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileInfo {
    /// `edition` as written (low byte = major, high byte = minor) ｜ 原样的 edition。
    pub edition: u16,
    /// Byte-layout patch level ｜ 字节布局补丁位。
    pub fmt: u8,
    /// Header length actually declared ｜ 头里声明的头长度。
    pub hdr_len: u8,
    /// File-level flags; bit 0 = 4 KiB segment alignment (§19.7 丙)
    /// 文件级标志位；位 0 = 段体 4 KiB 对齐。
    pub flags: u16,
    /// Alignment granule derived from `flags` bit 0 ｜ 由 `flags` 位 0 推出的对齐粒度。
    pub align: u64,
    /// Reserved for T4's trailing table; `0` in v1 ｜ T4 尾部表预留位，v1 恒 0。
    pub tail_off: u64,
    /// Segment table offset ｜ 段表偏移。
    pub table_off: u64,
    /// Total file size ｜ 文件总长。
    pub file_len: u64,
    /// Segments in table order ｜ 按表序的段。
    pub segments: Vec<Segment>,
}

impl FileInfo {
    /// Does any segment declare compression/encryption? (Only what T5 will handle.)
    /// 是否有段声明了压缩/加密（只有 T5 能处理）。
    pub fn is_compressed_or_encrypted(&self) -> bool {
        self.segments
            .iter()
            .any(|s| s.header.comp != 0 || s.header.enc != 0)
    }

    /// Does any segment carry an integrity digest? (T2-on dumps: yes; T1-era files: no.)
    /// 有没有段带完整性摘要？（T2 起写；T1 时代的文件没有。）
    pub fn has_digest(&self) -> bool {
        self.segments.iter().any(|s| has_digest(&s.header))
    }
}

/// Why the container layer refused. ｜ 容器层拒绝的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContainerError {
    /// File shorter than the fixed header ｜ 文件比固定头还短。
    TooShort {
        /// Actual length ｜ 实际长度。
        len: usize,
        /// Minimum length ｜ 最小需要。
        need: usize,
    },
    /// Wrong magic — almost always "this is not a `.smlb`" ｜ 魔数不对。
    BadMagic {
        /// The four bytes found ｜ 读到的四个字节。
        got: [u8; 4],
    },
    /// `fmt` this build does not know (D5's version gate) ｜ 本构建不认识的 `fmt`。
    UnsupportedFmt {
        /// The unknown value ｜ 未知值。
        fmt: u8,
    },
    /// `hdr_len` smaller than this build's header ｜ `hdr_len` 比本构建的头还小。
    HeaderTooShort {
        /// Declared length ｜ 声明的长度。
        hdr_len: u8,
    },
    /// Segment table lies outside the file ｜ 段表落在文件之外。
    BadSegmentTable {
        /// Table offset ｜ 段表偏移。
        at: u64,
        /// Bytes the table needs ｜ 段表需要多少字节。
        need: u64,
        /// File length ｜ 文件长度。
        file_len: u64,
    },
    /// A segment body runs past the end of the file ｜ 某段段体超出文件末尾。
    TruncatedSegment {
        /// Segment id ｜ 段 id。
        seg_id: u8,
        /// Body offset ｜ 段体偏移。
        at: u64,
        /// Declared length ｜ 声明长度。
        len: u64,
        /// File length ｜ 文件长度。
        file_len: u64,
    },
    /// A segment uses compression/encryption that T1 cannot undo ｜ 用了 T1 解不开的算法。
    UnsupportedAlgorithm {
        /// Segment id ｜ 段 id。
        seg_id: u8,
        /// Compression id ｜ 压缩 id。
        comp: u8,
        /// Encryption id ｜ 加密 id。
        enc: u8,
    },
    /// A segment `load` needs is absent ｜ `load` 需要的段不在。
    MissingSegment {
        /// The missing id ｜ 缺的 id。
        seg_id: u8,
    },
    /// The same critical segment id appears twice (D15: no implicit last-wins)
    /// 同一个关键段 id 出现两次（D15：不做隐式 last-wins）。
    DuplicateSegment {
        /// The repeated id ｜ 重复的 id。
        seg_id: u8,
    },
    /// A segment's bytes do not hash to the digest stored in its header (tamper/corruption).
    /// 段字节的摘要与段头里记的不一致（篡改或损坏）。
    DigestMismatch {
        /// Segment id ｜ 段 id。
        seg_id: u8,
        /// Offset of the body ｜ 段体偏移。
        at: u64,
        /// The digest recorded in the header ｜ 段头里记的摘要。
        expected: [u8; 32],
        /// The digest recomputed from the bytes ｜ 重算出来的摘要。
        actual: [u8; 32],
    },
    /// The `VALUE` body does not decode as canonical bytes ｜ `VALUE` 段体解不开。
    Canonical(CanonicalError),
    /// The lookup path is malformed or does not exist ｜ 路径写错或不存在的键。
    Path(sml_value::PathError),
    /// The `MANIFEST` segment does not decode ｜ `MANIFEST` 段解不开。
    BadManifest(ManifestError),
    /// `primary` points at a segment the file does not have ｜ `primary` 指向不存在的段。
    PrimarySegmentMissing {
        /// The id `primary` names ｜ `primary` 写的 id。
        id: u8,
    },
    /// `primary` points at a segment that is not a `Value` (e.g. an `EMBED` blob). `load`
    /// returns values; use `payload`/`unpack` for raw segments.
    /// `primary` 指的是非 `Value` 的段（如 `EMBED` 裸载荷）；`load` 只返回值。
    PrimaryNotValue {
        /// The id `primary` names ｜ `primary` 写的 id。
        id: u8,
    },
    /// A `required` capability this build does not understand (D11: must refuse)
    /// 声明了本构建不懂的 `required` 能力（D11：必须拒）。
    MissingCapability {
        /// The capability name ｜ 能力名。
        name: String,
    },
    /// The manifest's `segments[]` disagrees with the segment table (one of them was edited)
    /// manifest 的 `segments[]` 与段表不一致（两边有一边被改过）。
    ManifestMismatch {
        /// Segment id ｜ 段 id。
        seg_id: u8,
        /// Which field disagrees ｜ 哪个字段对不上。
        field: &'static str,
    },
    /// The digest algorithm is unknown or unimplemented (§20: never guess)
    /// 摘要算法未知或未实现（§20：绝不猜）。
    Hash(crate::hash::HashError),
    /// `pack` was given nothing to pack ｜ `pack` 一个文件都没给。
    EmptyArchive,
    /// Two payloads with the same name (`pack` refuses ambiguity, D15 spirit)
    /// 两个同名载荷（`pack` 拒绝歧义，与 D15 同精神）。
    DuplicatePayloadName {
        /// The repeated name ｜ 重复的名字。
        name: String,
    },
    /// No payload with that name in the manifest ｜ manifest 里没有这个载荷。
    PayloadNotFound {
        /// The name asked for ｜ 要的名字。
        name: String,
    },
    /// The `payloads[]` table disagrees with the `EMBED` segment body
    /// `payloads[]` 与 `EMBED` 段体不一致。
    PayloadTableMismatch {
        /// Payload name ｜ 载荷名。
        name: String,
        /// Which field disagrees ｜ 哪个字段对不上。
        field: &'static str,
    },
}

impl ContainerError {
    /// Stable machine-readable class name (the `E-SMLB-*` table lands in T2).
    /// 稳定的机器可读分类名（`E-SMLB-*` 码表在 T2 定）。
    pub fn kind(&self) -> &'static str {
        match self {
            ContainerError::TooShort { .. } => "too-short",
            ContainerError::BadMagic { .. } => "bad-magic",
            ContainerError::UnsupportedFmt { .. } => "unsupported-fmt",
            ContainerError::HeaderTooShort { .. } => "header-too-short",
            ContainerError::BadSegmentTable { .. } => "bad-segment-table",
            ContainerError::TruncatedSegment { .. } => "truncated-segment",
            ContainerError::UnsupportedAlgorithm { .. } => "unsupported-algorithm",
            ContainerError::MissingSegment { .. } => "missing-segment",
            ContainerError::DuplicateSegment { .. } => "duplicate-segment",
            ContainerError::DigestMismatch { .. } => "digest-mismatch",
            ContainerError::Canonical(_) => "bad-value",
            ContainerError::Path(_) => "bad-path",
            ContainerError::BadManifest(_) => "bad-manifest",
            ContainerError::PrimarySegmentMissing { .. } => "primary-segment-missing",
            ContainerError::PrimaryNotValue { .. } => "primary-not-value",
            ContainerError::MissingCapability { .. } => "missing-capability",
            ContainerError::ManifestMismatch { .. } => "manifest-mismatch",
            ContainerError::Hash(_) => "unsupported-hash",
            ContainerError::EmptyArchive => "empty-archive",
            ContainerError::DuplicatePayloadName { .. } => "duplicate-payload-name",
            ContainerError::PayloadNotFound { .. } => "payload-not-found",
            ContainerError::PayloadTableMismatch { .. } => "payload-table-mismatch",
        }
    }
}

impl std::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerError::TooShort { len, need } => {
                write!(f, "文件只有 {len} 字节，连 {need} 字节的文件头都不够")
            }
            ContainerError::BadMagic { got } => write!(
                f,
                "魔数不是 {:?}，读到 {:?} —— 这不是 .smlb 文件",
                String::from_utf8_lossy(&MAGIC),
                String::from_utf8_lossy(got)
            ),
            ContainerError::UnsupportedFmt { fmt } => write!(
                f,
                "本构建只认识 fmt={FMT}，文件是 fmt={fmt}（字节布局不同，D5 拒绝读）"
            ),
            ContainerError::HeaderTooShort { hdr_len } => write!(
                f,
                "hdr_len={hdr_len} 小于本构建的 {HEADER_LEN} 字节头"
            ),
            ContainerError::BadSegmentTable {
                at,
                need,
                file_len,
            } => write!(
                f,
                "段表在偏移 {at}，需要 {need} 字节，超出文件长度 {file_len}"
            ),
            ContainerError::TruncatedSegment {
                seg_id,
                at,
                len,
                file_len,
            } => write!(
                f,
                "段 0x{seg_id:02x} 在偏移 {at} 处声明 {len} 字节，超出文件长度 {file_len}"
            ),
            ContainerError::UnsupportedAlgorithm { seg_id, comp, enc } => write!(
                f,
                "段 0x{seg_id:02x} 声明 comp=0x{comp:02x} enc=0x{enc:02x}；T1 只支持明文未压缩（T5 才有算法实现）"
            ),
            ContainerError::MissingSegment { seg_id } => {
                write!(f, "缺少必需的段 0x{seg_id:02x}")
            }
            ContainerError::DuplicateSegment { seg_id } => {
                write!(f, "段 0x{seg_id:02x} 出现了不止一次（D15：不做隐式 last-wins）")
            }
            ContainerError::DigestMismatch {
                seg_id,
                at,
                expected,
                actual,
            } => write!(
                f,
                "段 0x{seg_id:02x}（偏移 {at}）摘要不符：记的是 {}，重算是 {} —— 字节被改过或文件损坏",
                hex8(expected),
                hex8(actual)
            ),
            ContainerError::Canonical(e) => write!(f, "VALUE 段体不合规：{e}"),
            ContainerError::Path(e) => write!(f, "路径取不到：{e}"),
            ContainerError::BadManifest(e) => write!(f, "{e}"),
            ContainerError::PrimarySegmentMissing { id } => write!(
                f,
                "manifest 的 primary 指向段 0x{id:02x}，但文件里没有这个段"
            ),
            ContainerError::PrimaryNotValue { id } => write!(
                f,
                "manifest 的 primary 指向段 0x{id:02x}，它不是 Value 段（load 只解值；裸载荷走 payload）"
            ),
            ContainerError::MissingCapability { name } => write!(
                f,
                "文件要求能力 `{name}`（caps.required），本构建不认识 —— 按 D11 拒绝打开，绝不半读"
            ),
            ContainerError::ManifestMismatch { seg_id, field } => write!(
                f,
                "段 0x{seg_id:02x} 的 `{field}` 在 manifest 与段表里不一致（两边有一边被改过）"
            ),
            ContainerError::Hash(e) => write!(f, "摘要算法不可用：{e}"),
            ContainerError::EmptyArchive => write!(f, "没有东西可打包（至少要一个文件）"),
            ContainerError::DuplicatePayloadName { name } => {
                write!(f, "载荷名 `{name}` 重复（拒绝歧义：解包时该覆盖谁？）")
            }
            ContainerError::PayloadNotFound { name } => {
                write!(f, "manifest 里没有名为 `{name}` 的载荷")
            }
            ContainerError::PayloadTableMismatch { name, field } => {
                write!(f, "载荷 `{name}` 的 `{field}` 与 EMBED 段体不一致")
            }
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<CanonicalError> for ContainerError {
    fn from(e: CanonicalError) -> Self {
        ContainerError::Canonical(e)
    }
}

/// Alignment granule declared by `flags` bit 0 (§19.7 丙).
/// `flags` 位 0 声明的段体对齐粒度。
///
/// 为什么粒度必须**写在文件里**：段数据的位置是靠走表算出来的（表项没有偏移字段），
/// 粒度猜错 → 整张表错位 → 读到垃圾。自描述是唯一不靠猜的做法。
/// The granule must be self-describing: segment positions are walked, not stored.
pub fn align_of(flags: u16) -> u64 {
    if flags & FLAG_ALIGN_BLOCK != 0 {
        ALIGN_BLOCK
    } else {
        ALIGN_MIN
    }
}

/// Round `x` up to the next multiple of `granule`. ｜ 把 `x` 向上取整到 `granule` 的倍数。
fn align_up(x: u64, granule: u64) -> u64 {
    let a = granule.max(1);
    (x + a - 1) / a * a
}

/// First 4 bytes of a digest as hex — enough to eyeball, short enough for a table row.
/// 摘要前 4 字节的十六进制 —— 够肉眼比对，也够短。
fn hex8(d: &[u8; 32]) -> String {
    d[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// Is this segment carrying a digest at all? (All-zero means "not covered" — T1 files.)
/// 这个段带摘要吗？（全 0 = 未覆盖 —— T1 的文件就是这样。）
fn has_digest(h: &SegmentHeader) -> bool {
    h.hash != [0u8; 32]
}

/// Verify one segment's digest — **no-op when the header carries none**.
/// 校验一个段的摘要 —— 段头没记摘要时**什么都不做**。
///
/// Why “no-op” instead of “refuse”: T1 wrote zeros, and a file without a digest is *less*
/// checked, not *invalid*. Refusing would break old files for no security gain.
/// 为什么「不做事」而不是「拒绝」：T1 的文件写的是全 0，没有摘要只是**校验更少**，
/// 不是**文件无效**；拒绝只会让旧文件打不开，换不来任何安全收益。
fn verify_digest(seg: &Segment, body: &[u8], algo: HashAlgo) -> Result<(), ContainerError> {
    if !has_digest(&seg.header) {
        return Ok(());
    }
    let actual = hash::digest(algo, body).map_err(ContainerError::Hash)?;
    if actual != seg.header.hash {
        return Err(ContainerError::DigestMismatch {
            seg_id: seg.header.seg_id,
            at: seg.offset,
            expected: seg.header.hash,
            actual,
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Write / 写
// ---------------------------------------------------------------------------

/// What `dump` may choose. ｜ `dump` 可选的两件事。
///
/// 只有两个旋钮，且都**由写者决定、写进文件**（读者永远照文件来）：
/// - `hash`：段摘要算法（默认 SHA-256；SM3 可选；BLAKE3 已登记未实现）；
/// - `block_align`：段体是否 4 KiB 对齐（明文不需要；**加密文件必须**，T5 会强制）。
/// ｜ Both knobs are writer-side and self-described in the file; readers never guess.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpOptions {
    /// Digest algorithm for every segment header ｜ 段摘要算法。
    pub hash: HashAlgo,
    /// Force 4 KiB alignment (required once encrypted) ｜ 强制 4 KiB 对齐（加密文件必须）。
    pub block_align: bool,
    /// `caps.required`: the writer declares "you must understand these" (D11)
    /// 写者声明「这些你不懂就别读」。
    pub caps_required: Vec<String>,
    /// `caps.optional`: "skip these if you don't" ｜ 「不懂就跳过」。
    pub caps_optional: Vec<String>,
}

impl Default for DumpOptions {
    fn default() -> Self {
        DumpOptions {
            hash: hash::DEFAULT,
            block_align: false,
            caps_required: Vec::new(),
            caps_optional: Vec::new(),
        }
    }
}

/// Serialize a `Value` into a complete `.smlb` file with default options.
/// 用默认选项把 `Value` 序列化成完整 `.smlb` 文件。
///
/// Segments written: `VALUE` always, `STRTAB` only when non-empty (an empty table has no
/// bytes), and `MANIFEST` always (it is what makes `primary` / `hash_id` / `caps` answerable).
/// 写的段：`VALUE` 必有；`STRTAB` 仅在非空时写；`MANIFEST` 必写。
///
/// Table order = ascending `seg_id` (§18.3's canonical ordering), data laid out in the same
/// order — so the file is a pure function of the value (C12 at container level).
/// 表序 = 段 id 升序，数据按同序摆放 ⇒ 文件是值的纯函数。
pub fn dump(v: &Value) -> Result<Vec<u8>, ContainerError> {
    dump_with(v, &DumpOptions::default())
}

/// Same as [`dump`], with explicit choices. ｜ 同 [`dump`]，但显式指定选项。
pub fn dump_with(v: &Value, opts: &DumpOptions) -> Result<Vec<u8>, ContainerError> {
    let blocks = v.canonical_blocks()?;

    // 内容段（manifest 的 segments[] 只列它们，**不含自己** —— 否则摘要循环）
    let mut segs: Vec<(u8, Vec<u8>)> = Vec::with_capacity(3);
    segs.push((SEG_VALUE, blocks.value));
    if !blocks.strtab.is_empty() {
        segs.push((SEG_STRTAB, blocks.strtab));
    }

    let manifest = build_manifest(SEG_VALUE, &segs, &[], opts)?;
    write_file(segs, manifest, opts)
}

/// Pack foreign blobs into one `.smlb` archive (D23 / §22 `smlb pack`).
/// 把外来载荷打成一个 `.smlb` 归档（D23 / §22 的 `smlb pack`）。
///
/// 一个文件**至多一个 `EMBED` 段**，段体是各载荷按入参顺序拼接；`payloads[]` 按同序描述
/// （名字 / 长度 / 摘要），偏移不存、靠 `len` 走出来 —— 与段表同一套思路。
/// `primary` 指向 `EMBED`（P8：主内容可以不是 `VALUE`），所以这类文件 `load` 会明确报
/// [`ContainerError::PrimaryNotValue`]，要用 [`payloads`] / [`payload`] 去取。
/// One `EMBED` segment per file; payloads are concatenated in order and described in the same
/// order. `primary` points at `EMBED`, so `load` says `PrimaryNotValue` by design.
pub fn pack(items: &[(&str, &[u8])]) -> Result<Vec<u8>, ContainerError> {
    pack_with(items, &DumpOptions::default())
}

/// Same as [`pack`], with explicit choices. ｜ 同 [`pack`]，但显式指定选项。
pub fn pack_with(
    items: &[(&str, &[u8])],
    opts: &DumpOptions,
) -> Result<Vec<u8>, ContainerError> {
    if items.is_empty() {
        return Err(ContainerError::EmptyArchive);
    }
    let mut names: Vec<&str> = Vec::with_capacity(items.len());
    for (name, _) in items {
        if names.contains(name) {
            return Err(ContainerError::DuplicatePayloadName {
                name: (*name).to_string(),
            });
        }
        names.push(name);
    }

    let mut body: Vec<u8> = Vec::new();
    let mut payloads: Vec<ManifestPayload> = Vec::with_capacity(items.len());
    for (name, data) in items {
        payloads.push(ManifestPayload {
            name: (*name).to_string(),
            len: body_len(data)?,
            digest: hash::digest(opts.hash, data).map_err(ContainerError::Hash)?,
        });
        body.extend_from_slice(data);
    }
    let body_len_total = body_len(&body)?;

    let segs = vec![(SEG_EMBED, body)];
    let mut manifest = build_manifest(SEG_EMBED, &segs, &payloads, opts)?;
    // 归档的「能力」就是 embed：读不懂 embed 的读者应当明说读不懂，而不是当空文件
    manifest.caps_required = if opts.caps_required.is_empty() {
        vec!["embed".to_string()]
    } else {
        opts.caps_required.clone()
    };
    let _ = body_len_total;
    write_file(segs, manifest, opts)
}

/// Assemble a manifest for `segs` (digests computed with `opts.hash`).
/// 为 `segs` 组装 manifest（摘要用 `opts.hash` 现算）。
fn build_manifest(
    primary: u8,
    segs: &[(u8, Vec<u8>)],
    payloads: &[ManifestPayload],
    opts: &DumpOptions,
) -> Result<Manifest, ContainerError> {
    let mut manifest_segments = Vec::with_capacity(segs.len());
    for (seg_id, body) in segs {
        manifest_segments.push(ManifestSegment {
            id: *seg_id,
            comp: 0,
            enc: 0,
            len: body_len(body)?,
            digest: hash::digest(opts.hash, body).map_err(ContainerError::Hash)?,
        });
    }
    let mut manifest = Manifest::minimal(primary, opts.hash, manifest_segments);
    manifest.payloads = payloads.to_vec();
    manifest.caps_required = opts.caps_required.clone();
    manifest.caps_optional = opts.caps_optional.clone();
    Ok(manifest)
}

/// Lay out header + segment table + segments (the part every writer shares).
/// 落版：头 + 段表 + 段数据（所有写入口共用的那段）。
fn write_file(
    mut segs: Vec<(u8, Vec<u8>)>,
    manifest: Manifest,
    opts: &DumpOptions,
) -> Result<Vec<u8>, ContainerError> {
    let manifest_bytes = manifest.to_bytes()?;
    segs.push((SEG_MANIFEST, manifest_bytes));
    segs.sort_by_key(|(id, _)| *id);

    let flags: u16 = if opts.block_align { FLAG_ALIGN_BLOCK } else { 0 };
    let granule = align_of(flags);
    let table_off = HEADER_LEN as u64;
    let table_end = table_off + (segs.len() * SEG_HEADER_LEN) as u64;
    let data_start = align_up(table_end, granule);
    let mut out = vec![0u8; data_start as usize];

    // --- header ---
    out[0..4].copy_from_slice(&MAGIC);
    out[4..6].copy_from_slice(&EDITION.to_le_bytes());
    out[6] = FMT;
    out[7] = HEADER_LEN as u8;
    out[8..16].copy_from_slice(&table_off.to_le_bytes());
    out[16..20].copy_from_slice(&(segs.len() as u32).to_le_bytes());
    out[20..22].copy_from_slice(&flags.to_le_bytes());
    out[22..24].copy_from_slice(&0u16.to_le_bytes()); // reserved
    out[24..32].copy_from_slice(&0u64.to_le_bytes()); // tail_off (T4)
    out[32..40].copy_from_slice(&0u64.to_le_bytes()); // reserved

    // --- table + data ---
    let mut data_off = data_start;
    for (i, (seg_id, body)) in segs.iter().enumerate() {
        let len = body_len(body)?;
        let at = table_off as usize + i * SEG_HEADER_LEN;
        out[at] = *seg_id;
        out[at + 1] = 0; // comp: none
        out[at + 2] = 0; // enc: plaintext
        out[at + 3..at + 5].copy_from_slice(&0u16.to_le_bytes()); // 段头 flags（T1 起恒 0）
        out[at + 5..at + 9].copy_from_slice(&len.to_le_bytes());
        let digest = hash::digest(opts.hash, body).map_err(ContainerError::Hash)?;
        out[at + 9..at + 41].copy_from_slice(&digest);

        if out.len() as u64 != data_off {
            out.resize(data_off as usize, 0);
        }
        out.extend_from_slice(body);
        data_off = align_up(out.len() as u64, granule);
    }
    Ok(out)
}

/// Segment body length as `u32` (4 GiB per segment, §19.1's `len:u32`).
/// 段体长度转 `u32`（单段 4 GiB 上限，§19.1 的 `len:u32`）。
fn body_len(body: &[u8]) -> Result<u32, ContainerError> {
    u32::try_from(body.len())
        .map_err(|_| ContainerError::Canonical(CanonicalError::SubtreeTooLarge { len: body.len() }))
}

// ---------------------------------------------------------------------------
// Read / 读
// ---------------------------------------------------------------------------

/// Parse the header + segment table (no `VALUE` decoding).
/// 解析文件头与段表（不解 `VALUE`）。
pub fn info(bytes: &[u8]) -> Result<FileInfo, ContainerError> {
    if bytes.len() < HEADER_LEN {
        return Err(ContainerError::TooShort {
            len: bytes.len(),
            need: HEADER_LEN,
        });
    }
    let magic: [u8; 4] = [bytes[0], bytes[1], bytes[2], bytes[3]];
    if magic != MAGIC {
        return Err(ContainerError::BadMagic { got: magic });
    }
    let edition = u16::from_le_bytes([bytes[4], bytes[5]]);
    let fmt = bytes[6];
    let hdr_len = bytes[7];
    if fmt != FMT {
        return Err(ContainerError::UnsupportedFmt { fmt });
    }
    if (hdr_len as usize) < HEADER_LEN {
        return Err(ContainerError::HeaderTooShort { hdr_len });
    }
    let table_off = u64::from_le_bytes(bytes[8..16].try_into().expect("8 bytes"));
    let seg_count = u32::from_le_bytes(bytes[16..20].try_into().expect("4 bytes"));
    let flags = u16::from_le_bytes([bytes[20], bytes[21]]);
    // bytes[22..24] reserved, bytes[32..40] reserved: 读时忽略（未知保留位不报错）
    let tail_off = u64::from_le_bytes(bytes[24..32].try_into().expect("8 bytes"));

    // checked：`table_off` 是文件里的 u64，伪造一个 u64::MAX 就会溢出成"看似合规"
    let table_len = (seg_count as u64) * (SEG_HEADER_LEN as u64);
    let Some(table_end) = table_off.checked_add(table_len) else {
        return Err(ContainerError::BadSegmentTable {
            at: table_off,
            need: table_len,
            file_len: bytes.len() as u64,
        });
    };
    if table_end > bytes.len() as u64 {
        return Err(ContainerError::BadSegmentTable {
            at: table_off,
            need: table_len,
            file_len: bytes.len() as u64,
        });
    }

    let granule = align_of(flags);
    let mut segments = Vec::with_capacity(seg_count as usize);
    let mut data_off = align_up(table_end, granule);
    for i in 0..seg_count as usize {
        let at = table_off as usize + i * SEG_HEADER_LEN;
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&bytes[at + 9..at + 41]);
        let header = SegmentHeader {
            seg_id: bytes[at],
            comp: bytes[at + 1],
            enc: bytes[at + 2],
            flags: u16::from_le_bytes([bytes[at + 3], bytes[at + 4]]),
            len: u32::from_le_bytes(bytes[at + 5..at + 9].try_into().expect("4 bytes")),
            hash,
        };
        let end = data_off + header.len as u64;
        if end > bytes.len() as u64 {
            return Err(ContainerError::TruncatedSegment {
                seg_id: header.seg_id,
                at: data_off,
                len: header.len as u64,
                file_len: bytes.len() as u64,
            });
        }
        segments.push(Segment {
            header,
            offset: data_off,
        });
        data_off = align_up(end, granule);
    }

    Ok(FileInfo {
        edition,
        fmt,
        hdr_len,
        flags,
        align: granule,
        tail_off,
        table_off,
        file_len: bytes.len() as u64,
        segments,
    })
}

/// The body bytes of one segment. ｜ 取某段的段体字节。
pub fn body<'a>(
    bytes: &'a [u8],
    seg: &Segment,
) -> Result<&'a [u8], ContainerError> {
    let start = seg.offset as usize;
    let end = start + seg.header.len as usize;
    if end > bytes.len() {
        return Err(ContainerError::TruncatedSegment {
            seg_id: seg.header.seg_id,
            at: seg.offset,
            len: seg.header.len as u64,
            file_len: bytes.len() as u64,
        });
    }
    Ok(&bytes[start..end])
}

/// Deserialize a complete `.smlb` file back into a `Value`.
/// 把完整的 `.smlb` 文件解回 `Value`。
///
/// **Invariant 1 in action**: segments this layer does not use are skipped by their declared
/// `len` — an unknown `seg_id` never blocks reading, and never needs an interpreter.
/// **不变量 1 的落点**：不认识的段一律按 `len` 跳过，不阻塞读取、不需要解释器。
///
/// Which segment is the main content: `manifest.primary`, or `VALUE` for manifest-less (T1) files.
/// 主内容是哪个段：有 manifest 用 `primary`，T1 时代的文件退回「就是 `VALUE`」。
///
/// `caps.required` 在这里生效（D11）：读不懂的必拒能力 ⇒ 直接拒绝打开，**绝不半读**。
/// 摘要算法取自 manifest 的 `hash_id`（缺 manifest 时按默认 SHA-256）。
/// ｜ `caps.required` is enforced here; the digest algorithm comes from `hash_id`.
///
/// Digests are verified **for the segments used here**; unknown segments are skipped without
/// verification (they cannot affect the value). `smlb check` is the thorough path — it verifies
/// every segment and cross-checks the manifest against the table.
/// 摘要只对**本函数用到的段**校验；逐段全验 + manifest 交叉核对是 `smlb check` 的事。
pub fn load(bytes: &[u8]) -> Result<Value, ContainerError> {
    let info = info(bytes)?;
    let manifest = read_manifest(bytes, &info)?;

    if let Some(m) = &manifest {
        let verdict = manifest::check_caps(m);
        if let Some(name) = verdict.missing_required.first() {
            return Err(ContainerError::MissingCapability { name: name.clone() });
        }
    }
    let algo = manifest.as_ref().map(|m| m.hash).unwrap_or(hash::DEFAULT);
    let primary_id = manifest.as_ref().map(|m| m.primary).unwrap_or(SEG_VALUE);
    if primary_id != SEG_VALUE {
        return Err(ContainerError::PrimaryNotValue { id: primary_id });
    }

    let mut value: Option<&[u8]> = None;
    let mut strtab: Option<&[u8]> = None;
    for seg in &info.segments {
        match seg.header.seg_id {
            SEG_VALUE => {
                if value.is_some() {
                    return Err(ContainerError::DuplicateSegment { seg_id: SEG_VALUE });
                }
                require_plain(seg)?;
                let b = body(bytes, seg)?;
                verify_digest(seg, b, algo)?;
                value = Some(b);
            }
            SEG_STRTAB => {
                if strtab.is_some() {
                    return Err(ContainerError::DuplicateSegment {
                        seg_id: SEG_STRTAB,
                    });
                }
                require_plain(seg)?;
                let b = body(bytes, seg)?;
                verify_digest(seg, b, algo)?;
                strtab = Some(b);
            }
            // 其余一律跳过：不认识 ≠ 读不了（不变量 1）
            _ => {}
        }
    }
    let value = value.ok_or(ContainerError::MissingSegment { seg_id: SEG_VALUE })?;
    decode_value(strtab.unwrap_or(&[]), value)
}

/// Read + decode the `MANIFEST` segment if the file has one. ｜ 有 manifest 就把它读出来。
///
/// **引导顺序**：先解码（manifest 自包含，不依赖任何别的段），拿到它的 `hash_id`，
/// **再用那个算法回头验它自己的摘要**。这条顺序是必需的 —— 摘要算法就声明在要验的那段
/// 东西里面，先验后读是不可能的；这也正是 manifest 必须用自包含的 L0 形态而不是
/// 「裸节点 + 外部 STRTAB」的原因。
/// Decode first (self-contained), then verify with the algorithm it declares — the algorithm
/// lives inside the payload being verified, so that is the only possible order.
///
/// 验摘要挡的是**意外损坏**（改一个字节），挡不了**蓄意伪造**（没有签名，谁都能重算）。
/// 蓄意那一路归 T4 的签名，这里不冒充。
/// The digest catches corruption, not forgery — forgery is T4's job.
fn read_manifest(bytes: &[u8], info: &FileInfo) -> Result<Option<Manifest>, ContainerError> {
    let Some((seg, b, m)) = decode_manifest(bytes, info)? else {
        return Ok(None);
    };
    verify_digest(&seg, b, m.hash)?;
    Ok(Some(m))
}

/// Decode without verifying — `check` needs to *report* a bad manifest digest instead of
/// aborting on it (that is the difference between `check` and `load`).
/// 只解码不校验 —— `check` 要「报告」manifest 摘要不符，而不是一遇错就停。
fn decode_manifest<'a>(
    bytes: &'a [u8],
    info: &FileInfo,
) -> Result<Option<(Segment, &'a [u8], Manifest)>, ContainerError> {
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == SEG_MANIFEST)
        .cloned();
    let Some(seg) = seg else {
        return Ok(None);
    };
    require_plain(&seg)?;
    let b = body(bytes, &seg)?;
    let m = Manifest::from_bytes(b).map_err(ContainerError::BadManifest)?;
    Ok(Some((seg, b, m)))
}

/// Refuse compression/encryption this build cannot undo. ｜ 拒绝本构建解不开的压缩/加密。
fn require_plain(seg: &Segment) -> Result<(), ContainerError> {
    if seg.header.comp != 0 || seg.header.enc != 0 {
        return Err(ContainerError::UnsupportedAlgorithm {
            seg_id: seg.header.seg_id,
            comp: seg.header.comp,
            enc: seg.header.enc,
        });
    }
    Ok(())
}

/// Result of `check`: everything is structural-or-hash checked, **nothing throws early**.
/// `check` 的结论：结构 + 摘要都查过，**不提前抛错**（好让工具把每段结论都打出来）。
#[derive(Debug, Clone)]
pub struct CheckReport {
    /// One line per finding, in table order ｜ 每段（及总体）一行结论。
    pub lines: Vec<String>,
    /// `true` only when every check passed ｜ 只有全过才是 `true`。
    pub ok: bool,
}

/// Structural + integrity check (§22 `smlb check`): header, table, truncation, digests,
/// manifest ↔ table cross-check, capabilities, and whether the value actually decodes.
/// 结构 + 完整性校验：头、段表、截断、摘要、manifest ↔ 段表交叉核对、能力位、能否解码。
///
/// Structural problems (header/table/truncation) are still hard errors — there is nothing
/// meaningful to report about segments that are not even inside the file.
/// 结构性问题（头/段表/截断）仍是硬错误 —— 段都不在文件里，逐段结论没有意义。
pub fn check(bytes: &[u8]) -> Result<CheckReport, ContainerError> {
    let info = info(bytes)?;
    let mut lines = Vec::new();
    let mut ok = true;

    // manifest 先**解**（不验证）：坏 manifest 要被报出来，而不是让 check 半途而废
    let manifest = match decode_manifest(bytes, &info) {
        Ok(Some((seg, b, m))) => {
            if let Err(e) = verify_digest(&seg, b, m.hash) {
                ok = false;
                lines.push(format!("manifest  段摘要：{e}"));
            }
            Some(m)
        }
        Ok(None) => None,
        Err(e) => {
            ok = false;
            lines.push(format!("manifest  解不开：{e}"));
            None
        }
    };
    let algo = manifest.as_ref().map(|m| m.hash).unwrap_or(hash::DEFAULT);
    lines.push(format!(
        "算法      {}（hash_id 0x{:02x}{}）",
        algo.name(),
        algo.id(),
        if manifest.is_some() { "" } else { "，无 manifest ⇒ 按默认" }
    ));

    // 1) 逐段摘要
    for seg in &info.segments {
        let name = seg_name(seg.header.seg_id);
        let b = body(bytes, seg)?;
        if !has_digest(&seg.header) {
            ok = false;
            lines.push(format!(
                "0x{:02x} {:<10} 未带摘要 —— 无法判断是否被改过",
                seg.header.seg_id, name
            ));
            continue;
        }
        match hash::digest(algo, b) {
            Ok(actual) if actual == seg.header.hash => lines.push(format!(
                "0x{:02x} {:<10} 摘要 OK ({})",
                seg.header.seg_id,
                name,
                hex8(&actual)
            )),
            Ok(actual) => {
                ok = false;
                lines.push(format!(
                    "0x{:02x} {:<10} 摘要不符：记 {} / 算 {}",
                    seg.header.seg_id,
                    name,
                    hex8(&seg.header.hash),
                    hex8(&actual)
                ));
            }
            Err(e) => {
                ok = false;
                lines.push(format!(
                    "0x{:02x} {:<10} 算不了摘要：{e}",
                    seg.header.seg_id, name
                ));
            }
        }
    }

    // 2) manifest ↔ 段表交叉核对（两个真相源互相验证，见 §19.9 的「故意重复」）
    match &manifest {
        None => lines.push(
            "manifest  无（T1 时代文件；主内容按约定 = VALUE，段头算法按默认）".to_string(),
        ),
        Some(m) => {
            if !info.segments.iter().any(|s| s.header.seg_id == m.primary) {
                ok = false;
                lines.push(format!(
                    "manifest  primary 指向段 0x{:02x}，但文件里没有",
                    m.primary
                ));
            } else {
                lines.push(format!("manifest  primary = 0x{:02x}", m.primary));
            }
            for ms in &m.segments {
                let Some(seg) = info.segments.iter().find(|s| s.header.seg_id == ms.id) else {
                    ok = false;
                    lines.push(format!(
                        "manifest  列了段 0x{:02x}，但段表里没有（签名覆盖到不存在的段）",
                        ms.id
                    ));
                    continue;
                };
                for (field, same) in [
                    ("len", seg.header.len == ms.len),
                    ("comp", seg.header.comp == ms.comp),
                    ("enc", seg.header.enc == ms.enc),
                    ("sha256", seg.header.hash == ms.digest),
                ] {
                    if !same {
                        ok = false;
                        lines.push(format!(
                            "manifest  段 0x{:02x} 的 `{field}` 与段表不一致",
                            ms.id
                        ));
                    }
                }
            }
            // 段表里有、manifest 没列的段（manifest 自己不算）：签名覆盖不到，报出来
            for seg in &info.segments {
                if seg.header.seg_id == SEG_MANIFEST {
                    continue;
                }
                if !m.segments.iter().any(|ms| ms.id == seg.header.seg_id) {
                    ok = false;
                    lines.push(format!(
                        "manifest  段 0x{:02x} 没进 segments[]（签名覆盖不到它）",
                        seg.header.seg_id
                    ));
                }
            }
            // 3) 能力位（D11）
            let verdict = manifest::check_caps(m);
            if verdict.is_ok() {
                lines.push("caps      全部认得".to_string());
            } else {
                ok = false;
                lines.push(format!(
                    "caps      读不懂的必拒能力：{}",
                    verdict.missing_required.join(", ")
                ));
            }
            if !verdict.ignored_optional.is_empty() {
                lines.push(format!(
                    "caps      可跳过（读不懂但不影响读）：{}",
                    verdict.ignored_optional.join(", ")
                ));
            }
            // 载荷表：`payloads[]` 与 EMBED 段体必须互相说得通
            if !m.payloads.is_empty() {
                match embed_body(bytes) {
                    Ok(b) => {
                        let mut cursor = 0usize;
                        for p in &m.payloads {
                            let end = cursor + p.len as usize;
                            if end > b.len() {
                                ok = false;
                                lines.push(format!(
                                    "payload   `{}` 的 len 越出 EMBED 段体（{} > {}）",
                                    p.name,
                                    end,
                                    b.len()
                                ));
                                cursor = end;
                                continue;
                            }
                            match hash::digest(algo, &b[cursor..end]) {
                                Ok(actual) if actual == p.digest => lines.push(format!(
                                    "payload   `{}` 摘要 OK（{} B）",
                                    p.name, p.len
                                )),
                                Ok(_) => {
                                    ok = false;
                                    lines.push(format!("payload   `{}` 摘要不符", p.name));
                                }
                                Err(e) => {
                                    ok = false;
                                    lines.push(format!("payload   `{}` 算不了摘要：{e}", p.name));
                                }
                            }
                            cursor = end;
                        }
                        if cursor != b.len() {
                            ok = false;
                            lines.push(format!(
                                "payload   payloads[] 合计 {cursor} B ≠ EMBED 段体 {} B",
                                b.len()
                            ));
                        }
                    }
                    Err(e) => {
                        ok = false;
                        lines.push(format!("payload   声明了载荷但取不到 EMBED 段体：{e}"));
                    }
                }
            } else if info.segments.iter().any(|s| s.header.seg_id == SEG_EMBED) {
                lines.push(
                    "payload   EMBED 段存在但 manifest 没列载荷（无人认领的裸字节）".to_string(),
                );
            }

            if m.created.is_some() || m.tool.is_some() {
                lines.push(format!(
                    "溯源      {} / {}（只展示，不验签 —— D17）",
                    m.created.as_deref().unwrap_or("-"),
                    m.tool.as_deref().unwrap_or("-")
                ));
            }
        }
    }

    // 4) 真的能不能解
    match load(bytes) {
        Ok(_) => lines.push("VALUE    段可解（按 canonical 解码成功）".to_string()),
        // 归档类文件（primary = EMBED）本来就**不该**解成 Value ⇒ 这不是失败，
        // 而是「按设计如此」。把它算作错误会让每个 pack 出来的文件都红。
        Err(ContainerError::PrimaryNotValue { id }) => lines.push(format!(
            "VALUE    主内容是段 0x{id:02x}，不是 Value（归档类文件，按设计如此）"
        )),
        Err(e) => {
            ok = false;
            lines.push(format!("VALUE    解不开：{e}"));
        }
    }

    lines.push(format!(
        "结论      {}（{} 段，{} 字节，对齐 {} B）",
        if ok { "通过" } else { "未通过" },
        info.segments.len(),
        info.file_len,
        info.align
    ));
    Ok(CheckReport { lines, ok })
}

fn decode_value(strtab: &[u8], value: &[u8]) -> Result<Value, ContainerError> {
    Ok(Value::from_canonical_blocks(strtab, value)?)
}

/// The `EMBED` segment's body (foreign blobs, stored verbatim — D23).
/// `EMBED` 段的段体（外来载荷，原样存取 —— D23）。
///
/// 为什么单独一个入口：`load` 只解 `Value`，而归档类文件的 `primary` 是 `EMBED`
/// （P8 允许），所以「取裸字节」必须有一条不走 `Value` 解码的路。
/// `load` decodes values; archives have `primary = EMBED`, so raw bytes need their own path.
pub fn embed_body<'a>(bytes: &'a [u8]) -> Result<&'a [u8], ContainerError> {
    let info = info(bytes)?;
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == SEG_EMBED)
        .ok_or(ContainerError::MissingSegment { seg_id: SEG_EMBED })?;
    require_plain(seg)?;
    let b = body(bytes, seg)?;
    let manifest = read_manifest(bytes, &info)?;
    if let Some(m) = manifest {
        verify_digest(seg, b, m.hash)?;
    }
    Ok(b)
}

/// List the payloads declared in the manifest, in body order.
/// 列出 manifest 里声明的载荷（按段体内顺序）。
pub fn payloads(bytes: &[u8]) -> Result<Vec<ManifestPayload>, ContainerError> {
    let info = info(bytes)?;
    Ok(read_manifest(bytes, &info)?
        .map(|m| m.payloads)
        .unwrap_or_default())
}

/// Extract one payload by name (§22 `smlb unpack` 的取值口).
/// 按名字取一段载荷（`smlb unpack` 的取值口）。
///
/// 偏移不存：`payloads[]` 按序给出 `len`，本函数沿着加起来走 —— 与段表同一套思路，
/// 好处是「表里没有位置」⇒ 位置不可能与内容不一致。
/// Offsets are walked from the ordered `len`s, never stored.
pub fn payload<'a>(bytes: &'a [u8], name: &str) -> Result<&'a [u8], ContainerError> {
    let info = info(bytes)?;
    let manifest = read_manifest(bytes, &info)?
        .ok_or(ContainerError::MissingSegment { seg_id: SEG_MANIFEST })?;
    let body = embed_body(bytes)?;

    let mut cursor = 0usize;
    for p in &manifest.payloads {
        let end = cursor + p.len as usize;
        if end > body.len() {
            return Err(ContainerError::PayloadTableMismatch {
                name: p.name.clone(),
                field: "len（越出 EMBED 段体）",
            });
        }
        if p.name == name {
            let slice = &body[cursor..end];
            let actual = hash::digest(manifest.hash, slice).map_err(ContainerError::Hash)?;
            if actual != p.digest {
                return Err(ContainerError::PayloadTableMismatch {
                    name: p.name.clone(),
                    field: "sha256",
                });
            }
            return Ok(slice);
        }
        cursor = end;
    }
    Err(ContainerError::PayloadNotFound {
        name: name.to_string(),
    })
}

/// Look one path up inside the primary value (§22 `smlb cat`) — `a.b[0].c` syntax.
/// 在主值里按路径取一个值（§22 的 `smlb cat`），路径语法 `a.b[0].c`。
///
/// T2 的实现是「整棵解出来再找路」；T3 的 `DIR` 段会把它降成 O(1) 跳转。这里刻意先做
/// 正确的慢版本 —— 语义定下来之后再谈索引，否则索引会替语义背锅。
/// T2 decodes the whole tree and then walks; T3's `DIR` makes it O(1). Correct-but-slow
/// first: an index must not be where the semantics are decided.
pub fn get(bytes: &[u8], path: &str) -> Result<Value, ContainerError> {
    let v = load(bytes)?;
    let p = sml_value::Path::parse(path).map_err(ContainerError::Path)?;
    let found = v.get_path(&p).map_err(ContainerError::Path)?;
    Ok(found.clone())
}

/// Human-readable name for a segment id (tools only; unknown ids print as `0xNN`).
/// 段 id 的可读名（只给工具用；未知 id 打成 `0xNN`）。
pub fn seg_name(seg_id: u8) -> String {
    match seg_id {
        SEG_VALUE => "VALUE".to_string(),
        SEG_STRTAB => "STRTAB".to_string(),
        SEG_DIR => "DIR".to_string(),
        SEG_EMBED => "EMBED".to_string(),
        SEG_MANIFEST => "MANIFEST".to_string(),
        SEG_PROVENANCE => "PROVENANCE".to_string(),
        other => format!("0x{other:02x}"),
    }
}

/// One line per segment, for `smlb ls`. ｜ 每段一行，供 `smlb ls`。
pub fn ls_lines(info: &FileInfo) -> Vec<String> {
    info.segments
        .iter()
        .map(|s| {
            let h = &s.header;
            let digest = if has_digest(h) {
                hex8(&h.hash)
            } else {
                "none".to_string()
            };
            format!(
                "0x{:02x} {:<12} off={:<10} len={:<8} comp={:<3} enc={:<3} sha256={}",
                h.seg_id,
                seg_name(h.seg_id),
                s.offset,
                h.len,
                h.comp,
                h.enc,
                digest
            )
        })
        .collect()
}

/// Where the “main content” of this file is — P8 wants an answer for every file.
/// 这个文件的「主内容」在哪 —— P8 要求任何文件都有答案。
///
/// 有 manifest 就照 `primary` 说；没有（T1 时代文件）就明说是**约定**，
/// 免得把「约定」读成「文件里存了指针」。
/// With a manifest: report `primary`; without: say plainly that it is a convention.
fn primary_hint(info: &FileInfo, manifest: Option<&Manifest>) -> String {
    let exists = |id: u8| info.segments.iter().any(|s| s.header.seg_id == id);
    match manifest {
        Some(m) if exists(m.primary) => format!("0x{:02x} {}（manifest 的 primary）", m.primary, seg_name(m.primary)),
        Some(m) => format!(
            "0x{:02x} —— **但文件里没有这个段**（manifest 与段表不一致）",
            m.primary
        ),
        None if exists(SEG_VALUE) => "VALUE（约定：无 manifest，按「VALUE 就是主内容」）".to_string(),
        None => "无 —— 既没有 manifest，也没有 VALUE 段".to_string(),
    }
}

/// A short multi-line summary for `smlb info`. ｜ 多行摘要，供 `smlb info`。
///
/// 需要 `bytes` 而不只是 `FileInfo`：`primary` / `hash_id` / `caps` 都在 manifest 段里。
/// ｜ Takes the bytes too, because `primary` / `hash_id` / `caps` live in the manifest segment.
pub fn info_lines(
    path: &str,
    bytes: &[u8],
    info: &FileInfo,
) -> Result<Vec<String>, ContainerError> {
    let manifest = read_manifest(bytes, info)?;
    let major = (info.edition & 0xff) as u8;
    let minor = (info.edition >> 8) as u8;
    let mut out = vec![
        format!("file      {path}"),
        format!("edition   {major}.{minor}  (raw 0x{:04x})", info.edition),
        format!("fmt       {}（本构建认识 {FMT}）", info.fmt),
        format!("hdr_len   {} B", info.hdr_len),
        format!("size      {} B", info.file_len),
        format!(
            "flags     0x{:04x}（段体对齐 {} B）",
            info.flags, info.align
        ),
        format!("tail_off  {}（T4 尾部表预留）", info.tail_off),
        format!("segments  {}", info.segments.len()),
        format!("primary   {}", primary_hint(info, manifest.as_ref())),
    ];
    match &manifest {
        Some(m) => {
            out.push(format!(
                "hash_id   0x{:02x} {}（{}）",
                m.hash.id(),
                m.hash.name(),
                if m.hash.is_implemented() { "本构建可算" } else { "本构建未实现" }
            ));
            out.push(format!(
                "caps      required [{}] · optional [{}]",
                m.caps_required.join(" "),
                m.caps_optional.join(" ")
            ));
            out.push(format!(
                "defaults  dir_depth {} · crypto_block {} · pad_quantum {} · patch {}",
                m.dir_depth, m.crypto_block, m.pad_quantum, m.patch_strategy
            ));
            if m.created.is_some() || m.tool.is_some() {
                out.push(format!(
                    "origin    {} / {}（只展示，不验签 —— D17）",
                    m.created.as_deref().unwrap_or("-"),
                    m.tool.as_deref().unwrap_or("-")
                ));
            }
        }
        None => out.push("manifest  无（T1 时代文件）".to_string()),
    }
    for line in ls_lines(info) {
        out.push(format!("  {line}"));
    }
    out.push(format!(
        "crypto    {}",
        if info.is_compressed_or_encrypted() {
            "有段声明压缩/加密（本构建解不开）"
        } else {
            "明文未压缩"
        }
    ));
    out.push(format!(
        "digest    {}",
        if info.has_digest() {
            format!(
                "{}（逐段；`smlb check` 会核）",
                manifest.map(|m| m.hash.name()).unwrap_or("SHA-256")
            )
        } else {
            "无 —— 这份文件没带摘要，**不要指望它检出篡改**".to_string()
        }
    ));
    Ok(out)
}
