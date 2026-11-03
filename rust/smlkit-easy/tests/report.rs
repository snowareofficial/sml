// SPDX-License-Identifier: MulanPSL-2.0
//! L2：诊断（`*_with_report` / `diagnostics()`）与逐后端输出（`out::*`）。

use smlkit_easy::Doc;

const DUP_KEY: &str = "a: 1\na: 2\n";

#[test]
fn report_entry_keeps_diagnostics() {
    let doc = Doc::read_with_report(DUP_KEY).unwrap();
    assert!(doc.has_diagnostics(), "重复键应产生诊断");
    assert_eq!(doc.diagnostics().len(), 1);
    // 语义不变：同名键仍提升为数组（诊断是**非致命**的）
    assert_eq!(doc.list("a").map(|l| l.len()), Some(2));
}

#[test]
fn plain_entry_has_no_diagnostics() {
    // 老入口走的是丢诊断的路径 —— 这不是 bug，是本层刻意的分档。
    let doc = Doc::read(DUP_KEY).unwrap();
    assert!(!doc.has_diagnostics());
    assert!(doc.diagnostics().is_empty());
}

#[test]
fn read_with_also_carries_diagnostics() {
    // `read_with` 转发的是 `parse_with`（返回 ParseOutput），诊断顺带就有。
    let doc = Doc::read_with(DUP_KEY, smlkit_easy::Contracts::On).unwrap();
    assert_eq!(doc.diagnostics().len(), 1);
}

#[test]
fn report_file_entry_expands_include_and_keeps_diagnostics() {
    let dir = std::env::temp_dir().join(format!("smlkit_easy_report_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sub.sml"), DUP_KEY).unwrap();
    let main = dir.join("main.sml");
    std::fs::write(&main, "include \"sub.sml\"\nroot: 1\n").unwrap();

    let doc = Doc::read_file_with_report(&main).unwrap();
    assert_eq!(doc.int("root"), Some(1), "include 应展开");
    assert_eq!(doc.diagnostics().len(), 1, "子文件里的重复键也要报");

    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(feature = "emit-markdown")]
#[test]
fn out_markdown_shoots_a_document() {
    let doc = Doc::read("h1 { text: \"标题\" }\np { text: \"正文\" }").unwrap();
    let md = smlkit_easy::out::markdown(&doc).unwrap();
    assert!(md.contains("# 标题"), "markdown 输出异常: {md}");
    assert!(md.contains("正文"), "markdown 输出异常: {md}");
}

#[cfg(feature = "emit-html")]
#[test]
fn out_html_shoots_a_document() {
    let doc = Doc::read("p { text: \"正文\" }").unwrap();
    let html = smlkit_easy::out::html(&doc).unwrap();
    assert!(html.contains("<!DOCTYPE html>"), "应为完整文档: {html}");
    assert!(html.contains("正文"), "html 输出异常: {html}");
}
