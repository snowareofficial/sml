// SPDX-License-Identifier: MulanPSL-2.0
//! T5 compression tests (per-segment `comp`, §20 `0x01 zstd`).
//! T5 压缩验收（段级 `comp` 字段，§20 的 `0x01 zstd`）。
#![cfg(all(feature = "preserve_order", feature = "compress"))]

use smlb::{DumpOptions, Map, Store, Value};

/// A document that compresses well (repeating text) — 200 rows of the same shape.
/// 可压性好的文档：200 行同形状数据。
fn repetitive_doc() -> Value {
    let rows: Vec<Value> = (0..200)
        .map(|i| {
            let mut m = Map::ordered();
            m.insert("idx".to_string(), Value::Int(i));
            m.insert(
                "text".to_string(),
                Value::Str(format!("row-{i}-{}", "abcdefgh".repeat(20))),
            );
            Value::Object(m)
        })
        .collect();
    let mut root = Map::ordered();
    root.insert("rows".to_string(), Value::Array(rows));
    Value::Object(root)
}

/// Compressed dump round-trips, is smaller, and `check` reports the compression.
/// 压缩后能往返、体积变小、`check` 会报出来。
#[test]
fn compressed_dump_round_trips_and_shrinks() {
    let doc = repetitive_doc();
    let plain = smlb::dump(&doc).unwrap();
    let opts = DumpOptions {
        compress: Some(3),
        ..Default::default()
    };
    let packed = smlb::dump_with(&doc, &opts).unwrap();

    assert!(
        packed.len() < plain.len(),
        "压缩后应更小：{} vs {}",
        packed.len(),
        plain.len()
    );
    assert!(smlb::load(&packed).unwrap().eq_ordered(&doc), "值必须原样回来");
    assert_eq!(
        smlb::dump(&smlb::load(&packed).unwrap()).unwrap(),
        smlb::dump(&doc).unwrap(),
        "解压后再写回（未压缩）应与原始一致"
    );

    let report = smlb::check(&packed).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    let text = report.lines.join("\n");
    assert!(text.contains("压缩 0x01"), "{text}");

    // 段表里的 comp 字段必须如实记录
    let info = smlb::info(&packed).unwrap();
    assert!(
        info.segments.iter().any(|s| s.header.comp == smlb::COMP_ZSTD),
        "至少有一个段声明了 zstd"
    );
    // manifest 自己不压缩（引导顺序：先读它才知道用什么算法）
    let manifest = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == smlb::SEG_MANIFEST)
        .unwrap();
    assert_eq!(manifest.header.comp, 0, "manifest 不许压缩");
}

/// The manifest's `segments[]` records the same `comp` as the table (cross-check stays green).
/// manifest 的 `segments[]` 记录同一个 `comp`（交叉核对仍通过）。
#[test]
fn manifest_records_compression() {
    let opts = DumpOptions {
        compress: Some(5),
        ..Default::default()
    };
    let f = smlb::dump_with(&repetitive_doc(), &opts).unwrap();
    let info = smlb::info(&f).unwrap();
    let report = smlb::check(&f).unwrap();
    assert!(report.ok, "{:?}", report.lines);

    for seg in &info.segments {
        if seg.header.seg_id == smlb::SEG_MANIFEST {
            continue;
        }
        assert_eq!(seg.header.comp, smlb::COMP_ZSTD, "段应压缩");
    }
}

/// Archives compress end to end; the payload comes back byte-identical.
/// 归档端到端压缩，载荷逐字节还原。
#[test]
fn compressed_archive_keeps_payloads_intact() {
    let data = "x".repeat(50_000);
    let blobs: [(&str, &[u8]); 2] = [("big.txt", data.as_bytes()), ("tiny.txt", b"hi")];
    let opts = DumpOptions {
        compress: Some(9),
        ..Default::default()
    };
    let plain = smlb::pack(&blobs).unwrap();
    let packed = smlb::pack_with(&blobs, &opts).unwrap();
    assert!(packed.len() < plain.len() / 2, "5 万字节的重复文本应压掉一半以上");

    assert_eq!(&*smlb::payload(&packed, "big.txt").unwrap(), data.as_bytes());
    assert_eq!(&*smlb::payload(&packed, "tiny.txt").unwrap(), b"hi");
    assert!(smlb::check(&packed).unwrap().ok);
}

/// An unknown compression id is refused, not guessed ( §20: never reuse, never guess ).
/// 未知压缩编号明确拒绝，绝不猜。
#[test]
fn unknown_compression_id_is_refused() {
    let opts = DumpOptions {
        compress: Some(3),
        ..Default::default()
    };
    let mut f = smlb::dump_with(&repetitive_doc(), &opts).unwrap();
    let info = smlb::info(&f).unwrap();
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.comp == smlb::COMP_ZSTD)
        .expect("应有一段是压缩的");
    // 段表里 comp 在段头第 2 字节
    let at = info.table_off as usize
        + info
            .segments
            .iter()
            .position(|s| s.header.seg_id == seg.header.seg_id)
            .unwrap()
            * smlb::SEG_HEADER_LEN;
    f[at + 1] = 0x02; // 0x02 = lz4（§20 预留，本构建不实现）

    let e = smlb::load(&f).expect_err("未知压缩编号必须拒绝");
    assert_eq!(e.kind(), "unsupported-algorithm", "{e}");
}

/// The decompression bomb guard: an explicit cap makes an over-large output an error.
/// 解压炸弹防护：显式上限让「膨胀过大」变成错误。
#[test]
fn decompression_cap_is_enforced() {
    let big = vec![b'a'; 64 * 1024];
    let packed = smlb::container::deflate_zstd(&big, 3).unwrap();
    assert!(packed.len() < big.len() / 10, "64 KiB 的 a 应压得很小");

    // 上限给足 ⇒ 正常解出
    assert_eq!(
        smlb::container::inflate_zstd(&packed, 1024 * 1024).unwrap(),
        big
    );
    // 上限给小 ⇒ 直接失败，而不是先分配再发现
    assert!(
        smlb::container::inflate_zstd(&packed, 1024).is_err(),
        "超出上限必须报错"
    );
}

/// Random access (T3) needs plain segments: a compressed file says so explicitly instead of
/// silently inflating everything. ｜ 随机访问要求明文：压缩文件要明确拒绝，而不是偷偷全解。
#[test]
fn zero_copy_refuses_compressed_segments() {
    let opts = DumpOptions {
        compress: Some(3),
        ..Default::default()
    };
    let f = smlb::dump_with(&repetitive_doc(), &opts).unwrap();

    let e = Store::open(&f).expect_err("压缩文件不该被当成可零拷贝打开");
    assert_eq!(e.kind(), "zero-copy-needs-plain", "{e}");
    assert!(e.to_string().contains("load"), "错误信息应指路：{e}");

    // 但整树读取照常
    assert!(smlb::load(&f).unwrap().eq_ordered(&repetitive_doc()));
    // `cat` 也是整树路径，照常可用
    assert_eq!(
        smlb::get(&f, "rows[0].idx").unwrap(),
        Value::Int(0)
    );
}
