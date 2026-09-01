// SPDX-License-Identifier: MulanPSL-2.0
// `@when <条件>` —— 解析期静态条件裁剪
//
// 设计约束（与这些测试一一对应）：
// 1. **opt-in**：默认关闭，需 `@feature enable when`，避免与旧文档中名为
//    `when` 的片段定义冲突（`@when { .. }` 是合法的片段定义语法）。
// 2. **闭集条件**：只支持 `$env.NAME` 与 `==` / `!=` 比较，无求值器。
// 3. **作用于紧邻的下一个**字段/块。

use sml::{Feature, Value, parse, parse_with_features_env};
use std::collections::BTreeMap;

/// 需要显式 enable，故统一加 `@feature enable when` 头。
const HDR: &str = "@version v1\n@feature enable when\n";

fn parse_env(body: &str, vars: &[(&str, &str)]) -> Result<Value, String> {
    let mut env = BTreeMap::new();
    for (k, v) in vars {
        env.insert((*k).to_string(), (*v).to_string());
    }
    let mut feats = sml::FeatureSet::baseline().with(Feature::When);
    // 保证 env 特性也在（baseline 已含，此处显式声明以防基线调整）
    feats = feats.with(Feature::Env);
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

/// 不加 `@feature enable when` 头（用于验证 opt-in：允许集开了、文档没开）。
fn err_no_enable(body: &str, vars: &[(&str, &str)]) -> String {
    let mut env = BTreeMap::new();
    for (k, v) in vars {
        env.insert((*k).to_string(), (*v).to_string());
    }
    let feats = sml::FeatureSet::baseline().with(Feature::When);
    match parse_with_features_env(body, feats, env) {
        Ok(_) => panic!("应失败，实际通过了:\n{body}"),
        Err(e) => e,
    }
}

// ---------------------------------------------------------------------------
// 基本裁剪行为
// ---------------------------------------------------------------------------

#[test]
fn keeps_field_when_condition_true() {
    let v = ok_env(
        "@when $env.DEPLOY == \"prod\"\nreplicas: 5\n",
        &[("DEPLOY", "prod")],
    );
    assert_eq!(v.get("replicas"), Some(&Value::Int(5)));
}

#[test]
fn drops_field_when_condition_false() {
    let v = ok_env(
        "@when $env.DEPLOY == \"prod\"\nreplicas: 5\n",
        &[("DEPLOY", "dev")],
    );
    assert_eq!(v.get("replicas"), None, "条件为假时字段应被丢弃");
}

#[test]
fn supports_not_equal() {
    let v = ok_env("@when $env.DEPLOY != \"prod\"\ndebug: true\n", &[("DEPLOY", "dev")]);
    assert_eq!(v.get("debug"), Some(&Value::Bool(true)));

    let v = ok_env("@when $env.DEPLOY != \"prod\"\ndebug: true\n", &[("DEPLOY", "prod")]);
    assert_eq!(v.get("debug"), None);
}

#[test]
fn truthiness_test() {
    // 非空且非 "0"/"false" 视为真
    let v = ok_env("@when $env.TOKEN\nauthed: true\n", &[("TOKEN", "abc")]);
    assert_eq!(v.get("authed"), Some(&Value::Bool(true)));

    for falsy in ["", "0", "false"] {
        let v = ok_env("@when $env.TOKEN\nauthed: true\n", &[("TOKEN", falsy)]);
        assert_eq!(v.get("authed"), None, "`{falsy}` 应视为假");
    }
}

// ---------------------------------------------------------------------------
// 作用范围：只作用于紧邻的下一个兄弟
// ---------------------------------------------------------------------------

#[test]
fn applies_only_to_next_sibling() {
    let v = ok_env("@when $env.D == \"prod\"\na: 1\nb: 2\n", &[("D", "dev")]);
    assert_eq!(v.get("a"), None, "a 应被丢弃");
    assert_eq!(v.get("b"), Some(&Value::Int(2)), "b 不应受影响");
}

#[test]
fn multiple_when_directives_are_independent() {
    let v = ok_env(
        "@when $env.D == \"prod\"\na: 1\n@when $env.D == \"dev\"\nb: 2\n",
        &[("D", "dev")],
    );
    assert_eq!(v.get("a"), None);
    assert_eq!(v.get("b"), Some(&Value::Int(2)));
}

#[test]
fn drops_whole_block_including_bare_block() {
    // 键值块
    let v = ok_env(
        "@when $env.D == \"prod\"\ntls { enabled: true  cert: x.pem }\n",
        &[("D", "dev")],
    );
    assert_eq!(v.get("tls"), None);

    // 裸块（如 LVGL 的 `screen settings { .. }`）
    let v = ok_env(
        "@when $env.D == \"prod\"\nscreen settings { width: 320 }\n",
        &[("D", "dev")],
    );
    assert_eq!(v.get("screen"), None);

    // 条件为真时保留
    let v = ok_env(
        "@when $env.D == \"prod\"\nscreen settings { width: 320 }\n",
        &[("D", "prod")],
    );
    assert!(v.get("screen").is_some());
}

#[test]
fn works_inside_nested_blocks() {
    let v = ok_env(
        "outer {\n @when $env.D == \"prod\"\n inner { x: 1 }\n keep: yes\n}\n",
        &[("D", "dev")],
    );
    let outer = v.get("outer").expect("outer 应保留");
    let m = match outer {
        Value::Object(m) => m,
        other => panic!("outer 应为对象，实际 {other:?}"),
    };
    assert!(!m.contains_key("inner"), "inner 应被丢弃");
    assert!(m.contains_key("keep"), "keep 不应受影响");
}

// ---------------------------------------------------------------------------
// opt-in：默认关闭，避免与片段定义冲突
// ---------------------------------------------------------------------------

#[test]
fn disabled_by_default() {
    // 允许集开了 `when`，但文档没写 `@feature enable when` -> 仍应报错。
    // 这是 opt-in 的核心：旧文档不会因为升级解析器而改变行为。
    let e = err_no_enable("@version v1\n@when $env.D == \"prod\"\na: 1\n", &[]);
    assert!(e.contains("特性"), "应提示需要 enable，got: {e}");
}

#[test]
fn fragment_named_when_still_works() {
    // 回归：`@when { .. }` 始终是「定义名为 when 的片段」，
    // 新增指令不得改变这一既有行为。
    let v = parse("@version v4\n@when { a: 1 }\nx: &when").unwrap_or_else(|e| panic!("{e}"));
    assert!(v.get("x").is_some(), "片段引用应生效");
}

// ---------------------------------------------------------------------------
// 错误诊断
// ---------------------------------------------------------------------------

#[test]
fn dangling_when_is_error() {
    let e = err_env("a: 1\n@when $env.D == \"prod\"\n", &[]);
    assert!(e.contains("未跟随"), "got: {e}");
}

#[test]
fn consecutive_when_is_error() {
    let e = err_env("@when $env.D == \"prod\"\n@when $env.D == \"dev\"\na: 1\n", &[]);
    assert!(e.contains("连续"), "got: {e}");
}

#[test]
fn non_env_lhs_is_error() {
    let e = err_env("@when foo == \"bar\"\na: 1\n", &[]);
    assert!(e.contains("$env.NAME"), "got: {e}");
}

#[test]
fn missing_rhs_gives_actionable_error() {
    // 漏写比较值时，下一个字段名会被吃掉；此处应提前识别并给出准确提示
    let e = err_env("@when $env.D ==\na: 1\n", &[]);
    assert!(e.contains("缺少比较值"), "应给出准确诊断，got: {e}");
}

// ---------------------------------------------------------------------------
// 安全：条件值来自环境，但只做字符串比较，绝不参与求值
// ---------------------------------------------------------------------------

#[test]
fn env_value_with_shell_metacharacters_is_only_compared() {
    for evil in ["1; rm -rf /", "`whoami`", "${x}", "{{y}}", "../../etc/passwd"] {
        let v = ok_env(
            &format!("@when $env.EVIL == \"{evil}\"\nsafe: yes\n"),
            &[("EVIL", evil)],
        );
        assert_eq!(v.get("safe"), Some(&Value::Str("yes".into())),
                   "`{evil}` 应只作为字符串比较，不产生副作用");
    }
}

#[test]
fn env_value_is_not_evaluated_as_expression() {
    // `$env.V` 为 "1 == 1" 时不得被当成表达式求值成 true
    let v = ok_env("@when $env.V == \"1 == 1\"\nhit: yes\n", &[("V", "1 == 1")]);
    assert_eq!(v.get("hit"), Some(&Value::Str("yes".into())));
}
