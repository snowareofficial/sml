// SPDX-License-Identifier: MulanPSL-2.0
//! weakbind 第一刀：`@weakbind` 的语法、作用域与旁路表（`docs/weakbind-spec-draft.md`）。
//!
//! 钉三件事：
//! 1. **核心判据**：有 / 无 `@weakbind`，主树与 `contract_map` 逐字节相同 ——
//!    弱绑定只活在 `ParseOutput::weak_binds` 里，绝不进数据树；
//! 2. **作用域**：只作用于紧邻的下一个块；悬空 / 连续 / 非块一律 `E-PARSE-029`；
//! 3. **门控**：不开 `type.weak` 就当没这个指令（`E-FEATURE-001`），v1–v4 既有文档一字不变。

use sml_parse::{parse_with, ParseOptions, PathSeg, Value};

/// 统一头部：opt-in 特性必须在文档里显式开（与 `when.rs` / `for.rs` 同一套写法）。
const HDR: &str = "@version v1\n@feature enable type.weak\n";

fn k(s: &str) -> PathSeg {
    PathSeg::Key(s.into())
}

#[test]
fn records_name_and_path_without_touching_tree() {
    let src = format!("{HDR}@weakbind tls\nweb {{\n  host: h\n}}\n");
    let out = parse_with(&src, ParseOptions::new()).unwrap();

    assert_eq!(out.weak_binds.len(), 1, "应记录一条弱绑定");
    let w = &out.weak_binds[0];
    assert_eq!(w.name, "tls");
    assert_eq!(w.path, vec![k("web")], "路径 = 目标块在结果树里的位置");
    assert!(w.body.is_none(), "只写名字时字段体应为 None");
    assert_eq!(w.line, Some(3), "行号应指向 `@weakbind` 那一行");

    // 库里**不消费**它：树里没有 `__weak` 之类的痕迹，块照常是普通块。
    assert_eq!(
        out.value.get("web").and_then(|v| v.get("host")),
        Some(&Value::Str("h".into())),
        "目标块的内容必须原样保留"
    );
}

/// 核心判据：弱绑定**不改变运行期语义**。红了就说明它越界了。
#[test]
fn same_tree_with_and_without_weakbind() {
    let opts = || ParseOptions::new().trace_contracts(true);
    let with = format!("{HDR}@weakbind tls\nweb {{\n  host: h\n}}\n");
    let without = format!("{HDR}web {{\n  host: h\n}}\n");

    let a = parse_with(&with, opts()).unwrap();
    let b = parse_with(&without, opts()).unwrap();

    assert_eq!(a.value, b.value, "主树必须逐字节相同");
    assert_eq!(a.contract_map, b.contract_map, "契约映射也必须相同");
    assert_eq!(b.weak_binds.len(), 0, "没写指令时旁路表为空");
    assert_eq!(a.weak_binds.len(), 1);
}

#[test]
fn optional_body_is_parsed_as_field_specs() {
    let src = format!("{HDR}@weakbind tls {{ host: str optional }}\nweb {{ host: h }}\n");
    let out = parse_with(&src, ParseOptions::new()).unwrap();

    let body = out.weak_binds[0].body.as_ref().expect("写了体就该有体");
    assert!(body.contains_key("host"), "体里应解析出 host 字段");
    assert!(!body["host"].required, "`optional` 修饰符要生效");
}

#[test]
fn feature_off_rejects_the_directive() {
    // 不写 `@feature enable type.weak` ⇒ 这条指令根本不存在。
    let src = "@version v1\n@weakbind tls\nweb { host: h }\n";
    let e = parse_with(src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-FEATURE-001", "实得：{e}");
}

#[test]
fn dangling_declaration_is_an_error() {
    let src = format!("{HDR}@weakbind tls\n");
    let e = parse_with(&src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-PARSE-029", "悬空应报作用域非法：{e}");
}

#[test]
fn consecutive_declarations_are_an_error() {
    let src = format!("{HDR}@weakbind a\n@weakbind b\nweb {{ }}\n");
    let e = parse_with(&src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-PARSE-029", "连续出现应报作用域非法：{e}");
}

#[test]
fn scalar_target_is_an_error() {
    let src = format!("{HDR}@weakbind tls\na: 1\n");
    let e = parse_with(&src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-PARSE-029", "标量不是块：{e}");
}

#[test]
fn array_target_is_an_error() {
    let src = format!("{HDR}@weakbind tls\na: [1 2]\n");
    let e = parse_with(&src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-PARSE-029", "数组不是块：{e}");
}
