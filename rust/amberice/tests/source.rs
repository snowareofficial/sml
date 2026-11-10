// SPDX-License-Identifier: MulanPSL-2.0
//! D46：`SOURCE`（0x50）只能显式写；默认 `dump` 不带；读取按字节透传。
//! `SOURCE` is opt-in only, never implicit; the text round-trips verbatim.

#![cfg(feature = "source")]

use amberice::{dump, dump_with_source, load, CanonicalValue, DumpOptions, Map, Store, Value};

fn doc() -> Value {
    let mut m = Map::ordered();
    m.insert("k".to_string(), Value::Int(1));
    Value::Object(m)
}

#[test]
fn source_is_explicit_and_round_trips() {
    let v = doc();
    let cv = CanonicalValue::try_from(&v).expect("ordered");
    let src = "k: 1\n// 原文注释与空白都保留\n";

    let bytes = dump_with_source(&cv, &DumpOptions::default(), src).expect("dump_with_source");

    // 1) `VALUE` 语义不变：canonical 相等 / 签名都不看 SOURCE。
    let back = load(&bytes).expect("load");
    assert_eq!(
        back.canonical_text().expect("text"),
        v.canonical_text().expect("text")
    );

    // 2) 读端拿得到原文，逐字节一致。
    let store = Store::open(&bytes).expect("open");
    assert_eq!(store.source_text().expect("utf8"), Some(src));
    assert_eq!(store.source_body(), Some(src.as_bytes()));

    // 3) 段表里确实有 0x50，且原文在文件字节里。
    assert!(store
        .info()
        .segments
        .iter()
        .any(|s| s.header.seg_id == 0x50));
    assert!(bytes.windows(src.len()).any(|w| w == src.as_bytes()));

    // 4) 默认 `dump` 绝不带 SOURCE。
    let plain = dump(&cv).expect("dump");
    let p = Store::open(&plain).expect("open plain");
    assert!(p.source_text().expect("utf8").is_none());
    assert!(!p.info().segments.iter().any(|s| s.header.seg_id == 0x50));
}

#[test]
fn duplicate_source_is_rejected() {
    use amberice::Writer;
    let mut buf = Vec::new();
    let mut w =
        Writer::new(&mut buf, &DumpOptions::default(), amberice::container::SEG_VALUE).expect("writer");
    w.put_source("a: 1\n").expect("first");
    assert!(w.put_source("a: 2\n").is_err(), "第二份原文必须被拒");
}
