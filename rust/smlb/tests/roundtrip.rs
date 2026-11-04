// SPDX-License-Identifier: MulanPSL-2.0
//! Container round-trip tests (T1) / 容器往返验收（T1）。
//!
//! The whole file is gated on `preserve_order`: without it there is no canonical byte form to
//! put in a container at all (C2), so there is nothing to test — that path's assertion lives in
//! `tests/vectors.rs` (the default build must **error**, not sort).
//! 整份文件由 `preserve_order` 门控：没有源序就没有可装的 canonical 字节（C2），
//! 那条断言在 `tests/vectors.rs` 里（默认构建必须**报错**，不许排序）。
#![cfg(feature = "preserve_order")]

use smlb::container::{
    self, seg_name, SEG_HEADER_LEN, SEG_STRTAB, SEG_VALUE,
};
use smlb::vectors::CASES;

const ALIGN: usize = 4096;

fn align4k(x: usize) -> usize {
    (x + ALIGN - 1) / ALIGN * ALIGN
}

/// Criterion: `dump → load → dump` is byte-identical, for **every** vector case.
/// 判据：每一例都 `dump → load → dump` 逐字节相同。
#[test]
fn every_case_roundtrips_byte_identically() {
    for case in CASES {
        let v = (case.build)();
        let file = smlb::dump(&v).unwrap_or_else(|e| panic!("[{}] dump 失败：{e}", case.id));
        let back = smlb::load(&file).unwrap_or_else(|e| panic!("[{}] load 失败：{e}", case.id));
        assert!(back.eq_ordered(&v), "[{}] 值/键序没原样回来", case.id);
        let again = smlb::dump(&back).unwrap();
        assert_eq!(again, file, "[{}] dump→load→dump 字节不同", case.id);
    }
}

/// The `VALUE` segment must be **exactly** the T0 node bytes — no wrapping, no re-encoding.
/// `VALUE` 段必须**就是** T0 的节点字节：不包裹、不重编码。
///
/// This is the bridge between T0 and T1: T0's vectors stay the contract, the container only
/// decides where those bytes live.
/// 这是 T0 与 T1 的接缝：T0 的向量仍是契约，容器只决定那些字节住在哪。
#[test]
fn value_segment_body_is_the_t0_bytes() {
    for case in CASES {
        let v = (case.build)();
        let blocks = v.canonical_blocks().unwrap();
        let file = smlb::dump(&v).unwrap();
        let info = smlb::info(&file).unwrap();

        let value = info
            .segments
            .iter()
            .find(|s| s.header.seg_id == SEG_VALUE)
            .expect("VALUE 段必须在");
        assert_eq!(container::body(&file, value).unwrap(), &blocks.value[..], "[{}] VALUE 段", case.id);
        // 与 T0 的 L0 字节相比：只多一个 `count` 前缀与 STRTAB 段（可缺）
        let l0 = v.canonical_bytes().unwrap();
        assert!(
            l0.ends_with(&blocks.value),
            "[{}] L0 字节尾部应等于 VALUE 段体",
            case.id
        );

        match info.segments.iter().find(|s| s.header.seg_id == SEG_STRTAB) {
            Some(seg) => assert_eq!(
                container::body(&file, seg).unwrap(),
                &blocks.strtab[..],
                "[{}] STRTAB 段",
                case.id
            ),
            None => assert!(
                blocks.strtab.is_empty(),
                "[{}] 空 STRTAB 不该落段",
                case.id
            ),
        }
    }
}

/// Header fields must match what T1 promised byte for byte.
/// 文件头字段必须与 T1 承诺的一致（逐字节）。
#[test]
fn header_is_the_agreed_layout() {
    let v = (CASES[0].build)();
    let f = smlb::dump(&v).unwrap();

    assert_eq!(&f[0..4], b"SWSB", "magic");
    assert_eq!(u16::from_le_bytes([f[4], f[5]]), smlb::EDITION, "edition");
    assert_eq!(f[6], smlb::FMT, "fmt");
    assert_eq!(f[7] as usize, smlb::HEADER_LEN, "hdr_len");
    assert_eq!(
        u64::from_le_bytes(f[8..16].try_into().unwrap()),
        ALIGN as u64,
        "seg_table_off 应钉在 4096"
    );
    assert_eq!(u32::from_le_bytes(f[16..20].try_into().unwrap()) as usize, 1, "Int 只有 VALUE 一个段");
    assert_eq!(u16::from_le_bytes([f[20], f[21]]), 0, "flags v1 恒 0");
    assert_eq!(u64::from_le_bytes(f[24..32].try_into().unwrap()), 0, "tail_off 留给 T4");

    // 头部之后的填充必须是 0
    assert!(f[smlb::HEADER_LEN..ALIGN].iter().all(|b| *b == 0), "头后应为零填充");
}

/// Every segment body starts on a 4 KiB boundary (§19.1 / D28).
/// 每个段体都从 4 KiB 边界开始。
#[test]
fn segments_start_on_4k_boundaries() {
    let v = (CASES[18].build)(); // 含多字符串 ⇒ 两个段
    let f = smlb::dump(&v).unwrap();
    let info = smlb::info(&f).unwrap();
    assert!(info.segments.len() >= 2, "这一例应有两个段");
    for s in &info.segments {
        assert_eq!(s.offset % ALIGN as u64, 0, "段 {} 未对齐", seg_name(s.header.seg_id));
    }
    // 最后一段之后不留填充：文件长度恰好等于「末段偏移 + 末段长度」
    let last = info.segments.last().expect("至少一段");
    assert_eq!(
        f.len() as u64,
        last.offset + last.header.len as u64,
        "末段之后不该有尾巴"
    );
}

/// Invariant 1: an unknown segment is skipped by its `len`, whatever it contains.
/// 不变量 1：未知段按其 `len` 跳过 —— 哪怕它声明了本构建根本不认识的算法。
#[test]
fn unknown_segment_is_skipped() {
    let v = (CASES[17].build)();
    let base = smlb::dump(&v).unwrap();

    for (seg_id, comp, enc) in [(smlb::SEG_MANIFEST, 0u8, 0u8), (0x7f, 9, 0x10)] {
        let with_extra = append_segment(base.clone(), seg_id, comp, enc, b"opaque payload");
        let info = smlb::info(&with_extra).unwrap();
        assert_eq!(info.segments.len(), 3, "应多出一段");
        // 仍然读得出来 —— 这是「未知段可跳过」的全部意义
        let back = smlb::load(&with_extra).expect("未知段不该阻塞读取");
        assert!(back.eq_ordered(&v), "跳过未知段后值应不变");
    }
}

/// A truncated file is rejected, not silently half-read.
/// 截断的文件必须报错，而不是「读一半当没事」。
#[test]
fn truncation_is_rejected() {
    let v = (CASES[1].build)();
    let f = smlb::dump(&v).unwrap();
    for cut in [0usize, 20, smlb::HEADER_LEN, 4096, f.len() - 1] {
        assert!(
            smlb::info(&f[..cut]).is_err() || smlb::load(&f[..cut]).is_err(),
            "cut={cut} 应报错"
        );
    }
}

/// Version gate (D5): a different `fmt` means a different byte layout ⇒ refuse.
/// 版本闸门（D5）：`fmt` 不同就是另一套字节布局 ⇒ 拒绝。
#[test]
fn unsupported_fmt_is_rejected() {
    let mut f = smlb::dump(&(CASES[0].build)()).unwrap();
    f[6] = 1;
    let e = smlb::info(&f).expect_err("fmt=1 应被拒");
    assert_eq!(e.kind(), "unsupported-fmt");
}

/// Wrong magic ⇒ "this is not a `.smlb`", stated plainly.
/// 魔数不对 ⇒ 明说「这不是 .smlb 文件」。
#[test]
fn bad_magic_is_rejected() {
    let mut f = smlb::dump(&(CASES[0].build)()).unwrap();
    f[0] = b'X';
    assert_eq!(smlb::info(&f).unwrap_err().kind(), "bad-magic");
}

/// Without a `VALUE` segment there is nothing to load (T2 adds `primary`, T1 does not).
/// 没有 `VALUE` 段就没东西可读（`primary` 是 T2 的事）。
#[test]
fn missing_value_segment_is_rejected() {
    let mut f = smlb::dump(&(CASES[0].build)()).unwrap();
    let table_off = u64::from_le_bytes(f[8..16].try_into().unwrap()) as usize;
    f[table_off] = 0x60; // VALUE → PROVENANCE
    assert_eq!(smlb::load(&f).unwrap_err().kind(), "missing-segment");
}

/// Guard rail: T1 writes **no** digest — tools must not claim tamper detection.
/// 护栏：T1 **不写**摘要 —— 工具不许声称能检出篡改。
#[test]
fn t1_writes_no_digest() {
    let f = smlb::dump(&(CASES[10].build)()).unwrap();
    let info = smlb::info(&f).unwrap();
    assert!(!info.has_digest(), "T1 不该写摘要");
    assert!(info
        .segments
        .iter()
        .all(|s| s.header.hash == [0u8; 32]));
    // 头里的自述要把这件事说出来（否则用户以为文件被校验过）
    let lines = container::info_lines("t.smlb", &info).join("\n");
    assert!(lines.contains("不要指望"), "info 应明说没有摘要：{lines}");
}

/// Copy a file, append one extra segment (and its table entry).
/// 复制一份，追加一个额外段（并在段表里补一项）。
fn append_segment(
    mut f: Vec<u8>,
    seg_id: u8,
    comp: u8,
    enc: u8,
    body: &[u8],
) -> Vec<u8> {
    let table_off = u64::from_le_bytes(f[8..16].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(f[16..20].try_into().unwrap()) as usize;
    let info = smlb::info(&f).unwrap();
    let prev_end = info
        .segments
        .last()
        .map(|s| s.offset as usize + s.header.len as usize)
        .unwrap_or(ALIGN);
    let off = align4k(prev_end);

    let at = table_off + count * SEG_HEADER_LEN;
    assert!(
        at + SEG_HEADER_LEN <= off,
        "段表放不下额外一项（这个用例假设段数很少）"
    );
    f[at] = seg_id;
    f[at + 1] = comp;
    f[at + 2] = enc;
    f[at + 5..at + 9].copy_from_slice(&(body.len() as u32).to_le_bytes());
    f[16..20].copy_from_slice(&((count + 1) as u32).to_le_bytes());
    if f.len() < off {
        f.resize(off, 0);
    }
    f.extend_from_slice(body);
    f
}
