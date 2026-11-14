//! 解析深度与栈预算（回归护栏）。
//!
//! 为什么单独钉这两个断言：解析器是递归下降 + 显式深度栈，**帧大小**决定「128 层要多少栈」。
//! 曾经 `parse_block_inner` 是一个 ~558 行的巨函数，debug 构建下 128 层就能顶穿默认 2 MiB
//! 线程栈；把分支抽成 `parse_at_directive` 之后才安全。宿主不一定是 2 MiB —— 编辑器、
//! WASM、沙箱常给 64 KB~512 KB，所以这里用 **512 KB** 的线程跑满 128 层。
//!
//! 129 层必须报 `E-LIMIT-001`（与 C / C++ / JS / Lua 同码），而不是栈溢出或别的码。

use std::thread;

/// `n` 层嵌套块，最内层一个标量字段。
fn nested(n: usize) -> String {
    let mut s = String::new();
    for _ in 0..n {
        s.push_str("a {\n");
    }
    s.push_str("x: 1\n");
    for _ in 0..n {
        s.push_str("}\n");
    }
    s
}

#[test]
fn max_depth_parses_in_512k_stack() {
    let doc = nested(128);
    let h = thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(move || sml_parse::parse(&doc))
        .expect("spawn");
    let r = h.join().expect("join（栈溢出会让 join 拿到 None）");
    assert!(r.is_ok(), "128 层应解析成功，实际：{:?}", r.err().map(|e| e.code()));
}

#[test]
fn depth_129_reports_limit_code() {
    let doc = nested(129);
    let err = sml_parse::parse(&doc).expect_err("129 层应拒绝");
    assert_eq!(err.code(), "E-LIMIT-001", "129 层应报 E-LIMIT-001，实际：{}", err.code());
}
