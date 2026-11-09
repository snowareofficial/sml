// SPDX-License-Identifier: MulanPSL-2.0
//! 基准共用件：计时器 + **峰值堆**计数 + 表格输出。
//!
//! 为什么值层与容器层的基准单独开文件（`value.rs` / `amberice.rs`），而这个模块只放共用件：
//! `perf.rs` 的历史基线已入库，**一个字节都不改**（改了就没法跟 `docs/swsml/ref/perf-baseline.md` 比）；
//! 新两层共用同一套口径（release、5 轮取最小、同时报吞吐与峰值堆），故抽到这里。
//! ｜ Shared harness for the new benches; `perf.rs` stays untouched so its baseline remains comparable.
//!
//! 峰值堆怎么来的：全局计数分配器维护 `LIVE`（当前存活字节）与 `PEAK`（高水位）。
//! 每次测量前把 `PEAK` 重置成当前 `LIVE`，测完 `PEAK - LIVE₀` 就是这次操作真正占用的堆峰值。
//! 只报「分配总量」会把「反复分配又释放」算成内存，那是错的 —— 这张表的核心卖点是内存。
//! ｜ Peak (not total) allocation: the metric that makes the mmap / zero-copy claims checkable.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

struct Counting;

#[inline]
fn note(size: usize) {
    let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
    // 高水位：只在真的更高时写回（CAS 循环避免两个线程互相覆盖；基准是单线程，代价可忽略）
    let mut peak = PEAK.load(Ordering::Relaxed);
    while live > peak {
        match PEAK.compare_exchange_weak(peak, live, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(p) => peak = p,
        }
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        note(l.size());
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
        note(new_size);
        System.realloc(p, l, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// 每项测量的轮数（与 `perf.rs` 同口径：取最小值）。
pub const ROUNDS: u32 = 5;

/// 一次操作的内存画像。
#[derive(Debug, Clone, Copy, Default)]
pub struct Mem {
    /// 峰值堆增量（字节）
    pub peak: usize,
    /// 分配次数
    pub allocs: usize,
}

/// 测一次 `f` 的峰值堆与分配次数。
pub fn measure<F: FnOnce()>(f: F) -> Mem {
    let live0 = LIVE.load(Ordering::Relaxed);
    PEAK.store(live0, Ordering::Relaxed);
    let a0 = ALLOCS.load(Ordering::Relaxed);
    f();
    Mem {
        peak: PEAK.load(Ordering::Relaxed).saturating_sub(live0),
        allocs: ALLOCS.load(Ordering::Relaxed) - a0,
    }
}

/// `--quick`：迭代数减半，冒烟用。
pub fn quick() -> bool {
    std::env::args().any(|a| a == "--quick")
}

pub fn iters(base: u64, quick: bool) -> u64 {
    if quick {
        (base / 2).max(1)
    } else {
        base
    }
}

/// 跑 `iters` 次、重复 [`ROUNDS`] 轮，返回**单次**耗时（秒），取各轮最小值。
pub fn bench<F: FnMut()>(iters: u64, mut f: F) -> f64 {
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

/// 表头（两个 bench 共用一套列，方便并排抄进文档）。
pub fn header(title: &str) {
    println!("# {title}");
    println!();
    println!(
        "- 构建档位：{}",
        if cfg!(debug_assertions) {
            "debug（数字没有参考价值，请用 `cargo bench`）"
        } else {
            "release / bench profile"
        }
    );
    println!("- 口径：{} 轮取最小值；峰值堆 = 该次操作的堆高水位增量", ROUNDS);
    println!();
    println!("| 负载 | 规模 | 操作 | ns/op | 吞吐 | 峰值堆 | allocs |");
    println!("|---|---|---|---|---|---|---|");
}

/// 一行结果。`bytes_per_op` 用于把「每次操作处理多少输入字节」换算成 MB/s。
pub fn row(label: &str, size: &str, op: &str, per_op: f64, bytes_per_op: usize, mem: Mem) {
    let ns = per_op * 1e9;
    let rate = if per_op > 0.0 && bytes_per_op > 0 {
        format!(
            "{:.1} MB/s",
            bytes_per_op as f64 / per_op / (1024.0 * 1024.0)
        )
    } else {
        "—".to_string()
    };
    println!(
        "| `{label}` | {size} | `{op}` | {ns:.0} | {rate} | {} | {} |",
        human_bytes(mem.peak as f64),
        mem.allocs
    );
}

/// 只报内存、不报耗时的行（用于 `mmap` 这类「时间不是卖点」的判据）。
#[allow(dead_code)] // 只有开了 `amberice-mmap` 的 bench 用它
pub fn row_mem(label: &str, size: &str, op: &str, mem: Mem) {
    println!(
        "| `{label}` | {size} | `{op}` | — | — | {} | {} |",
        human_bytes(mem.peak as f64),
        mem.allocs
    );
}

pub fn human_bytes(b: f64) -> String {
    if b >= 1024.0 * 1024.0 {
        format!("{:.1} MiB", b / 1024.0 / 1024.0)
    } else if b >= 1024.0 {
        format!("{:.1} KiB", b / 1024.0)
    } else {
        format!("{b:.0} B")
    }
}

// ---------------------------------------------------------------------------
// 负载构造：三层（小 / 中 / 深），两层基准共用，保证「同一份输入」可比
// ---------------------------------------------------------------------------

/// 10 个键的配置片段。
pub fn build_small() -> String {
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

/// 中等文档：`n` 行长键值（默认 4000 行，约 100 KB），代表「真实配置文件」。
pub fn build_flat(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n {
        s.push_str(&format!("k{i}: value-{i}\n"));
    }
    s
}

/// 深文档：`n` 层嵌套块（默认 120 层，`MAX_VALUE_DEPTH` 是 128）。
#[allow(dead_code)] // 只有 value bench 用它
pub fn build_deep(n: usize) -> String {
    let mut s = String::new();
    for i in 0..n {
        for _ in 0..i {
            s.push_str("  ");
        }
        s.push_str(&format!("k{i} {{\n"));
    }
    for i in (0..n).rev() {
        for _ in 0..i {
            s.push_str("  ");
        }
        s.push_str("}\n");
    }
    s
}

/// 深文档对应的取值路径（`k0.k1.…`）。
#[allow(dead_code)] // 只有 value bench 用它
pub fn deep_path(n: usize) -> String {
    (0..n).map(|i| format!("k{i}")).collect::<Vec<_>>().join(".")
}
