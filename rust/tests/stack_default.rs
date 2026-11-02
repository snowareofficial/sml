//! TODO 雷 1 的根因验收：**默认栈线程**（`std::thread::spawn`，2 MB）解析深嵌套不得 abort。
//!
//! 历史上递归漏斗在 256 KB 探针栈下 128 层即崩；迭代式（显式栈）落地后原生栈深度恒定。
//! 这里刻意用 `std::thread::spawn`（默认栈），而不是 `_stack_probe.rs`（本地探针、改栈大小）。

/// 生成 N 层嵌套块：`a { a { … x: 1 … } }`。
fn nested_blocks(n: usize) -> String {
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

/// 128 层（上限内）在默认栈线程上必须**解析成功**。
#[test]
fn default_stack_thread_parses_128_levels() {
    let text = nested_blocks(128);
    let ok = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024) // 显式 2 MB：与 Rust 默认主线程同量级
        .spawn(move || sml::parse(&text).map(|_| ()))
        .unwrap()
        .join()
        .expect("默认栈线程不应 abort（栈溢出会直接 terminate）");
    assert!(ok.is_ok(), "128 层应在上限内解析成功：{ok:?}");
}

/// 200 层（超上限）必须**报 E-LIMIT-001**，而不是打穿栈。
#[test]
fn default_stack_thread_rejects_200_levels_with_limit_code() {
    let text = nested_blocks(200);
    let code = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(move || sml::parse(&text).map(|_| ()).map_err(|e| e.code()))
        .unwrap()
        .join()
        .expect("默认栈线程不应 abort（栈溢出会直接 terminate）");
    assert_eq!(
        code,
        Err("E-LIMIT-001"),
        "超上限应报 E-LIMIT-001 而非崩溃"
    );
}
