// SPDX-License-Identifier: MulanPSL-2.0
//! Error types: one stable `kind()` taxonomy for parse / patch / conversion.
//! 错误类型：解析 / patch / 互转共用一套稳定的 `kind()` 分类。
//!
//! These errors do **not** occupy slots in `errors/codes.sml` — that table is for the
//! five SML language endpoints. smlite is a data-only runtime, so its errors are
//! self-contained and machine-readable via [`ErrorKind`].
//! 这些错误**不占** `errors/codes.sml` 的五端码位；smlite 是纯数据运行时，
//! 错误自洽，通过 [`ErrorKind`] 提供稳定的机器可读分类。

use core::fmt;

/// Stable machine-readable error category. ｜ 稳定的机器可读错误分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Input ended before a complete value. ｜ 输入在完整值之前结束。
    UnexpectedEof,
    /// General syntax error. ｜ 一般语法错误。
    InvalidSyntax,
    /// A key was not followed by `:`. ｜ 键之后缺少 `:`。
    ExpectedColon,
    /// Unknown or malformed escape sequence. ｜ 未知或非法的转义序列。
    InvalidEscape,
    /// `\\uXXXX` could not be decoded. ｜ `\\uXXXX` 无法解码。
    InvalidUnicode,
    /// Numeric literal out of range or malformed. ｜ 数值字面量越界或非法。
    InvalidNumber,
    /// Input is not valid UTF-8. ｜ 输入不是合法 UTF-8。
    InvalidUtf8,
    /// Nesting exceeded `MAX_VALUE_DEPTH`. ｜ 嵌套超过 `MAX_VALUE_DEPTH`。
    DepthLimit,
    /// Trailing bytes after a complete document. ｜ 完整文档之后还有多余字节。
    TrailingData,
    /// A value cannot be represented in the target model. ｜ 值无法在目标模型中表示。
    UnsupportedType,
    /// Conversion refused because conversion depth would overflow. ｜ 互转深度超限。
    ConversionDepth,
}

impl ErrorKind {
    /// Short stable label of this category. ｜ 本分类的简短稳定标签。
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorKind::UnexpectedEof => "unexpected-eof",
            ErrorKind::InvalidSyntax => "invalid-syntax",
            ErrorKind::ExpectedColon => "expected-colon",
            ErrorKind::InvalidEscape => "invalid-escape",
            ErrorKind::InvalidUnicode => "invalid-unicode",
            ErrorKind::InvalidNumber => "invalid-number",
            ErrorKind::InvalidUtf8 => "invalid-utf8",
            ErrorKind::DepthLimit => "depth-limit",
            ErrorKind::TrailingData => "trailing-data",
            ErrorKind::UnsupportedType => "unsupported-type",
            ErrorKind::ConversionDepth => "conversion-depth",
        }
    }
}

/// Parse failure with a byte offset. ｜ 带字节偏移的解析失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    kind: ErrorKind,
    pos: usize,
}

impl ParseError {
    pub(crate) fn new(kind: ErrorKind, pos: usize) -> Self {
        Self { kind, pos }
    }

    /// Stable category of this failure. ｜ 本失败的稳定分类。
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Byte offset where the failure was detected. ｜ 检测到失败的字节偏移。
    pub fn pos(&self) -> usize {
        self.pos
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}: {}", self.pos, self.kind.as_str())
    }
}

/// Merge-patch failure. ｜ Merge Patch 失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchError {
    kind: ErrorKind,
}

impl PatchError {
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    /// Stable category of this failure. ｜ 本失败的稳定分类。
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "merge-patch error: {}", self.kind.as_str())
    }
}

/// Value-layer conversion failure. ｜ 值层互转失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionError {
    kind: ErrorKind,
}

impl ConversionError {
    #[allow(dead_code)] // 只在 swsml-compat 下使用
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self { kind }
    }

    /// Stable category of this failure. ｜ 本失败的稳定分类。
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conversion error: {}", self.kind.as_str())
    }
}

/// Unified error type of smlite. ｜ smlite 的统一错误类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmliteError {
    /// Parsing failed. ｜ 解析失败。
    Parse(ParseError),
    /// Merge Patch failed. ｜ Merge Patch 失败。
    Patch(PatchError),
    /// Value-layer conversion failed. ｜ 值层互转失败。
    Conversion(ConversionError),
}

impl SmliteError {
    /// Stable category of this failure. ｜ 本失败的稳定分类。
    pub fn kind(&self) -> ErrorKind {
        match self {
            SmliteError::Parse(e) => e.kind(),
            SmliteError::Patch(e) => e.kind(),
            SmliteError::Conversion(e) => e.kind(),
        }
    }
}

impl fmt::Display for SmliteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SmliteError::Parse(e) => e.fmt(f),
            SmliteError::Patch(e) => e.fmt(f),
            SmliteError::Conversion(e) => e.fmt(f),
        }
    }
}

impl From<ParseError> for SmliteError {
    fn from(e: ParseError) -> Self {
        SmliteError::Parse(e)
    }
}

impl From<PatchError> for SmliteError {
    fn from(e: PatchError) -> Self {
        SmliteError::Patch(e)
    }
}

impl From<ConversionError> for SmliteError {
    fn from(e: ConversionError) -> Self {
        SmliteError::Conversion(e)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SmliteError {}

#[cfg(feature = "std")]
impl std::error::Error for ConversionError {}

#[cfg(feature = "std")]
impl std::error::Error for ParseError {}

#[cfg(feature = "std")]
impl std::error::Error for PatchError {}
