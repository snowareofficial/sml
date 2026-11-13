// SPDX-License-Identifier: MulanPSL-2.0
//! `.ambr` 的统一错误类型：稳定码 + 机器可判的类别 + 人读细节。
//!
//! # 为什么单独一层
//!
//! 容器内部有 9 个模块各自定义了细粒度错误（`ContainerError` / `CanonicalError` /
//! `HashError` / `SignError` / …）。它们对库内部很有用，但对**调用方**太碎：
//! 程序只想按「格式坏了 / 不支持 / 校验失败 / 宿主问题」分支，不想 match 44 个变体。
//! 本模块把它们收敛成 8 个正交类别，每类带一个**稳定码**（`errors/codes.sml` 的
//! `E-AMBR-*` 域，唯一事实来源），因此：
//!
//! - Rust：`match err.kind()` 或 `err.code()`，不用解析字符串；
//! - C-ABI / 其它语言：`ambr_text` 等的 `err` 缓冲里是
//!   `AMBR: [E-AMBR-002] <detail>`，用正则 `\[E-AMBR-\d{3}\]` 就能取码；
//! - 文案随便改（含翻译），码不变 —— 与其它领域的码纪律一致。
//!
//! 细节文本保留原文（中文），英文规范文案在 `errors/msg.en.json`。

use std::fmt;

/// `.ambr` 的统一错误 ｜ Unified `.ambr` error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AmbrError {
    /// `E-AMBR-001` 空指针 / 空输入 / 无效 UTF-8 / 文本含 NUL。
    InvalidInput(String),
    /// `E-AMBR-002` 魔数、布局、文件头、段表、段体不合规（含截断与缺段）。
    BadFormat(String),
    /// `E-AMBR-003` 本构建不支持的算法 / 压缩 / feature / 能力位。
    Unsupported(String),
    /// `E-AMBR-004` 摘要、manifest 声明或签名校验失败。
    Integrity(String),
    /// `E-AMBR-005` 缺密钥、KDF 不匹配或解密失败。
    Crypto(String),
    /// `E-AMBR-006` canonical 化 / 载荷转换 / SML 源解析失败。
    Canonical(String),
    /// `E-AMBR-007` 本地库未加载、文件 IO、缓冲分配等宿主问题。
    Host(String),
    /// `E-AMBR-008` panic 或未预期的内部状态。
    Internal(String),
}

impl AmbrError {
    /// 稳定码（`errors/codes.sml` 的 `E-AMBR-*`）｜ Stable code.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "E-AMBR-001",
            Self::BadFormat(_) => "E-AMBR-002",
            Self::Unsupported(_) => "E-AMBR-003",
            Self::Integrity(_) => "E-AMBR-004",
            Self::Crypto(_) => "E-AMBR-005",
            Self::Canonical(_) => "E-AMBR-006",
            Self::Host(_) => "E-AMBR-007",
            Self::Internal(_) => "E-AMBR-008",
        }
    }

    /// 机器可判的类别名（kebab-case，不是文案）｜ Machine-readable kind.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "invalid-input",
            Self::BadFormat(_) => "bad-format",
            Self::Unsupported(_) => "unsupported",
            Self::Integrity(_) => "integrity",
            Self::Crypto(_) => "crypto",
            Self::Canonical(_) => "canonical",
            Self::Host(_) => "host",
            Self::Internal(_) => "internal",
        }
    }

    /// 人读细节（不含码）｜ Human-readable detail.
    pub fn detail(&self) -> &str {
        match self {
            Self::InvalidInput(s)
            | Self::BadFormat(s)
            | Self::Unsupported(s)
            | Self::Integrity(s)
            | Self::Crypto(s)
            | Self::Canonical(s)
            | Self::Host(s)
            | Self::Internal(s) => s,
        }
    }

    /// 带码的可显示串：`AMBR: [E-AMBR-002] <detail>`。
    /// C-ABI 的 `err` 缓冲直接用它，宿主解析方括号里的码即可。
    pub fn render(&self) -> String {
        format!("AMBR: [{}] {}", self.code(), self.detail())
    }
}

impl fmt::Display for AmbrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code(), self.detail())
    }
}

impl std::error::Error for AmbrError {}

/// 把 container 的 44 个细粒度变体收敛成 8 类。
///
/// **无兜底**：全部分组显式列出，`ContainerError` 新增变体时编译器会强制在这里
/// 归类（不会静默落进 Internal）。形状按其定义（`{ .. }` 结构体 / `(..)` 元组 / 单元）。
/// `Internal` 只由 ABI 边界的 panic / 意外状态构造，不由容器错误映射而来。
impl From<crate::container::ContainerError> for AmbrError {
    fn from(e: crate::container::ContainerError) -> Self {
        use crate::container::ContainerError as C;
        let detail = e.to_string();
        match &e {
            // —— 支持性 / 能力位 ——
            C::UnsupportedAlgorithm { .. }
            | C::CompressFeatureDisabled { .. }
            | C::SourceFeatureDisabled
            | C::KdfUnsupported { .. }
            | C::EncryptedNotZeroCopy => Self::Unsupported(detail),
            // —— 完整性 ——
            C::DigestMismatch { .. }
            | C::AuthFailed { .. }
            | C::ManifestMismatch { .. }
            | C::BadManifest(..)
            | C::Hash(..)
            | C::Layer(..) => Self::Integrity(detail),
            // —— 密钥 / 加解密 ——
            C::KeyRequired
            | C::KdfRequired
            | C::KdfMismatch(..)
            | C::Crypto(..) => Self::Crypto(detail),
            // —— canonical / 载荷语义 ——
            C::Canonical(..) | C::Typed(..) | C::Dir(..) => Self::Canonical(detail),
            // —— 宿主 IO ——
            C::Io { .. } | C::PayloadRead(..) | C::CompressFailed(..) => Self::Host(detail),
            // —— 结构 / 布局（其余全部归「格式非法」）——
            C::TooShort { .. }
            | C::BadMagic { .. }
            | C::UnsupportedFmt { .. }
            | C::HeaderTooShort { .. }
            | C::BadSegmentTable { .. }
            | C::TruncatedSegment { .. }
            | C::MissingSegment { .. }
            | C::PlaintextSegmentInEncryptedFile { .. }
            | C::EncryptedSegmentInPlainFile { .. }
            | C::DuplicateSegment { .. }
            | C::Path(..)
            | C::PrimarySegmentMissing { .. }
            | C::PrimaryNotValue { .. }
            | C::BadCapabilityName { .. }
            | C::DuplicateCapability { .. }
            | C::MissingCapability { .. }
            | C::BadFrameIndex(..)
            | C::ZeroCopyNeedsPlain { .. }
            | C::EmptyArchive
            | C::DuplicatePayloadName { .. }
            | C::PayloadNotFound { .. }
            | C::PayloadTableMismatch { .. }
            | C::DecompressTooLarge { .. } => Self::BadFormat(detail),
        }
    }
}

impl From<crate::CanonicalError> for AmbrError {
    fn from(e: crate::CanonicalError) -> Self {
        Self::Canonical(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::container::ContainerError;

    #[test]
    fn every_kind_has_a_unique_stable_code() {
        let all = [
            AmbrError::InvalidInput(String::new()),
            AmbrError::BadFormat(String::new()),
            AmbrError::Unsupported(String::new()),
            AmbrError::Integrity(String::new()),
            AmbrError::Crypto(String::new()),
            AmbrError::Canonical(String::new()),
            AmbrError::Host(String::new()),
            AmbrError::Internal(String::new()),
        ];
        let mut codes: Vec<&str> = all.iter().map(AmbrError::code).collect();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), 8);
        assert!(codes.iter().all(|c| c.starts_with("E-AMBR-")));
    }

    #[test]
    fn render_is_parseable_by_hosts() {
        let e = AmbrError::BadFormat("magic mismatch".into());
        assert_eq!(e.code(), "E-AMBR-002");
        assert_eq!(e.kind(), "bad-format");
        let s = e.render();
        assert!(s.contains("AMBR: [E-AMBR-002]"), "{s}");
        assert!(s.contains("magic mismatch"));
    }

    #[test]
    fn container_variants_are_classified() {
        let cases: Vec<(ContainerError, &str)> = vec![
            (ContainerError::BadMagic { got: *b"NOPE" }, "E-AMBR-002"),
            (ContainerError::TruncatedSegment { seg_id: 1, at: 0, len: 9, file_len: 2 }, "E-AMBR-002"),
            (ContainerError::UnsupportedAlgorithm { seg_id: 1, comp: 9, enc: 9 }, "E-AMBR-003"),
            (ContainerError::DigestMismatch { seg_id: 1, at: 0, expected: [0u8; 32], actual: [1u8; 32] }, "E-AMBR-004"),
            (ContainerError::KeyRequired, "E-AMBR-005"),
        ];
        for (raw, want) in cases {
            let e = AmbrError::from(raw);
            assert_eq!(e.code(), want, "{e:?}");
        }
    }
}
