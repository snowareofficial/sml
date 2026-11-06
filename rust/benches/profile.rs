// SPDX-License-Identifier: MulanPSL-2.0

//! 阶段 0 归因基准：补三条原基准没测到的行。
//! Attribution bench: the three measurements the historical baselines lack.
//!
//! 为什么不改 `benches/perf.rs`：那份是 `docs/swsml/ref/perf-baseline.md` 的历史基线，
//! **一个字节都不改**才能与既有数字并排比较。这里新开一份，复用 `common` 的口径。
//! ｜ `perf.rs` stays untouched so its numbers remain comparable; this file adds rows only.
//!
//! ```text
//! cargo bench --bench profile                                  # 前两组
//! cargo bench --bench profile --features amberice               # + get×1000
//! cargo bench --bench profile --features amberice -- --quick   # 冒烟
//! ```

mod common;

fn main() {
    let quick = common::quick();
    common::header("归因基准（parse 峰值堆 / tokenize 时间 / get×1000）");

    let cases: [(&str, String, u64); 3] = [
        ("small", common::build_small(), 2_000),
        ("flat@4000", common::build_flat(4000), 40),
        ("deep@120", common::build_deep(120), 300),
    ];

    // 1) parse 峰值堆：原 perf.rs 只有 allocs / 总字节，没有高水位。
    for (label, text, base) in &cases {
        let n = common::iters(*base, quick);
        let t = common::bench(n, || {
            let _ = std::hint::black_box(sml::parse(text));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(sml::parse(text));
        });
        common::row(
            label,
            &common::human_bytes(text.len() as f64),
            "parse",
            t,
            text.len(),
            mem,
        );
    }

    // 2) tokenize 时间：原基准只报了它的分配数（2.8/键），没有耗时占比。
    //    SIMD 词法值不值得做，全靠这一行。
    for (label, text, base) in &cases {
        let n = common::iters(*base * 2, quick);
        let t = common::bench(n, || {
            let _ = std::hint::black_box(sml_lex::tokenize(text));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(sml_lex::tokenize(text));
        });
        common::row(
            label,
            &common::human_bytes(text.len() as f64),
            "tokenize",
            t,
            text.len(),
            mem,
        );
    }

    // 3) 同键取 1000 次，跨 1k / 4k / 16k 键三种规模，**取中间那个键**。
    //    - allocs / 峰值堆不随规模增长 ⇒ 证明「每次 get 重扫 STRTAB」已被 A1 去掉；
    //    - 耗时若仍随规模增长，那是 `subtree_at` 的**兄弟比较 O(兄弟数)**（已知取舍，
    //      `DIR` 段将来要省的就是它），不是表重扫。两件事别混。
    #[cfg(feature = "amberice")]
    {
        const PER_OP: u64 = 1000;
        for nkeys in [1_000usize, 4_000, 16_000] {
            let text = common::build_flat(nkeys);
            let v = sml::parse_ordered(&text).expect("flat 必须能解析");
            let cv = sml::CanonicalValue::try_from(&v).expect("flat 必须有序");
            let file = amberice::dump(&cv).expect("dump 失败");
            let store = amberice::Store::open(&file).expect("Store::open 失败");
            let key = format!("k{}", nkeys / 2);
            // 预热：首次 get 才建 STRTAB 表，别把这一次性的钱算进稳态。
            let _ = store.get(&key).expect("预热失败");

            let n = common::iters(20, quick);
            let t = common::bench(n, || {
                for _ in 0..PER_OP {
                    let _ = std::hint::black_box(store.get(&key).expect("get 失败"));
                }
            });
            let mem = common::measure(|| {
                for _ in 0..PER_OP {
                    let _ = std::hint::black_box(store.get(&key).expect("get 失败"));
                }
            });
            common::row(
                &format!("flat@{nkeys}"),
                &common::human_bytes(file.len() as f64),
                "get×1000 均摊",
                t / PER_OP as f64,
                0,
                mem,
            );

            // N05 判据：同一份值带 `DIR` 时，中键 get 应当从 O(兄弟数) 变成 O(log n)。
            // ｜ Same file with the optional `DIR` index: sibling scan → binary search.
            let dir_opts = amberice::DumpOptions {
                dir: true,
                ..Default::default()
            };
            let file_dir = amberice::dump_with(&cv, &dir_opts).expect("dump(DIR) 失败");
            let store_dir = amberice::Store::open(&file_dir).expect("Store::open(DIR) 失败");
            let _ = store_dir.get(&key).expect("预热失败");
            let t = common::bench(n, || {
                for _ in 0..PER_OP {
                    let _ = std::hint::black_box(store_dir.get(&key).expect("get 失败"));
                }
            });
            let mem = common::measure(|| {
                for _ in 0..PER_OP {
                    let _ = std::hint::black_box(store_dir.get(&key).expect("get 失败"));
                }
            });
            common::row(
                &format!("flat@{nkeys}"),
                &common::human_bytes(file_dir.len() as f64),
                "get×1000 均摊（DIR）",
                t / PER_OP as f64,
                0,
                mem,
            );
        }
    }
    #[cfg(not(feature = "amberice"))]
    println!("\n> `get×1000` 组已跳过：加 `--features amberice` 才有容器层。");

    println!();
    println!("> `get×1000 均摊` 的 ns/op 是**单次 get**（整批 1000 次除以 1000），取的是中间键；");
    println!("> 该行的峰值堆是**整批 1000 次**的高水位，稳态下应不随规模增长。");
    println!("> 带 `--features amberice` 时会连带打开 `preserve_order`：本表的 parse 数字是");
    println!("> **保序档**，默认档请看 `benches/perf.rs`（两者不可直接相减）。");
}
