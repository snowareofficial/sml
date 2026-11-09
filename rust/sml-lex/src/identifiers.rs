//! Identifier whitelist (S1b) ｜ 标识符白名单。
//!
//! 门控在特性位 `lex.ident`（默认**关**）—— 关掉就回到 v4 行为，既有文档一字不变。
//! 唯一权威是 `docs/swsml/plan/featag-slices.md` §2 的那两张表；本文件是它在 Rust 侧的实现。
//!
//! # 允许
//!
//! | 范围 | 说明 |
//! |---|---|
//! | `A-Z` `a-z` `0-9` | ASCII |
//! | `_` `-` | 下划线与连字符（路径分隔只用 `.` 与 `[]`，不冲突） |
//! | U+4E00–U+9FFF | CJK 统一汉字 |
//! | U+3040–U+30FF | 平假名 / 片假名 |
//! | U+AC00–U+D7AF | 谚文音节 |
//!
//! # 拒绝（六类，共用 `E-LEX-007`，靠 detail 区分）
//!
//! 1. `.` `[` `]` —— **路径语法保留**（标识符里出现它们，`Path` 解析就永远有歧义）；
//! 2. 空白 / 控制字符（Cc）—— 不可见，且会把一个键拆成两个；
//! 3. 零宽 / 双向控制（Cf）—— 不可见、可伪造显示；
//! 4. **组合标记（Mn/Mc）** —— 只收**预组合**码位（`é` = U+00E9），于是 **NFC 自动成立**，
//!    不需要归一化库，也不需要 Unicode 数据表（五端各写一段范围判断即可）；
//! 5. 西里尔 / 希腊 / 亚美尼亚等同形字母（`а`(U+0430) 与 `a`）—— 白名单天然排除，单独报是为了说清「为什么」；
//! 6. 全角 / 兼容区字符 —— 「看起来一样、比较不等」。
//!
//! **「每契约单一语种」不在这里**：那条规则要比较**同一契约内多个标识符**
//! （见 `docs/swsml/plan/featag-slices.md` §2.2），落在契约声明收集（S4）那一层。

use std::fmt;

use sml_codes::{E_LEX_007, SmlError};

/// 为什么一个标识符不合规 ｜ why an identifier is rejected.
///
/// `at` 一律是 **1 起算的字符序号**（不是字节偏移），与错误文案一致。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentError {
    /// 空标识符（`""` 或裸的空键）。
    Empty,
    /// `.` `[` `]`：路径语法保留。
    Reserved { ch: char, at: usize },
    /// 空白 / 控制字符（Cc）。
    Blank { ch: char, at: usize },
    /// 不可见字符：零宽 / 双向控制（Cf）或组合标记（Mn/Mc，⇒ 非 NFC）。
    Invisible {
        ch: char,
        at: usize,
        /// `"零宽/格式字符（Cf）"` 或 `"组合标记（Mn/Mc，非预组合 ⇒ 非 NFC）"`。
        kind: &'static str,
    },
    /// 同形字母：西里尔 / 希腊 / 亚美尼亚。
    Homoglyph { ch: char, at: usize },
    /// 全角 / 兼容区字符。
    Wide { ch: char, at: usize },
    /// 其余不在白名单里的字符。
    Other { ch: char, at: usize },
}

fn addr(c: char) -> String {
    format!("U+{:04X}", c as u32)
}

impl fmt::Display for IdentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IdentError::Empty => write!(f, "标识符为空"),
            IdentError::Reserved { ch, at } => write!(
                f,
                "第 {at} 个字符 `{ch}`（{}）是路径语法保留字符，键名里不能出现 `.` `[` `]`",
                addr(*ch)
            ),
            IdentError::Blank { ch, at } => {
                write!(f, "第 {at} 个字符是空白或控制字符（{}）", addr(*ch))
            }
            IdentError::Invisible { ch, at, kind } => write!(
                f,
                "第 {at} 个字符是{kind}（{}）——不可见，且能伪造看起来一样的名字",
                addr(*ch)
            ),
            IdentError::Homoglyph { ch, at } => write!(
                f,
                "第 {at} 个字符 `{ch}`（{}）是与拉丁字母同形的西里尔 / 希腊 / 亚美尼亚字母；\
                 声明处只允许一种字形，请改用 ASCII",
                addr(*ch)
            ),
            IdentError::Wide { ch, at } => write!(
                f,
                "第 {at} 个字符 `{ch}`（{}）是全角 / 兼容区字符：与半角混用会「看起来一样、比较不等」",
                addr(*ch)
            ),
            IdentError::Other { ch, at } => write!(
                f,
                "第 {at} 个字符 `{ch}`（{}）不在白名单内（只允许字母数字、`_` `-`、汉字 / 假名 / 谚文）",
                addr(*ch)
            ),
        }
    }
}

impl IdentError {
    /// 包成带码的错误（`E-LEX-007`），detail 带上原文以便定位。
    pub fn into_error(self, ident: &str) -> SmlError {
        SmlError::from_code_msg(E_LEX_007).with_detail(format!("`{ident}`：{self}"))
    }
}

/// 单个字符是否在允许范围里 ｜ whether one char is allowed.
///
/// 返回值只用于「通过/不通过」，`Ok(())` 不区分是 ASCII 还是 CJK —— 语种级规则不在此层。
fn check_char(c: char) -> Result<(), IdentError> {
    let at_placeholder = 0usize; // 由调用方补真实位置
    match c {
        'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | '-' => Ok(()),
        '\u{4E00}'..='\u{9FFF}' => Ok(()), // CJK 汉字
        '\u{3040}'..='\u{30FF}' => Ok(()), // 假名
        '\u{AC00}'..='\u{D7AF}' => Ok(()), // 谚文音节
        '.' | '[' | ']' => Err(IdentError::Reserved { ch: c, at: at_placeholder }),
        _ if c.is_whitespace() || (c as u32) < 0x20 || c as u32 == 0x7F => {
            Err(IdentError::Blank { ch: c, at: at_placeholder })
        }
        // 组合标记 Mn/Mc（含「组合用西里尔字母」，故必须排在下面的西里尔块**之前**）
        _ if in_ranges(
            c,
            &[
                (0x0300, 0x036F),
                (0x0483, 0x0489),
                (0x1AB0, 0x1AFF),
                (0x1DC0, 0x1DFF),
                (0x20D0, 0x20FF),
                (0xFE20, 0xFE2F),
            ],
        ) =>
        {
            Err(IdentError::Invisible {
                ch: c,
                at: at_placeholder,
                kind: "组合标记（Mn/Mc，非预组合 ⇒ 非 NFC）",
            })
        }
        // 零宽 / 双向控制 / 其它格式字符 Cf
        _ if in_ranges(
            c,
            &[
                (0x00AD, 0x00AD),
                (0x0600, 0x0605),
                (0x061C, 0x061C),
                (0x06DD, 0x06DD),
                (0x070F, 0x070F),
                (0x180E, 0x180E),
                (0x200B, 0x200F),
                (0x202A, 0x202E),
                (0x2060, 0x2064),
                (0x2066, 0x206F),
                (0xFEFF, 0xFEFF),
                (0xFFF9, 0xFFFB),
                (0x1D173, 0x1D17A),
                (0xE0001, 0xE0001),
                (0xE0020, 0xE007F),
            ],
        ) =>
        {
            Err(IdentError::Invisible {
                ch: c,
                at: at_placeholder,
                kind: "零宽/格式字符（Cf）",
            })
        }
        // 同形字母：西里尔 / 希腊（含扩展）/ 亚美尼亚
        _ if in_ranges(
            c,
            &[
                (0x0370, 0x03FF),
                (0x0400, 0x04FF),
                (0x0500, 0x052F),
                (0x0530, 0x058F),
                (0x1F00, 0x1FFF),
            ],
        ) =>
        {
            Err(IdentError::Homoglyph { ch: c, at: at_placeholder })
        }
        // 全角 / 兼容区
        _ if in_ranges(
            c,
            &[
                (0x2E80, 0x2EFF),
                (0x3000, 0x303F),
                (0x31C0, 0x31EF),
                (0xFE30, 0xFE4F),
                (0xFF01, 0xFF60),
                (0xFFE0, 0xFFE6),
            ],
        ) =>
        {
            Err(IdentError::Wide { ch: c, at: at_placeholder })
        }
        _ => Err(IdentError::Other { ch: c, at: at_placeholder }),
    }
}

fn in_ranges(c: char, ranges: &[(u32, u32)]) -> bool {
    let u = c as u32;
    ranges.iter().any(|&(lo, hi)| u >= lo && u <= hi)
}

/// 标识符用的是什么**文字系统**（只服务「每契约单一语种」，不参与单字符合法性判定）。
///
/// 汉字与假名算**同一族**（`Script::Cjk`）：日文里汉字与假名本来就混写
/// （`設定ファイル`），硬拆成两族会让日文标识符无法书写。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    /// ASCII 拉丁字母。
    Latin,
    /// 汉字 + 假名。
    Cjk,
    /// 谚文音节。
    Hangul,
}

impl fmt::Display for Script {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Script::Latin => write!(f, "拉丁字母"),
            Script::Cjk => write!(f, "汉字/假名"),
            Script::Hangul => write!(f, "谚文"),
        }
    }
}

/// 取标识符里的**第一种**文字系统；纯数字 / `_` / `-`（不含字母）返回 `None`。
///
/// 只看第一种：同一标识符内部混排（`api接口`）**不**在此层拒绝 —— 那属于书写习惯，
/// 不是同形字风险（同形字是「不同脚本长得一样」，而这里所有允许的脚本都已过白名单）。
/// 「每契约单一语种」比较的是**不同标识符之间**，那才是同形字真正生效的地方。
pub fn script_of(s: &str) -> Option<Script> {
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' => return Some(Script::Latin),
            '\u{4E00}'..='\u{9FFF}' | '\u{3040}'..='\u{30FF}' => return Some(Script::Cjk),
            '\u{AC00}'..='\u{D7AF}' => return Some(Script::Hangul),
            _ => {}
        }
    }
    None
}

/// 校验一个标识符（键名 / 契约名 / featag 名）｜ check one identifier.
///
/// 调用方负责**门控**（`Feature::Ident`）：本函数只判规则，不看特性集。
pub fn check_ident(s: &str) -> Result<(), IdentError> {
    if s.is_empty() {
        return Err(IdentError::Empty);
    }
    for (i, c) in s.chars().enumerate() {
        let at = i + 1;
        if let Err(mut e) = check_char(c) {
            // 把占位位置换成真实序号（每个变体都带 `at`）
            match &mut e {
                IdentError::Reserved { at: a, .. }
                | IdentError::Blank { at: a, .. }
                | IdentError::Invisible { at: a, .. }
                | IdentError::Homoglyph { at: a, .. }
                | IdentError::Wide { at: a, .. }
                | IdentError::Other { at: a, .. } => *a = at,
                IdentError::Empty => {}
            }
            return Err(e);
        }
    }
    Ok(())
}

/// 校验并直接给带码错误 ｜ check and wrap into a coded error.
pub fn check_ident_err(s: &str) -> Result<(), SmlError> {
    check_ident(s).map_err(|e| e.into_error(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) {
        assert_eq!(check_ident(s), Ok(()), "应通过：{s:?}");
    }

    #[test]
    fn allows_the_whitelist() {
        for s in ["a", "user_id", "x-y", "A1", "端口", "設定ファイル", "이름"] {
            ok(s);
        }
    }

    #[test]
    fn rejects_path_reserved_chars() {
        assert_eq!(
            check_ident("a.b"),
            Err(IdentError::Reserved { ch: '.', at: 2 })
        );
        assert_eq!(
            check_ident("a[0]"),
            Err(IdentError::Reserved { ch: '[', at: 2 })
        );
    }

    #[test]
    fn rejects_blank_control_invisible() {
        assert!(matches!(
            check_ident("a b"),
            Err(IdentError::Blank { ch: ' ', at: 2 })
        ));
        assert!(matches!(
            check_ident("a\tb"),
            Err(IdentError::Blank { ch: '\t', at: 2 })
        ));
        // 零宽空格 U+200B
        assert!(matches!(
            check_ident("a\u{200B}b"),
            Err(IdentError::Invisible { ch: '\u{200B}', at: 2, .. })
        ));
        // 组合用重音符 U+0301 ⇒ 非预组合，等价于「非 NFC」
        assert!(matches!(
            check_ident("e\u{0301}"),
            Err(IdentError::Invisible { ch: '\u{0301}', at: 2, .. })
        ));
    }

    #[test]
    fn rejects_homoglyphs_and_wide() {
        assert!(matches!(
            check_ident("а"),
            Err(IdentError::Homoglyph { ch: 'а', at: 1 })
        ));
        assert!(matches!(
            check_ident("ａ"),
            Err(IdentError::Wide { ch: 'ａ', at: 1 })
        ));
    }

    #[test]
    fn empty_and_position_reporting() {
        assert_eq!(check_ident(""), Err(IdentError::Empty));
        // 位置是**字符**序号，不是字节：`端口.x` 的点在第 3 个字符
        assert_eq!(
            check_ident("端口.x"),
            Err(IdentError::Reserved { ch: '.', at: 3 })
        );
    }
}
