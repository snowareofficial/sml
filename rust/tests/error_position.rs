//! A3 验收（TASK-agent-w24）：解析错误要带**行列**，且指令行剥离后行列不漂移。
//!
//! 背景：Rust 侧此前报错**没有位置**（`SmlError` 无 pos）；而剥离 `@version` /
//! `@feature` 的实现是「删行」，就算有位置也会整体前移。本文件钉两件事：
//! 1. 文案带 `（第 N 行第 M 列）`；
//! 2. **有指令行的文档，行号必须对得上原文**（这是 scan.rs 等长替换的意义）。

use sml::parse;

/// 无指令行：`a: B::C` 里的第一个 `:` 在第 1 行第 9 列。
#[test]
fn parse_error_carries_line_col() {
    let e = parse("X { a: B::C }\n").unwrap_err();
    assert_eq!(e.code(), "E-PARSE-006");
    assert!(
        e.message().contains("第 1 行第 9 列"),
        "文案应含行列：{}",
        e.message()
    );
}

/// **核心 payoff**：前面有 `@feature` / `@version` 行时，行号不得前移。
/// （改动前这里会落在第 1 行 —— 指令行被删掉后整篇上移了两行。）
#[test]
fn line_col_survives_directive_stripping() {
    let src = "@feature enable contract\n@version v1\nX { a: B::C }\n";
    let e = parse(src).unwrap_err();
    assert_eq!(e.code(), "E-PARSE-006");
    assert!(
        e.message().contains("第 3 行第 9 列"),
        "错误在第 3 行（指令行占 1、2 行），实际文案：{}",
        e.message()
    );
}

/// 未定位的错误（如文件入口的 include 展开）**不得**凭空造位置。
#[test]
fn no_position_means_no_position_text() {
    // 顶层标量（E-PARSE-008）在带位置接线之前 —— 不应出现行列后缀
    let e = parse("42\n").unwrap_err();
    assert_eq!(e.code(), "E-PARSE-008");
    assert!(
        !e.message().contains("第 "),
        "未接线的错误不得出现行列：{}",
        e.message()
    );
}
