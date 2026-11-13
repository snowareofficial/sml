// SPDX-License-Identifier: MulanPSL-2.0
//! `USER`（`0xF0`）扩展段：打包可带、`info` 可看、默认不写。

use amberice::container::{SEG_USER, UserSegment};

fn opts_with_pkg() -> amberice::DumpOptions {
    let mut opts = amberice::DumpOptions::default();
    opts.user_segments.push(UserSegment {
        seg_id: SEG_USER,
        submagic: *b"PKG\0",
        version: 1,
        payload: b"name: api-docs\nversion: 1.0.0\nentry: api-docs.smlex\n".to_vec(),
    });
    opts
}

#[test]
fn user_segment_is_written_and_reported() {
    let bytes = amberice::pack_with(
        &[("api-docs.smlex", b"name: api-docs\nchecks: []\n")],
        &opts_with_pkg(),
    )
    .unwrap();

    let info = amberice::info(&bytes).unwrap();
    assert_eq!(
        info.segments.iter().filter(|s| s.header.seg_id == SEG_USER).count(),
        1,
        "应当恰好写一个 USER 段"
    );

    let lines = amberice::container::info_lines("pkg.ambr", &bytes, &info).unwrap();
    let joined = lines.join("\n");
    assert!(joined.contains("PKG"), "info 没展示 submagic：{joined}");
    assert!(joined.contains("v1"), "info 没展示版本：{joined}");

    // 载荷仍然可读：USER 段不参与 VALUE/EMBED 语义。
    let body = amberice::payload(&bytes, "api-docs.smlex").unwrap();
    assert_eq!(&*body, b"name: api-docs\nchecks: []\n");
}

#[test]
fn no_user_segment_by_default() {
    let bytes = amberice::pack_with(
        &[("a", b"x")],
        &amberice::DumpOptions::default(),
    )
    .unwrap();
    let info = amberice::info(&bytes).unwrap();
    assert!(
        info.segments.iter().all(|s| s.header.seg_id != SEG_USER),
        "默认不应写 USER 段（否则旧文件字节会变）"
    );
}
