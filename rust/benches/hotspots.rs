//! 函数级/阶段级热点归因：用**不同文档形状的差分**拆出「词法 / 结构构建 / 契约 / 数组」的成本。
//!
//! 为什么不用 profiler：本仓不引 dev 依赖，`perf` 只报总量。而阶段占比可以用**控制变量**得到：
//! 同一规模下换一种写法，两两相减就是那段语法的净成本。
//!
//! 为什么每个形状跑多批：本机 bench 漂移实测可达 ±8%（同一份代码两次 3.70 / 3.90 ms），
//! 单次测量无法比较。这里每形状跑 `BATCHES` 批，**同时给出 min 与 median** ——
//! 两者差距大就说明当轮环境不稳，数字不能用来做 A/B。
//!
//! 用法：`cargo bench --bench hotspots`

mod common;

use std::hint::black_box;
use std::time::Instant;

/// 每形状的批数。取 9：min 抗抖动，median 看典型值。
const BATCHES: usize = 9;

/// 每批内重复次数（够到毫秒级，又不至于让一轮跑太久）。
const REPS: usize = 5;

fn sample<F: FnMut()>(mut f: F) -> (f64, f64) {
    // 热身：让惰性初始化与 CPU 频率先稳定。
    f();
    let mut v = Vec::with_capacity(BATCHES);
    for _ in 0..BATCHES {
        let t = Instant::now();
        for _ in 0..REPS {
            f();
        }
        v.push(t.elapsed().as_nanos() as f64 / REPS as f64);
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[0], v[v.len() / 2])
}

/// 基线形状：`k0: v0` —— 只有词法 + 标量 coerce + 一次对象插入。
fn scalars(n: usize) -> String {
    let mut s = String::with_capacity(n * 16);
    for i in 0..n {
        s.push_str(&format!("k{i}: v{i}\n"));
    }
    s
}

/// 块形状：每项是一个只有 1 个字段的块 —— 相比基线多的就是「块帧 + __type 之类」。
fn blocks(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n {
        s.push_str(&format!("b{i} {{ x: 1 }}\n"));
    }
    s
}

/// 数组形状：每项一个 5 元素数组 —— 相比基线多的是数组构建。
fn arrays(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n {
        s.push_str(&format!("a{i}: [ 1 2 3 4 5 ]\n"));
    }
    s
}

/// 契约形状：声明一份契约，然后每个块 `@is` 应用一次 —— 相比块形状多的是契约校验/回填。
fn with_contract(n: usize) -> String {
    let mut s = String::from("@contract C {\n    a: int\n    b: str\n}\n");
    s.reserve(n * 32);
    for i in 0..n {
        s.push_str(&format!("c{i} {{ @is C a: {i} b: s }}\n"));
    }
    s
}

/// 特性形状：头部带 `@version` + `@feature` —— 相比基线多的是特性扫描与版本求交。
fn with_features(n: usize) -> String {
    let mut s = String::from("@version v1\n@feature enable when, typed-block\n");
    s.push_str(&scalars(n));
    s
}

fn main() {
    let quick = common::quick();
    let n = if quick { 1000 } else { 4000 };

    println!("# 阶段归因（多批取 min / median）\n");
    println!(
        "- 口径：每形状 {} 批 × {} 次；min 抗抖动，median 看典型值\n- 机器漂移参考：两者差距 >10% 时本轮数字不可用于 A/B\n",
        BATCHES, REPS
    );
    println!("| 形状 | 规模 | tokenize min | parse min | parse median | min/median 差 | 构建段（parse−tokenize） |");
    println!("|---|---|---|---|---|---|---|");

    let shapes: Vec<(&str, String)> = vec![
        ("标量（基线）", scalars(n)),
        ("块 ×1 字段", blocks(n)),
        ("数组 ×5 元素", arrays(n)),
        ("契约 @is", with_contract(n)),
        ("带 @feature", with_features(n)),
    ];

    for (label, text) in &shapes {
        let size = format!("{:.1} KiB", text.len() as f64 / 1024.0);
        let (tk_min, _) = sample(|| {
            let _ = black_box(sml_lex::tokenize(black_box(text.as_str())));
        });
        let (p_min, p_med) = sample(|| {
            let _ = black_box(sml_parse::parse(black_box(text.as_str())));
        });
        let drift = if p_min > 0.0 {
            (p_med - p_min) / p_min * 100.0
        } else {
            0.0
        };
        println!(
            "| {} | {} | {:.0} | {:.0} | {:.0} | {:+.1}% | {:.0}（{:.0}%） |",
            label,
            size,
            tk_min,
            p_min,
            p_med,
            drift,
            p_min - tk_min,
            (p_min - tk_min) / p_min * 100.0
        );
    }

    println!("\n> 读法：`构建段` = parse 减 tokenize，含 coerce、对象/数组插入、契约校验。");
    println!("> 与「标量（基线）」那行相减，即该形状**额外**引入的成本。");
}
