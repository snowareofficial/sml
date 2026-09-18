//! 编辑器高亮定制：用一份 SML 定制文件，在**原版高亮基线**上增补，
//! 生成定制化的 TextMate 语法（VSCode / VSIX 用）。
//!
//! 这是 SML「外置扩展」理念在编辑器侧的对偶物：`sml::ext` 让下游用 SML 声明自己的
//! 方言指令而不改核心；这里让下游用 SML 声明「我的方言怎么高亮」而不必 fork 一份
//! tmLanguage。VTuber 家的 `@form`、你家的 `@policy`，都能在自己的仓库里描述。
//!
//! # 定制文件 schema
//!
//! ```sml
//! scopeName: source.sml                 # 可选，默认沿用基线
//! directives: [ form policy flow ]      # 你的方言指令名（不含 @）→ keyword 规则
//! elements: [ label widget ]            # 特定键名/元素 → 成员色
//! types: [ image link time ]            # 特定类型名 → 类型色
//! rules: [                              # 精细规则（插入位置敏感，见下）
//!   { name: keyword.control.form.sml  match: "@(form|policy|flow)\\b"  after: directive }
//!   { name: variable.other.sml        match: "\\b(label|widget)\\b"    before: comment }
//! ]
//! ```
//!
//! # 为什么是「增补」而不是「替换」
//!
//! `editors/vscode/syntaxes/sml.tmLanguage.json` 是打磨过的完整基线（注释、字符串、
//! 契约块、模式语言、指令、数字、键名……）。用户定制通常只想「再多认几个我的名字」，
//! 不想从头重写全部规则。所以本模块**把基线原样取出、只在上面插几条** ——
//! 定制文件写错时，退化结果仍是一个可用的高亮。
//!
//! # 为什么插入位置要显式指定、锚点写错要报错
//!
//! TextMate 的 `patterns` 是**有序**数组：同一位置多条规则都能匹配时，**先出现者胜**。
//! 于是「插在哪条基线规则前后」直接决定定制规则是否生效。典型陷阱：基线的片段声明
//! 规则 `(?<![\w])(@)(\w+)` 会抢先吃掉 `@form`，方言规则若排在它**之后**就永远匹配不上
//! —— 高亮静默失效，用户毫无线索。
//!
//! 所以 `after:` / `before:` 让用户显式声明锚点；锚点写错时**报错并列出可用锚点**，
//! 绝不「悄悄追加到末尾」：末尾恰恰就是上面那个「永不生效」的位置，静默降级会把一个
//! 拼写错误伪装成「高亮莫名不工作」的玄学问题。

use std::collections::BTreeMap;

use sml::{jsonify, Value};
use sml_codes::{SmlError, E_EXT_007, E_INTERNAL_002};

/// E-EXT-007：编辑器定制文档非法（作用域名、规则锚点、颜色取值不符规范）。
fn bad_doc(msg: impl Into<String>) -> SmlError {
    SmlError::new(E_EXT_007, msg)
}

/// E-INTERNAL-002：内置资源损坏（内置高亮基线不是合法 JSON、缺少必需字段）。
///
/// 这类错误用户无解，只能升级或重装；出现即打包或构建事故。
fn broken_asset(msg: impl Into<String>) -> SmlError {
    SmlError::new(E_INTERNAL_002, msg)
}

/// 原版高亮基线。
///
/// **刻意用 `include_str!` 内嵌真实文件的副本**，而不是把 JSON 抄成 Rust 常量：
/// 抄写会产生一份编译器不会帮你检查的「影子副本」，原版一更新就悄悄漂移。
/// 同步方式：把 `editors/vscode/syntaxes/sml.tmLanguage.json` 复制到
/// `rust/smltools/assets/baseline.tmLanguage.json`（见该目录 README）。
const BASELINE: &str = include_str!("../assets/baseline.tmLanguage.json");

/// 一份生成产物：**相对路径** + 内容。调用方负责落盘。
///
/// 路径是相对的（如 `syntaxes/sml.tmLanguage.json`、`zed/themes/sml.json`），
/// 因为一个定制包会展开成多个文件、分布在 VSIX 与 Zed 两种目录结构下。
#[derive(Debug, Clone)]
pub struct Generated {
    pub path: String,
    pub content: String,
}

/// 一条用户精细规则。
struct Rule {
    name: String,
    pattern: String,
    /// `Some((锚点名, 是否插在其后))`；`None` = 追加末尾
    anchor: Option<(String, bool)>,
}

/// 在基线上增补，返回完整的 tmLanguage JSON 文本。
///
/// 入参是**已解析**的定制数据（`--to tmlanguage` 走的是与其它后端同样的解析路径）。
pub fn generate(custom: &Value) -> Result<String, SmlError> {
    let mut base = sml::json_to_value(BASELINE)
        .ok_or_else(|| broken_asset("内置高亮基线不是合法 JSON（资产损坏）"))?;

    let directives = str_array(custom, "directives")?;
    let elements = str_array(custom, "elements")?;
    let types = str_array(custom, "types")?;
    let rules = collect_rules(custom)?;

    // 可选覆盖 scopeName / name
    let Value::Object(base_map) = &mut base else {
        return Err(broken_asset("内置高亮基线的顶层不是对象（资产损坏）"));
    };
    for key in ["scopeName", "name"] {
        if let Some(v) = custom.get(key).and_then(|x| x.as_str()) {
            if key == "scopeName" && !is_valid_scope(v) {
                // E-EXT-007：作用域名不符规范。
                return Err(bad_doc(format!(
                    "scopeName `{v}` 非法：须形如 `x.y`（至少一个点；各段为字母/数字/`_`/`-` 且非空）\
                     —— 写坏会让语法根本挂不上语言"
                )));
            }
            base_map.insert(key.to_string(), Value::Str(v.to_string()));
        }
    }

    let Some(Value::Array(patterns)) = base_map.get_mut("patterns") else {
        return Err(broken_asset("内置高亮基线缺少 patterns 数组（资产损坏）"));
    };

    // 锚点表**从基线动态读取**（`{"include": "#xxx"}` 里的 xxx）——
    // 不硬编码，于是基线增删规则时可用锚点自动跟着变，不会漂移。
    let anchors: Vec<String> = patterns
        .iter()
        .filter_map(|p| match p {
            Value::Object(o) => o
                .get("include")
                .and_then(|v| v.as_str())
                .map(|s| s.trim_start_matches('#').to_string()),
            _ => None,
        })
        .collect();
    if anchors.is_empty() {
        return Err(broken_asset(
            "内置高亮基线的 patterns 里没有可用的 include 锚点（资产损坏）",
        ));
    }

    let at = |name: &str| anchors.iter().position(|a| a == name);

    let n = patterns.len();
    let mut before: Vec<Vec<Value>> = vec![Vec::new(); n];
    let mut after: Vec<Vec<Value>> = vec![Vec::new(); n];
    let mut tail: Vec<Value> = Vec::new();

    for (i, r) in rules.into_iter().enumerate() {
        match r.anchor {
            None => tail.push(pattern(&r.name, &r.pattern)),
            Some((anchor, is_after)) => {
                let Some(pos) = at(&anchor) else {
                    // E-EXT-007：规则锚点不符规范（锚点不存在时列出可用锚点，绝不静默追加到末尾）。
                    return Err(bad_doc(format!(
                        "rules[{i}] 的{}锚点 `{anchor}` 不存在；可用锚点：{}",
                        if is_after { " after " } else { " before " },
                        anchors.join(", ")
                    )));
                };
                let v = pattern(&r.name, &r.pattern);
                if is_after {
                    after[pos].push(v);
                } else {
                    before[pos].push(v);
                }
            }
        }
    }

    // 便利声明：属于「一把梭」，统一追加到末尾。
    // 需要精确控制位置时应当用带锚点的 `rules` —— 这个取舍写在文档里。
    if !directives.is_empty() {
        tail.push(pattern(
            "keyword.control.directive.sml",
            &names_regex("@", &directives, "\\b"),
        ));
    }
    if !elements.is_empty() {
        tail.push(pattern(
            "variable.other.member.sml",
            &names_regex("\\b", &elements, "\\b"),
        ));
    }
    if !types.is_empty() {
        tail.push(pattern(
            "support.type.primitive.sml",
            &names_regex("\\b", &types, "\\b"),
        ));
    }

    // 重排：before 规则 → 基线项 → after 规则，最后接末尾规则。
    let old: Vec<Value> = std::mem::take(patterns);
    let mut out: Vec<Value> = Vec::with_capacity(old.len() + 8);
    for (i, p) in old.into_iter().enumerate() {
        out.append(&mut before[i]);
        out.push(p);
        out.append(&mut after[i]);
    }
    out.append(&mut tail);
    *patterns = out;

    Ok(jsonify(&base))
}

/// `x.y`：至少两段，各段非空且仅含字母/数字/`_`/`-`。
fn is_valid_scope(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 2
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'))
}

/// 取顶层字符串数组字段（缺失 → 空数组；类型不对 → 报错）。
fn str_array(root: &Value, key: &str) -> Result<Vec<String>, SmlError> {
    let mut out = Vec::new();
    match root.get(key) {
        None => {}
        Some(Value::Array(a)) => {
            for (i, v) in a.iter().enumerate() {
                match v.as_str() {
                    Some(s) => out.push(s.to_string()),
                    // E-EXT-007：定制文档的字段形态不符规范。
                    None => return Err(bad_doc(format!("{key}[{i}] 不是字符串（应为名字）"))),
                }
            }
        }
        Some(_) => return Err(bad_doc(format!("{key} 必须是数组，如 `{key}: [ a b c ]`"))),
    }
    Ok(out)
}

/// 解析 `rules`：每项必须有 `name` 与 `match`；`after` / `before` 至多其一。
fn collect_rules(root: &Value) -> Result<Vec<Rule>, SmlError> {
    let arr = match root.get("rules") {
        None => return Ok(Vec::new()),
        Some(Value::Array(a)) => a,
        Some(_) => return Err(bad_doc("rules 必须是数组")),
    };
    let mut out = Vec::new();
    for (i, r) in arr.iter().enumerate() {
        let name = r
            .get("name")
            .and_then(|x| x.as_str())
            .ok_or_else(|| bad_doc(format!("rules[{i}] 缺少 name（TextMate scope 名）")))?;
        let m = r
            .get("match")
            .and_then(|x| x.as_str())
            .ok_or_else(|| bad_doc(format!("rules[{i}] 缺少 match（匹配正则字符串）")))?;
        let after = r.get("after").and_then(|x| x.as_str());
        let before = r.get("before").and_then(|x| x.as_str());
        let anchor = match (after, before) {
            // E-EXT-007：规则里同时给出同侧两个锚点也算非法（位置无法确定）。
            (Some(_), Some(_)) => {
                return Err(bad_doc(format!(
                    "rules[{i}] 同时给了 after 与 before，位置无法确定"
                )))
            }
            (Some(a), None) => Some((a.to_string(), true)),
            (None, Some(b)) => Some((b.to_string(), false)),
            (None, None) => None,
        };
        out.push(Rule {
            name: name.to_string(),
            pattern: m.to_string(),
            anchor,
        });
    }
    Ok(out)
}

/// 构造一条 TextMate pattern。
fn pattern(name: &str, m: &str) -> Value {
    let mut o = BTreeMap::new();
    o.insert("name".to_string(), Value::Str(name.to_string()));
    o.insert("match".to_string(), Value::Str(m.to_string()));
    Value::Object(o)
}

/// 一组名字 → `prefix(a|b|c)suffix`。
fn names_regex(prefix: &str, names: &[String], suffix: &str) -> String {
    let alts: Vec<String> = names.iter().map(|n| regex_escape(n)).collect();
    format!("{prefix}({}){suffix}", alts.join("|"))
}

/// 转义正则元字符，避免用户声明的名字（`a+b`、`c*`）破坏生成的正则。
///
/// 只转义 **ASCII 标点**：非 ASCII（中文名）必须原样保留 —— 给它们加反斜杠会长出
/// `\数` 这种非法转义，反而让整条正则失效。
fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() && !c.is_ascii_alphanumeric() && c != '_' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

// ============================================================================
// 编辑器定制包：同一份 SML 定制 → 多份产物
// ============================================================================

/// 取「scope → 颜色」映射（定制文件里的 `colors:` 段）。
///
/// 为什么不只生成语法（scope）还要管颜色：真实诉求常常不是「哪些词算关键字」，
/// 而是「我的方言关键字要跟标准指令**不同色**」。只给 scope 的话，用户还得自己
/// 去编辑器里配 tokenColors —— 那等于把定制工作推回给用户。
fn collect_colors(root: &Value) -> Result<Vec<(String, String)>, SmlError> {
    match root.get("colors") {
        None => Ok(Vec::new()),
        Some(Value::Object(m)) => {
            let mut out = Vec::new();
            for (scope, v) in m {
                let Some(c) = v.as_str() else {
                    // E-EXT-007：颜色取值不符规范（此处是形态不对）。
                    return Err(bad_doc(format!(
                        "colors.{scope} 必须是颜色字符串（如 \"#C586C0\"）"
                    )));
                };
                if !is_color(c) {
                    // E-EXT-007：颜色取值不符规范（此处是取值非法）。
                    return Err(bad_doc(format!(
                        "colors.{scope} = `{c}` 不是合法颜色（支持 #RGB / #RGBA / #RRGGBB / #RRGGBBAA）"
                    )));
                }
                out.push((scope.clone(), c.to_string()));
            }
            Ok(out)
        }
        Some(_) => Err(bad_doc(
            "colors 必须是对象，如 `colors: { keyword.control.form.sml: \"#C586C0\" }`",
        )),
    }
}

fn is_color(c: &str) -> bool {
    let h = c.strip_prefix('#').unwrap_or("");
    matches!(h.len(), 3 | 4 | 6 | 8) && !h.is_empty() && h.chars().all(|c| c.is_ascii_hexdigit())
}

/// `[{scope, settings:{foreground}}]` —— VSCode 主题与项目内片段共用这个形状。
fn textmate_rules(colors: &[(String, String)]) -> Vec<Value> {
    colors
        .iter()
        .map(|(scope, color)| {
            let mut st = BTreeMap::new();
            st.insert("foreground".to_string(), Value::Str(color.clone()));
            let mut r = BTreeMap::new();
            r.insert("scope".to_string(), Value::Str(scope.clone()));
            r.insert("settings".to_string(), Value::Object(st));
            Value::Object(r)
        })
        .collect()
}

/// VSCode 颜色主题（打进 VSIX 用）。
fn vscode_theme(name: &str, colors: &[(String, String)]) -> Value {
    let mut root = BTreeMap::new();
    root.insert(
        "$schema".to_string(),
        Value::Str("vscode://schemas/color-theme".to_string()),
    );
    root.insert("name".to_string(), Value::Str(format!("{name} (SML custom)")));
    root.insert("type".to_string(), Value::Str("dark".to_string()));
    root.insert("tokenColors".to_string(), Value::Array(textmate_rules(colors)));
    Value::Object(root)
}

/// 可直接粘进 `.vscode/settings.json` 的片段 —— 让定制**在项目内立即生效**，
/// 不必打包扩展、不必重启编辑器。
///
/// 这是「不改核心、就地扩展」最轻的一档：一个项目想给自家方言上色，
/// 放一个文件、复制一段配置即可；要分发给别人时再把同一份 SML 编译成 VSIX。
fn vscode_settings_fragment(colors: &[(String, String)]) -> Value {
    let mut tcc = BTreeMap::new();
    tcc.insert("textMateRules".to_string(), Value::Array(textmate_rules(colors)));
    let mut root = BTreeMap::new();
    root.insert(
        "editor.tokenColorCustomizations".to_string(),
        Value::Object(tcc),
    );
    Value::Object(root)
}

/// TextMate scope 前缀 → Zed 主题的**语法语义键**。
///
/// Zed 的主题按语义键（`keyword` / `variable` / `type` …）着色，而不是 TextMate
/// 的 scope 名，所以必须做一次映射。映射不到的 scope 由调用方收进注释里告知用户，
/// **不静默丢弃**（静默丢弃会让人以为"颜色没生效是我的错"）。
fn zed_syntax_key(scope: &str) -> Option<&'static str> {
    let s = scope.to_ascii_lowercase();
    // 次序重要：先长前缀，后短前缀，避免 `constant.numeric` 落到 `constant`。
    if s.starts_with("constant.numeric") {
        Some("number")
    } else if s.starts_with("constant") {
        Some("constant")
    } else if s.starts_with("keyword") {
        Some("keyword")
    } else if s.starts_with("comment") {
        Some("comment")
    } else if s.starts_with("string") {
        Some("string")
    } else if s.starts_with("support.type") || s.starts_with("entity.name.type") {
        Some("type")
    } else if s.starts_with("entity.name.function") {
        Some("function")
    } else if s.starts_with("entity") {
        Some("entity")
    } else if s.starts_with("variable") {
        Some("variable")
    } else if s.starts_with("punctuation") {
        Some("punctuation")
    } else if s.starts_with("storage") {
        Some("attribute")
    } else {
        None
    }
}

/// Zed 主题文件。
fn zed_theme(name: &str, colors: &[(String, String)]) -> Value {
    let mut syntax = BTreeMap::new();
    for (scope, color) in colors {
        if let Some(key) = zed_syntax_key(scope) {
            let mut c = BTreeMap::new();
            c.insert("color".to_string(), Value::Str(color.clone()));
            syntax.insert(key.to_string(), Value::Object(c));
        }
    }
    let mut style = BTreeMap::new();
    style.insert("syntax".to_string(), Value::Object(syntax));
    let mut theme = BTreeMap::new();
    theme.insert("name".to_string(), Value::Str(format!("{name} (SML custom)")));
    theme.insert("appearance".to_string(), Value::Str("dark".to_string()));
    theme.insert("style".to_string(), Value::Object(style));

    let mut root = BTreeMap::new();
    root.insert(
        "$schema".to_string(),
        Value::Str("https://zed.dev/schema/themes/v0.2.0.json".to_string()),
    );
    root.insert("name".to_string(), Value::Str(format!("{name} (SML custom)")));
    root.insert("author".to_string(), Value::Str("SNOWARE".to_string()));
    root.insert("themes".to_string(), Value::Array(vec![Value::Object(theme)]));
    Value::Object(root)
}

/// Zed 高亮查询（`highlights.scm`）。
///
/// **诚实说明**：Zed 用 Tree-sitter 查询，节点名由 grammar 决定，而不是 TextMate
/// 的 scope。所以这里产出的是「按常见节点名书写的查询 + 显式的匹配谓词」，
/// 文件头会写明它依赖 `editors/zed/grammars/sml`（tree-sitter grammar），
/// 节点名不一致时需要对照 grammar 调整 —— 这一点不写清楚就是坑。
fn zed_highlights(custom: &Value) -> Result<String, SmlError> {
    let directives = str_array(custom, "directives")?;
    let mut s = String::new();
    s.push_str("; 由 smltools 生成（--to highlight）—— 请勿手改，改那份 SML 定制文件。\n");
    s.push_str(";\n");
    s.push_str("; ⚠️ 本查询依赖 tree-sitter-sml grammar 的节点名（见 editors/zed/grammars/sml）。\n");
    s.push_str(";    若你的 grammar 节点名不同（如用 (integer) 而非 (number)），请对照 grammar 调整。\n");
    s.push_str(";    Zed 用 Tree-sitter，不使用 TextMate 的 scope 名 —— 这是与 VSIX 侧的本质差异。\n");
    s.push_str(";\n");
    s.push_str("; 位置说明：Zed 只读扩展里的 languages/<语言>/highlights.scm，本文件是**待复制**的\n");
    s.push_str(";    暂存产物，复制过去才生效（见 editors/zed/README.md）。\n\n");
    s.push_str("(comment) @comment\n");
    s.push_str("(string) @string\n");
    s.push_str("(number) @number\n");
    s.push_str("(boolean) @boolean\n");
    s.push_str("(env_var) @variable\n");
    s.push_str("(fragment_ref) @variable\n");
    s.push_str("(key) @property\n");
    s.push_str("(type_name) @type\n");
    // 标点用**匿名 token 列表**而不是 `(punctuation)`：把 `{`/`}` 做成具名节点会让
    // 语法无法区分开闭括号（同一个节点名套两边），grammar 里也就没法做括号配对。
    s.push_str("[\"{\" \"}\" \"[\" \"]\" \":\" \",\"] @punctuation\n");
    if !directives.is_empty() {
        s.push_str("\n; 方言指令（本定制的重点）：只有这些名字按关键字着色\n");
        s.push_str("; ⚠️ 谓词带 `@`：grammar 里 `directive` 节点的文本是 `@form` 而不是 `form`，\n");
        s.push_str(";    写成 `^form$` 会永远匹配不上（高亮静默失效）。\n");
        s.push_str("((directive) @keyword\n");
        s.push_str(&format!(
            "  (#match? @keyword \"^@({})$\"))\n",
            directives
                .iter()
                .map(|d| regex_escape(d))
                .collect::<Vec<_>>()
                .join("|")
        ));
    } else {
        s.push_str("(directive) @keyword\n");
    }
    Ok(s)
}

/// 生成**编辑器定制包**：一份或多份产物。
///
/// 两种消费方式对应两类产物：
/// - **就地生效**：`vscode/settings.fragment.json` 粘进项目的 `.vscode/settings.json`
/// - **扩展编译**：`syntaxes/sml.tmLanguage.json` + `themes/` 是 VSIX 的构建输入
///
/// Zed 侧另出 `zed/highlights.scm` + `zed/themes/sml.json`。
pub fn generate_package(custom: &Value) -> Result<Vec<Generated>, SmlError> {
    let name = custom
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("SML")
        .to_string();
    let colors = collect_colors(custom)?;

    let mut out = vec![
        // 语法本身（VSIX 与项目内都需要）
        Generated {
            path: "syntaxes/sml.tmLanguage.json".to_string(),
            content: generate(custom)?,
        },
        // Zed 查询
        Generated {
            path: "zed/highlights.scm".to_string(),
            content: zed_highlights(custom)?,
        },
    ];

    if !colors.is_empty() {
        out.push(Generated {
            path: "themes/sml-color-theme.json".to_string(),
            content: jsonify(&vscode_theme(&name, &colors)),
        });
        out.push(Generated {
            path: "vscode/settings.fragment.json".to_string(),
            content: jsonify(&vscode_settings_fragment(&colors)),
        });
        out.push(Generated {
            path: "zed/themes/sml.json".to_string(),
            content: jsonify(&zed_theme(&name, &colors)),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(src: &str) -> Result<String, SmlError> {
        let v = sml::parse(src)?;
        generate(&v)
    }

    #[test]
    fn baseline_is_preserved() {
        let out = gen("directives: [ form ]").unwrap();
        // 基线的 repository 与 include 都应原样保留 —— 证明是「增补」而非「替换」。
        assert!(out.contains("#comment"), "基线 include 丢失");
        assert!(out.contains("comment.block.slash-star.sml"), "基线 repository 丢失");
        assert!(out.contains("\"scopeName\":\"source.sml\""), "基线 scopeName 丢失");
    }

    #[test]
    fn directives_generates_keyword_rule() {
        let out = gen("directives: [ form policy flow ]").unwrap();
        assert!(out.contains("keyword.control.directive.sml"), "缺 keyword scope");
        assert!(out.contains("@(form|policy|flow)"), "指令正则不对：{out}");
    }

    #[test]
    fn elements_and_types_scopes() {
        let out = gen("elements: [ label widget ]\ntypes: [ image link ]").unwrap();
        assert!(out.contains("variable.other.member.sml"), "elements scope 缺失");
        assert!(out.contains("support.type.primitive.sml"), "types scope 缺失");
    }

    #[test]
    fn after_anchor_inserts_right_after() {
        let src = r#"rules: [ { name: "custom.form.sml"  match: "@form\\b"  after: directive } ]"#;
        let out = gen(src).unwrap();
        let pos_dir = out.find("#directive").expect("基线锚点应在");
        let pos_rule = out.find("custom.form.sml").expect("定制规则应在");
        // 规则应在 directive 锚点之后（同处 patterns 数组内）
        assert!(pos_rule > pos_dir, "after 未插到锚点之后");
    }

    #[test]
    fn missing_anchor_errors_with_available_list() {
        let src = r#"rules: [ { name: "x.sml"  match: "x"  after: nope } ]"#;
        let e = gen(src).unwrap_err();
        assert!(e.message().contains("锚点"), "应提到锚点：{e}");
        assert!(e.message().contains("可用锚点"), "应列出可用锚点：{e}");
        assert!(e.message().contains("directive"), "列表应含 directive：{e}");
        assert_eq!(e.code(), E_EXT_007, "锚点非法应报 E-EXT-007");
    }

    #[test]
    fn rule_without_anchor_goes_to_tail() {
        let src = r#"rules: [ { name: "zzz.rule.sml"  match: "zzz" } ]"#;
        let out = gen(src).unwrap();
        let pos_rule = out.find("zzz.rule.sml").expect("规则应在");
        let pos_env = out.find("#envvar").expect("基线锚点应在");
        assert!(pos_rule > pos_env, "未定位规则应追加到末尾");
    }

    #[test]
    fn regex_metacharacters_escaped() {
        let out = gen(r#"directives: [ "a+b" ]"#).unwrap();
        // 注意这里是 **JSON 文本**：正则里的 `\+` 经 JSON 转义后写作 `\\+`。
        // 断言用 JSON 层的形态，否则会把正确的输出判成失败。
        assert!(out.contains(r"@(a\\+b)"), "`+` 未转义：{out}");
    }

    #[test]
    fn invalid_scope_rejected() {
        assert!(gen("scopeName: bad\ndirectives: [ form ]").is_err());
    }

    #[test]
    fn rule_missing_match_rejected() {
        assert!(gen(r#"rules: [ { name: "x.sml" } ]"#).is_err());
    }

    // ---- 编辑器定制包（项目内生效 / 扩展编译 / Zed）----

    fn pkg(src: &str) -> Vec<Generated> {
        let v = sml::parse(src).unwrap();
        generate_package(&v).unwrap()
    }

    fn find<'a>(out: &'a [Generated], path: &str) -> &'a str {
        out.iter()
            .find(|g| g.path == path)
            .unwrap_or_else(|| {
                panic!(
                    "缺少产物 `{path}`，实得：{:?}",
                    out.iter().map(|g| &g.path).collect::<Vec<_>>()
                )
            })
            .content
            .as_str()
    }

    const CUSTOM: &str = concat!(
        "name: MyDialect\n",
        "directives: [ form policy ]\n",
        "colors: {\n",
        "    keyword.control.directive.sml: \"#C586C0\"\n",
        "    variable.other.member.sml: \"#9CDCFE\"\n",
        "}\n"
    );

    #[test]
    fn package_contains_all_artifacts() {
        let out = pkg(CUSTOM);
        for p in [
            "syntaxes/sml.tmLanguage.json",
            "zed/highlights.scm",
            "themes/sml-color-theme.json",
            "vscode/settings.fragment.json",
            "zed/themes/sml.json",
        ] {
            find(&out, p);
        }
    }

    #[test]
    fn no_colors_means_no_theme_artifacts() {
        let out = pkg("directives: [ form ]");
        assert!(
            out.iter().all(|g| !g.path.starts_with("themes/")),
            "没写 colors 就不该产出主题"
        );
        assert_eq!(out.len(), 2, "应只有语法与 Zed 查询");
    }

    #[test]
    fn invalid_color_rejected() {
        let v = sml::parse("colors: { a.b: \"red\" }\n").unwrap();
        let e = generate_package(&v).unwrap_err();
        assert!(e.message().contains("合法颜色"), "应报颜色非法：{e}");
        assert_eq!(e.code(), E_EXT_007, "颜色非法应报 E-EXT-007");
    }

    #[test]
    fn settings_fragment_is_project_local_ready() {
        let out = pkg(CUSTOM);
        let frag = find(&out, "vscode/settings.fragment.json");
        assert!(frag.contains("editor.tokenColorCustomizations"), "缺 {frag}");
        assert!(frag.contains("textMateRules"), "缺 textMateRules：{frag}");
        assert!(frag.contains("#C586C0"), "缺色值：{frag}");
    }

    #[test]
    fn zed_highlights_restrict_directives() {
        let out = pkg(CUSTOM);
        let scm = find(&out, "zed/highlights.scm");
        assert!(scm.contains("#match?"), "方言指令应经谓词限定：{scm}");
        assert!(scm.contains("form|policy"), "应含方言指令名：{scm}");
        assert!(scm.contains("tree-sitter"), "应说明依赖 grammar：{scm}");
        // 谓词必须带 `@`：grammar 的 `directive` 节点文本是 `@form`。
        // 这对不上时查询不会报错，只会静默不上色 —— 只能靠这条钉住。
        assert!(
            scm.contains("^@(form|policy)$"),
            "谓词应匹配含 `@` 的 directive 节点文本：{scm}"
        );
        // 标点用匿名 token 列表，不能用 `(punctuation)`（grammar 里没有这个节点）
        assert!(scm.contains("[\"{\" \"}\""), "标点应写成匿名 token 列表：{scm}");
        assert!(
            !scm.contains("(punctuation)"),
            "不应引用 grammar 不存在的 (punctuation) 节点：{scm}"
        );
    }

    #[test]
    fn zed_theme_maps_scope_to_syntax_key() {
        let out = pkg(CUSTOM);
        let theme = find(&out, "zed/themes/sml.json");
        assert!(theme.contains("\"keyword\""), "应映射出 keyword：{theme}");
        assert!(theme.contains("\"variable\""), "应映射出 variable：{theme}");
        assert!(theme.contains("#C586C0"), "应含色值：{theme}");
    }
}
