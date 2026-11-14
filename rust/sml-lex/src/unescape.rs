//! 引号串的**转义解码**：规则只此一份。
//!
//! 为什么单独成模块：词法器要它（`"a\nb"` 这种含转义串必须解码），
//! 解析器的单遍快速路径也要它（`fast.rs` 直接按字节扫，不走 token 流）。
//! 两处各写一份转义表迟早分叉，而分叉的表现是「同一份输入，两条路径解出不同字符串」
//! —— 静默错解里最难查的一类。
//!
//! 支持：`\n` `\t` `\r` `\0` `\"` `\\` `\uXXXX` `\u{XXXX}`。
//! 未知转义 ⇒ `E-LEX-004`；`\u` 后非法/不足 ⇒ `E-LEX-005`。

use sml_codes::SmlError;
use sml_codes::{E_LEX_004, E_LEX_005};

/// 解码引号串的**内容**（不含首尾引号）。
///
/// 调用方保证 `content` 是已配对的两个引号之间的原文；未闭合/悬空反斜杠这类
/// **结构性**错误由扫描方（词法器 / 快速路径）自己报（`E-LEX-001` / `E-LEX-006`）。
pub fn unescape_quoted(content: &str) -> Result<String, SmlError> {
    let mut out = String::with_capacity(content.len());
    let mut it = content.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        // 转义：\n \t \r \0 \" \\ \u{XXXX} \uXXXX
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('u') => {
                let mut hex = String::new();
                if it.peek() == Some(&'{') {
                    it.next();
                    for c2 in it.by_ref() {
                        if c2 == '}' {
                            break;
                        }
                        hex.push(c2);
                    }
                } else {
                    for _ in 0..4 {
                        match it.next() {
                            Some(c2) => hex.push(c2),
                            // 定长读 4 位，不足说明输入被截断
                            None => {
                                return Err(SmlError::from_code_title(E_LEX_005)
                                    .with_detail(format!("期望 4 位，得 {hex:?}")));
                            }
                        }
                    }
                }
                if hex.is_empty() {
                    return Err(SmlError::from_code_title(E_LEX_005)
                        .with_detail("缺少十六进制数字"));
                }
                let cp = u32::from_str_radix(&hex, 16).map_err(|_| {
                    SmlError::from_code_title(E_LEX_005)
                        .with_detail(format!("含非十六进制数字：{hex:?}"))
                })?;
                let ch = char::from_u32(cp).ok_or_else(|| {
                    SmlError::from_code_title(E_LEX_005)
                        .with_detail(format!("非法 Unicode 码点：U+{cp:04X}"))
                })?;
                out.push(ch);
            }
            Some(other) => {
                return Err(SmlError::from_code_title(E_LEX_004).with_detail(format!(
                    "\\{other}（仅支持 \\n \\t \\r \\0 \\\" \\\\ \\uXXXX）"
                )));
            }
            None => {
                // 内容以孤立反斜杠结尾（扫描方本应先报 E-LEX-006，这里兜底）
                return Err(SmlError::from_code_title(E_LEX_005)
                    .with_detail("转义符后缺少字符"));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_content_is_copied() {
        assert_eq!(unescape_quoted("hello world").unwrap(), "hello world");
        assert_eq!(unescape_quoted("").unwrap(), "");
    }

    #[test]
    fn known_escapes() {
        assert_eq!(unescape_quoted(r"a\nb").unwrap(), "a\nb");
        assert_eq!(unescape_quoted(r"a\tb").unwrap(), "a\tb");
        assert_eq!(unescape_quoted(r"a\rb").unwrap(), "a\rb");
        assert_eq!(unescape_quoted(r"a\0b").unwrap(), "a\0b");
        assert_eq!(unescape_quoted(r#"a\"b"#).unwrap(), "a\"b");
        assert_eq!(unescape_quoted(r"a\\b").unwrap(), "a\\b");
        assert_eq!(unescape_quoted(r"\u{4F60}").unwrap(), "\u{4F60}");
        assert_eq!(unescape_quoted(r"\u0041").unwrap(), "A");
    }

    #[test]
    fn bad_escapes_are_reported() {
        assert!(unescape_quoted(r"\q").is_err());       // E-LEX-004
        assert!(unescape_quoted(r"\u12").is_err());     // 不足 4 位
        assert!(unescape_quoted(r"\u{}").is_err());     // 空 hex
        assert!(unescape_quoted(r"\uD800").is_err());   // 非法码点
        assert!(unescape_quoted(r"\uZZZZ").is_err());   // 非 hex
    }
}
