// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! 错误类型：解析错误（10 种）与解码错误两类，位置为 1 基行列 + 字节偏移。
//! Error types: ten parse errors plus a decode error; positions are 1-based
//! line/column together with the byte offset.
//!
//! 文案与 C++ `getErrorMessage(ParseErrorEnum)` 逐字一致，便于两端对照；`code()` 给出
//! C++ 枚举名，供脚本与测试引用。
//! Messages are identical to the C++ `getErrorMessage(ParseErrorEnum)` so both
//! implementations can be compared directly; `code()` exposes the C++ enumerator name.

use core::fmt;

/// 解析错误种类 ｜ Parse error kinds.
///
/// 与 C++ `enum ParseErrorEnum` 一一对应（`pe_user_asked_for_termination` 除外：
/// 本 crate 的 `ParseHandler` 不提供中途终止通道）。
/// One-to-one with C++ `enum ParseErrorEnum`, except `pe_user_asked_for_termination`:
/// this crate's `ParseHandler` has no early-termination channel.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ParseErrorKind {
    /// 输入在语法未完成处结束 ｜ input ended where more syntax was required.
    UnexpectedEndOfFile,
    /// 此处不允许该字符 ｜ character not allowed here.
    UnexpectedCharacter,
    /// 无法识别的转义 ｜ unrecognized escape sequence.
    UnrecognizedEscape,
    /// `\u` 转义不完整或超出行范围 ｜ malformed `\u` escape (bad digits, `}` or range).
    InvalidUnicodeEscape,
    /// UTF-16 代理对不成对 ｜ unpaired UTF-16 surrogate.
    InvalidUtf16Surrogate,
    /// 引号串内出现未转义控制字符 ｜ unescaped control character in a quoted string.
    UnescapedControlCharacter,
    /// 定界符超过 16 个字符 ｜ string delimiter longer than 16 characters.
    StringDelimiterTooLong,
    /// 定界符缺失或非法 ｜ missing or invalid string delimiter.
    InvalidStringDelimiter,
    /// heredoc 定界符为空 ｜ empty heredoc delimiter.
    EmptyStringDelimiter,
    /// 裸词键使用了保留字 ｜ unquoted key spelled as a reserved word.
    InvalidKey,
}

impl ParseErrorKind {
    /// 全部种类（不含已声明的 `user asked for termination`）。
    /// All kinds, excluding the C++-only `user asked for termination`.
    pub const ALL: [Self; 10] = [
        Self::UnexpectedEndOfFile,
        Self::UnexpectedCharacter,
        Self::UnrecognizedEscape,
        Self::InvalidUnicodeEscape,
        Self::InvalidUtf16Surrogate,
        Self::UnescapedControlCharacter,
        Self::StringDelimiterTooLong,
        Self::InvalidStringDelimiter,
        Self::EmptyStringDelimiter,
        Self::InvalidKey,
    ];

    /// 人类可读文案，与 C++ `getErrorMessage` 相同。
    /// Human-readable message, identical to C++ `getErrorMessage`.
    pub const fn message(self) -> &'static str {
        match self {
            Self::UnexpectedEndOfFile => "unexpected end of file",
            Self::UnexpectedCharacter => "unexpected character",
            Self::UnrecognizedEscape => "unrecognized escape",
            Self::InvalidUnicodeEscape => "invalid unicode escape",
            Self::InvalidUtf16Surrogate => "invalid UTF-16 surrogate",
            Self::UnescapedControlCharacter => "unescaped control character",
            Self::StringDelimiterTooLong => "string delimiter too long",
            Self::InvalidStringDelimiter => "invalid string delimiter",
            Self::EmptyStringDelimiter => "empty string delimiter",
            Self::InvalidKey => "invalid key",
        }
    }

    /// C++ 枚举名，供脚本/测试引用 ｜ the C++ enumerator name, for scripts and tests.
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnexpectedEndOfFile => "pe_unexpected_end_of_file",
            Self::UnexpectedCharacter => "pe_unexpected_character",
            Self::UnrecognizedEscape => "pe_unrecognized_escape",
            Self::InvalidUnicodeEscape => "pe_invalid_unicode_escape",
            Self::InvalidUtf16Surrogate => "pe_invalid_utf16_surrogate",
            Self::UnescapedControlCharacter => "pe_unescaped_control_character",
            Self::StringDelimiterTooLong => "pe_string_delimiter_too_long",
            Self::InvalidStringDelimiter => "pe_invalid_string_delimiter",
            Self::EmptyStringDelimiter => "pe_empty_string_delimiter",
            Self::InvalidKey => "pe_invalid_key",
        }
    }
}

impl fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

/// 错误大类 ｜ Error category.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ErrorKind {
    /// 语法错误 ｜ syntax error.
    Parse(ParseErrorKind),
    /// 输入不是合法 UTF-8（对应 C++ `ec_decode_error`，**不是** parse error）。
    /// Input is not valid UTF-8 (C++ `ec_decode_error`, **not** a parse error).
    Decode,
}

/// 带位置的错误 ｜ Error with source position.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Error {
    kind: ErrorKind,
    line: u32,
    column: u32,
    offset: usize,
}

impl Error {
    /// 直接构造 ｜ Construct from parts.
    pub const fn new(kind: ErrorKind, line: u32, column: u32, offset: usize) -> Self {
        Self {
            kind,
            line,
            column,
            offset,
        }
    }

    /// 错误大类 ｜ Error category.
    pub const fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// 若是解析错误，返回具体种类 ｜ the parse kind, when applicable.
    pub const fn parse_kind(&self) -> Option<ParseErrorKind> {
        match self.kind {
            ErrorKind::Parse(k) => Some(k),
            ErrorKind::Decode => None,
        }
    }

    /// 1 基行号 ｜ 1-based line.
    pub const fn line(&self) -> u32 {
        self.line
    }

    /// 1 基列号（按字符计，不按字节）｜ 1-based column, counted in characters.
    pub const fn column(&self) -> u32 {
        self.column
    }

    /// 出错处在输入中的字节偏移 ｜ byte offset of the offending position.
    pub const fn offset(&self) -> usize {
        self.offset
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self.kind {
            ErrorKind::Parse(k) => k.message(),
            ErrorKind::Decode => "invalid UTF-8 input",
        };
        write!(
            f,
            "{} at line {} column {}",
            message, self.line, self.column
        )
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
