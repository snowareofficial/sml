// SPDX-License-Identifier: MulanPSL-2.0
// `@for var in a b c { ... }` —— 解析期有界循环展开
//
// 设计约束（与这些测试一一对应）：
// 1. **opt-in**：默认关闭，需 `@feature enable for`，避免与旧文档中名为
//    `for` 的片段定义冲突。
// 2. **有界**：只遍历 `in` 后的有限枚举列表，无 `while`、无递归——LOOP 语言，
//    非图灵完备（算不了 Ackermann 函数）。
// 3. **变量只读**：`${var}` 是只读绑定，循环体不能修改它或列表。
// 4. **组合陷阱**：外层 `@when` 作用于整个 `hosts: @for ...`（条件为假则整段数组
//    不出现），即「`@when` 过滤的是字段，不是某一轮迭代」。

use sml::{Feature, Value, parse, parse_with_features_env};
use std::collections::BTreeMap;

/// 需要显式 enable，故统一加 `@feature enable for` 头（组合测试还会用到 `when`）。
const HDR: &str = "@version v1\n@feature enable for\n@feature enable when\n";

fn parse_env(body: &str, vars: &[(&str, &str)]) -> Result<Value, String> {
    let mut env = BTreeMap::new();
    for (k, v) in vars {
        env.insert((*k).to_string(), (*v).to_string());
    }
    let mut feats = sml::FeatureSet::baseline().with(Feature::For).with(Feature::When);
    feats = feats.with(Feature::Env); // 循环体内可能同时引用 $env
    parse_with_features_env(&format!("{HDR}{body}"), feats, env).map(|(v, _)| v)
}

fn ok_env(body: &str, vars: &[(&str, &str)]) -> Value {
    match parse_env(body, vars) {
        Ok(v) => v,
        Err(e) => panic!("应解析成功，实际失败: {e}\n---\n{HDR}{body}\n---"),
    }
}

fn err_env(body: &str, vars: &[(&str, &str)]) -> String {
    match parse_env(body, vars) {
        Ok(_) => panic!("应失败，实际通过了:\n{HDR}{body}"),
        Err(e) => e,
    }
}

fn as_array<'a>(v: &'a Value, key: &str) -> &'a Vec<Value> {
    match v.get(key) {
        Some(Value::Array(a)) => a,
        other => panic!("字段 `{key}` 应为数组，得 {other:?}"),
    }
}

// ── 1. 基本展开 + ${var} 插值 ─────────────────────────────────────────────
#[test]
fn basic_for_expands_to_array() {
    let doc = r#"
hosts: @for h in alpha beta gamma {
  name: "${h}"
}
"#;
    let v = ok_env(doc, &[]);
    let arr = as_array(&v, "hosts");
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].get("name"), Some(&Value::Str("alpha".into())));
    assert_eq!(arr[1].get("name"), Some(&Value::Str("beta".into())));
    assert_eq!(arr[2].get("name"), Some(&Value::Str("gamma".into())));
}

// ── 2. opt-in：未 enable 报错 ─────────────────────────────────────────────
#[test]
fn for_requires_feature_enable() {
    let doc = "hosts: @for h in a b { name: \"${h}\" }\n";
    let mut feats = sml::FeatureSet::baseline().with(Feature::Env);
    let v = parse_with_features_env(doc, feats, BTreeMap::new())
        .expect_err("未 enable for 应报错");
    assert!(
        v.contains("for"),
        "报错应提示未启用 for，实际: {v}"
    );
}

// ── 3. 组合陷阱：@when 作用于整个 @for 字段 ──────────────────────────────
//
// 这是最易踩的坑：`@when $env.DEBUG == "1"` 后跟 `hosts: @for ...`，
// 条件为假时 **整个 hosts 字段都不会出现**（不是数组为空，而是键不存在）。
#[test]
fn when_when_false_drops_whole_for_field() {
    let doc = r#"
@when $env.DEBUG == "1"
hosts: @for h in a b c {
  name: "${h}"
}
"#;
    // DEBUG != "1" → 整个 hosts 字段被丢弃
    let v = ok_env(doc, &[("DEBUG", "0")]);
    assert!(
        v.get("hosts").is_none(),
        "条件为假时 hosts 应整体消失，得 {v:?}"
    );
}

#[test]
fn when_when_true_keeps_whole_for_field() {
    let doc = r#"
@when $env.DEBUG == "1"
hosts: @for h in a b c {
  name: "${h}"
}
"#;
    let v = ok_env(doc, &[("DEBUG", "1")]);
    assert_eq!(as_array(&v, "hosts").len(), 3);
}

// ── 4. 循环体内的 @when：条件作用于每一轮迭代（正确组合） ─────────────────
//
// 与陷阱 3 不同：`@when` 写在 `@for` 块内部时，它作用于块内「紧邻的下一个字段」，
// 因此会按每个 item 独立求值。这是期望的逐元素过滤。
#[test]
fn when_inside_for_filters_per_item() {
    let doc = r#"
hosts: @for h in a b c {
  @when $env.KEEP == "1"
  name: "${h}"
}
"#;
    // KEEP 关闭 → 每个 item 的 name 字段都被丢弃，但 hosts 仍是 3 个空对象
    let v = ok_env(doc, &[("KEEP", "0")]);
    let arr = as_array(&v, "hosts");
    assert_eq!(arr.len(), 3);
    for item in arr {
        assert!(item.get("name").is_none(), "逐元素 @when 应丢弃 name");
    }
}

// ── 5. 嵌套 @for：外层绑定在内层可见（${var} 作用域叠加） ────────────────
#[test]
fn nested_for_sees_outer_binding() {
    let doc = r#"
matrix: @for r in 1 2 {
  row: "${r}"
  cols: @for c in x y {
    cell: "${r}-${c}"
  }
}
"#;
    let v = ok_env(doc, &[]);
    let rows = as_array(&v, "matrix");
    assert_eq!(rows.len(), 2);
    let r0 = &rows[0];
    assert_eq!(r0.get("row"), Some(&Value::Str("1".into())));
    let cols0 = match r0.get("cols") {
        Some(Value::Array(a)) => a,
        o => panic!("cols 应为数组，得 {o:?}"),
    };
    assert_eq!(cols0.len(), 2);
    assert_eq!(
        cols0[0].get("cell"),
        Some(&Value::Str("1-x".into()))
    );
    assert_eq!(
        cols0[1].get("cell"),
        Some(&Value::Str("1-y".into()))
    );
}

// ── 6. 错误：列表为空 / 缺 in ────────────────────────────────────────────
#[test]
fn for_empty_list_errors() {
    let doc = "hosts: @for h in { name: \"${h}\" }\n";
    let e = err_env(doc, &[]);
    assert!(e.contains("枚举项"), "应报列表为空，实际: {e}");
}

#[test]
fn for_missing_in_errors() {
    let doc = "hosts: @for h a b { name: \"${h}\" }\n";
    let e = err_env(doc, &[]);
    assert!(e.contains("in"), "应报缺 in 关键字，实际: {e}");
}

// ── 7. `${var}` 未绑定原样保留（避免静默空串掩盖笔误） ───────────────────
#[test]
fn unbound_loop_var_is_preserved() {
    let doc = r#"
hosts: @for h in a b {
  name: "${typo}"
}
"#;
    let v = ok_env(doc, &[]);
    let arr = as_array(&v, "hosts");
    assert_eq!(
        arr[0].get("name"),
        Some(&Value::Str("${typo}".into())),
        "未绑定的 ${{typo}} 应原样保留"
    );
}

// ── 8. 端到端：examples/for_when.sml 整体可解析且结构正确 ─────────────────
#[test]
fn example_for_when_sml_parses() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/for_when.sml");
    let src = std::fs::read_to_string(path).expect("应能读取示例文件");
    let v = ok_env(&src, &[("ENV", "prod"), ("DEBUG", "1")]);

    // @when $env.ENV == "prod" → 出现
    assert_eq!(v.get("log_level"), Some(&Value::Str("warn".into())));
    // @when $env.DEBUG（真值）→ 出现
    assert_eq!(v.get("verbose"), Some(&Value::Bool(true)));

    // @for h in web api db → 3 个元素
    assert_eq!(as_array(&v, "hosts").len(), 3);

    // 嵌套 @for → matrix 2 行，每行 cols 2 列
    let matrix = as_array(&v, "matrix");
    assert_eq!(matrix.len(), 2);
    let r0 = &matrix[0];
    assert_eq!(r0.get("row"), Some(&Value::Str("1".into())));
    let cols0 = match r0.get("cols") {
        Some(Value::Array(a)) => a,
        o => panic!("cols 应为数组，得 {o:?}"),
    };
    assert_eq!(cols0.len(), 2);
    assert_eq!(
        cols0[1].get("cell"),
        Some(&Value::Str("1-y".into()))
    );
}

// 组合陷阱端到端：ENV != prod 时 log_level 整段消失
#[test]
fn example_for_when_env_off_drops_log_level() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/for_when.sml");
    let src = std::fs::read_to_string(path).expect("应能读取示例文件");
    let v = ok_env(&src, &[("ENV", "dev"), ("DEBUG", "0")]);
    assert!(
        v.get("log_level").is_none(),
        "ENV != prod 时 log_level 应被 @when 裁剪"
    );
    // hosts 不依赖 env，仍正常展开
    assert_eq!(as_array(&v, "hosts").len(), 3);
}
