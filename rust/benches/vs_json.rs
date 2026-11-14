//! 与 `serde_json` 的逐项对拍 —— 回答「为什么比 serde_json 慢」。
//!
//! 同一份**内容**分别写成 SML 与 JSON，跑同一个形状的三种规模：
//!
//! | 形状 | SML | JSON |
//! |---|---|---|
//! | flat  | `k0: v0` 逐行 | `{"k0":"v0",...}` |
//! | block | `b0 { x: 1 y: s }` 逐行 | `{"b0":{"x":1,"y":"s"},...}` |
//!
//! 测三件事：**耗时**（交错多批取 min）、**分配次数**、**输入体积**。
//! 三者一起看才能判断差异来自「算法多做了事」还是「同事实做得更差」。
//!
//! 运行：`cargo bench --bench vs_json`

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// ---------------------------------------------------------------------------
// 计数分配器（各 bench 是独立二进制，这里自带一份，避免与 common 的冲突）
// ---------------------------------------------------------------------------
struct Counting;

static ALLOCS: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(l.size() as u64, Ordering::Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new as u64, Ordering::Relaxed);
        System.realloc(p, l, new)
    }
}

#[global_allocator]
static A: Counting = Counting;

fn snapshot() -> (u64, u64) {
    (ALLOCS.load(Ordering::Relaxed), BYTES.load(Ordering::Relaxed))
}

/// 跑一次、返回（耗时 ns, 分配次数, 分配字节）
fn once<F: FnOnce()>(f: F) -> (f64, u64, u64) {
    let a0 = snapshot();
    let t = Instant::now();
    f();
    let ns = t.elapsed().as_nanos() as f64;
    let a1 = snapshot();
    (ns, a1.0 - a0.0, a1.1 - a0.1)
}

fn main() {
    const BATCHES: usize = 11;

    let cases: Vec<(&str, usize, String, String)> = vec![
        ("flat@4000", 4000, flat_sml(4000), flat_json(4000)),
        ("block@2000", 2000, block_sml(2000), block_json(2000)),
    ];

    // 预热（避免首个案例吃到冷启动）
    for (_, _, s, j) in &cases {
        let _ = black_box(sml_parse::parse(black_box(s.as_str())));
        let _ = black_box(serde_json::from_str::<serde_json::Value>(black_box(j.as_str())));
    }

    println!("\n## vs serde_json（{BATCHES} 批交错，取 min）\n");
    println!("| 形状 | 输入 | serde_json | sml | 倍数 | json allocs | sml allocs | json B/op | sml B/op |");
    println!("|---|---|---|---|---|---|---|---|---|");

    for (label, _, sml_src, json_src) in &cases {
        let mut js: Vec<(f64, u64, u64)> = Vec::with_capacity(BATCHES);
        let mut sm: Vec<(f64, u64, u64)> = Vec::with_capacity(BATCHES);
        for _ in 0..BATCHES {
            // 交错：同一批里两个解析器各跑一次，减小机器漂移对「谁更快」的影响
            js.push(once(|| {
                let _ = black_box(serde_json::from_str::<serde_json::Value>(black_box(
                    json_src,
                )));
            }));
            sm.push(once(|| {
                let _ = black_box(sml_parse::parse(black_box(sml_src)));
            }));
        }
        let pick = |v: &mut Vec<(f64, u64, u64)>| {
            v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            v[0]
        };
        let j = pick(&mut js);
        let s = pick(&mut sm);
        let ratio = s.0 / j.0;
        let kib = |n: usize| n as f64 / 1024.0;
        println!(
            "| {} | {:.1}/{:.1} KiB | {:.2} ms | {:.2} ms | **{:.1}×** | {} | {} | {:.0} | {:.0} |",
            label,
            kib(sml_src.len()),
            kib(json_src.len()),
            j.0 / 1e6,
            s.0 / 1e6,
            ratio,
            j.1,
            s.1,
            j.2 as f64,
            s.2 as f64
        );

        // 词法单独计时：SML 是「先切 token 再建树」，serde_json 是一遍过。
        // 同时记它的分配 —— 若 tokenize 的分配很小，说明词法是 Cow 借用、真正吃内存的是建树。
        let mut tk: Vec<(f64, u64, u64)> = Vec::with_capacity(BATCHES);
        for _ in 0..BATCHES {
            tk.push(once(|| {
                let _ = black_box(sml_lex::tokenize(black_box(sml_src.as_str())));
            }));
        }
        tk.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let t0 = tk[0];
        println!(
            "| ↳ 其中 tokenize | — | — | {:.2} ms（{:.0}%）| — | {} | — | {:.0} | — |",
            t0.0 / 1e6,
            t0.0 / s.0 * 100.0,
            t0.1,
            t0.2 as f64
        );
        // 建树段＝总量 − 词法：这段才是与 serde_json 正面比的部分
        println!(
            "| ↳ 其中建树 | — | — | {:.2} ms | — | {} | {} | {:.0} | {:.0} |",
            (s.0 - t0.0) / 1e6,
            j.1,
            s.1 - t0.1,
            j.2 as f64,
            (s.2 - t0.2) as f64
        );
    }

    println!("\n注：两边的**值模型不同** —— sml 的 `Value` 是 tag + 内联负载，serde_json 的");
    println!("`Value` 也是；差异主要看「解析路径做了多少额外的事」，见 README/提交说明。\n");
}

// ---------------------------------------------------------------------------
// 负载
// ---------------------------------------------------------------------------
fn flat_sml(n: usize) -> String {
    let mut s = String::with_capacity(n * 20);
    for i in 0..n {
        s.push_str(&format!("k{i}: value-{i}\n"));
    }
    s
}

fn flat_json(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    s.push('{');
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("\"k{i}\":\"value-{i}\""));
    }
    s.push('}');
    s
}

fn block_sml(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n {
        s.push_str(&format!("b{i} {{ x: {i} y: s }}\n"));
    }
    s
}

fn block_json(n: usize) -> String {
    let mut s = String::with_capacity(n * 40);
    s.push('{');
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("\"b{i}\":{{\"x\":{i},\"y\":\"s\"}}"));
    }
    s.push('}');
    s
}
