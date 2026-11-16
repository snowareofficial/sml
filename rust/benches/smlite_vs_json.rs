//! smlite 与 serde_json 的对拍 —— 照 `vs_json.rs` 口径：11 批交错取 min，
//! 计数分配器，flat/block/array 三形状；耗时 / 分配次数 / 输入体积三列并排。
//! 运行：`cargo bench --bench smlite_vs_json`

use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

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
    let cases: [(&str, String, String); 3] = [
        ("flat@4000", flat_sml(4000), flat_json(4000)),
        ("block@2000", block_sml(2000), block_json(2000)),
        ("array@2000", array_sml(2000), array_json(2000)),
    ];

    for (_, s, j) in &cases {
        let _ = black_box(smlite::parse(black_box(s.as_bytes())));
        let _ = black_box(serde_json::from_str::<serde_json::Value>(black_box(j.as_str())));
    }

    println!("\n## smlite vs serde_json（{BATCHES} 批交错，取 min）\n");
    println!("| 形状 | 输入 | serde_json | smlite | 倍数 | json allocs | smlite allocs | json B/op | smlite B/op |");
    println!("|---|---|---|---|---|---|---|---|---|");

    for (label, sml_src, json_src) in &cases {
        let mut js = Vec::with_capacity(BATCHES);
        let mut sm = Vec::with_capacity(BATCHES);
        for _ in 0..BATCHES {
            js.push(once(|| {
                let _ = black_box(serde_json::from_str::<serde_json::Value>(black_box(
                    json_src.as_str(),
                )));
            }));
            sm.push(once(|| {
                let _ = black_box(smlite::parse(black_box(sml_src.as_bytes())));
            }));
        }
        let pick = |v: &mut Vec<(f64, u64, u64)>| {
            v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
            v[0]
        };
        let j = pick(&mut js);
        let s = pick(&mut sm);
        println!(
            "| {} | {:.1}/{:.1} KiB | {:.2} ms | {:.2} ms | **{:.2}×** | {} | {} | {:.0} | {:.0} |",
            label,
            sml_src.len() as f64 / 1024.0,
            json_src.len() as f64 / 1024.0,
            j.0 / 1e6,
            s.0 / 1e6,
            s.0 / j.0,
            j.1,
            s.1,
            j.2 as f64,
            s.2 as f64
        );
    }
    println!("\n注：倍数 < 1 表示 smlite 更快；数字为本机一次实测，不进正式基线。\n");
}

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

fn array_sml(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n {
        s.push_str(&format!("a{i}: [1 2 3 4 5]\n"));
    }
    s
}

fn array_json(n: usize) -> String {
    let mut s = String::with_capacity(n * 32);
    s.push('{');
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("\"a{i}\":[1,2,3,4,5]"));
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
