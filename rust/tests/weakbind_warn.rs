// SPDX-License-Identifier: MulanPSL-2.0
//! `@weakbind` 软校验（`W-CONTRACT-001`）：与内置契约表比对，**只告警、不改树**。
//!
//! 八档 detail 走消息前缀（`Diagnostic` 没有 detail 字段），见
//! `docs/swsml/spec/weakbind.md` §4。这里钉住四件事：
//! 1. 声明侧三档（unknown_contract / unknown_field / type_mismatch）；
//! 2. 块内容四档（missing_required / type_mismatch / out_of_range / enum_mismatch）；
//! 3. `block.undeclared_field` 只在 **strict** 契约上报；
//! 4. **只读**：不填默认值、不改主树 —— 这是本项的存在底线。

use sml_parse::{parse_with, ParseOptions};

const HDR: &str = "@version v1\n@feature enable type.weak\n";

/// 用一份「内置契约表」SML 构建解析选项：先 `capture_contracts` 读回、再原样注入。
/// 这条链路本身就是 §6.1 的验收（内置集只能用 SML 写）。
fn opts_with(builtin_sml: &str) -> ParseOptions {
    let table = parse_with(builtin_sml, ParseOptions::new().capture_contracts(true))
        .expect("内置契约表本身应能解析")
        .contracts
        .expect("capture_contracts 应回传契约表");
    ParseOptions::new().builtin_contracts(table)
}

/// 解析文档，返回（诊断里的码, 全部消息拼接）。
fn run(doc: &str, builtin_sml: &str) -> (Vec<String>, String, sml_parse::ParseOutput) {
    let out = parse_with(doc, opts_with(builtin_sml)).expect("文档本身应能解析");
    let codes: Vec<String> = out.diagnostics.iter().filter_map(|d| d.code()).map(|c| c.to_string()).collect();
    let msgs = out.diagnostics.iter().map(|d| d.message.clone()).collect::<Vec<_>>().join("\n");
    (codes, msgs, out)
}

#[test]
fn decl_unknown_contract() {
    let (codes, msgs, _) = run(
        &format!("{HDR}@weakbind nosuch\nweb {{ a: 1 }}\n"),
        "@contract tls { host: str }\n",
    );
    assert!(codes.iter().all(|c| c == "W-CONTRACT-001"), "码应全是 W-CONTRACT-001：{codes:?}");
    assert!(msgs.contains("decl.unknown_contract"), "应命中名字档：{msgs}");
}

#[test]
fn decl_unknown_field() {
    let (_, msgs, _) = run(
        &format!("{HDR}@weakbind tls {{ post: int }}\nweb {{ host: h }}\n"),
        "@contract tls { host: str }\n",
    );
    assert!(msgs.contains("decl.unknown_field"), "应命中字段名档：{msgs}");
}

#[test]
fn decl_type_mismatch() {
    let (_, msgs, _) = run(
        &format!("{HDR}@weakbind tls {{ host: int }}\nweb {{ host: h }}\n"),
        "@contract tls { host: str }\n",
    );
    assert!(msgs.contains("decl.type_mismatch"), "应命中类型档：{msgs}");
}

#[test]
fn block_missing_required() {
    let (_, msgs, out) = run(
        &format!("{HDR}@weakbind tls\nweb {{ other: 1 }}\n"),
        "@contract tls { host: str required }\n",
    );
    assert!(msgs.contains("block.missing_required"), "应命中缺必填：{msgs}");
    // 只读：库绝不替它补字段
    assert_eq!(
        out.value.get("web").and_then(|v| v.get("host")),
        None,
        "弱校验不得填默认值/补字段"
    );
}

#[test]
fn block_type_mismatch() {
    let (_, msgs, _) = run(
        &format!("{HDR}@weakbind tls\nweb {{ port: abc }}\n"),
        "@contract tls { port: int }\n",
    );
    assert!(msgs.contains("block.type_mismatch"), "应命中类型不符：{msgs}");
}

#[test]
fn block_out_of_range() {
    let (_, msgs, _) = run(
        &format!("{HDR}@weakbind tls\nweb {{ port: 99 }}\n"),
        "@contract tls { port: int min 1 max 10 }\n",
    );
    assert!(msgs.contains("block.out_of_range"), "应命中越界：{msgs}");
}

#[test]
fn block_enum_mismatch() {
    let (_, msgs, _) = run(
        &format!("{HDR}@weakbind tls\nweb {{ mode: c }}\n"),
        "@contract tls { mode: enum [a b] }\n",
    );
    assert!(msgs.contains("block.enum_mismatch"), "应命中枚举越界：{msgs}");
}

/// 未声明字段只在 **strict** 契约上报：`warn` / `loose` 本来就是宽容档。
#[test]
fn undeclared_field_only_on_strict() {
    let doc = format!("{HDR}@weakbind C\nweb {{ a: 1 extra: 2 }}\n");

    let (_, strict_msgs, _) = run(&doc, "@contract C { a: int }\n");
    assert!(strict_msgs.contains("block.undeclared_field"), "strict 应报：{strict_msgs}");

    let (_, loose_msgs, _) = run(&doc, "@contract C loose { a: int }\n");
    assert!(!loose_msgs.contains("block.undeclared_field"), "loose 不该报：{loose_msgs}");
}

/// 空内置表 ⇒ 整段软校验跳过（否则一落地所有弱声明都会报 unknown_contract）。
#[test]
fn empty_table_skips_everything() {
    let out = parse_with(
        &format!("{HDR}@weakbind whatever\nweb {{ a: 1 }}\n"),
        ParseOptions::new(),
    )
    .expect("空表时文档照常解析");
    assert!(out.diagnostics.is_empty(), "空表不该产生任何告警：{:?}", out.diagnostics);
}

/// 体里缺 required 字段**不报**（体是部分描述，允许只写关心的字段）。
#[test]
fn partial_body_without_required_is_fine() {
    let (_, msgs, _) = run(
        &format!("{HDR}@weakbind tls {{ host: str }}\nweb {{ host: h port: 1 }}\n"),
        "@contract tls {\n  host: str required\n  port: int required\n}\n",
    );
    assert!(msgs.is_empty(), "体只写部分字段是合法用法，不该有告警：{msgs}");
}
