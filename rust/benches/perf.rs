// SPDX-License-Identifier: MulanPSL-2.0
//! 性能基准（**零依赖**手写计时器，不引 criterion）。
//!
//! 为什么不用 criterion：它是 dev-dependency，会进入 CI 构建图，而本仓库的纪律是
//! 「dev 依赖也锁到已验证的小版本下限」；更要紧的是——**没有基准才是当前的病灶**
//! （IDEAS I-23）。先用最少的机制把数字量出来，等真要做 A/B 再谈要不要上统计工具。
//!
//! 用法：
//! ```text
//! cargo bench --bench perf              # 默认口径
//! cargo bench --bench perf -- --quick   # 迭代数减半（冒烟用）
//! ```
//!
//! 输出是给人看的表格 + 可直接抄进 `docs/perf-baseline.md` 的数值（ns/op、MB/s）。
//!
//! ⚠️ 三组负载覆盖的是**三种不同形状**，别只看一个数：
//! - `small`   —— 10 个键，代表「配置片段」；固定开销（入口组装、tokenize 启动）占比最高
//! - `large`   —— 约 35 万字，代表「长篇文档」（`story/` 那份压测稿的量级）
//! - `deep`    —— 120 层嵌套（`MAX_VALUE_DEPTH` 是 128），代表「深度路径」
//!
//! 每项都按 `ROUNDS` 轮跑、取**最小值**（最小值受调度抖动影响最小；这里不追求置信区间）。

use std::time::Instant;

/// 每项测量的轮数与每轮迭代数。`--quick` 减半，用于冒烟。
const ROUNDS: u32 = 5;

fn main() {
    let quick = std::env::args().any(|a| a == "--quick");

    let small = build_small();
    let large = build_large();
    let deep = build_deep();

    println!("# SML 性能基准");
    println!();
    println!(
        "- crate 版本：{}（{} {}）",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "- 构建档位：{}",
        if cfg!(debug_assertions) {
            "debug（⚠️ 数字没有参考价值，请用 `cargo bench`）"
        } else {
            "release / bench profile"
        }
    );
    println!("- 口径：{} 轮取最小值；`--quick` = 迭代数减半", ROUNDS);
    println!();
    println!("| 负载 | 输入规模 | 操作 | ns/op | 吞吐 |");
    println!("|---|---|---|---|---|");

    // —— 小文档：parse ——
    let n_small = iters(20_000, quick);
    let t = bench(n_small, || {
        let v = sml::parse(&small);
        debug_assert!(v.is_ok());
    });
    row("small", &small, "parse", t, n_small);

    // —— 小文档：to_sml（往返的一半）——
    let vsmall = sml::parse(&small).expect("small 必须能解析");
    let t = bench(n_small, || {
        let s = sml::to_sml(&vsmall);
        debug_assert!(!s.is_empty());
    });
    row("small", &small, "to_sml", t, n_small);

    // —— 大文档：parse ——
    let n_large = iters(20, quick);
    let t = bench(n_large, || {
        let v = sml::parse(&large);
        debug_assert!(v.is_ok());
    });
    row("large", &large, "parse", t, n_large);

    // —— 大文档：to_sml ——
    let vlarge = sml::parse(&large).expect("large 必须能解析");
    let t = bench(n_large, || {
        let s = sml::to_sml(&vlarge);
        debug_assert!(!s.is_empty());
    });
    row("large", &large, "to_sml", t, n_large);

    // —— 深嵌套：parse ——
    let n_deep = iters(2_000, quick);
    let t = bench(n_deep, || {
        let v = sml::parse(&deep);
        debug_assert!(v.is_ok());
    });
    row("deep", &deep, "parse", t, n_deep);

    println!();
    println!("> `MB/s` 按**输入字节数**算（`to_sml` 行同样是输入侧字节），便于横向比较。");
}

fn iters(base: u64, quick: bool) -> u64 {
    if quick {
        (base / 2).max(1)
    } else {
        base
    }
}

/// 跑 `iters` 次、重复 `ROUNDS` 轮，返回**单次**耗时（秒），取各轮最小值。
fn bench<F: FnMut()>(iters: u64, mut f: F) -> f64 {
    // 预热：让首次的惰性初始化（比如正则/表构建）不落在计时窗口里
    for _ in 0..(iters / 10).max(1) {
        f();
    }
    let mut best = f64::INFINITY;
    for _ in 0..ROUNDS {
        let t0 = Instant::now();
        for _ in 0..iters {
            f();
        }
        let per = t0.elapsed().as_secs_f64() / iters as f64;
        if per < best {
            best = per;
        }
    }
    best
}

fn row(label: &str, input: &str, op: &str, per_sec: f64, _iters: u64) {
    let bytes = input.len() as f64;
    let ns = per_sec * 1e9;
    let mbps = if per_sec > 0.0 {
        bytes / per_sec / (1024.0 * 1024.0)
    } else {
        0.0
    };
    println!(
        "| `{label}` | {} | `{op}` | {:.0} | {:.1} MB/s |",
        human_bytes(bytes),
        ns,
        mbps
    );
}

fn human_bytes(b: f64) -> String {
    if b >= 1024.0 * 1024.0 {
        format!("{:.1} MiB", b / 1024.0 / 1024.0)
    } else if b >= 1024.0 {
        format!("{:.1} KiB", b / 1024.0)
    } else {
        format!("{} B", b)
    }
}

/// 10 个键的小配置片段（含注释、嵌套一层、数组）。
fn build_small() -> String {
    let mut s = String::from("# 配置片段：10 个键\nserver {\n");
    for i in 0..7 {
        s.push_str(&format!("  key{i}: value-{i}\n"));
    }
    s.push_str("}\n");
    s.push_str("ports: [ 8080 8443 ]\n");
    s.push_str("debug: false\n");
    s.push_str("name: \"性能基准\"\n");
    s
}

/// 约 35 万字的长文档：重复「标题 + 段落 + 几个字段」的块，直到超过目标字符数。
///
/// 用固定内容生成而不是读 `story/` 的稿子：基准要**可复现**，不能依赖某份私有素材
/// 是否存在；字符数与 `story/` 那份（351,217 个非 ASCII 字符）同量级。
fn build_large() -> String {
    const TARGET_CHARS: usize = 350_000;
    let mut s = String::with_capacity(TARGET_CHARS * 2);
    let para = "他沿着结冰的河道向北走，风把雪粒压成细小的刃，割在脸上像一排排看不见的针。";
    let mut chapter = 1;
    while s.chars().count() < TARGET_CHARS {
        s.push_str(&format!("chapter{chapter} {{\n"));
        s.push_str(&format!("  title: \"第 {chapter} 章 · 星海回响\"\n"));
        s.push_str("  body: \"\"\"\n");
        for _ in 0..12 {
            s.push_str(para);
            s.push('\n');
        }
        s.push_str("  \"\"\"\n");
        s.push_str("  words: 2400\n");
        s.push_str("  done: false\n");
        s.push_str("}\n");
        chapter += 1;
    }
    s
}

/// 120 层嵌套块（`MAX_VALUE_DEPTH` = 128，留 8 层余量）。
fn build_deep() -> String {
    let depth = 120;
    let mut s = String::with_capacity(depth * 8);
    for _ in 0..depth {
        s.push_str("a { ");
    }
    s.push_str("leaf: 1");
    for _ in 0..depth {
        s.push_str(" }");
    }
    s.push('\n');
    s
}
