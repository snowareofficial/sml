//! TODO L16：一次报出**多条**错误（`ParseOptions::collect_errors(true)`）。
//!
//! 默认（`false`）仍**遇错即停**并返回 `Err` —— 既有行为逐字不变。
//! 开启后 `parse_with` 返回 `Ok(ParseOutput)`，`errors` 收集解析期全部错误、
//! `value` 为**尽力而为的部分结果**（当前实现：块/数组内按同步点恢复继续）。

use sml_parse::{parse_with, ParseOptions};

fn collect(text: &str) -> sml_parse::ParseOutput {
    parse_with(text, ParseOptions::new().collect_errors(true))
        .expect("collect_errors 下应返回部分结果")
}

#[test]
fn collects_multiple_errors_in_one_block() {
    // 两处「键位置裸 `:`」分别在两个字段之后 ⇒ 应各报一次。
    let text = "@version v1\ndoc {\n  a: 1\n  : bad\n  b: 2\n  : bad2\n}\n";
    let out = collect(text);
    let codes: Vec<&str> = out.errors.iter().map(|e| e.code()).collect();
    assert!(
        out.errors.len() >= 2,
        "应收集多条错误，实际 {} 条：{codes:?}",
        out.errors.len()
    );
    // 部分结果里两处合法字段都保留下来（恢复后继续解析）。
    assert_eq!(out.value.get("doc.a").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(out.value.get("doc.b").and_then(|v| v.as_i64()), Some(2));
}

#[test]
fn default_still_fails_fast() {
    let text = "@version v1\ndoc {\n  a: 1\n  : bad\n  b: 2\n}\n";
    let e = sml::parse(text).expect_err("默认应遇错即停");
    assert_eq!(e.code(), "E-PARSE-006");
}

#[test]
fn valid_document_has_no_errors() {
    let out = collect("@version v1\ndoc {\n  a: 1\n  b: 2\n}\n");
    assert!(out.errors.is_empty(), "合法文档不应有错误: {:?}",
        out.errors.iter().map(|e| e.code()).collect::<Vec<_>>());
    assert_eq!(out.value.get("doc.a").and_then(|v| v.as_i64()), Some(1));
}

/// 契约**应用**失败发生在块收尾（`finish_block`）——`collect_errors` 下也应被收集，
/// 而不是半路中断。
#[test]
fn collects_contract_application_error_at_block_finish() {
    let text = "@version v1\n@contract P { n: int }\ndoc {\n  @is P\n  n: \"str\"\n}\n";
    let out = collect(text);
    let codes: Vec<&str> = out.errors.iter().map(|e| e.code()).collect();
    assert!(
        codes.iter().any(|c| c.starts_with("E-CONTRACT")),
        "契约应用错误应在 finish 阶段被收集：{codes:?}"
    );
    // 尽力而为的部分结果：未通过校验也不丢原文数据。
    assert_eq!(out.value.get("doc.n").and_then(|v| v.as_str()), Some("str"));
}

#[test]
fn recovers_inside_nested_block() {
    // 错误在 child 内；恢复应回到 child 的 `}`，父块的 `ok` 仍能解析。
    let text = "@version v1\nroot {\n  child {\n    : bad\n  }\n  ok: 1\n}\n";
    let out = collect(text);
    assert!(!out.errors.is_empty(), "嵌套块内的错误也应被收集");
    assert_eq!(
        out.value.get("root.ok").and_then(|v| v.as_i64()),
        Some(1),
        "恢复后父块后续字段应继续解析"
    );
}

/// L16 最后一块：`@for` 子解析器的错误也应并入 `errors`，而不是整体中断。
#[test]
fn collects_errors_from_for_body() {
    // `@for` 体内 `k: B::C` 触发「键位置裸 `:`」⇒ E-PARSE-006。
    let text = "@feature enable for\nlist: @for x in a b {\n  k: B::C\n}\n";
    let out = collect(text);
    let codes: Vec<&str> = out.errors.iter().map(|e| e.code()).collect();
    assert!(
        codes.iter().any(|c| *c == "E-PARSE-006"),
        "for 体内的解析错误应被收集：{codes:?}"
    );
    // 两个枚举项都跑完（失败的以 Null 占位，不中断整个数组）。
    assert_eq!(
        out.value
            .get("list")
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(2),
        "应继续跑完其余枚举项"
    );
}
