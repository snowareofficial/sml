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
//! | warning | 同一父块下键名重复（`W-PARSE-001`：**已提升为数组，值不丢**） |
//! | warning | 片段展开覆盖父块键（`W-PARSE-002`：**父块原值丢失**） |
//! | warning | 空值字段（`key:` 后既无值，下一有效行也没缩进进去） |
//! | warning | 嵌套过深（超过 `MAX_VALUE_DEPTH` 的一半） |
//!
//! 输出格式 `路径:行号: 级别: 说明 [码]`，grep / 编辑器 / CI 都好吃。
//! 码取自 `errors/codes.sml` 的 LINT 领域（`E-LINT-001`、`W-LINT-001..005`）；
//! 解析类问题（含 `W-PARSE-001/002`）**原样透传**语言层的码，不包成 `E-LINT-*`。
//!
//! > 重复键 / 片段覆盖曾由此处自写缩进扫描产出 `W-LINT-003`，其文案「后者覆盖前者」与
//! > 事实相反（实为提升为数组），且漏掉唯一真丢值的 `&name` 覆盖路径。2026-11-02 起
//! > 改为消费 `sml::parse_with_warnings` 的诊断；`W-LINT-003` 不再产出（码保留，标废弃）。

use std::path::PathBuf;

use sml::{split_kv, strip_line_comment, MAX_VALUE_DEPTH};
use sml_codes::{E_LINT_001, W_LINT_001, W_LINT_002, W_LINT_004, W_LINT_005};

/// Result of one lint run ｜ 检查结果。
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

/// Lint a text; `path` is only used as a message prefix ｜ 对文本做检查（`path` 仅用于消息前缀）。
pub fn check(text: &str, path: &Option<PathBuf>) -> Report {
    let name = path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<stdin>".to_string());
    let mut report = Report {
        messages: Vec::new(),
        has_error: false,
    };

    // ---- 1. 解析 + 解析期诊断 ----
    // 码**原样透传**（E-LEX-* / E-PARSE-* / E-CONTRACT-* / W-PARSE-*），不换成 E-LINT-*：
    // lint 只是呈现者，语言层的码才是事实。
    //
    // 重复键 / 片段覆盖（`W-PARSE-001/002`）也走这里 —— 它们由**解析器**在实际发生改写时
    // 产出（带行号），比 lint 自己按缩进猜准得多：旧的 `W-LINT-003` 把"提升为数组"误描述成
    // "覆盖"，还漏掉 `&name` 覆盖父块（唯一真丢值的路径）（TODO 雷3 四条登记）。
    let parsed = sml::parse_with_warnings(text);
    match &parsed {
        Ok((_, diags)) => {
            for d in diags {
                if let Some(code) = d.code() {
                    report.push(&name, d.line.unwrap_or(0), "warning", code, &d.message);
                }
            }
        }
        Err(e) => report.push(&name, 0, "error", e.code(), e.message()),
    }

    // ---- 2. 逐行文本检查 ----
    let lines: Vec<(usize, String, usize)> = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.to_string(), indent_of(l)))
        .collect();

    // 契约/片段的定义与引用：**单一来源**（库的 `sml::scan_source_refs`，TODO 雷 3）。
    let refs = sml::scan_source_refs(text);

    for (no, raw, _indent) in &lines {
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
    }

    for (nm, no) in &refs.fragment_defs {
        if !refs.fragment_uses.iter().any(|(n, _)| n == nm) {
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
    for (nm, no) in &refs.contract_defs {
        if !refs.contract_uses.iter().any(|(n, _)| n == nm) {
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

    // ---- 3. 空值字段（基于缩进）----
    // 重复键 / 片段覆盖已由第 1 步的解析期诊断（`W-PARSE-001/002`）覆盖，不再自写扫描。
    let mut stack: Vec<(usize, String)> = Vec::new();
    for idx in 0..lines.len() {
        let (no, raw, indent) = &lines[idx];
        let body = strip_line_comment(raw).trim().to_string();
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
        // 空值字段：本行无值，且后面第一个非空行的缩进没有更深
        if v.is_empty() {
            let next_indent = lines[idx + 1..]
                .iter()
                .find(|(_, r, _)| !strip_line_comment(r).trim().is_empty())
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
    if let Ok((v, _)) = &parsed {
        let d = sml::depth_of(v);
        if d > MAX_VALUE_DEPTH / 2 {
            // W-LINT-005：嵌套超过建议阈值（只提醒可读性，不是拒绝 —— 解析上限见 E-LIMIT-001）。
            // 阈值 = MAX_VALUE_DEPTH / 2，库改 MAX_VALUE_DEPTH 时自动跟随。
            report.push(
                &name,
                0,
                "warning",
                W_LINT_005,
                &format!(
                    "嵌套深度 {d} 已超过建议阈值 {}（解析上限为 {}）",
                    MAX_VALUE_DEPTH / 2,
                    MAX_VALUE_DEPTH
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



/// 值的嵌套深度（**单一来源**：直接复用 `sml::depth_of`，见该函数文档）。
#[cfg(test)]
mod tests {
    use super::*;
    use sml::Value;

    fn messages(src: &str) -> String {
        check(src, &None).messages.join("\n")
    }

    /// 每条诊断都带上码 —— 这些码是 lint 的对外契约（`errors/codes.sml` 的 LINT 领域）。
    #[test]
    fn every_diagnostic_carries_a_code() {
        // E-LINT-001：tab 缩进
        assert!(messages("\tk: 1\n").contains("[E-LINT-001]"), "tab 缩进应带码");
        // W-LINT-001 / 002 / 004 + 解析期 W-PARSE-001（重复键）
        let src = "@frag { a: 1 }\n@contract Unused { a: int }\nk: 1\nk: 2\nempty:\n";
        let out = messages(src);
        for want in ["[W-LINT-001]", "[W-LINT-002]", "[W-PARSE-001]", "[W-LINT-004]"] {
            assert!(out.contains(want), "期望含 {want}：\n{out}");
        }
    }

    /// `depth_of` 是库的单一来源：根容器与标量各计 1。
    #[test]
    fn depth_of_is_library_single_source() {
        let v = sml::parse("a { b { c: 1 } }").unwrap();
        // 根(1) -> a 的对象(2) -> b 的对象(3) -> 标量 c:1(4)
        assert_eq!(sml::depth_of(&v), 4, "根(1)+a(2)+b(3)+标量(4)");
        assert_eq!(sml::depth_of(&Value::Int(1)), 1, "标量 = 1");
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

    /// 单一来源：`scan_source_refs` 复用库的 `BUILTIN_DIRECTIVES`（含 `include`），
    /// 库的指令集变动时自动跟随，不会静默误判新指令为片段。
    #[test]
    fn builtin_directives_is_single_source() {
        assert!(sml::BUILTIN_DIRECTIVES.contains(&"include"));
        assert_eq!(sml::BUILTIN_DIRECTIVES.len(), 8);
        let r = sml::scan_source_refs("@contract C { }\n@include \"x.sml\"\n@frag { }\n");
        // 名单里的都不是片段定义；名单外的才算。
        assert!(!r
            .fragment_defs
            .iter()
            .any(|(n, _)| n == "contract" || n == "include"));
        assert!(r.fragment_defs.iter().any(|(n, _)| n == "frag"));
        assert!(r.contract_defs.iter().any(|(n, _)| n == "C"));
    }

    /// `strip_comment` 现在复用 `sml::strip_line_comment`（库的单一来源）：
    /// 行尾 `#` 之后被剥掉，引号内的 `#` 不剥，且不要求 `#` 前有空白。
    #[test]
    fn strip_comment_matches_library() {
        assert_eq!(sml::strip_line_comment("a: 1 # 注释"), "a: 1 ");
        assert_eq!(sml::strip_line_comment("s: \"a#b\""), "s: \"a#b\"");
        // 库不要求 `#` 前有空白。
        assert_eq!(sml::strip_line_comment("a#b"), "a");
        // 片段定义后跟注释，仍应被识别为片段定义（W-LINT-001）。
        let out = messages("@frag { a: 1 } # 注释\n");
        assert!(out.contains("W-LINT-001"), "注释后的片段定义应被识别：\n{out}");
    }

    /// `split_kv` 现在复用 `sml::split_kv`（与解析器同口径：冒号即分隔符），
    /// 因此无空格的 `k:1` 也会被识别为字段——重复键由 W-LINT-003 捕获。
    #[test]
    fn split_kv_matches_parser() {
        assert_eq!(sml::split_kv("k: 1"), Some(("k".to_string(), "1".to_string())));
        assert_eq!(sml::split_kv("k:1"), Some(("k".to_string(), "1".to_string())));
        assert_eq!(
            sml::split_kv("k: 'a:b'"),
            Some(("k".to_string(), "'a:b'".to_string()))
        );
        assert_eq!(sml::split_kv("notakv"), None);
        // 无空格重复键：由解析期的 W-PARSE-001 报出（lint 不再自写扫描）。
        let out = messages("k:1\nk:2\n");
        assert!(out.contains("W-PARSE-001"), "无空格重复键应被捕获：\n{out}");
    }

    /// 片段展开覆盖父块键（`&base` over `x`）是**唯一真丢值**的路径 ——
    /// 旧 lint（自写 `k: v` 扫描）漏检，现由解析期 `W-PARSE-002` 报出。
    #[test]
    fn fragment_overwrite_is_reported() {
        let src = "@base { x: 1 }\nw {\n  x: 9\n  &base\n}\n";
        let out = messages(src);
        assert!(out.contains("W-PARSE-002"), "片段覆盖应被报出：\n{out}");
    }

    /// 嵌套阈值 = `MAX_VALUE_DEPTH / 2`，库改 `MAX_VALUE_DEPTH` 时自动跟随。
    /// 注意 `depth_of` 把根对象也算一层：N 个 `{` 嵌套实际深度为 N+1，
    /// 故边界是 63 个 `{`（深度 64，不触发）vs 64 个 `{`（深度 65，触发）。
    #[test]
    fn depth_warn_threshold_tracks_max() {
        assert_eq!(sml::MAX_VALUE_DEPTH / 2, 64);
        let mut deep = String::new();
        for i in 0..64 {
            deep.push_str(&"  ".repeat(i));
            deep.push_str(&format!("k{i} {{\n"));
        }
        for i in (0..64).rev() {
            deep.push_str(&"  ".repeat(i));
            deep.push_str("}\n");
        }
        assert!(messages(&deep).contains("W-LINT-005"), "深度 65（64 层嵌套）应触发");
        let mut shallow = String::new();
        for i in 0..63 {
            shallow.push_str(&"  ".repeat(i));
            shallow.push_str(&format!("k{i} {{\n"));
        }
        for i in (0..63).rev() {
            shallow.push_str(&"  ".repeat(i));
            shallow.push_str("}\n");
        }
        assert!(
            !messages(&shallow).contains("W-LINT-005"),
            "深度 64（63 层嵌套）不应触发"
        );
    }
}
