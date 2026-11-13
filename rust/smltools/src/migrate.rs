// SPDX-License-Identifier: MulanPSL-2.0
//! `--migrate`：把「已废弃 / 非规范写法」改成规范写法（`docs/swsml/spec/version.md` §4.5）。
//!
//! 三段式（用户 2026-11-04 定）：**scan 只报告**、**apply 写回**、**rules 列规则**。
//! 三条硬约束：
//! 1. **默认 scan** —— 不显式 `apply` 就绝不改文件；
//! 2. **幂等** —— apply 之后再次 scan 必须 0 命中（`tests/migrate.rs` 钉住）；
//! 3. **规则带 id 且可单跑** —— `--rule a,b` 让 CI 一次只放行一部分。
//!
//! 规则登记处就在本文件 [`RULES`]（单一事实来源）：每条破坏性改动**附一条规则**，
//! 不要在 CHANGELOG 里另抄一份 id。

/// 一条迁移规则 ｜ A migration rule.
pub struct Rule {
    /// 稳定 id（CI 用 `--rule` 点名）。
    pub id: &'static str,
    /// 适用版本集（人读）。
    pub applies: &'static str,
    /// 一句话说明「改什么」。
    pub desc: &'static str,
}

/// 全部规则 ｜ The registry (single source of truth).
pub const RULES: &[Rule] = &[
    Rule {
        id: "version-literal",
        applies: "v1–v4（5.0 未启用，跳过）",
        desc: "`@version` 字面量改为规范连写：`v3` / `3` / `3.0` → `30`",
    },
    Rule {
        id: "feature-base-literal",
        applies: "v1–v4（5.0 未启用，跳过）",
        desc: "`@feature base` 字面量改为规范连写：`v3` / `3` / `3.0` → `30`",
    },
    Rule {
        id: "directive-name-arg",
        applies: "v4 起（位置参数已废弃）",
        desc: "扩展指令的位置参数改为显式 `name:`：`@form F { }` → `@form name: F { }`",
    },
];

/// 一次迁移的选项 ｜ Options for one pass.
pub struct Opts<'a> {
    /// `--rule` 过滤（None = 全跑）。
    pub only: Option<&'a [String]>,
    /// `--migrate-directive`：**哪些 `@名` 是扩展指令**。
    ///
    /// `directive-name-arg` 必须靠它：纯文本扫描分不清「扩展指令的位置参数」与
    /// 「片段定义 `@name Type Name { }`」，猜错就会把片段定义改坏。空表 ⇒ 该规则不动作。
    pub directives: &'a [String],
}

impl<'a> Opts<'a> {
    /// 规则是否被选中。
    fn enabled(&self, id: &str) -> bool {
        match self.only {
            Some(sel) => sel.iter().any(|s| s == id),
            None => true,
        }
    }
}

/// 命中 / 待改的一处 ｜ One finding.
pub struct Hit {
    /// 1 起行号。
    pub line: usize,
    /// 命中的规则 id。
    pub rule: &'static str,
    /// 原文行。
    pub before: String,
    /// 改写后的行。
    pub after: String,
}

/// scan 的完整结果：可自动改的命中 + 只能人工处理的提示 ｜ findings + notes.
pub struct Report {
    pub hits: Vec<Hit>,
    /// 不能自动迁移、但值得说一句的行（如 `@version 5.0`：写法规范但该代未启用）。
    pub notes: Vec<String>,
}

/// 版本字面量 → 规范连写（`1..4` ⇒ `10..40`）。不认识的、以及 5.0 族一律 `None`。
///
/// 与五端解析器（`version_lit_major`）同口径，但**故意**不处理 5.0 族：
/// 迁到 `50` 仍是 `E-FEATURE-004`，改了等于没改 —— 那种情况交给 `notes` 提示人工。
fn canonical_literal(lit: &str) -> Option<String> {
    let s = lit.trim_matches('"');
    if s == "camelia" {
        return None;
    }
    let body = s.strip_prefix(['v', 'V']).unwrap_or(s);
    let bytes = body.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_digit() || bytes[0] == b'0' {
        return None;
    }
    let major = bytes[0] - b'0';
    let ok = body.len() == 1
        || (body.len() == 3 && &body[1..] == ".0")
        || (body.len() == 2 && bytes[1] == b'0');
    if !ok || major > 4 {
        return None;
    }
    Some(format!("{major}0"))
}

/// 是不是 5.0 族（写法合法但该代未启用）。
fn is_future_family(lit: &str) -> bool {
    let s = lit.trim_matches('"');
    if s == "camelia" {
        return true;
    }
    let body = s.strip_prefix(['v', 'V']).unwrap_or(s);
    let bytes = body.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_digit() {
        return false;
    }
    let ok = body.len() == 1
        || (body.len() == 3 && &body[1..] == ".0")
        || (body.len() == 2 && bytes[1] == b'0');
    ok && (bytes[0] - b'0') >= 5
}

/// 把一行拆成「指令前缀 / 字面量 / 其余」 ｜ split `@version <lit> <rest>`.
///
/// 只认**行首**（允许缩进）的指令，注释里的 `@version` 不算。
fn split_directive<'a>(line: &'a str, rule: &str) -> Option<(&'a str, &'a str, &'a str)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let (prefix, rest) = match rule {
        "version-literal" => {
            let r = trimmed.strip_prefix("@version")?;
            if !r.starts_with([' ', '\t']) {
                return None;
            }
            (&line[..indent + "@version".len()], r)
        }
        "feature-base-literal" => {
            let r = trimmed.strip_prefix("@feature")?;
            let after = r.trim_start_matches([' ', '\t']);
            let rest = after.strip_prefix("base")?;
            if !rest.starts_with([' ', '\t']) {
                return None;
            }
            let consumed = indent + "@feature".len() + (r.len() - after.len()) + "base".len();
            (&line[..consumed], rest)
        }
        _ => return None,
    };
    let rest_trimmed = rest.trim_start_matches([' ', '\t']);
    let gap = rest.len() - rest_trimmed.len();
    if rest_trimmed.is_empty() {
        return None;
    }
    let lit_end = rest_trimmed
        .find([' ', '\t'])
        .unwrap_or(rest_trimmed.len());
    let lit = &rest_trimmed[..lit_end];
    let prefix_end = prefix.len() + gap;
    Some((&line[..prefix_end], lit, &rest_trimmed[lit_end..]))
}

/// `@dir ARG {` → `@dir name: ARG {`（只对白名单里的 `dir`）。
///
/// 判据与 `parser.rs::take_directive_arg` 的**位置参数分支**一一对应：
/// 裸词参数**紧跟** `{` 才算位置参数；`name:` 已显式、引号参数、`@dir` 后不是 `{`
/// 一律不动 —— 宁可少改，也不能把片段定义或普通字段改坏。
fn rewrite_directive_arg(line: &str, directives: &[String]) -> Option<String> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let after_at = trimmed.strip_prefix('@')?;
    let dname_end = after_at
        .find(|c: char| c.is_whitespace() || c == '{')
        .unwrap_or(after_at.len());
    let dname = &after_at[..dname_end];
    if dname.is_empty() || !directives.iter().any(|d| d == dname) {
        return None;
    }
    let after_name = &after_at[dname_end..];
    if !after_name.starts_with([' ', '\t']) {
        return None;
    }
    let arg_area = after_name.trim_start_matches([' ', '\t']);
    if arg_area.is_empty() {
        return None;
    }
    let arg_end = arg_area
        .find(|c: char| c.is_whitespace())
        .unwrap_or(arg_area.len());
    let arg = &arg_area[..arg_end];
    // 已显式（`name:`）、引号参数、或参数本身带冒号 ⇒ 不是位置参数形式
    if arg.starts_with('"') || arg.contains(':') || arg == "name" {
        return None;
    }
    // 位置参数的判据：参数之后**紧跟** `{`
    let tail = arg_area[arg_end..].trim_start_matches([' ', '\t']);
    if !tail.starts_with('{') {
        return None;
    }
    let head = &line[..indent + 1 + dname_end]; // 含 `@` 与指令名
    let gap = after_name.len() - arg_area.len();
    Some(format!("{head}{}name: {arg}{}", &after_name[..gap], &arg_area[arg_end..]))
}

/// 扫一遍文本，收集可迁移处与提示 ｜ Scan the text.
pub fn scan_text(text: &str, opts: &Opts<'_>) -> Report {
    let mut report = Report { hits: Vec::new(), notes: Vec::new() };
    for (idx, line) in text.lines().enumerate() {
        for rule in RULES {
            if !opts.enabled(rule.id) {
                continue;
            }
            if rule.id == "directive-name-arg" {
                if opts.directives.is_empty() {
                    continue; // 没有指令表 ⇒ 不猜（见 `Opts::directives` 的说明）
                }
                if let Some(after) = rewrite_directive_arg(line, opts.directives) {
                    report.hits.push(Hit {
                        line: idx + 1,
                        rule: rule.id,
                        before: line.to_string(),
                        after,
                    });
                }
                continue;
            }
            let Some((prefix, lit, tail)) = split_directive(line, rule.id) else {
                continue;
            };
            if let Some(canon) = canonical_literal(lit) {
                if canon == lit {
                    continue; // 已是规范写法 ⇒ 零命中（幂等的前提）
                }
                report.hits.push(Hit {
                    line: idx + 1,
                    rule: rule.id,
                    before: line.to_string(),
                    after: format!("{prefix}{canon}{tail}"),
                });
            } else if is_future_family(lit) {
                report.notes.push(format!(
                    "{}:{}: `{}` 属 5.0 族（写法合法但该代未启用），不自动迁移",
                    idx + 1,
                    rule.id,
                    lit
                ));
            }
        }
    }
    report
}

/// 按规则改写文本，返回（新文本, 改动行数）｜ Rewrite; returns (text, changed lines).
pub fn apply_text(text: &str, opts: &Opts<'_>) -> (String, usize) {
    let report = scan_text(text, opts);
    if report.hits.is_empty() {
        return (text.to_string(), 0);
    }
    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();
    for hit in &report.hits {
        if let Some(slot) = lines.get_mut(hit.line - 1) {
            *slot = hit.after.clone();
        }
    }
    // `lines()` 吃掉了结尾换行：原文有就以换行收尾，保持文件形状。
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    (out, report.hits.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Opts<'static> {
        Opts { only: None, directives: &[] }
    }

    #[test]
    fn canonical_forms() {
        for (lit, want) in [
            ("v1", Some("10")),
            ("1", Some("10")),
            ("1.0", Some("10")),
            ("10", Some("10")),
            ("v4", Some("40")),
            ("4.0", Some("40")),
        ] {
            assert_eq!(canonical_literal(lit).as_deref(), want, "{lit}");
        }
        for lit in ["50", "5.0", "v5", "camelia", "v9", "42", "abc"] {
            assert_eq!(canonical_literal(lit), None, "{lit}");
        }
    }

    #[test]
    fn scan_is_idempotent_and_ignores_comments() {
        let src = "# @version v1 注释里不算\n@version v3\nx: 1\n";
        let r = scan_text(src, &plain());
        assert_eq!(r.hits.len(), 1);
        assert_eq!(r.hits[0].line, 2);
        assert_eq!(r.hits[0].after, "@version 30");
        let (out, n) = apply_text(src, &plain());
        assert_eq!(n, 1);
        assert!(out.contains("@version 30"));
        assert!(out.contains("# @version v1"), "注释行必须原样保留");
        assert!(scan_text(&out, &plain()).hits.is_empty());
        assert_eq!(apply_text(&out, &plain()).1, 0);
    }

    #[test]
    fn keeps_trailing_codename_and_dev() {
        let src = "@version 50 \"camilia\" dev\n";
        let r = scan_text(src, &plain());
        assert!(r.hits.is_empty(), "5.0 族不自动迁移");
        assert_eq!(r.notes.len(), 1);
        let (out, n) = apply_text("@version v1 \"camelia\"\n", &plain());
        assert_eq!(n, 1);
        assert_eq!(out, "@version 10 \"camelia\"\n");
    }

    #[test]
    fn rule_filter_and_feature_base() {
        let src = "@version v3\n@feature base v2\n";
        let only = vec!["feature-base-literal".to_string()];
        let opts = Opts { only: Some(&only), directives: &[] };
        let (out, n) = apply_text(src, &opts);
        assert_eq!(n, 1);
        assert_eq!(out, "@version v3\n@feature base 20\n");
        assert_eq!(scan_text(&out, &plain()).hits.len(), 1, "剩下的 version 仍未迁移");
    }

    #[test]
    fn directive_arg_needs_a_whitelist() {
        let src = "@form F { a: 1 }\n";
        // 没有指令表：一个字都不动（分不清扩展指令与片段定义）
        assert!(scan_text(src, &plain()).hits.is_empty());
        let dirs = vec!["form".to_string()];
        let opts = Opts { only: None, directives: &dirs };
        let r = scan_text(src, &opts);
        assert_eq!(r.hits.len(), 1);
        assert_eq!(r.hits[0].after, "@form name: F { a: 1 }");
        let (out, n) = apply_text(src, &opts);
        assert_eq!(n, 1);
        assert_eq!(out, "@form name: F { a: 1 }\n");
        // 幂等
        assert!(scan_text(&out, &opts).hits.is_empty());
    }

    #[test]
    fn directive_arg_leaves_other_shapes_alone() {
        let dirs = vec!["form".to_string()];
        let opts = Opts { only: None, directives: &dirs };
        for src in [
            "@form name: F { a: 1 }\n",   // 已显式
            "@form \"F\" { a: 1 }\n",     // 引号参数（位置参数只收裸词）
            "@form F\n",                  // 后面不是 `{`
            "@other F { a: 1 }\n",        // 不在白名单
            "@form { a: 1 }\n",           // 无参数
        ] {
            assert!(
                scan_text(src, &opts).hits.is_empty(),
                "不该改：{src}"
            );
        }
    }
}
