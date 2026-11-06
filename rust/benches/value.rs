// SPDX-License-Identifier: MulanPSL-2.0
//! 值层基准：`to_sml` / `canonical_text` / `canonical_bytes` / `Value::get` / `eq_ordered`。
//!
//! 为什么单独一层（HANDOFF §5.2）：值层是**解析与容器之间的公共底座**，
//! `parse`、`dump`、`smlb::load` 全都压在它上面。把这层的数字先拿到，
//! 后面两层的异常才有归因 —— 否则「`load` 慢」到底慢在解码还是慢在值层，说不清。
//!
//! 用法：
//! ```text
//! cargo bench --bench value
//! cargo bench --bench value -- --quick
//! ```
//!
//! ⚠️ 计时闭包里**不能用 `debug_assert!` 兜结果**：bench 档 `debug-assertions = false`，
//! 断言整句被删，纯读操作（`eq_ordered` / `Value::get`）会连带被优化掉 ——
//! 实测会得到 `0 ns/op`。故统一用 `std::hint::black_box` 把结果钉住。
//!
//! `MB/s` 一律按**输入侧字节数**算（`canonical_*` 那几行用的是源文本长度），
//! 这样同一份文档的四个操作可以横向比。
mod common;

use sml::Value;

fn main() {
    let quick = common::quick();
    common::header("值层基准（to_sml / canonical / get / eq_ordered）");

    let small_t = common::build_small();
    let flat_t = common::build_flat(4000);
    let deep_t = common::build_deep(120);

    // canonical 与 to_sml 都要求键序是值的显式事实（HANDOFF §3 第 4 条）⇒ 统一用 parse_ordered。
    let small = sml::parse_ordered(&small_t).expect("small 必须能解析");
    let flat = sml::parse_ordered(&flat_t).expect("flat 必须能解析");
    let deep = sml::parse_ordered(&deep_t).expect("deep 必须能解析");

    let cases: [(&str, &str, &Value, u64); 3] = [
        ("small", &small_t, &small, 2_000),
        ("flat@4000", &flat_t, &flat, 40),
        ("deep@120", &deep_t, &deep, 300),
    ];

    // —— to_sml（往返的一半，`parse` 的对手面）——
    for (label, text, v, base) in cases {
        let n = common::iters(base, quick);
        let t = common::bench(n, || {
            let _ = std::hint::black_box(sml::to_sml(v));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(sml::to_sml(v));
        });
        common::row(label, &human(text.len()), "to_sml", t, text.len(), mem);
    }

    // —— canonical_text：签名对象的字节形态（C 系列规则全在这条路上）——
    for (label, text, v, base) in cases {
        let n = common::iters(base, quick);
        let t = common::bench(n, || {
            let _ = std::hint::black_box(v.canonical_text().expect("有序值必须能写 canonical 文本"));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(v.canonical_text());
        });
        common::row(label, &human(text.len()), "canonical_text", t, text.len(), mem);
    }

    // —— canonical_bytes：容器 `VALUE` / `STRTAB` 段的来源 ——
    for (label, text, v, base) in cases {
        let n = common::iters(base, quick);
        let t = common::bench(n, || {
            let _ = std::hint::black_box(v.canonical_bytes().expect("有序值必须能写 canonical 字节"));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(v.canonical_bytes());
        });
        common::row(label, &human(text.len()), "canonical_bytes", t, text.len(), mem);
    }

    // —— eq_ordered：键序敏感的比较（`load` 的验收判据就是它）——
    for (label, text, v, base) in cases {
        let n = common::iters(base, quick);
        let t = common::bench(n, || {
            let _ = std::hint::black_box(v.eq_ordered(v));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(v.eq_ordered(v));
        });
        common::row(label, &human(text.len()), "eq_ordered", t, 0, mem);
    }

    // —— Value::get：深树 vs 宽树。深树考「每层一段路径」，宽树考「同层线性查找」——
    let deep_path = common::deep_path(120);
    let n = common::iters(20_000, quick);
    let t = common::bench(n, || {
        let _ = std::hint::black_box(deep.get(&deep_path));
    });
    let mem = common::measure(|| {
        let _ = std::hint::black_box(deep.get(&deep_path));
    });
    common::row("deep@120", "120 层", "Value::get(整条路径)", t, 0, mem);

    let n = common::iters(50_000, quick);
    let t = common::bench(n, || {
        let _ = std::hint::black_box(flat.get("k2000"));
    });
    let mem = common::measure(|| {
        let _ = std::hint::black_box(flat.get("k2000"));
    });
    common::row("flat@4000", "4000 键", "Value::get(命中)", t, 0, mem);

    // 未命中：宽树的线性查找要扫完整层，这条是「最坏情况」的数字。
    let t = common::bench(n, || {
        let _ = std::hint::black_box(flat.get("不存在"));
    });
    let mem = common::measure(|| {
        let _ = std::hint::black_box(flat.get("不存在"));
    });
    common::row("flat@4000", "4000 键", "Value::get(未命中)", t, 0, mem);

    println!();
    println!("> `MB/s` 按输入字节算；`eq_ordered` / `get` 的时间与字节数不成比例，故不报吞吐。");
    println!("> 峰值堆按**一次操作**计：`canonical_bytes` 的峰值 ≈ 编码结果本身，`get` 只解命中那棵子树。");
}

fn human(bytes: usize) -> String {
    common::human_bytes(bytes as f64)
}
