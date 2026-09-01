//! 解析错误边界行为回归测试。
//!
//! 覆盖审计中暴露的「错误边界行为不一致」问题：
//! - 块未闭合（EOF 收尾）必须报错，而非静默吞掉后续内容
//! - 顶层多余的 `}` / `]` 必须报错，而非静默忽略
//! - 嵌套块括号类型不匹配（如 `}` 与 `]` 混用）必须报错
//! - 顶层数组未闭合（缺少 `]`）必须报错

use sml::parse;

#[test]
fn unclosed_block_must_error() {
    // `a { x: 1` 缺少结束 `}`，按 EOF 收尾会静默吞掉未闭合错误。
    let r = parse("a { x: 1");
    assert!(r.is_err(), "未闭合块 {{ x: 1 应报错，实际 ok: {:?}", r);
}

#[test]
fn unclosed_top_array_must_error() {
    // 顶层数组 `[ 1 2 3` 缺少结束 `]`。
    let r = parse("[ 1 2 3");
    assert!(r.is_err(), "未闭合数组应报错，实际 ok: {:?}", r);
}

#[test]
fn unclosed_nested_block_must_error() {
    // 嵌套块内部未闭合：外层有 `}`，但内层 `b {` 缺 `}`。
    let r = parse("a {\n  b { x: 1\n}");
    assert!(r.is_err(), "嵌套未闭合块应报错，实际 ok: {:?}", r);
}

#[test]
fn stray_rbrace_top_must_error() {
    // 顶层多余的 `}`。
    let r = parse("name: John\n}");
    assert!(r.is_err(), "顶层多余 }} 应报错，实际 ok: {:?}", r);
}

#[test]
fn stray_rbrack_top_must_error() {
    // 顶层多余的 `]`。
    let r = parse("name: John\n]");
    assert!(r.is_err(), "顶层多余 ]] 应报错，实际 ok: {:?}", r);
}

#[test]
fn bracket_mismatch_in_block_must_error() {
    // 块内用 `]` 代替 `}` 闭合：类型不匹配必须报错。
    let r = parse("a { x: 1 ]");
    assert!(r.is_err(), "块内 }}/] 混用应报错，实际 ok: {:?}", r);
}

#[test]
fn bracket_mismatch_in_array_must_error() {
    // 数组内用 `}` 代替 `]` 闭合：类型不匹配必须报错。
    let r = parse("[ 1 2 3 }");
    assert!(r.is_err(), "数组内 ]/}} 混用应报错，实际 ok: {:?}", r);
}

#[test]
fn well_formed_still_ok() {
    // 健全性：合法文档（含嵌套块/数组）仍应正常解析。
    let doc = "a {\n  x: 1\n  b { y: 2 }\n  list: [ 1 2 3 ]\n}";
    let r = parse(doc);
    assert!(r.is_ok(), "合法嵌套文档应 ok，实际 err: {:?}", r);
}
