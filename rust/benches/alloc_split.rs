//! 分配归因：把一次 `parse` / `tokenize` 的分配按**大小桶**拆开。
//!
//! 为什么需要它：`perf` / `profile` 只报「总分配次数与字节数」（`large` 6592 allocs /
//! 3.3 MB），但「该优化短串内联，还是该优化容器节点」取决于**分配的大小分布**：
//!   - 大量 ≤ 32 B 的分配 ⇒ 键名 / 小标量为主，收益在「短串内联」；
//!   - 大量 64–256 B 的分配 ⇒ 容器节点 / 数组缓冲为主，收益在「预分配 / 换容器」。
//! 猜错方向会让优化白做，所以先量。
//!
//! 本 bench **不复用** `benches/common/mod.rs`：那里已经装了一个全局计数分配器，
//! 再定义一个会冲突；而改它会污染所有既有基线数字（每次分配多几次原子操作）。
//! 所以这里自带分配器与负载生成，与基线完全隔离。
//!
//! 用法：`cargo bench --bench alloc_split`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static TOTAL: AtomicUsize = AtomicUsize::new(0);
static B32: AtomicUsize = AtomicUsize::new(0);
static B64: AtomicUsize = AtomicUsize::new(0);
static B128: AtomicUsize = AtomicUsize::new(0);
static B256: AtomicUsize = AtomicUsize::new(0);
static BIG: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record(new_size);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[inline]
fn record(n: usize) {
    TOTAL.fetch_add(1, Ordering::Relaxed);
    let bucket = if n <= 32 {
        &B32
    } else if n <= 64 {
        &B64
    } else if n <= 128 {
        &B128
    } else if n <= 256 {
        &B256
    } else {
        &BIG
    };
    bucket.fetch_add(1, Ordering::Relaxed);
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn reset() {
    for c in [&TOTAL, &B32, &B64, &B128, &B256, &BIG] {
        c.store(0, Ordering::Relaxed);
    }
}

/// 与 `benches/common/mod.rs::build_flat` 同构（刻意各写一份，避免共享分配器）。
fn build_flat(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n {
        s.push_str(&format!("k{i}: value-{i}\n"));
    }
    s
}

fn row(name: &str, size: &str, total: usize) {
    let (a, b, c, d, e) = (
        B32.load(Ordering::Relaxed),
        B64.load(Ordering::Relaxed),
        B128.load(Ordering::Relaxed),
        B256.load(Ordering::Relaxed),
        BIG.load(Ordering::Relaxed),
    );
    let pct = |x: usize| {
        if total == 0 {
            "—".to_string()
        } else {
            format!("{:.0}%", x as f64 * 100.0 / total as f64)
        }
    };
    println!(
        "| `{}` | {} | {} | {} ({}) | {} ({}) | {} ({}) | {} ({}) | {} ({}) |",
        name,
        size,
        total,
        a,
        pct(a),
        b,
        pct(b),
        c,
        pct(c),
        d,
        pct(d),
        e,
        pct(e)
    );
}

fn main() {
    println!("# 分配归因（按大小桶）\n");
    println!("- 口径：`System` 之上加原子计数；每项热身一次后清零，再跑一轮\n");
    println!("| 负载 | 输入 | allocs | ≤32 B | ≤64 B | ≤128 B | ≤256 B | >256 B |");
    println!("|---|---|---|---|---|---|---|---|");

    for &n in &[10usize, 4000] {
        let text = build_flat(n);
        let size = format!("{:.1} KiB", text.len() as f64 / 1024.0);

        let _ = sml_parse::parse(&text);
        reset();
        let _ = sml_parse::parse(&text);
        row(&format!("parse flat@{}", n), &size, TOTAL.load(Ordering::Relaxed));

        let _ = sml_parse::tokenize(&text);
        reset();
        let _ = sml_parse::tokenize(&text);
        row(
            &format!("tokenize flat@{}", n),
            &size,
            TOTAL.load(Ordering::Relaxed),
        );

        if n == 4000 {
            let v = sml_parse::parse(&text).expect("parse");
            let _ = sml_value::to_sml(&v);
            reset();
            let _ = sml_value::to_sml(&v);
            row("to_sml flat@4000", &size, TOTAL.load(Ordering::Relaxed));
        }
    }

    println!("\n> 判读：≤32 B 占绝大多数 ⇒ 键名 / 小标量；128–256 B 居多 ⇒ 容器节点或缓冲。");
}
