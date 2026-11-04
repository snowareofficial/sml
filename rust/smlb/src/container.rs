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
//! # 还没做的两件事（不是忘了）
//!
//! - `sha256:32` **v1 写全 0**：零依赖不引 crate，自己实现 SHA-256 放 T2；
//!   期间 `info` 会明说「未校验」——**不要在 T1 声称能检出篡改**。
//! - **manifest 段（0x04）只登记、不写**：它的字段（算法 / profile / `primary` /
//!   能力位 / 默认值）归 T2。T1 的文件靠 header + 段表就自描述。
//! ｜ `sha256` is all zeros in T1, and the `MANIFEST` segment is registered but unwritten.

use sml_value::{CanonicalError, Value};

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
///  8  8  seg_table_off   u64（用户 2026-11-04：u64 现在就付）
/// 16  4  seg_count
/// 20  2  flags           v1 全 0；未知位忽略（§19.5 L2 同口径）
/// 22  2  reserved        0
/// 24  8  tail_off        T4 尾部表预留，v1 恒 0
/// 32  8  reserved        0
/// ```
pub const HEADER_LEN: usize = 40;

/// Segment (and first-data) alignment. ｜ 段（及首个段数据）的对齐粒度。
pub const ALIGN: usize = 4096;

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
    /// File-level flags (v1 always 0) ｜ 文件级标志位（v1 恒 0）。
    pub flags: u16,
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

    /// Is there any integrity digest at all? In v1: never (T1 writes zeros).
    /// 有没有任何完整性摘要？v1 里永远没有（T1 写全 0）。
    pub fn has_digest(&self) -> bool {
        self.segments.iter().any(|s| s.header.hash != [0u8; 32])
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
    /// The `VALUE` body does not decode as canonical bytes ｜ `VALUE` 段体解不开。
    Canonical(CanonicalError),
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
            ContainerError::Canonical(_) => "bad-value",
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
            ContainerError::Canonical(e) => write!(f, "VALUE 段体不合规：{e}"),
        }
    }
}

impl std::error::Error for ContainerError {}

impl From<CanonicalError> for ContainerError {
    fn from(e: CanonicalError) -> Self {
        ContainerError::Canonical(e)
    }
}

/// Round `x` up to the next [`ALIGN`] boundary.
/// 把 `x` 向上取整到 [`ALIGN`] 边界。
fn align_up(x: u64) -> u64 {
    let a = ALIGN as u64;
    (x + a - 1) / a * a
}

// ---------------------------------------------------------------------------
// Write / 写
// ---------------------------------------------------------------------------

/// Serialize a `Value` into a complete `.smlb` file (T1: one `VALUE`, optional `STRTAB`).
/// 把一个 `Value` 序列化成完整的 `.smlb` 文件。
///
/// Segments written: `VALUE` always, `STRTAB` only when non-empty (an empty table has no
/// bytes, and a zero-length segment would only add 4 KiB of padding for nothing).
/// 写的段：`VALUE` 必有；`STRTAB` 仅在非空时写（空表没有字节，写它只是白白多 4 KiB 填充）。
///
/// Table order = ascending `seg_id` (§18.3's manifest ordering), data laid out in the same
/// order — so the file is a pure function of the value (C12 at container level).
/// 表序 = 段 id 升序，数据按同序摆放 ⇒ 文件是值的纯函数。
pub fn dump(v: &Value) -> Result<Vec<u8>, ContainerError> {
    let blocks = v.canonical_blocks()?;
    let mut segs: Vec<(u8, Vec<u8>)> = Vec::with_capacity(2);
    segs.push((SEG_VALUE, blocks.value));
    if !blocks.strtab.is_empty() {
        segs.push((SEG_STRTAB, blocks.strtab));
    }
    segs.sort_by_key(|(id, _)| *id);

    let table_off = ALIGN as u64;
    let table_end = table_off + (segs.len() * SEG_HEADER_LEN) as u64;
    let mut out = vec![0u8; align_up(table_end) as usize];

    // --- header ---
    out[0..4].copy_from_slice(&MAGIC);
    out[4..6].copy_from_slice(&EDITION.to_le_bytes());
    out[6] = FMT;
    out[7] = HEADER_LEN as u8;
    out[8..16].copy_from_slice(&table_off.to_le_bytes());
    out[16..20].copy_from_slice(&(segs.len() as u32).to_le_bytes());
    out[20..22].copy_from_slice(&0u16.to_le_bytes()); // flags
    out[22..24].copy_from_slice(&0u16.to_le_bytes()); // reserved
    out[24..32].copy_from_slice(&0u64.to_le_bytes()); // tail_off (T4)
    out[32..40].copy_from_slice(&0u64.to_le_bytes()); // reserved

    // --- table + data ---
    let mut data_off = align_up(table_end);
    for (i, (seg_id, body)) in segs.iter().enumerate() {
        let len = u32::try_from(body.len()).map_err(|_| ContainerError::Canonical(
            CanonicalError::SubtreeTooLarge { len: body.len() },
        ))?;
        let at = table_off as usize + i * SEG_HEADER_LEN;
        out[at] = *seg_id;
        out[at + 1] = 0; // comp: none
        out[at + 2] = 0; // enc: plaintext
        out[at + 3..at + 5].copy_from_slice(&0u16.to_le_bytes()); // flags
        out[at + 5..at + 9].copy_from_slice(&len.to_le_bytes());
        // at+9..at+41 留全 0：T1 不写摘要（T2 落 SHA-256）

        if out.len() as u64 != data_off {
            out.resize(data_off as usize, 0);
        }
        out.extend_from_slice(body);
        data_off = align_up(out.len() as u64);
    }
    Ok(out)
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

    let mut segments = Vec::with_capacity(seg_count as usize);
    let mut data_off = align_up(table_end);
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
        data_off = align_up(end);
    }

    Ok(FileInfo {
        edition,
        fmt,
        hdr_len,
        flags,
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
/// T1 requires a `VALUE` segment (no manifest ⇒ no `primary` pointer yet); T2 relaxes this via
/// `primary`. `STRTAB` is optional (absent ⇒ empty table).
/// T1 必须有 `VALUE`（还没有 manifest ⇒ 没有 `primary` 指针）；`STRTAB` 可缺（缺 = 空表）。
pub fn load(bytes: &[u8]) -> Result<Value, ContainerError> {
    let info = info(bytes)?;
    let mut value: Option<&[u8]> = None;
    let mut strtab: Option<&[u8]> = None;
    for seg in &info.segments {
        match seg.header.seg_id {
            SEG_VALUE => {
                if value.is_some() {
                    return Err(ContainerError::DuplicateSegment { seg_id: SEG_VALUE });
                }
                if seg.header.comp != 0 || seg.header.enc != 0 {
                    return Err(ContainerError::UnsupportedAlgorithm {
                        seg_id: SEG_VALUE,
                        comp: seg.header.comp,
                        enc: seg.header.enc,
                    });
                }
                value = Some(body(bytes, seg)?);
            }
            SEG_STRTAB => {
                if strtab.is_some() {
                    return Err(ContainerError::DuplicateSegment {
                        seg_id: SEG_STRTAB,
                    });
                }
                if seg.header.comp != 0 || seg.header.enc != 0 {
                    return Err(ContainerError::UnsupportedAlgorithm {
                        seg_id: SEG_STRTAB,
                        comp: seg.header.comp,
                        enc: seg.header.enc,
                    });
                }
                strtab = Some(body(bytes, seg)?);
            }
            // 其余一律跳过：不认识 ≠ 读不了（不变量 1）
            _ => {}
        }
    }
    let value = value.ok_or(ContainerError::MissingSegment { seg_id: SEG_VALUE })?;
    decode_value(strtab.unwrap_or(&[]), value)
}

fn decode_value(strtab: &[u8], value: &[u8]) -> Result<Value, ContainerError> {
    Ok(Value::from_canonical_blocks(strtab, value)?)
}

/// Human-readable name for a segment id (tools only; unknown ids print as `0xNN`).
/// 段 id 的可读名（只给工具用；未知 id 打成 `0xNN`）。
pub fn seg_name(seg_id: u8) -> String {
    match seg_id {
        SEG_VALUE => "VALUE".to_string(),
        SEG_STRTAB => "STRTAB".to_string(),
        SEG_DIR => "DIR".to_string(),
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
            let digest = if h.hash == [0u8; 32] {
                "none(T1)".to_string()
            } else {
                h.hash[..4].iter().map(|b| format!("{b:02x}")).collect::<String>()
            };
            format!(
                "0x{:02x} {:<10} off={:<10} len={:<8} comp={:<3} enc={:<3} hash={}",
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

/// A short multi-line summary for `smlb info`. ｜ 多行摘要，供 `smlb info`。
pub fn info_lines(path: &str, info: &FileInfo) -> Vec<String> {
    let major = (info.edition & 0xff) as u8;
    let minor = (info.edition >> 8) as u8;
    let mut out = vec![
        format!("file      {path}"),
        format!("edition   {major}.{minor}  (raw 0x{:04x})", info.edition),
        format!("fmt       {}（本构建认识 {FMT}）", info.fmt),
        format!("hdr_len   {} B", info.hdr_len),
        format!("size      {} B", info.file_len),
        format!("flags     0x{:04x}", info.flags),
        format!("tail_off  {}（T4 尾部表预留）", info.tail_off),
        format!("segments  {}", info.segments.len()),
    ];
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
            "有（算法见 manifest）"
        } else {
            "无 —— T1 不写 sha256，**不要指望它检出篡改**"
        }
    ));
    out
}
