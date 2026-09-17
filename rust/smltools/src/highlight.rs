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

/// 原版高亮基线。
///
/// **刻意用 `include_str!` 内嵌真实文件的副本**，而不是把 JSON 抄成 Rust 常量：
/// 抄写会产生一份编译器不会帮你检查的「影子副本」，原版一更新就悄悄漂移。
/// 同步方式：把 `editors/vscode/syntaxes/sml.tmLanguage.json` 复制到
/// `rust/smltools/assets/baseline.tmLanguage.json`（见该目录 README）。
const BASELINE: &str = include_str!("../assets/baseline.tmLanguage.json");

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
pub fn generate(custom: &Value) -> Result<String, String> {
    let mut base = sml::json_to_value(BASELINE)
        .ok_or_else(|| "内置高亮基线不是合法 JSON（资产损坏）".to_string())?;

    let directives = str_array(custom, "directives")?;
    let elements = str_array(custom, "elements")?;
    let types = str_array(custom, "types")?;
    let rules = collect_rules(custom)?;

    // 可选覆盖 scopeName / name
    let Value::Object(base_map) = &mut base else {
        return Err("内置高亮基线的顶层不是对象（资产损坏）".to_string());
    };
    for key in ["scopeName", "name"] {
        if let Some(v) = custom.get(key).and_then(|x| x.as_str()) {
            if key == "scopeName" && !is_valid_scope(v) {
                return Err(format!(
                    "scopeName `{v}` 非法：须形如 `x.y`（至少一个点；各段为字母/数字/`_`/`-` 且非空）\
                     —— 写坏会让语法根本挂不上语言"
                ));
            }
            base_map.insert(key.to_string(), Value::Str(v.to_string()));
        }
    }

    let Some(Value::Array(patterns)) = base_map.get_mut("patterns") else {
        return Err("内置高亮基线缺少 patterns 数组（资产损坏）".to_string());
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
        return Err("内置高亮基线的 patterns 里没有可用的 include 锚点（资产损坏）".to_string());
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
                    return Err(format!(
                        "rules[{i}] 的{}锚点 `{anchor}` 不存在；可用锚点：{}",
                        if is_after { " after " } else { " before " },
                        anchors.join(", ")
                    ));
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
fn str_array(root: &Value, key: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    match root.get(key) {
        None => {}
        Some(Value::Array(a)) => {
            for (i, v) in a.iter().enumerate() {
                match v.as_str() {
                    Some(s) => out.push(s.to_string()),
                    None => return Err(format!("{key}[{i}] 不是字符串（应为名字）")),
                }
            }
        }
        Some(_) => return Err(format!("{key} 必须是数组，如 `{key}: [ a b c ]`")),
    }
    Ok(out)
}

/// 解析 `rules`：每项必须有 `name` 与 `match`；`after` / `before` 至多其一。
fn collect_rules(root: &Value) -> Result<Vec<Rule>, String> {
    let arr = match root.get("rules") {
        None => return Ok(Vec::new()),
        Some(Value::Array(a)) => a,
        Some(_) => return Err("rules 必须是数组".to_string()),
    };
    let mut out = Vec::new();
    for (i, r) in arr.iter().enumerate() {
        let name = r
            .get("name")
            .and_then(|x| x.as_str())
            .ok_or_else(|| format!("rules[{i}] 缺少 name（TextMate scope 名）"))?;
        let m = r
            .get("match")
            .and_then(|x| x.as_str())
            .ok_or_else(|| format!("rules[{i}] 缺少 match（匹配正则字符串）"))?;
        let after = r.get("after").and_then(|x| x.as_str());
        let before = r.get("before").and_then(|x| x.as_str());
        let anchor = match (after, before) {
            (Some(_), Some(_)) => {
                return Err(format!("rules[{i}] 同时给了 after 与 before，位置无法确定"))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(src: &str) -> Result<String, String> {
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
        assert!(e.contains("锚点"), "应提到锚点：{e}");
        assert!(e.contains("可用锚点"), "应列出可用锚点：{e}");
        assert!(e.contains("directive"), "列表应含 directive：{e}");
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
}
