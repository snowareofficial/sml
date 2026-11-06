// SPDX-License-Identifier: MulanPSL-2.0
//! `DIR` (N05) tests: index is optional, derived, and never changes an answer.
//! `DIR`（N05）测试：索引可选、派生，永远不改变答案。

use amberice::{dir, CanonicalValue, DumpOptions, Map, Store, Value};

fn nested() -> Value {
    let mut inner = Map::ordered();
    inner.insert("b".to_string(), Value::Int(1));
    inner.insert("c".to_string(), Value::Str("x".to_string()));

    let mut mid = Map::ordered();
    mid.insert("a".to_string(), Value::Object(inner));
    mid.insert("top".to_string(), Value::Int(7));

    let mut root = Map::ordered();
    root.insert("m".to_string(), Value::Object(mid));
    root.insert("list".to_string(), Value::Array(vec![Value::Int(1), Value::Int(2)]));
    Value::Object(root)
}

fn dump(dir: bool, depth: u32) -> (Vec<u8>, Vec<u8>) {
    let v = nested();
    let cv = CanonicalValue::try_from(&v).expect("ordered");
    let plain = amberice::dump(&cv).expect("dump");
    let opts = DumpOptions {
        dir,
        dir_depth: depth,
        ..Default::default()
    };
    let with = amberice::dump_with(&cv, &opts).expect("dump_with");
    (plain, with)
}

#[test]
fn dir_is_optional_and_answer_preserving() {
    let (plain, with) = dump(true, 4);
    assert!(with.len() > plain.len(), "带 DIR 的文件应当更大");

    let a = Store::open(&plain).expect("open plain");
    let b = Store::open(&with).expect("open with dir");
    assert!(a.dir_body().is_none());
    assert!(b.dir_body().is_some(), "DIR 段应当存在");

    for path in ["m", "m.a", "m.a.b", "m.top", "list"] {
        assert_eq!(
            a.get(path).expect("plain get").canonical_text().unwrap(),
            b.get(path).expect("dir get").canonical_text().unwrap(),
            "路径 {path} 在两份文件里应当同值"
        );
    }
    // 整树也必须一致
    assert_eq!(
        a.value().unwrap().canonical_text().unwrap(),
        b.value().unwrap().canonical_text().unwrap()
    );
}

#[test]
fn dir_depth_zero_still_reads() {
    // 不索引任何层：`DIR` 段为空表，`get` 必须回退线性走。
    let (_, with) = dump(true, 0);
    let s = Store::open(&with).expect("open");
    let body = s.dir_body().expect("DIR 段仍应写出");
    let parsed = dir::parse(body).expect("parse DIR");
    assert!(parsed.entries.is_empty(), "depth=0 不应有索引项");
    assert_eq!(s.get("m.a.b").unwrap().as_i64(), Some(1));
}

#[test]
fn dir_entries_are_sorted_and_findable() {
    let (_, with) = dump(true, 4);
    let s = Store::open(&with).expect("open");
    let parsed = dir::parse(s.dir_body().expect("DIR")).expect("parse");
    assert!(!parsed.entries.is_empty());
    assert!(
        parsed
            .entries
            .windows(2)
            .all(|w| (w[0].parent, w[0].key) <= (w[1].parent, w[1].key)),
        "索引项必须按 (parent, key) 升序"
    );
    // 根对象 0 上，键 `m` 的项应当能二分到
    let key_id = parsed
        .entries
        .iter()
        .find(|e| e.parent == 0)
        .map(|e| e.key)
        .expect("根对象至少有一项");
    assert!(parsed.find(0, key_id).is_some());
}

#[test]
fn dir_body_roundtrips_through_build() {
    let v = nested();
    let cv = CanonicalValue::try_from(&v).expect("ordered");
    let blocks = cv.canonical_blocks().expect("blocks");
    let body = dir::build(&blocks.value, 3, 4).expect("build");
    let parsed = dir::parse(&body).expect("parse");
    assert_eq!(parsed.head_layer, 3);
    assert!(!parsed.entries.is_empty());
    // head_layer 不匹配 ⇒ Store 必须忽略索引（这里只验证字段被读出来）
    assert_eq!(parsed.head_layer, 3);
}
