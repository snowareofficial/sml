// SPDX-License-Identifier: MulanPSL-2.0
//! Container round-trip tests (T1 + T2) / 容器往返与自描述验收。
//!
//! 用例里的对象由 `Map::ordered()` 显式构造，canonical 字节在任何 feature 组合下都可产出，
//! 故本文件不设档位门控；C2 的「显式无序值必须报错」由 `tests/vectors.rs` 单独钉住。
//! ｜ Objects here are built with `Map::ordered()`; canonical bytes are available in every
//!    feature profile. The C2 "unordered value must error" criterion lives in `tests/vectors.rs`.

use amberice::container::{self, seg_name, SEG_HEADER_LEN, SEG_MANIFEST, SEG_STRTAB, SEG_VALUE};
use amberice::vectors::CASES;
use amberice::{DumpOptions, HashAlgo};

fn align4k(x: usize) -> usize {
    x.div_ceil(4096) * 4096
}

/// Criterion: `dump → load → dump` is byte-identical, for **every** vector case.
/// 判据：每一例都 `dump → load → dump` 逐字节相同。
#[test]
fn every_case_roundtrips_byte_identically() {
    for case in CASES {
        let v = (case.build)();
        let file = amberice::dump( &cv(&v) ).unwrap_or_else(|e| panic!("[{}] dump 失败：{e}", case.id));
        let back = amberice::load(&file).unwrap_or_else(|e| panic!("[{}] load 失败：{e}", case.id));
        assert!(back.eq_ordered(&v), "[{}] 值/键序没原样回来", case.id);
        let again = amberice::dump( &cv(&back) ).unwrap();
        assert_eq!(again, file, "[{}] dump→load→dump 字节不同", case.id);
    }
}

/// The `VALUE` segment must be **exactly** the T0 node bytes — no wrapping, no re-encoding.
/// `VALUE` 段必须**就是** T0 的节点字节：不包裹、不重编码。
#[test]
fn value_segment_body_is_the_t0_bytes() {
    for case in CASES {
        let v = (case.build)();
        let blocks = v.canonical_blocks().unwrap();
        let file = amberice::dump( &cv(&v) ).unwrap();
        let info = amberice::info(&file).unwrap();

        let value = info
            .segments
            .iter()
            .find(|s| s.header.seg_id == SEG_VALUE)
            .expect("VALUE 段必须在");
        assert_eq!(
            container::body(&file, value).unwrap(),
            &blocks.value[..],
            "[{}] VALUE 段",
            case.id
        );
        assert!(
            v.canonical_bytes().unwrap().ends_with(&blocks.value),
            "[{}] L0 字节尾部应等于 VALUE 段体",
            case.id
        );

        match info
            .segments
            .iter()
            .find(|s| s.header.seg_id == SEG_STRTAB)
        {
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

/// Header fields must match what the spec promises byte for byte (§19.6).
/// 文件头字段必须与 §19.6 承诺的一致（逐字节）。
#[test]
fn header_is_the_agreed_layout() {
    let v = (CASES[0].build)(); // scalar-null：只有 VALUE + MANIFEST 两个段
    let f = amberice::dump( &cv(&v) ).unwrap();

    assert_eq!(&f[0..4], b"AMBR", "magic");
    assert_eq!(u16::from_le_bytes([f[4], f[5]]), amberice::container::EDITION, "edition");
    assert_eq!(f[6], amberice::container::FMT, "fmt");
    assert_eq!(f[7] as usize, amberice::container::HEADER_LEN, "hdr_len");
    assert_eq!(
        u64::from_le_bytes(f[8..16].try_into().unwrap()),
        amberice::container::HEADER_LEN as u64,
        "段表紧跟头（不再钉在 4096）"
    );
    assert_eq!(
        u32::from_le_bytes(f[16..20].try_into().unwrap()),
        2,
        "VALUE + MANIFEST"
    );
    assert_eq!(
        u16::from_le_bytes([f[20], f[21]]),
        0,
        "明文默认：flags 位 0 = 0 ⇒ 8 B 对齐"
    );
    assert_eq!(
        u64::from_le_bytes(f[24..32].try_into().unwrap()),
        0,
        "tail_off 留给 T4"
    );
}

/// Alignment is declared by `flags` bit 0 (§19.7 丙): 8 B plaintext, 4 KiB when asked.
/// 对齐由 `flags` 位 0 声明：明文 8 B，指定时 4 KiB。
#[test]
fn alignment_follows_the_header_flag() {
    let v = (CASES[18].build)(); // 有字符串 ⇒ 两个内容段

    let plain = amberice::dump( &cv(&v) ).unwrap();
    let pi = amberice::info(&plain).unwrap();
    assert_eq!(pi.align, amberice::container::ALIGN_MIN);
    assert_eq!(pi.flags & amberice::container::FLAG_ALIGN_BLOCK, 0);
    for s in &pi.segments {
        assert_eq!(s.offset % amberice::container::ALIGN_MIN, 0, "段 {} 未按 8 B 对齐", seg_name(s.header.seg_id));
    }
    // 明文小文档不再吃 4 KiB 的固定税（T1 时这里是 12 KB 量级）
    assert!(
        plain.len() < 1024,
        "明文小文档应远小于 4 KiB，实得 {} B",
        plain.len()
    );

    let opts = DumpOptions {
        block_align: true,
        ..Default::default()
    };
    let blocked = amberice::dump_with( &cv(&v) , &opts).unwrap();
    let bi = amberice::info(&blocked).unwrap();
    assert_eq!(bi.align, amberice::container::ALIGN_BLOCK);
    assert_ne!(bi.flags & amberice::container::FLAG_ALIGN_BLOCK, 0);
    for s in &bi.segments {
        assert_eq!(s.offset % amberice::container::ALIGN_BLOCK, 0, "段 {} 未按 4 KiB 对齐", seg_name(s.header.seg_id));
    }
    // 两种对齐下值与段体都不变，只有位置变
    assert!(amberice::load(&blocked).unwrap().eq_ordered(&v));
}

/// `MANIFEST` is written, lists every content segment, and **does not list itself**.
/// `MANIFEST` 会写、列全内容段、且**不列自己**（否则摘要循环）。
#[test]
fn manifest_describes_the_other_segments_only() {
    let v = (CASES[18].build)();
    let f = amberice::dump( &cv(&v) ).unwrap();
    let info = amberice::info(&f).unwrap();
    let m = amberice::manifest::Manifest::from_bytes({
        let seg = info
            .segments
            .iter()
            .find(|s| s.header.seg_id == SEG_MANIFEST)
            .expect("MANIFEST 段必须存在");
        container::body(&f, seg).unwrap()
    })
    .unwrap();

    assert_eq!(m.primary, SEG_VALUE, "P8：primary 指向主内容");
    assert_eq!(m.hash, HashAlgo::Sha256, "默认 SHA-256");
    assert!(
        !m.segments.iter().any(|s| s.id == SEG_MANIFEST),
        "segments[] 不许含 manifest 自己"
    );
    // 列出的段与段表一致（除 manifest 自己）
    for seg in &info.segments {
        if seg.header.seg_id == SEG_MANIFEST {
            continue;
        }
        let entry = m
            .segments
            .iter()
            .find(|e| e.id == seg.header.seg_id)
            .unwrap_or_else(|| panic!("段 0x{:02x} 没进 manifest", seg.header.seg_id));
        assert_eq!(entry.len, seg.header.len);
        assert_eq!(entry.digest, seg.header.hash);
    }
}

/// SM3 is selectable end to end (dump → load → check all by `hash_id`).
/// SM3 可选，且端到端（dump → load → check）都按 `hash_id` 走。
#[test]
fn sm3_digests_work_end_to_end() {
    let v = (CASES[17].build)();
    let opts = DumpOptions {
        hash: HashAlgo::Sm3,
        ..Default::default()
    };
    let f = amberice::dump_with( &cv(&v) , &opts).unwrap();
    assert!(amberice::load(&f).unwrap().eq_ordered(&v), "SM3 文件应能读回");

    let report = amberice::check(&f).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    assert!(report.lines.join("\n").contains("SM3"), "{:?}", report.lines);

    // 换个算法验同一份字节 ⇒ 必须不符（否则 hash_id 等于装饰）
    let sha = amberice::dump( &cv(&v) ).unwrap();
    let value_seg = amberice::info(&sha)
        .unwrap()
        .segments
        .into_iter()
        .find(|s| s.header.seg_id == SEG_VALUE)
        .unwrap();
    let sha_digest = value_seg.header.hash;
    let sm3_file_seg = amberice::info(&f)
        .unwrap()
        .segments
        .into_iter()
        .find(|s| s.header.seg_id == SEG_VALUE)
        .unwrap();
    assert_ne!(sha_digest, sm3_file_seg.header.hash);
}

/// A `required` capability this build does not understand ⇒ refuse to open (D11).
/// 读不懂的 `required` 能力 ⇒ 拒绝打开（D11）。
#[test]
fn unknown_required_capability_is_refused() {
    let v = (CASES[0].build)();
    let opts = DumpOptions {
        caps_required: vec!["sig".into()],
        caps_optional: vec!["dir".into()],
        ..Default::default()
    };
    let f = amberice::dump_with( &cv(&v) , &opts).unwrap();

    let e = amberice::load(&f).expect_err("required 里读不懂的能力必须拒");
    assert_eq!(e.kind(), "missing-capability", "{e}");

    let report = amberice::check(&f).unwrap();
    assert!(!report.ok);
    let text = report.lines.join("\n");
    assert!(text.contains("读不懂的必拒能力：sig"), "{text}");
    assert!(text.contains("可跳过"), "{text}");

    // 认得的 required 不拦
    let ok_opts = DumpOptions {
        caps_required: vec!["value".into(), "strtab".into()],
        ..Default::default()
    };
    let ok_file = amberice::dump_with( &cv(&v) , &ok_opts).unwrap();
    assert!(amberice::load(&ok_file).is_ok());
}

/// 写侧只校验**格式**（合法标识符、不重复），不按闭集硬拒 ——
/// 后者会掐死「声明未来能力」这条唯一扩展通道。
/// Writer-side validation is about **form**, not membership.
#[test]
fn capability_declaration_is_validated_on_write() {
    let v = (CASES[0].build)();

    // 名字不合法：空格 / 大写 / 数字开头
    for bad in ["Bad Name", "Sig", "1dir", ""] {
        let opts = DumpOptions {
            caps_required: vec![bad.into()],
            ..Default::default()
        };
        let e = amberice::dump_with( &cv(&v) , &opts).unwrap_err();
        assert_eq!(e.kind(), "bad-capability-name", "`{bad}` 应被拒：{e}");
    }

    // 同一个名字在 required 与 optional 之间重复
    let dup = DumpOptions {
        caps_required: vec!["sig".into()],
        caps_optional: vec!["sig".into()],
        ..Default::default()
    };
    let e = amberice::dump_with( &cv(&v) , &dup).unwrap_err();
    assert_eq!(e.kind(), "duplicate-capability", "{e}");

    // 合法的未来能力仍然写得出来（`sig` / `dir` 不在 known 闭集里）
    let future = DumpOptions {
        caps_required: vec!["sig".into()],
        caps_optional: vec!["dir".into()],
        ..Default::default()
    };
    assert!(
        amberice::dump_with( &cv(&v) , &future).is_ok(),
        "已登记但本构建不处理的能力必须仍可声明"
    );
}

/// `pack_iter` 与 `pack` 必须产出**同一份字节**（前者只是少收一次税）。
/// The owning-input entry point must be byte-identical to the borrowed one.
#[test]
fn pack_iter_matches_pack_byte_for_byte() {
    let a: &[u8] = b"hello";
    let b: &[u8] = &[0x00, 0xFF, 0x01];

    let borrowed = amberice::pack(&[("a.txt", a), ("b.bin", b)]).unwrap();
    let owned = amberice::pack_iter(vec![
        ("a.txt".to_string(), a.to_vec()),
        ("b.bin".to_string(), b.to_vec()),
    ])
    .unwrap();

    assert_eq!(borrowed, owned, "两个入口必须逐字节相同");
    assert_eq!(&*amberice::payload(&owned, "b.bin").unwrap(), b);

    // 空归档仍然是明确错误，不会产出「零载荷的文件」
    assert_eq!(
        amberice::pack_iter(Vec::<(String, Vec<u8>)>::new())
            .unwrap_err()
            .kind(),
        "empty-archive"
    );
}

/// `Writer`（段体落临时文件、内存 O(1)）必须与 `dump`（全内存）产出**同一份字节**。
/// The streaming writer must be byte-identical to the in-memory one.
#[test]
fn writer_matches_dump_byte_for_byte() {
    use amberice::{CanonicalValue, DumpOptions, Writer};

    let doc = (CASES[3].build)();
    let cv = CanonicalValue::try_from(doc).unwrap();
    let expected = amberice::dump(&cv).unwrap();

    let mut buf: Vec<u8> = Vec::new();
    let mut w = Writer::new(&mut buf, &DumpOptions::default(), SEG_VALUE).unwrap();
    let blocks = cv.canonical_blocks().unwrap();
    // 段按 id 升序：VALUE(0x01) 在 STRTAB(0x02) 之前
    w.add_segment(SEG_VALUE, 0, 0, &blocks.value).unwrap();
    // `dump` 不写**空段**（STRTAB 为空时省略），这里保持一致
    if !blocks.strtab.is_empty() {
        w.add_segment(SEG_STRTAB, 0, 0, &blocks.strtab).unwrap();
    }
    w.finish().unwrap();

    assert_eq!(expected, buf, "Writer 与 dump 必须逐字节相同");
    assert!(amberice::check(&buf).unwrap().ok);

    // 升序约束是硬要求：倒序加入必须被拒
    let mut buf2: Vec<u8> = Vec::new();
    let mut w2 = Writer::new(&mut buf2, &DumpOptions::default(), SEG_VALUE).unwrap();
    w2.add_segment(SEG_STRTAB, 0, 0, &blocks.strtab).unwrap();
    assert!(
        w2.add_segment(SEG_VALUE, 0, 0, &blocks.value).is_err(),
        "倒序加入必须报错，否则段体顺序与段表顺序会不一致"
    );
}

/// 流式载荷入口（`impl Read`）与内存入口必须产出**同一份字节**。
/// The reader-based entry point must be byte-identical to the buffer-based one.
#[test]
fn pack_from_readers_matches_pack_byte_for_byte() {
    use std::io::Cursor;

    let a: &[u8] = b"hello";
    let b: &[u8] = &[0x00, 0xFF, 0x01];

    let borrowed = amberice::pack(&[("a.txt", a), ("b.bin", b)]).unwrap();
    let streamed = amberice::pack_from_readers(vec![
        ("a.txt".to_string(), Cursor::new(a.to_vec())),
        ("b.bin".to_string(), Cursor::new(b.to_vec())),
    ])
    .unwrap();

    assert_eq!(borrowed, streamed, "两个入口必须逐字节相同");
    assert_eq!(&*amberice::payload(&streamed, "b.bin").unwrap(), b);
}

/// Every segment carries a real digest, and `check` says so.
/// 每个段都带真摘要，`check` 会把结论打出来。
#[test]
fn digests_are_written() {
    let f = amberice::dump( &cv(&(CASES[10].build)()) ).unwrap();
    let info = amberice::info(&f).unwrap();
    assert!(info.has_digest(), "T2 起应写摘要");
    assert!(info.segments.iter().all(|s| s.header.hash != [0u8; 32]));

    let report = amberice::check(&f).unwrap();
    assert!(report.ok, "好文件应通过：{:?}", report.lines);
    let joined = report.lines.join("\n");
    assert!(joined.contains("摘要 OK"), "{joined}");
    assert!(joined.contains("通过"), "{joined}");
}

/// **The** T2 acceptance: flip one byte ⇒ it is caught.
/// T2 的核心判据：改一个字节必须被检出。
#[test]
fn one_changed_byte_is_caught() {
    let f = amberice::dump( &cv(&(CASES[17].build)()) ).unwrap();
    let info = amberice::info(&f).unwrap();
    let value_seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == SEG_VALUE)
        .expect("VALUE 段");
    let at = value_seg.offset as usize + 3;

    let mut bad = f.clone();
    bad[at] ^= 0x01;

    let e = amberice::load(&bad).expect_err("改了一个字节，load 必须报错");
    assert_eq!(e.kind(), "digest-mismatch", "{e}");

    let report = amberice::check(&bad).unwrap();
    assert!(!report.ok, "check 必须判未通过");
    assert!(
        report.lines.join("\n").contains("摘要不符"),
        "{:?}",
        report.lines
    );
}

/// A file without digests (T1-era, all zeros) still loads — “less checked”, not “invalid”.
/// 没带摘要（T1 时代，全 0）的文件仍能读 —— 只是校验更少，不是无效。
#[test]
fn zeroed_digest_means_unverified_not_invalid() {
    let mut f = amberice::dump( &cv(&(CASES[17].build)()) ).unwrap();
    let info = amberice::info(&f).unwrap();
    for i in 0..info.segments.len() {
        let at = info.table_off as usize + i * SEG_HEADER_LEN;
        f[at + 9..at + 41].fill(0);
    }
    assert!(amberice::load(&f).is_ok(), "全 0 摘要应视作「未校验」而不是损坏");
    let report = amberice::check(&f).unwrap();
    assert!(!report.ok, "但 check 必须说「无法判断是否被改过」");
    assert!(
        report.lines.join("\n").contains("未带摘要"),
        "{:?}",
        report.lines
    );
}

/// Invariant 1: an unknown segment is skipped by its `len`, whatever it contains.
/// 不变量 1：未知段按其 `len` 跳过 —— 哪怕它声明了本构建根本不认识的算法。
#[test]
fn unknown_segment_is_skipped() {
    let v = (CASES[17].build)();
    // 用 4 KiB 对齐的布局，段表后面才有空位插一条额外表项
    let base = amberice::dump_with( &cv(&v) ,
        &DumpOptions {
            block_align: true,
            ..Default::default()
        },
    )
    .unwrap();

    for (seg_id, comp, enc) in [(amberice::container::SEG_MANIFEST + 0x50, 0u8, 0u8), (0x7f, 9, 0x10)] {
        let with_extra = append_segment(base.clone(), seg_id, comp, enc, b"opaque payload");
        let info = amberice::info(&with_extra).unwrap();
        assert_eq!(info.segments.len(), 4, "应多出一段");
        let back = amberice::load(&with_extra).expect("未知段不该阻塞读取");
        assert!(back.eq_ordered(&v), "跳过未知段后值应不变");
    }
}

/// A truncated file is rejected, not silently half-read.
/// 截断的文件必须报错，而不是「读一半当没事」。
#[test]
fn truncation_is_rejected() {
    let v = (CASES[1].build)();
    let f = amberice::dump( &cv(&v) ).unwrap();
    for cut in [0usize, 20, amberice::container::HEADER_LEN, f.len() - 1] {
        assert!(
            amberice::info(&f[..cut]).is_err() || amberice::load(&f[..cut]).is_err(),
            "cut={cut} 应报错"
        );
    }
}

/// Version gate (D5): a different `fmt` means a different byte layout ⇒ refuse.
/// 版本闸门（D5）：`fmt` 不同就是另一套字节布局 ⇒ 拒绝。
#[test]
fn unsupported_fmt_is_rejected() {
    let mut f = amberice::dump( &cv(&(CASES[0].build)()) ).unwrap();
    f[6] = 1;
    let e = amberice::info(&f).expect_err("fmt=1 应被拒");
    assert_eq!(e.kind(), "unsupported-fmt");
}

/// Wrong magic ⇒ "this is not a `.ambr`", stated plainly.
/// 魔数不对 ⇒ 明说「这不是 .ambr 文件」。
#[test]
fn bad_magic_is_rejected() {
    let mut f = amberice::dump( &cv(&(CASES[0].build)()) ).unwrap();
    f[0] = b'X';
    assert_eq!(amberice::info(&f).unwrap_err().kind(), "bad-magic");
}

/// Without a `VALUE` segment there is nothing to load.
/// 没有 `VALUE` 段就没东西可读。
#[test]
fn missing_value_segment_is_rejected() {
    let mut f = amberice::dump( &cv(&(CASES[0].build)()) ).unwrap();
    let table_off = u64::from_le_bytes(f[8..16].try_into().unwrap()) as usize;
    f[table_off] = 0x60; // VALUE → PROVENANCE
    assert_eq!(amberice::load(&f).unwrap_err().kind(), "missing-segment");
}

/// `cat`: path lookup against the decoded value (§22).
/// `cat`：在解出来的值上按路径取值。
#[test]
fn cat_looks_up_by_path() {
    let v = (CASES[19].build)(); // nested-containers：config.port = 8080
    let f = amberice::dump( &cv(&v) ).unwrap();
    assert_eq!(amberice::get(&f, "config.port").unwrap(), amberice::Value::Int(8080));
    assert_eq!(
        amberice::get(&f, "config.hosts[0]").unwrap(),
        amberice::Value::Str("a".into())
    );
    let e = amberice::get(&f, "config.nope").expect_err("不存在的键应报错");
    assert_eq!(e.kind(), "bad-path");
}

/// Robustness: **any** single-byte mutation must come back as an error, never a panic.
/// 健壮性：**任一**单字节改动都得以错误返回，绝不许 panic。
#[test]
fn single_byte_mutations_never_panic() {
    let f = amberice::dump( &cv(&(CASES[16].build)()) ).unwrap();
    for i in 0..f.len() {
        for flip in [0x01u8, 0x80] {
            let mut bad = f.clone();
            bad[i] ^= flip;
            let _ = amberice::info(&bad);
            let _ = amberice::load(&bad);
            let _ = amberice::check(&bad);
        }
    }
}

/// Depth limit propagates through the container (same `E-LIMIT-004` as canonical).
/// 深度上限在容器层同样生效（与 canonical 同一个 `E-LIMIT-004`）。
#[test]
fn depth_limit_propagates() {
    let mut v = amberice::Value::Int(1);
    for _ in 0..=amberice::MAX_VALUE_DEPTH {
        v = amberice::Value::Array(vec![v]);
    }
    let e = amberice::dump( &cv(&v) ).expect_err("超深值必须被拒");
    assert!(matches!(
        e,
        amberice::ContainerError::Canonical(amberice::CanonicalError::DepthExceeded { .. })
    ));
}

/// `dump` is a pure function of the value: two calls, same bytes.
/// `dump` 是值的纯函数：调两次，字节相同。
#[test]
fn dump_is_deterministic() {
    for case in CASES {
        let v = (case.build)();
        assert_eq!(
            amberice::dump( &cv(&v) ).unwrap(),
            amberice::dump( &cv(&v) ).unwrap(),
            "[{}]",
            case.id
        );
    }
}

/// `info` must answer P8's question (“what is the main content?”).
/// `info` 必须回答 P8 的问题（「主内容是什么」）。
#[test]
fn info_names_the_primary() {
    let f = amberice::dump( &cv(&(CASES[0].build)()) ).unwrap();
    let info = amberice::info(&f).unwrap();
    let text = container::info_lines("t.ambr", &f, &info)
        .unwrap()
        .join("\n");
    assert!(text.contains("0x01 VALUE（manifest 的 primary）"), "{text}");
    assert!(text.contains("hash_id   0x30 SHA-256"), "{text}");
    assert!(text.contains("段体对齐 8 B"), "{text}");
}

// ---------------------------------------------------------------------------
// 归档：EMBED + payloads[]（D23 / §22 pack·unpack）
// ---------------------------------------------------------------------------

/// `pack` → `payloads` / `payload` → `check` 全通，且 `load` 明确说「主内容不是 Value」。
#[test]
fn pack_round_trips_payloads() {
    let blobs: [(&str, &[u8]); 3] = [
        ("notes/a.md", b"# hello\n"),
        ("bin/blob.dat", &[0u8, 1, 2, 0xff, 0xfe]),
        ("empty.txt", b""),
    ];
    let f = amberice::pack(&blobs).unwrap();

    let list = amberice::payloads(&f).unwrap();
    assert_eq!(
        list.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        vec!["notes/a.md", "bin/blob.dat", "empty.txt"]
    );
    for (name, data) in blobs {
        assert_eq!(&*amberice::payload(&f, name).unwrap(), data, "载荷 {name} 字节不符");
    }

    let report = amberice::check(&f).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    let text = report.lines.join("\n");
    assert!(text.contains("payload   `notes/a.md` 摘要 OK"), "{text}");

    // 归档的主内容是 EMBED 段 ⇒ `load` 必须明说，而不是硬解成 Value
    let e = amberice::load(&f).unwrap_err();
    assert_eq!(e.kind(), "primary-not-value", "{e}");

    let info = amberice::info(&f).unwrap();
    let lines = container::info_lines("a.ambr", &f, &info).unwrap().join("\n");
    assert!(lines.contains("primary   0x20 EMBED（manifest 的 primary）"), "{lines}");
    assert!(lines.contains("caps      required [embed]"), "{lines}");
}

/// 同名载荷一律拒绝：解包时该覆盖谁是没有答案的问题。
#[test]
fn pack_rejects_duplicate_and_empty() {
    assert_eq!(
        amberice::pack(&[]).unwrap_err().kind(),
        "empty-archive"
    );
    let dup: [(&str, &[u8]); 2] = [("a.txt", b"1"), ("a.txt", b"2")];
    assert_eq!(
        amberice::pack(&dup).unwrap_err().kind(),
        "duplicate-payload-name"
    );
}

/// 改载荷里的一个字节 ⇒ 段摘要 + 载荷摘要双重报错，且 `payload()` 拒绝给数据。
#[test]
fn payload_tamper_is_caught() {
    let blobs: [(&str, &[u8]); 1] = [("a.bin", b"0123456789")];
    let mut f = amberice::pack(&blobs).unwrap();
    let info = amberice::info(&f).unwrap();
    let embed = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == amberice::container::SEG_EMBED)
        .unwrap();
    let at = embed.offset as usize + 3;
    f[at] ^= 0x01;

    let report = amberice::check(&f).unwrap();
    assert!(!report.ok);
    let text = report.lines.join("\n");
    assert!(text.contains("摘要不符"), "{text}");

    // payload() 自己也会核：段摘要已不符 ⇒ 直接拒
    assert!(amberice::payload(&f, "a.bin").is_err());
}

/// 不存在的载荷名要给明确的错，而不是空切片。
#[test]
fn missing_payload_is_named_in_the_error() {
    let blobs: [(&str, &[u8]); 1] = [("a.txt", b"x")];
    let f = amberice::pack(&blobs).unwrap();
    let e = amberice::payload(&f, "nope.txt").unwrap_err();
    assert_eq!(e.kind(), "payload-not-found");
    assert!(e.to_string().contains("nope.txt"), "{e}");
}

/// Copy a file, append one extra segment (and its table entry).
/// 复制一份，追加一个额外段（并在段表里补一项）。
fn append_segment(mut f: Vec<u8>, seg_id: u8, comp: u8, enc: u8, body: &[u8]) -> Vec<u8> {
    let table_off = u64::from_le_bytes(f[8..16].try_into().unwrap()) as usize;
    let count = u32::from_le_bytes(f[16..20].try_into().unwrap()) as usize;
    let info = amberice::info(&f).unwrap();
    assert_eq!(info.align, amberice::container::ALIGN_BLOCK, "这个用例要求 4 KiB 布局");
    let prev_end = info
        .segments
        .last()
        .map(|s| s.offset as usize + s.header.len as usize)
        .unwrap_or(amberice::container::HEADER_LEN);
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

/// 测试用：把值断言成 `CanonicalValue`（无序在测试里就该 panic，而不是静默通过）。
fn cv(v: &amberice::Value) -> amberice::CanonicalValue {
    amberice::CanonicalValue::try_from(v).expect("测试值必须是有序构造的")
}
