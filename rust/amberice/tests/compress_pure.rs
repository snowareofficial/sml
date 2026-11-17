// SPDX-License-Identifier: MulanPSL-2.0
//! `compress-pure`（纯 Rust 的 ruzstd，只解压）的验收。
//!
//! 关键点：被测的文件是 **zstd C 绑定写的**（`examples/showcase-zstd.ambr`），
//! 由纯 Rust 解码器读 —— 两套实现之间的差分测试，而不是自己跟自己对拍。
//! 只在「开了 `compress-pure` 且**没**开 `compress`」时运行：两者同开时走 C 后端。
#![cfg(all(feature = "compress-pure", not(feature = "compress")))]

use amberice::{container, Value};

/// 1694 B 的真实文件：段级 zstd（`compress: Some(9)`）由 C 绑定写出。
const FILE: &[u8] = include_bytes!("../examples/showcase-zstd.ambr");

#[test]
fn compressed_segment_is_declared() {
    let info = amberice::info(FILE).expect("info");
    assert!(
        info.segments.iter().any(|s| s.header.comp != 0),
        "这份夹具必须带压缩段，否则测不到纯 Rust 解压"
    );
}

#[test]
fn loads_file_written_by_the_c_backend() {
    let v = amberice::load(FILE).expect("纯 Rust 解压应当读得回 C 绑定的产物");
    assert!(matches!(v, Value::Object(_)), "根必须是对象");
}

#[test]
fn check_passes() {
    let report = amberice::check(FILE).expect("check");
    assert!(report.ok, "{:?}", report.lines);
}

/// 炸弹防护：同一个帧，上限设成比真实输出小 ⇒ 必须报错，而不是先分配再发现。
#[test]
fn output_cap_is_enforced() {
    let info = amberice::info(FILE).unwrap();
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.comp != 0)
        .expect("有压缩段");
    let raw = container::body(FILE, seg).expect("段体");

    assert!(
        container::inflate_zstd(raw, 8).is_err(),
        "上限 8 B 必须拒绝一个更大的帧"
    );
    assert!(
        container::inflate_zstd(raw, 1 << 20).is_ok(),
        "正常上限应当解得开"
    );
}

/// 数据损坏 ⇒ 明确报错，不 panic、不返回半截值。
#[test]
fn corrupted_frame_is_an_error() {
    let info = amberice::info(FILE).unwrap();
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.comp != 0)
        .expect("有压缩段");
    let raw = container::body(FILE, seg).expect("段体");

    let mut bad = raw.to_vec();
    for b in bad.iter_mut().take(8) {
        *b ^= 0xA5;
    }
    assert!(container::inflate_zstd(&bad, 1 << 20).is_err());
}
