// SPDX-License-Identifier: MulanPSL-2.0
//! JSON-compatible rendering of a `Value` back to the data-subset text form.
//! 把 `Value` 渲染回数据子集文本，输出与 JSON 兼容。
//!
//! Bare-word / escaping / float rules mirror `sml-value/text_rules.rs` (data-only subset).
//! Floats are rendered with `raw = None`: scientific notation is not preserved (I-28),
//! non-finite values become quoted strings so the output stays a valid data document.
//! 裸词 / 转义 / 浮点规则与 `sml-value/text_rules.rs` 同口径（纯数据子集）。
//! 浮点按 `raw = None` 渲染：不保留科学计数法（I-28）；非有限数写成带引号字符串，
//! 保证输出仍是合法数据文档。

use alloc::borrow::Cow;
use alloc::format;
use alloc::string::String;

use crate::value::Value;

/// Render a value to text. ｜ 把值渲染成文本。
pub fn to_string(value: &Value) -> String {
    let mut out = String::new();
    emit(value, &mut out);
    out
}

/// Render a value to UTF-8 bytes. ｜ 把值渲染成 UTF-8 字节。
pub fn to_vec(value: &Value) -> alloc::vec::Vec<u8> {
    to_string(value).into_bytes()
}

fn emit(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&format!("{i}")),
        Value::Float(f) => out.push_str(&format_float(*f)),
        Value::Str(s) => out.push_str(&quote_if_needed(s)),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                emit(item, out);
            }
            out.push(']');
        }
        Value::Object(obj) => {
            out.push('{');
            for (i, (k, v)) in obj.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                out.push_str(&quote_if_needed(k));
                out.push_str(": ");
                emit(v, out);
            }
            out.push('}');
        }
    }
}

/// Whether a string would lose its type or break syntax if emitted bare.
/// 字符串裸写是否会丢失类型或破坏语法。
fn needs_quote(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    if matches!(s, "true" | "false" | "null" | "inf" | "nan") {
        return true;
    }
    if s.parse::<i64>().is_ok() || s.parse::<f64>().is_ok() {
        return true;
    }
    if s.starts_with("--")
        || s.starts_with("//")
        || s.starts_with("/*")
        || s.starts_with("*/")
        || s.starts_with('*')
        || s.starts_with("_*")
    {
        return true;
    }
    if s.contains([' ', '\t', '\n', '\r', ':', '#', '{', '}', ',', '[', ']', '"', '\\', '/', '*'])
    {
        return true;
    }
    match s.chars().next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return true,
    }
    !s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// Emit a string bare when safe, else quoted and escaped.
/// 能裸就裸，否则加引号并转义。
fn quote_if_needed(s: &str) -> Cow<'_, str> {
    if needs_quote(s) {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                '\0' => out.push_str("\\0"),
                c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                    out.push_str(&format!("\\u{:04X}", c as u32));
                }
                c => out.push(c),
            }
        }
        out.push('"');
        Cow::Owned(out)
    } else {
        Cow::Borrowed(s)
    }
}

/// Format a float with no raw literal (I-28: no scientific-notation round-trip).
/// 按无 raw 字面量格式化浮点（I-28：不保留科学计数法 round-trip）。
fn format_float(f: f64) -> String {
    if !f.is_finite() {
        format!("\"{f}\"")
    } else if f == (f as i64) as f64 {
        // `f64::fract()` 在 core 下不可用；用往返 i64 判定整数值（大数截断时按原值输出）。
        format!("{f:.1}")
    } else {
        format!("{f}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_words_stay_bare() {
        assert_eq!(to_string(&Value::from("hello-world")), "hello-world");
        assert_eq!(to_string(&Value::from("a b")), "\"a b\"");
    }

    #[test]
    fn float_spelling() {
        assert_eq!(to_string(&Value::Float(1.0)), "1.0");
        assert_eq!(to_string(&Value::Float(1.5)), "1.5");
        assert_eq!(to_string(&Value::Float(f64::NAN)), "\"NaN\"");
    }
}
