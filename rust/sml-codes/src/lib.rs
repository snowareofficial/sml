// SPDX-License-Identifier: MulanPSL-2.0
//! SML 错误码与**带码错误**类型。
//!
//! 这个 crate 只做两件事：
//!
//! 1. [`codes`] —— 全部错误码常量（条数见 [`COUNT`]，别在注释里另记一份）。**它不是手写的**，而是由
//!    `errors/gen_codes.py` 从唯一事实来源 `errors/codes.sml` 生成，
//!    同一份码表还生成 JS 常量与 C 头（`js/sml-codes.mjs`、`c/sml_codes.h`）。
//!    各端只写「这里该报哪个码」，不写「码长什么样」。
//! 2. [`SmlError`] —— 把「码」与「文案」放在一起的错误类型。
//!
//! # 为什么码要单独存在，而不是塞进文案里
//!
//! 码是**稳定契约**，文案不是：`字段 port 类型错误` 这句话早晚会被改写，
//! 用户写的判断脚本与统计脚本不该跟着失效。所以：
//!
//! - [`SmlError::code`] 给出机器可判的码；
//! - [`SmlError::message`] / [`Display`](std::fmt::Display) 给出人读的文案；
//! - **同一触发条件在五端必须给同一个码**（`errors/README.md` 的纪律）。
//!
//! 注意 [`SmlError`] 的 `Display` 会把码缀在文案之后（`文案 [E-PARSE-008]`），
//! 而 `From<SmlError> for String` **只取文案** —— 后者是为既有那些
//! `Result<_, String>` 的接口保留的兼容通道，它们的行为一字不变。
//!
//! ```
//! use sml_codes::{SmlError, E_PARSE_008};
//!
//! let e = SmlError::new(E_PARSE_008, "顶层须为容器");
//! assert_eq!(e.code(), "E-PARSE-008");
//! assert_eq!(e.to_string(), "顶层须为容器 [E-PARSE-008]");
//! assert_eq!(String::from(e), "顶层须为容器");
//! ```

pub mod codes;

pub use codes::*;

use std::fmt;

/// 带**错误码**的错误。
///
/// 码是编译期常量（见 [`codes`]），文案是运行时拼的 —— 这个分工让
/// 「同因同码」可以被测试钉住，而文案随便改。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmlError {
    code: &'static str,
    message: String,
    /// 出错位置：原文中的**字节**偏移。**可选** —— 早期构造点不传就是 `None`。
    ///
    /// ⚠️ `Display` / `render` **不输出**它：码与文案才是稳定契约，位置属于
    /// 「给人看的附加信息」，不同端的呈现方式可以不同。需要行列时由调用方
    /// 拿 [`SmlError::pos`] 配合原文换算（见 `sml-parse` 的出口）。
    pos: Option<usize>,
}

impl SmlError {
    /// 用码与文案构造。码应当是 [`codes`] 里的常量而不是手写字面量 ——
    /// 写字面量能过编译，但打错一个数字就悄悄变成了另一个码。
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        SmlError {
            code,
            message: message.into(),
            pos: None,
        }
    }

    /// 记下出错位置（原文字节偏移），链式：`SmlError::new(C, "…").at(pos)`。
    pub fn at(mut self, pos: usize) -> Self {
        self.pos = Some(pos);
        self
    }

    /// 机器可判的错误码，如 `E-PARSE-008`。
    pub fn code(&self) -> &'static str {
        self.code
    }

    /// 出错位置（原文字节偏移）；未记录时为 `None`。
    pub fn pos(&self) -> Option<usize> {
        self.pos
    }

    /// 人读的文案（**不含**码）。
    pub fn message(&self) -> &str {
        &self.message
    }

    /// 规范完整形态：`文案 [码]`，与 `Display` 一致。
    pub fn render(&self) -> String {
        format!("{} [{}]", self.message, self.code)
    }

    /// 换一个码，保留文案。用于「内层错误被外层包装」时补上外层的码。
    pub fn with_code(self, code: &'static str) -> Self {
        SmlError {
            code,
            message: self.message,
            pos: self.pos,
        }
    }

    /// 给文案加上前缀，保留码。用于补上下文（如「include 展开失败：」）。
    pub fn context(mut self, prefix: &str) -> Self {
        self.message = format!("{prefix}{}", self.message);
        self
    }

    /// 给文案加上后缀，保留码。用于补**尾巴**（如「（第 3 行第 8 列）」）——
    /// `Display` 的码始终在最后，故后缀落在「文案之后、码之前」。
    pub fn suffix(mut self, suffix: &str) -> Self {
        self.message = format!("{}{}", self.message, suffix);
        self
    }
}

impl fmt::Display for SmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 码缀在最后：读文案时不受打扰，需要码时一眼可见。
        write!(f, "{} [{}]", self.message, self.code)
    }
}

impl std::error::Error for SmlError {}

/// 兼容通道：**只取文案**，丢掉码。
///
/// 存在的原因是本仓库大量既有接口仍是 `Result<_, String>`：有了这条 `From`，
/// 那些调用方 `foo()?` 不必改一行就能继续编译，行为也一字不变。
/// 新接口（语言层）请直接用 `Result<_, SmlError>`，别再退回去用 `String`。
impl From<SmlError> for String {
    fn from(e: SmlError) -> String {
        e.message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_keeps_message_and_appends_code() {
        let e = SmlError::new(E_LEX_001, "字符串未闭合");
        assert_eq!(e.to_string(), "字符串未闭合 [E-LEX-001]");
        assert_eq!(e.message(), "字符串未闭合");
        assert_eq!(e.code(), "E-LEX-001");
    }

    #[test]
    fn suffix_appends_after_message_and_before_code() {
        let e = SmlError::new(E_PARSE_006, "sml: 期望键").suffix("（第 3 行第 8 列）");
        assert_eq!(e.to_string(), "sml: 期望键（第 3 行第 8 列） [E-PARSE-006]");
        assert_eq!(e.pos(), None, "后缀不带位置，不该凭空造出 pos");
    }

    #[test]
    fn string_conversion_drops_code() {
        let e = SmlError::new(E_PARSE_008, "顶层须为容器");
        let s: String = e.into();
        assert_eq!(s, "顶层须为容器");
    }

    #[test]
    fn with_code_keeps_message_and_context_prepends() {
        let e = SmlError::new(E_INCLUDE_011, "词法失败")
            .with_code(E_INCLUDE_001)
            .context("include 展开：");
        assert_eq!(e.code(), "E-INCLUDE-001");
        assert_eq!(e.message(), "include 展开：词法失败");
    }

    #[test]
    fn pos_is_optional_and_survives_code_and_context() {
        let e = SmlError::new(E_LEX_001, "字符串未闭合");
        assert_eq!(e.pos(), None, "未记位置时必须是 None（旧构造点行为不变）");

        let e = SmlError::new(E_LEX_001, "字符串未闭合").at(12);
        assert_eq!(e.pos(), Some(12));

        // 换码 / 加前缀都不该把位置弄丢
        let e = e.with_code(E_PARSE_012).context("外层：");
        assert_eq!(e.pos(), Some(12));
    }

    /// 位置**不属于**稳定契约：同一个错误带不带位置，`Display` / `render` /
    /// `From<SmlError> for String` 必须逐字相同（位置由出口按需附加）。
    #[test]
    fn pos_does_not_change_display_or_render() {
        let plain = SmlError::new(E_LEX_001, "字符串未闭合");
        let with_pos = SmlError::new(E_LEX_001, "字符串未闭合").at(12);
        assert_eq!(plain.to_string(), with_pos.to_string());
        assert_eq!(plain.render(), with_pos.render());
        let s: String = with_pos.into();
        assert_eq!(s, "字符串未闭合");
    }

    #[test]
    fn code_table_is_sorted_and_unique() {
        let mut sorted = ALL.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, ALL.to_vec(), "ALL 应按 id 升序");
        sorted.dedup();
        assert_eq!(sorted.len(), ALL.len(), "ALL 里有重复的码");
        assert_eq!(ALL.len(), COUNT);
    }

    #[test]
    fn code_shape_is_stable() {
        for c in ALL {
            let parts: Vec<&str> = c.split('-').collect();
            assert_eq!(parts.len(), 3, "码形状应为 级别-领域-序号：{c}");
            assert!(parts[0] == "E" || parts[0] == "W" || parts[0] == "I", "{c}");
            assert_eq!(parts[2].len(), 3, "序号应为三位：{c}");
            assert!(
                parts[2].chars().all(|ch| ch.is_ascii_digit()),
                "序号应为数字：{c}"
            );
            assert!(
                parts[1].chars().all(|ch| ch.is_ascii_uppercase()),
                "领域应全大写：{c}"
            );
        }
    }
}
