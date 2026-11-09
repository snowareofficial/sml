// SPDX-License-Identifier: MulanPSL-2.0
//! `smltools --lint` 复用的扫描工具；不挂 crate 根，避免污染 L0 补全列表。
//! ｜ Scan helpers for `smltools --lint`; kept off the crate root.
//!
//! 名单与切分规则必须与解析器同源，故留在库内由 CLI 消费，而不是在 CLI 里重写一份。
//! ｜ The directive list and key/value split rule stay parser-side so the CLI cannot drift.

use crate::{strip_line_comment, Value};

/// 判断 `@xxx` 是否为内置指令：解析层名单 + 由 `sml-include` 处理的 `include`。
/// ｜ Whether `@xxx` is a builtin directive: the parser's reserved list plus `include`.
///
/// 名单直接取 [`crate::ext::BUILTIN_DIRECTIVES`]，新增内置指令时 lint 自动跟随。
/// ｜ Reads [`crate::ext::BUILTIN_DIRECTIVES`] directly, so new directives cannot drift.
pub fn is_builtin_directive(word: &str) -> bool {
    word == "include" || crate::ext::BUILTIN_DIRECTIVES.contains(&word)
}

/// 切分 `键: 值`，与解析器同口径：冒号即分隔符，引号内的冒号不算。
/// ｜ Split `key: value` like the parser: a colon outside quotes is the separator.
pub fn split_kv(s: &str) -> Option<(String, String)> {
    let b = s.as_bytes();
    let (mut in_s, mut in_d) = (false, false);
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !in_d => in_s = !in_s,
            b'"' if !in_s => in_d = !in_d,
            b':' if !in_s && !in_d => {
                let k = s[..i].trim().trim_matches(['"', '\'']).to_string();
                if k.is_empty() {
                    return None;
                }
                return Some((k, s[i + 1..].trim().to_string()));
            }
            _ => {}
        }
    }
    None
}

/// 值的嵌套深度：根容器算 1，标量算 1。
/// ｜ Nesting depth of a value; a root container and a scalar each count 1.
pub fn depth_of(value: &Value) -> usize {
    match value {
        Value::Array(a) => 1 + a.iter().map(depth_of).max().unwrap_or(0),
        Value::Object(m) => 1 + m.values().map(depth_of).max().unwrap_or(0),
        _ => 1,
    }
}

/// 源码扫描结果：契约 / 片段的定义与引用，行号从 1 起。
/// ｜ Source-scan result: contract / fragment definitions and uses, 1-based lines.
#[derive(Debug, Default, Clone)]
pub struct SourceRefs {
    pub contract_defs: Vec<(String, usize)>,
    pub contract_uses: Vec<(String, usize)>,
    pub fragment_defs: Vec<(String, usize)>,
    pub fragment_uses: Vec<(String, usize)>,
}

/// 行级扫描源码里的契约 / 片段定义与引用，供 `smltools --lint` 判未使用。
/// ｜ Line-level scan of contract / fragment definitions and uses for `smltools --lint`.
///
/// 不是解析器；与解析器共用 `strip_line_comment` 与内置指令名单，故收在库内。
/// ｜ Not a parser; shares `strip_line_comment` and the directive list with the parser.
pub fn scan_source_refs(text: &str) -> SourceRefs {
    let mut out = SourceRefs::default();
    for (i, raw) in text.lines().enumerate() {
        let no = i + 1;
        let body = strip_line_comment(raw).trim();
        if body.is_empty() {
            continue;
        }
        if let Some(d) = body.strip_prefix('@') {
            let word = d
                .split(|c: char| c.is_whitespace() || c == ':' || c == '{')
                .next()
                .unwrap_or("");
            if word == "contract" {
                if let Some(nm) = first_word_after(d, "contract") {
                    out.contract_defs.push((nm, no));
                }
            } else if word == "is" {
                if let Some(nm) = first_word_after(d, "is") {
                    out.contract_uses.push((nm, no));
                }
            } else if !word.is_empty() && !is_builtin_directive(word) {
                out.fragment_defs.push((word.to_string(), no));
            }
        }
        for (idx, _) in body.match_indices('&') {
            let rest = &body[idx + 1..];
            let nm: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if !nm.is_empty() {
                out.fragment_uses.push((nm, no));
            }
        }
    }
    out
}

/// 取 `指令` 之后的第一个词（如 `contract Server {` → `Server`）。
fn first_word_after(s: &str, kw: &str) -> Option<String> {
    let rest = s.trim_start().strip_prefix(kw)?.trim_start_matches([':', ' ']);
    let w: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if w.is_empty() { None } else { Some(w) }
}
