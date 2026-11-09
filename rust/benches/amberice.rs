// SPDX-License-Identifier: MulanPSL-2.0
//! 容器层基准：`dump` / `load` / `pack` / `Store::get` / `Writer` / 解密 / mmap。
//!
//! 为什么与值层分开（HANDOFF §5.2/§5.5）：容器层指标自成一套（吞吐 + **峰值堆**），
//! 而且它的卖点就是内存 —— 只报速度等于没测。合并成一个总分则出问题定位不到。
//!
//! 用法：
//! ```text
//! cargo bench --bench amberice --features amberice                                # 基础组
//! cargo bench --bench amberice --features "amberice-compress"                     # + zstd
//! cargo bench --bench amberice --features "amberice-compress,amberice-crypto,amberice-mmap"  # 全量
//! ```
//!
//! 同 `value.rs`：计时闭包里不能用 `debug_assert!`（bench 档会整句删掉，
//! 纯读操作连带被优化成 0 ns），结果一律过 `std::hint::black_box`。
//!
//! 可比性：`MB/s` 按**文件字节数**算（`dump` 那几行按值的规范字节数），
//! 峰值堆按**一次操作**计。
mod common;

use sml::CanonicalValue;
use amberice::{DumpOptions, SEG_STRTAB, SEG_VALUE};

fn main() {
    let quick = common::quick();
    common::header("容器层基准（dump / load / pack / get / Writer / 解密）");

    // —— 输入：真实形状的中等文档（4000 键 ≈ 68 KB）——
    let flat_t = common::build_flat(4000);
    let small_t = common::build_small();
    let small_v = sml::parse_ordered(&small_t).expect("small 必须能解析");
    let flat_v = sml::parse_ordered(&flat_t).expect("flat 必须能解析");
    let small_cv = CanonicalValue::try_from(&small_v).expect("small 必须有序");
    let flat_cv = CanonicalValue::try_from(&flat_v).expect("flat 必须有序");

    let small_file = amberice::dump(&small_cv).expect("dump 失败");
    let flat_file = amberice::dump(&flat_cv).expect("dump 失败");

    // —— dump：值 → 文件（含组头、段表、摘要、manifest）——
    for (label, cv, text_len, base) in [
        ("small", &small_cv, small_t.len(), 1_000u64),
        ("flat@4000", &flat_cv, flat_t.len(), 30),
    ] {
        let n = common::iters(base, quick);
        let t = common::bench(n, || {
            let _ = std::hint::black_box(amberice::dump(cv).expect("dump 失败"));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(amberice::dump(cv));
        });
        common::row(label, &human(text_len), "dump", t, text_len, mem);
    }

    // —— load：文件 → 值（整树解码）——
    for (label, file, base) in [
        ("small", &small_file, 1_000u64),
        ("flat@4000", &flat_file, 30),
    ] {
        let n = common::iters(base, quick);
        let t = common::bench(n, || {
            let _ = std::hint::black_box(amberice::load(file).expect("load 失败"));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(amberice::load(file));
        });
        common::row(label, &human(file.len()), "load(整树)", t, file.len(), mem);
    }

    // —— Store::get：打开只解析头 + 段表 + manifest，取值只解命中的子树 ——
    let store = amberice::Store::open(&flat_file).expect("Store::open 失败");
    let n = common::iters(20_000, quick);
    let t = common::bench(n, || {
        let _ = std::hint::black_box(store.get("k2000").expect("get 失败"));
    });
    let mem = common::measure(|| {
        let _ = std::hint::black_box(store.get("k2000"));
    });
    common::row("flat@4000", &human(flat_file.len()), "Store::get(命中)", t, 0, mem);

    // 对照：同一路径走 `load` + `get_path`（整树解码）—— 差距就是 T3 的全部价值。
    let t = common::bench(common::iters(30, quick), || {
        let _ = std::hint::black_box(amberice::get(&flat_file, "k2000").expect("get 失败"));
    });
    let mem = common::measure(|| {
        let _ = std::hint::black_box(amberice::get(&flat_file, "k2000"));
    });
    common::row("flat@4000", &human(flat_file.len()), "load+get_path(对照)", t, 0, mem);

    // —— pack：四个 64 KiB 载荷（归档场景）——
    let payloads: Vec<(String, Vec<u8>)> = (0..4)
        .map(|i| {
            let body = format!("payload-{i}: {}\n", "x".repeat(64 * 1024));
            (format!("p{i}.txt"), body.into_bytes())
        })
        .collect();
    let total: usize = payloads.iter().map(|(_, b)| b.len()).sum();
    let items = || {
        payloads
            .iter()
            .map(|(n, b)| (n.as_str(), &b[..]))
            .collect::<Vec<_>>()
    };

    let n = common::iters(30, quick);
    let t = common::bench(n, || {
        let _ = std::hint::black_box(amberice::pack_from_readers(items()).expect("pack 失败"));
    });
    let mem = common::measure(|| {
        let _ = std::hint::black_box(amberice::pack_from_readers(items()));
    });
    common::row("pack@4x64KiB", &human(total), "pack_from_readers(不压)", t, total, mem);

    // —— Writer：流式分段写（段体走临时文件，不进堆；峰值堆 = 输出缓冲 + manifest）——
    let blocks = flat_cv.canonical_blocks().expect("canonical_blocks 失败");
    let write_stream = || {
        let mut buf: Vec<u8> = Vec::new();
        let mut w =
            amberice::Writer::new(&mut buf, &DumpOptions::default(), SEG_VALUE).expect("Writer::new");
        w.add_segment_reader(SEG_VALUE, 0, 0, &blocks.value[..])
            .expect("add VALUE");
        if !blocks.strtab.is_empty() {
            w.add_segment_reader(SEG_STRTAB, 0, 0, &blocks.strtab[..])
                .expect("add STRTAB");
        }
        w.finish().expect("finish");
        buf
    };
    let t = common::bench(common::iters(20, quick), || {
        let _ = std::hint::black_box(write_stream());
    });
    let mem = common::measure(|| {
        let _ = std::hint::black_box(write_stream());
    });
    common::row(
        "flat@4000",
        &human(flat_file.len()),
        "Writer(流式)",
        t,
        flat_file.len(),
        mem,
    );

    // —— zstd（可选）：同一份文档，压缩开/关的写入与读取 ——
    #[cfg(feature = "amberice-compress")]
    {
        let opts = DumpOptions {
            compress: Some(3),
            frame_size: Some(16 * 1024),
            ..Default::default()
        };
        let zfile = amberice::dump_with(&flat_cv, &opts).expect("带压缩 dump 失败");
        println!(
            "| `flat@4000` | {} | `zstd 体积` | — | — | {}（明文 {}） | — |",
            human(flat_t.len()),
            human(zfile.len()),
            human(flat_file.len())
        );

        let n = common::iters(20, quick);
        let t = common::bench(n, || {
            let _ = std::hint::black_box(amberice::dump_with(&flat_cv, &opts).expect("dump 失败"));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(amberice::dump_with(&flat_cv, &opts));
        });
        common::row(
            "flat@4000",
            &human(flat_t.len()),
            "dump(zstd+分帧)",
            t,
            flat_t.len(),
            mem,
        );

        let t = common::bench(n, || {
            let _ = std::hint::black_box(amberice::load(&zfile).expect("load 失败"));
        });
        let mem = common::measure(|| {
            let _ = std::hint::black_box(amberice::load(&zfile));
        });
        common::row("flat@4000", &human(zfile.len()), "load(zstd+分帧)", t, zfile.len(), mem);
    }
    #[cfg(not(feature = "amberice-compress"))]
    println!("| `—` | — | `zstd 组已跳过` | — | — | — | 加 `--features amberice-compress` |");

    // —— 解密（可选）：1 KiB / 64 KiB / 1 MiB 三段 ——
    #[cfg(feature = "amberice-crypto")]
    {
        const KEY: [u8; 32] = [7u8; 32];
        let ks = amberice::KeySource::Bytes(&KEY);
        for (label, lines) in [("1 KiB", 20usize), ("64 KiB", 1_300), ("1 MiB", 21_000)] {
            let text = common::build_flat(lines);
            let v = sml::parse_ordered(&text).expect("解析失败");
            let cv = CanonicalValue::try_from(&v).expect("必须有序");
            let enc =
                amberice::dump_encrypted(&cv, &DumpOptions::default(), &ks).expect("加密 dump 失败");
            let n = common::iters(10, quick);
            let t = common::bench(n, || {
                let _ = std::hint::black_box(amberice::load_encrypted(&enc, &ks).expect("解密失败"));
            });
            let mem = common::measure(|| {
                let _ = std::hint::black_box(amberice::load_encrypted(&enc, &ks));
            });
            common::row(label, &human(enc.len()), "load_encrypted", t, enc.len(), mem);
        }
    }
    #[cfg(not(feature = "amberice-crypto"))]
    println!("| `—` | — | `解密组已跳过` | — | — | — | 加 `--features amberice-crypto` |");

    // —— mmap（可选）：T3 的判据 —— 打开文件不把文档搬上堆 ——
    //
    // 三行分开报，因为「堆增与文件大小无关」这句话只对前两行成立：
    // - `open`：只建立映射，堆增应为 0；
    // - `+Store::open`：加上逐段摘要校验，堆增仍与**文件大小**无关；
    // - `+get`：解命中的子树 + 建 `STRTAB` 查找表 —— 后者是 O(键数)，
    //   在 `DIR` 段落地（§19.3）之前它就是这条路径的固定成本。
    #[cfg(feature = "amberice-mmap")]
    {
        let path =
            std::env::temp_dir().join(format!("amberice-bench-mmap-{}.ambr", std::process::id()));
        std::fs::write(&path, &flat_file).expect("写临时文件失败");

        let mem = common::measure(|| {
            let _ = std::hint::black_box(amberice::MappedFile::open(&path).expect("mmap 失败"));
        });
        common::row_mem("flat@4000", &human(flat_file.len()), "MappedFile::open", mem);

        let f = amberice::MappedFile::open(&path).expect("mmap 失败");
        // `Store::open` 借用映射，窗口里只有「解析头/段表/manifest + 验摘要」
        let mem = common::measure(|| {
            let _ = std::hint::black_box(f.store().expect("store 失败"));
        });
        common::row_mem("flat@4000", &human(flat_file.len()), "MappedFile::store", mem);

        let mem = common::measure(|| {
            let s = f.store().expect("store 失败");
            let _ = std::hint::black_box(s.get("k2000").expect("get 失败"));
        });
        common::row_mem("flat@4000", &human(flat_file.len()), "MappedFile::store+get", mem);

        let _ = std::fs::remove_file(&path);
    }
    #[cfg(not(feature = "amberice-mmap"))]
    println!("| `—` | — | `mmap 组已跳过` | — | — | — | 加 `--features amberice-mmap` |");

    println!();
    println!("> `MB/s` 按**落盘字节数**算（`dump` 行按值的规范字节数，`load` 行按文件大小）。");
    println!("> 峰值堆按**一次操作**计：`load` 的峰值 ≈ 解出的整棵树，`Store::get` 只含命中子树 + `STRTAB` 表。");
    println!("> `Writer` 的段体经临时文件落盘，故峰值只随输出缓冲与 manifest 走。");
}

fn human(bytes: usize) -> String {
    common::human_bytes(bytes as f64)
}
