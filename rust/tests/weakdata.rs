// SPDX-License-Identifier: MulanPSL-2.0
//! `@weakdata`（I-15）：**不进主树**的载荷，只给拓展按名读取。
//!
//! 钉四件事（`docs/swsml/spec/weakdata.md`）：
//! 1. 载荷只出现在 `weak_data` 里，主树**查不到**同名键；
//! 2. 有 / 无 `@weakdata`，主树与 `contract_map` 逐字节相同（核心判据）；
//! 3. 同一 `(path, name)` 后者覆盖，只留一条；
//! 4. 门控与错误形态：不开特性 ⇒ `E-FEATURE-001`；缺载荷块 ⇒ `E-PARSE-019`。

use sml_parse::{parse_with, ParseOptions, PathSeg, Value};

const HDR: &str = "@version v1\n@feature enable type.weak\n";

fn k(s: &str) -> PathSeg {
    PathSeg::Key(s.into())
}

#[test]
fn payload_is_readable_and_absent_from_the_tree() {
    let src = format!("{HDR}@weakdata highlight {{\n  keyword: blue\n}}\n");
    let out = parse_with(&src, ParseOptions::new()).unwrap();

    assert_eq!(out.weak_data.len(), 1);
    let wd = &out.weak_data[0];
    assert_eq!(wd.name, "highlight");
    assert!(wd.path.is_empty(), "顶层声明的路径应为空");
    assert_eq!(
        wd.value.get("keyword"),
        Some(&Value::Str("blue".into())),
        "载荷应可用普通 Value API 读"
    );
    assert_eq!(wd.line, Some(3), "行号应指向 `@weakdata` 那行（前两行是 HDR）");

    // 核心：主树里查不到 `highlight`
    assert_eq!(out.value.get("highlight"), None, "载荷绝不能进主树");
}

/// 核心判据：弱数据不改变运行期语义。
#[test]
fn same_tree_with_and_without_weakdata() {
    let opts = || ParseOptions::new().trace_contracts(true);
    let with = format!("{HDR}@weakdata hint {{ a: 1 }}\nweb {{\n  host: h\n}}\n");
    let without = format!("{HDR}web {{\n  host: h\n}}\n");

    let a = parse_with(&with, opts()).unwrap();
    let b = parse_with(&without, opts()).unwrap();

    assert_eq!(a.value, b.value, "主树必须逐字节相同");
    assert_eq!(a.contract_map, b.contract_map, "契约映射也必须相同");
    assert_eq!(b.weak_data.len(), 0, "没写指令时旁路表为空");
    assert_eq!(a.weak_data.len(), 1);
}

#[test]
fn later_same_name_wins() {
    let src = format!("{HDR}@weakdata hint {{ a: 1 }}\n@weakdata hint {{ a: 2 }}\n");
    let out = parse_with(&src, ParseOptions::new()).unwrap();

    assert_eq!(out.weak_data.len(), 1, "同名同路径只留一条");
    assert_eq!(
        out.weak_data[0].value.get("a"),
        Some(&Value::Int(2)),
        "后者覆盖前者"
    );
}

#[test]
fn nested_declaration_records_its_path() {
    let src = format!("{HDR}section {{\n  @weakdata hint {{ a: 1 }}\n}}\n");
    let out = parse_with(&src, ParseOptions::new()).unwrap();

    assert_eq!(out.weak_data.len(), 1);
    assert_eq!(out.weak_data[0].path, vec![k("section")], "块内声明要带路径");
}

#[test]
fn distinct_names_coexist() {
    let src = format!("{HDR}@weakdata a {{ x: 1 }}\n@weakdata b {{ x: 2 }}\n");
    let out = parse_with(&src, ParseOptions::new()).unwrap();
    assert_eq!(out.weak_data.len(), 2);
}

#[test]
fn feature_off_rejects_the_directive() {
    let src = "@version v1\n@weakdata hint { a: 1 }\n";
    let e = parse_with(src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-FEATURE-001", "实得：{e}");
}

#[test]
fn missing_payload_block_is_a_directive_header_error() {
    let src = format!("{HDR}@weakdata hint\n");
    let e = parse_with(&src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-PARSE-019", "缺载荷块应报指令头非法：{e}");
}
