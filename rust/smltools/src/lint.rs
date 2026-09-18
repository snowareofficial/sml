//! `smltools --lint`：对 SML 文档做静态检查（不产出转换结果）。
//!
//! 检查项刻意选**能从文本与解析结果可靠判定**的那些，不做需要完整语义分析的事。
//! 宁可少报，也不要误报 —— 一个爱喊狼来了的 linter 会被直接关掉。
//!
//! | 级别 | 检查 |
//! |---|---|
//! | error | 解析失败（语法 / 契约 / include / `$env` 等） |
//! | error | 行首用 tab 缩进 |
//! | warning | 片段 `@name` 定义后从未被 `&name` 引用 |
//! | warning | 契约 `@contract Name` 定义后从未被 `@is Name` 应用 |
//! | warning | 同一父块下键名重复（后者覆盖前者） |
//! | warning | 空值字段（`key:` 后既无值，下一有效行也没缩进进去） |
//! | warning | 嵌套过深（超过 `MAX_VALUE_DEPTH` 的一半） |
//!
//! 输出格式 `路径:行号: 级别: 说明 [码]`，grep / 编辑器 / CI 都好吃。
//! 码取自 `errors/codes.sml` 的 LINT 领域（`E-LINT-001`、`W-LINT-001..005`）；
//! 解析类问题**原样透传**语言层的码，不包成 `E-LINT-*`。

use std::collections::BTreeMap;
use std::path::PathBuf;

use sml::{parse, Value};
use sml_codes::{E_LINT_001, W_LINT_001, W_LINT_002, W_LINT_003, W_LINT_004, W_LINT_005};

/// 检查结果。
pub struct Report {
    /// 每条一行，已带路径与行号
    pub messages: Vec<String>,
    /// 是否含 error 级问题（调用方据此决定退出码）
    pub has_error: bool,
}

impl Report {
    /// 追加一条诊断：`路径:行号: 级别: 说明 [码]`。
    ///
    /// 码缀在文案之后（与 `SmlError` 的 `Display` 同口径）：读文案时不受打扰，
    /// 需要码时一眼可见 —— lint 输出也是对外交互面，用户据此能查到码表。
    fn push(&mut self, name: &str, line: usize, level: &str, code: &str, msg: &str) {
        if level == "error" {
            self.has_error = true;
        }
        if line == 0 {
            self.messages.push(format!("{name}: {level}: {msg} [{code}]"));
        } else {
            self.messages
                .push(format!("{name}:{line}: {level}: {msg} [{code}]"));
        }
    }
}

/// SML 内置指令：这些 `@xxx` 不是片段定义。
const BUILTIN_DIRECTIVES: &[&str] = &[
    "contract", "is", "type", "version", "feature", "when", "for", "include",
];

/// `MAX_VALUE_DEPTH` 的一半，作为「嵌套过深」的提醒阈值。
const DEPTH_WARN: usize = 64;

/// 对文本做检查。`path` 仅用于消息前缀。
pub fn check(text: &str, path: &Option<PathBuf>) -> Report {
    let name = path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<stdin>".to_string());
    let mut report = Report {
        messages: Vec::new(),
        has_error: false,
    };

    // ---- 1. 解析 ----
    let parsed = parse(text);
    if let Err(e) = &parsed {
        // 解析错误的码**原样透传**（E-LEX-* / E-PARSE-* / E-CONTRACT-* …），
        // 不换成 E-LINT-*：lint 只是呈现者，语言层的码才是事实。
        report.push(&name, 0, "error", e.code(), e.message());
    }

    // ---- 2. 逐行文本检查 ----
    let lines: Vec<(usize, String, usize)> = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.to_string(), indent_of(l)))
        .collect();

    let mut fragment_defs: Vec<(String, usize)> = Vec::new();
    let mut contract_defs: Vec<(String, usize)> = Vec::new();
    let mut used_frags: BTreeMap<String, usize> = BTreeMap::new();
    let mut used_contracts: BTreeMap<String, usize> = BTreeMap::new();

    for (no, raw, indent) in &lines {
        // tab 缩进
        if raw.starts_with('\t') || raw.starts_with(" \t") {
            // E-LINT-001：缩进里出现制表符（按错误处理 —— 制表符宽度因编辑器而异）。
            report.push(
                &name,
                *no,
                "error",
                E_LINT_001,
                "缩进里出现 tab；SML 缩进敏感，请统一用空格",
            );
        }
        let body = strip_comment(raw).trim();
        if body.is_empty() {
            continue;
        }
        // 片段定义 / 契约定义 / 应用
        if let Some(d) = body.strip_prefix('@') {
            let word = d
                .split(|c: char| c.is_whitespace() || c == ':' || c == '{')
                .next()
                .unwrap_or("");
            if word == "contract" {
                if let Some(nm) = first_word_after(d, "contract") {
                    contract_defs.push((nm, *no));
                }
            } else if word == "is" {
                if let Some(nm) = first_word_after(d, "is") {
                    used_contracts.insert(nm, *no);
                }
            } else if !word.is_empty() && !BUILTIN_DIRECTIVES.contains(&word) {
                fragment_defs.push((word.to_string(), *no));
            }
        }
        // 片段/契约引用（`&name`、`@is name` 已在上面处理）
        for (idx, _) in body.match_indices('&') {
            let rest = &body[idx + 1..];
            let nm: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if !nm.is_empty() {
                used_frags.insert(nm, *no);
            }
        }
        let _ = indent;
    }

    for (nm, no) in &fragment_defs {
        if !used_frags.contains_key(nm) {
            // W-LINT-001：片段定义后从未被引用。
            report.push(
                &name,
                *no,
                "warning",
                W_LINT_001,
                &format!("片段 `@{nm}` 定义后从未被 `&{nm}` 引用"),
            );
        }
    }
    for (nm, no) in &contract_defs {
        if !used_contracts.contains_key(nm) {
            // W-LINT-002：契约定义后从未被应用。
            report.push(
                &name,
                *no,
                "warning",
                W_LINT_002,
                &format!("契约 `@contract {nm}` 定义后从未被 `@is {nm}` 应用"),
            );
        }
    }

    // ---- 3. 重复键 / 空值字段（基于缩进）----
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for idx in 0..lines.len() {
        let (no, raw, indent) = &lines[idx];
        let body = strip_comment(raw).trim().to_string();
        if body.is_empty() {
            continue;
        }
        let Some((k, v)) = split_kv(&body) else {
            continue;
        };
        while let Some((ind, _)) = stack.last() {
            if *ind >= *indent {
                stack.pop();
            } else {
                break;
            }
        }
        let path: Vec<&str> = stack.iter().map(|(_, k)| k.as_str()).collect();
        let full = format!("{}\u{1}{}", path.join("/"), k);
        if let Some(prev) = seen.get(&full) {
            // W-LINT-003：同一块内键名重复（静默数据丢失的常见来源）。
            report.push(
                &name,
                *no,
                "warning",
                W_LINT_003,
                &format!("键 `{k}` 在同一块内重复（第 {prev} 行已出现），后者会覆盖前者"),
            );
        } else {
            seen.insert(full, *no);
        }
        // 空值字段：本行无值，且后面第一个非空行的缩进没有更深
        if v.is_empty() {
            let next_indent = lines[idx + 1..]
                .iter()
                .find(|(_, r, _)| !strip_comment(r).trim().is_empty())
                .map(|(_, _, i)| *i);
            let empty = match next_indent {
                Some(ni) => ni <= *indent,
                None => true,
            };
            if empty {
                // W-LINT-004：字段是空值。
                report.push(
                    &name,
                    *no,
                    "warning",
                    W_LINT_004,
                    &format!("字段 `{k}` 是空值"),
                );
            }
        }
        stack.push((*indent, k));
    }

    // ---- 4. 结构检查（仅解析成功时）----
    if let Ok(v) = &parsed {
        let d = depth_of(v);
        if d > DEPTH_WARN {
            // W-LINT-005：嵌套超过建议阈值（只提醒可读性，不是拒绝 —— 解析上限见 E-LIMIT-001）。
            report.push(
                &name,
                0,
                "warning",
                W_LINT_005,
                &format!(
                    "嵌套深度 {d} 已超过建议阈值 {DEPTH_WARN}（解析上限为 MAX_VALUE_DEPTH）"
                ),
            );
        }
    }

    report
}

/// 行首空白宽度（tab 按 8 计，仅用于比较层级）。
fn indent_of(line: &str) -> usize {
    let mut w = 0;
    for c in line.chars() {
        match c {
            ' ' => w += 1,
            '\t' => w += 8,
            _ => break,
        }
    }
    w
}

/// 去掉行尾注释（`#` 前须为空白或行首）。
fn strip_comment(s: &str) -> &str {
    let b = s.as_bytes();
    let (mut in_s, mut in_d) = (false, false);
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !in_d => in_s = !in_s,
            b'"' if !in_s => in_d = !in_d,
            b'#' if !in_s && !in_d => {
                if i == 0 || b[i - 1] == b' ' || b[i - 1] == b'\t' {
                    return &s[..i];
                }
            }
            _ => {}
        }
    }
    s
}

/// 取 `指令` 之后的第一个词（如 `contract Server {` → `Server`）。
fn first_word_after(s: &str, kw: &str) -> Option<String> {
    let rest = s.trim_start().strip_prefix(kw)?.trim_start_matches([':', ' ']);
    let w: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if w.is_empty() {
        None
    } else {
        Some(w)
    }
}

/// 切分 `键: 值`（冒号须后跟空白或行尾）。
fn split_kv(s: &str) -> Option<(String, String)> {
    let b = s.as_bytes();
    let (mut in_s, mut in_d) = (false, false);
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !in_d => in_s = !in_s,
            b'"' if !in_s => in_d = !in_d,
            b':' if !in_s && !in_d => {
                let next_ok = i + 1 >= b.len() || b[i + 1] == b' ' || b[i + 1] == b'\t';
                if next_ok {
                    let k = s[..i].trim().trim_matches(['"', '\'']).to_string();
                    if k.is_empty() {
                        return None;
                    }
                    return Some((k, s[i + 1..].trim().to_string()));
                }
            }
            _ => {}
        }
    }
    None
}

/// 值的嵌套深度。
fn depth_of(v: &Value) -> usize {
    match v {
        Value::Array(a) => 1 + a.iter().map(depth_of).max().unwrap_or(0),
        Value::Object(m) => 1 + m.values().map(depth_of).max().unwrap_or(0),
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(src: &str) -> String {
        check(src, &None).messages.join("\n")
    }

    /// 每条诊断都带上码 —— 这些码是 lint 的对外契约（`errors/codes.sml` 的 LINT 领域）。
    #[test]
    fn every_diagnostic_carries_a_code() {
        // E-LINT-001：tab 缩进
        assert!(messages("\tk: 1\n").contains("[E-LINT-001]"), "tab 缩进应带码");
        // W-LINT-001 / 002 / 003 / 004：四类告警
        let src = "@frag { a: 1 }\n@contract Unused { a: int }\nk: 1\nk: 2\nempty:\n";
        let out = messages(src);
        for want in ["[W-LINT-001]", "[W-LINT-002]", "[W-LINT-003]", "[W-LINT-004]"] {
            assert!(out.contains(want), "期望含 {want}：\n{out}");
        }
    }

    /// W-LINT-005：嵌套超过建议阈值（阈值 64，解析上限 128）。
    #[test]
    fn deep_nesting_warns_with_code() {
        let mut src = String::new();
        for i in 0..70 {
            src.push_str(&"  ".repeat(i));
            src.push_str(&format!("k{i} {{\n"));
        }
        for i in (0..70).rev() {
            src.push_str(&"  ".repeat(i));
            src.push_str("}\n");
        }
        assert!(messages(&src).contains("[W-LINT-005]"), "深嵌套应带码");
    }

    /// 解析类问题**透传语言层的码**，不换成 E-LINT-*。
    #[test]
    fn parse_error_keeps_language_layer_code() {
        let out = messages("a {\n");
        assert!(out.contains("[E-PARSE-001]"), "应透传 E-PARSE-001：\n{out}");
    }
}
