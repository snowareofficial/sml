// SPDX-License-Identifier: MulanPSL-2.0
//! T5 frame-level compression tests (`FRAMES` 0x10, §19.12).
//! T5 帧级压缩验收（`FRAMES` 0x10，§19.12）。
#![cfg(all(feature = "preserve_order", feature = "compress"))]

use smlb::container;
use smlb::{ContainerError, DumpOptions, Map, Store, Value, SEG_FRAMES};

/// A document comfortably larger than the frame size, so several frames exist.
/// 明显大于帧大小的文档，保证有多帧。
fn wide_doc() -> Value {
    let rows: Vec<Value> = (0..300)
        .map(|i| {
            let mut m = Map::ordered();
            m.insert("idx".to_string(), Value::Int(i));
            m.insert(
                "text".to_string(),
                Value::Str(format!("row-{i}-{}", "abcdefgh".repeat(24))),
            );
            Value::Object(m)
        })
        .collect();
    let mut root = Map::ordered();
    root.insert("rows".to_string(), Value::Array(rows));
    Value::Object(root)
}

fn framed_opts(size: u32) -> DumpOptions {
    DumpOptions {
        frame_size: Some(size),
        ..Default::default()
    }
}

/// Framed dump round-trips, and the manifest points at `FRAMES` (not `VALUE`).
/// 分帧能往返，manifest 的 primary 指向 `FRAMES`。
#[test]
fn framed_dump_round_trips() {
    let doc = wide_doc();
    let plain = smlb::dump( &cv(&doc) ).unwrap();
    let framed = smlb::dump_with( &cv(&doc) , &framed_opts(4096)).unwrap();

    assert!(
        framed.len() < plain.len(),
        "分帧压缩后应更小：{} vs {}",
        framed.len(),
        plain.len()
    );
    assert!(smlb::load(&framed).unwrap().eq_ordered(&doc), "值必须原样回来");

    let info = smlb::info(&framed).unwrap();
    assert!(
        info.segments.iter().any(|s| s.header.seg_id == SEG_FRAMES),
        "应有 FRAMES 段"
    );
    assert!(
        !info.segments.iter().any(|s| s.header.seg_id == smlb::SEG_VALUE),
        "分帧后不该再有 VALUE 段"
    );
    let m = smlb::manifest_of(&framed).unwrap();
    assert_eq!(m.primary, SEG_FRAMES, "primary 必须指 FRAMES");

    let report = smlb::check(&framed).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    assert!(report.lines.join("\n").contains("帧"), "{:?}", report.lines);
}

/// The index is validated: continuity, bounds, and “frames never overlap the index”.
/// 索引要校验：连续、不越界、不压到索引区。
#[test]
fn frame_index_is_validated() {
    let framed = smlb::dump_with( &cv(&wide_doc()) , &framed_opts(4096)).unwrap();
    let info = smlb::info(&framed).unwrap();
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == SEG_FRAMES)
        .unwrap();
    let body = container::body(&framed, seg).unwrap();
    let entries = smlb::parse_frame_index(body).unwrap();
    assert!(entries.len() >= 2, "应有至少两帧");

    // 把第 1 帧的 uncomp_off 改坏 ⇒ 断档
    let mut bad = body.to_vec();
    let at = 4 + smlb::FRAME_ENTRY_LEN;
    bad[at] = 0x7f;
    let e = smlb::parse_frame_index(&bad).unwrap_err();
    assert_eq!(e.kind(), "bad-frame-index", "{e}");

    // 帧数改成 0 ⇒ 拒绝
    let mut zero = body.to_vec();
    zero[0..4].copy_from_slice(&0u32.to_le_bytes());
    assert!(smlb::parse_frame_index(&zero).is_err());
}

/// **The point of framing**: breaking one frame must not stop another frame from being read.
/// **分帧的意义**：弄坏一帧，不该影响另一帧的读取。
#[test]
fn only_the_touched_frame_is_inflated() {
    // 用 1 KiB 的帧：这份文档的**节点树**不大（字符串都在 STRTAB 里去重了），
    // 要大一点的帧数只能把帧切细；这也顺带钉住「帧大小是写者旋钮」。
    let framed = smlb::dump_with( &cv(&wide_doc()) , &framed_opts(1024)).unwrap();
    let info = smlb::info(&framed).unwrap();
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == SEG_FRAMES)
        .unwrap();
    let body = container::body(&framed, seg).unwrap();
    let entries = smlb::parse_frame_index(body).unwrap();
    assert!(entries.len() >= 3, "至少三帧才能证「互不影响」（实得 {}）", entries.len());

    // 把第 1 帧的压缩数据翻一位
    let mut broken = body.to_vec();
    broken[entries[1].comp_off as usize + 3] ^= 0x01;

    // 第 0 帧仍能单独解出来（只解了它自己）
    let (e0, bytes0) = smlb::frame_bytes_at(&broken, entries[0].uncomp_off).unwrap();
    assert_eq!(e0.uncomp_off, 0);
    assert_eq!(bytes0.len(), entries[0].uncomp_len as usize);

    // 最后一帧同样能单独解（证明扫索引时不碰帧数据）
    let last = entries.last().unwrap();
    let (el, bytesl) = smlb::frame_bytes_at(&broken, last.uncomp_off).unwrap();
    assert_eq!(el.comp_off, last.comp_off);
    assert_eq!(bytesl.len(), last.uncomp_len as usize);

    // 全解则会失败（坏帧在里面）
    assert!(
        smlb::inflate_frames_all(&broken).is_err(),
        "整解应当因为坏帧失败"
    );
}

/// `Store` reads framed files too — navigation runs on a cursor, so it can skip across frames.
/// `Store` 也能读分帧文件：导航跑在游标上，跨帧只解需要的那几帧。
#[test]
fn store_reads_framed_files_on_demand() {
    let framed = smlb::dump_with( &cv(&wide_doc()) , &framed_opts(1024)).unwrap();

    let store = Store::open(&framed).expect("分帧文件应能以随机访问方式打开");
    assert_eq!(store.get("rows[0].idx").unwrap(), Value::Int(0));
    assert_eq!(store.get("rows[299].idx").unwrap(), Value::Int(299));
    assert_eq!(
        store.text_of("rows[7].idx").unwrap(),
        "7\n",
        "取到的子树文本必须精确"
    );

    // 零拷贝切片对分帧做不到（跨帧没法借用），但物化版本可以
    assert_eq!(
        store.subtree_bytes("rows[0]").unwrap_err().kind(),
        "framed-not-zero-copy"
    );
    assert!(!store.subtree_vec("rows[0]").unwrap().is_empty());
}

/// **The payoff**: corrupt one frame and a lookup landing in *another* frame still works.
/// **收益所在**：弄坏一帧，落在**另一帧**的查询仍然成功。
#[test]
fn store_inflates_only_the_frames_it_needs() {
    let framed = smlb::dump_with( &cv(&wide_doc()) , &framed_opts(1024)).unwrap();
    let info = smlb::info(&framed).unwrap();
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == SEG_FRAMES)
        .unwrap();
    let body = container::body(&framed, seg).unwrap();
    let entries = smlb::parse_frame_index(body).unwrap();

    // 把**最后**一帧的压缩数据翻一位
    let last = *entries.last().unwrap();
    let mut broken = framed.clone();
    let at = seg.offset as usize + last.comp_off as usize + 3;
    broken[at] ^= 0x01;

    // 不验摘要（摘要覆盖整段，必然不符），直接看「按需解帧」的行为
    let store = Store::open_unverified(&broken).unwrap();
    assert_eq!(
        store.get("rows[0].idx").unwrap(),
        Value::Int(0),
        "第 0 行在完好的帧里，必须取得到"
    );
    assert!(
        store.get("rows[299].idx").is_err(),
        "最后一行在坏帧里，必须报错而不是给出错值"
    );
    // 整解则必然失败
    assert!(smlb::load(&broken).is_err());
}

/// A framed file is still a valid `.smlb`: `check` reports frames and passes.
/// 分帧文件仍是合法 `.smlb`：`check` 报帧并通过。
#[test]
fn check_reports_frames() {
    let framed = smlb::dump_with( &cv(&wide_doc()) , &framed_opts(2048)).unwrap();
    let report = smlb::check(&framed).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    let text = report.lines.join("\n");
    assert!(text.contains("FRAMES"), "{text}");
    assert!(text.contains("解压后"), "{text}");
}

/// Both knobs can be used together: the tree is framed, `STRTAB` is compressed **once**.
/// 两个旋钮可共存：树分帧、`STRTAB` 再按段压一次（不是压两遍）。
///
/// 这一条来自实测：300 行文档的节点树只有 7.3 KB，而 `STRTAB` 有 60.5 KB —— 只分帧等于
/// 只压了小头，必须让段级压缩把表也压了。
/// ｜ Measured: tree 7.3 KB vs STRTAB 60.5 KB — framing alone compresses the wrong half.
#[test]
fn frames_and_segment_compression_coexist() {
    let plain = smlb::dump( &cv(&wide_doc()) ).unwrap();
    let opts = DumpOptions {
        frame_size: Some(1024),
        compress: Some(5),
        ..Default::default()
    };
    let both = smlb::dump_with( &cv(&wide_doc()) , &opts).unwrap();

    let info = smlb::info(&both).unwrap();
    let strtab = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == smlb::SEG_STRTAB)
        .expect("这份文档有 STRTAB");
    assert_eq!(
        strtab.header.comp,
        smlb::COMP_ZSTD,
        "STRTAB 应按段压缩（它才是大头）"
    );
    let frames = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == SEG_FRAMES)
        .expect("应有 FRAMES 段");
    assert_eq!(frames.header.comp, 0, "FRAMES 内部已压缩，不许再套一层");

    assert!(
        both.len() < plain.len() / 2,
        "两个旋钮合用应显著变小：{} vs {}",
        both.len(),
        plain.len()
    );
    assert!(smlb::load(&both).unwrap().eq_ordered(&wide_doc()));
    assert!(smlb::check(&both).unwrap().ok);
}

/// The frame index catches a truncated body instead of reading past it.
/// 段体被截断时，索引校验要拦住，而不是越界读。
#[test]
fn truncated_frames_body_is_rejected() {
    let framed = smlb::dump_with( &cv(&wide_doc()) , &framed_opts(4096)).unwrap();
    let info = smlb::info(&framed).unwrap();
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == SEG_FRAMES)
        .unwrap();
    let body = container::body(&framed, seg).unwrap();
    let cut = &body[..body.len() - 8];
    let e = smlb::parse_frame_index(cut).unwrap_err();
    assert_eq!(e.kind(), "bad-frame-index", "{e}");
}

/// `PrimaryNotValue` still applies to genuinely non-value primaries (e.g. archives).
/// 真正非值的 primary（如归档）仍报 `PrimaryNotValue`。
#[test]
fn archives_still_report_primary_not_value() {
    let blobs: [(&str, &[u8]); 1] = [("a.txt", b"hi")];
    let arc = smlb::pack(&blobs).unwrap();
    assert!(matches!(
        smlb::load(&arc).unwrap_err(),
        ContainerError::PrimaryNotValue { .. }
    ));
}

/// 测试用：把值断言成 `CanonicalValue`（无序在测试里就该 panic，而不是静默通过）。
fn cv(v: &smlb::Value) -> smlb::CanonicalValue {
    smlb::CanonicalValue::try_from(v).expect("测试值必须是有序构造的")
}
