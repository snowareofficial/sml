// SPDX-License-Identifier: MulanPSL-2.0
//! `@weakdoc`（I-17）：只登记 profile **名字**，库不读文件系统。
//!
//! 钉四件事（`docs/weakdoc-spec-draft.md`）：
//! 1. 主树零污染、运行期语义不变（有 / 无 `@weakdoc` 主树与 `contract_map` 相同）；
//! 2. 判重是**文档级**：同一个 profile 挂两次 ⇒ `E-PARSE-030`（跨块也算）；
//! 3. `@weakdoc` **不接受载荷块**（那是 `@weakdata` 的形态）⇒ `E-PARSE-019`；
//! 4. 门控：不开 `type.weak` ⇒ `E-FEATURE-001`。

use sml_parse::{parse_with, ParseOptions, PathSeg};

const HDR: &str = "@version v1\n@feature enable type.weak\n";

fn k(s: &str) -> PathSeg {
    PathSeg::Key(s.into())
}

#[test]
fn records_name_and_does_not_touch_the_tree() {
    let src = format!("{HDR}@weakdoc api-docs\nweb {{\n  host: h\n}}\n");
    let out = parse_with(&src, ParseOptions::new()).unwrap();

    assert_eq!(out.weak_docs.len(), 1);
    let wd = &out.weak_docs[0];
    assert_eq!(wd.name, "api-docs");
    assert!(wd.path.is_empty(), "顶层引用的路径为空");
    assert_eq!(wd.line, Some(3), "行号指向 `@weakdoc` 那行（前两行是 HDR）");

    assert_eq!(out.value.get("api-docs"), None, "引用本身绝不能进主树");
    assert_eq!(
        out.value.get("web").and_then(|v| v.get("host")).is_some(),
        true,
        "文档其余部分照常"
    );
}

/// 核心判据：有没有 profile 引用，运行期语义完全一致。
#[test]
fn same_tree_with_and_without_weakdoc() {
    let opts = || ParseOptions::new().trace_contracts(true);
    let with = format!("{HDR}@weakdoc api-docs\nweb {{\n  host: h\n}}\n");
    let without = format!("{HDR}web {{\n  host: h\n}}\n");

    let a = parse_with(&with, opts()).unwrap();
    let b = parse_with(&without, opts()).unwrap();
    assert_eq!(a.value, b.value, "主树必须逐字节相同");
    assert_eq!(a.contract_map, b.contract_map, "契约映射也必须相同");
    assert_eq!(b.weak_docs.len(), 0);
}

#[test]
fn duplicate_reference_is_an_error_even_across_blocks() {
    // 顶层挂一次，块内再挂一次 —— 判重是文档级，不看路径。
    let src = format!("{HDR}@weakdoc api-docs\nsection {{\n  @weakdoc api-docs\n}}\n");
    let e = parse_with(&src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-PARSE-030", "重复引用应报专门码：{e}");
}

#[test]
fn distinct_profiles_coexist_and_keep_their_paths() {
    let src = format!("{HDR}@weakdoc a\nsection {{\n  @weakdoc b\n}}\n");
    let out = parse_with(&src, ParseOptions::new()).unwrap();
    assert_eq!(out.weak_docs.len(), 2);
    assert_eq!(out.weak_docs[0].path, Vec::new());
    assert_eq!(out.weak_docs[1].path, vec![k("section")]);
}

#[test]
fn payload_block_is_rejected_with_a_pointer_to_weakdata() {
    let src = format!("{HDR}@weakdoc api-docs {{ a: 1 }}\n");
    let e = parse_with(&src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-PARSE-019", "载荷块应被拒：{e}");
    assert!(e.message().contains("@weakdata"), "错误应指向正确指令：{e}");
}

#[test]
fn feature_off_rejects_the_directive() {
    let src = "@version v1\n@weakdoc api-docs\n";
    let e = parse_with(src, ParseOptions::new()).unwrap_err();
    assert_eq!(e.code(), "E-FEATURE-001", "实得：{e}");
}
