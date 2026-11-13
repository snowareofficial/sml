// SPDX-License-Identifier: MulanPSL-2.0
//! `StreamReader`：按需读载荷，不把整份归档载入内存。

use std::io::Cursor;

use amberice::stream::StreamReader;

#[test]
fn stream_info_matches_whole_file_info() {
    let items: [(&str, &[u8]); 3] = [
        ("a.txt", b"hello"),
        ("b/c.smlex", b"name: x\n"),
        ("d.bin", &[0u8, 1, 2, 3, 4][..]),
    ];
    let bytes = amberice::pack_with(&items, &amberice::DumpOptions::default()).unwrap();

    let mut r = StreamReader::open(Cursor::new(bytes.clone())).unwrap();
    assert_eq!(
        r.info(),
        &amberice::info(&bytes).unwrap(),
        "流式 info 必须与整份 info 一致"
    );

    assert_eq!(r.payload("a.txt").unwrap(), b"hello");
    assert_eq!(r.payload("b/c.smlex").unwrap(), b"name: x\n");
    assert_eq!(r.payload("d.bin").unwrap(), &[0u8, 1, 2, 3, 4][..]);
    assert!(
        r.payload("nope").is_err(),
        "缺失载荷必须报错，而不是返回空"
    );

    let names: Vec<String> = r
        .payloads()
        .unwrap()
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, vec!["a.txt", "b/c.smlex", "d.bin"]);
}

#[test]
fn stream_reports_bad_magic() {
    let mut bytes = amberice::pack_with(&[("a", b"x")], &amberice::DumpOptions::default()).unwrap();
    bytes[0] = b'X';
    assert!(StreamReader::open(Cursor::new(bytes)).is_err());
}
