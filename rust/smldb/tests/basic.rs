// SPDX-License-Identifier: MulanPSL-2.0
//! 第一刀验收：合法的 `.ambr` 能入库、按 path 精确取回、按标签反查、删除后索引同步清掉；
//! 非法字节与重复 path 必须响亮拒绝。

use sml_value::{CanonicalValue, Value};
use smldb::{normalize_tags, Backend, Doc, DocId, Error, MemoryBackend, Meta};

fn ambr_bytes(n: i64) -> Vec<u8> {
    let cv = CanonicalValue::try_from(Value::Int(n)).expect("标量可证明键序");
    amberice::dump(&cv).expect("生成合法 .ambr")
}

fn meta(path: &str, tags: &[&str]) -> Meta {
    Meta {
        path: path.into(),
        title: format!("title of {path}"),
        created: "2026-11-17T00:00:00Z".into(),
        modified: "2026-11-17T00:00:00Z".into(),
        tags: tags.iter().map(|s| (*s).into()).collect(),
    }
}

#[test]
fn insert_and_get_by_path_round_trip_the_ambr_bytes() {
    let bytes = ambr_bytes(7);
    let mut db = MemoryBackend::new();
    let doc = Doc::from_ambr(bytes.clone()).unwrap();
    let id = db.insert(meta("docs/a.sml", &["api"]), doc).unwrap();

    let got = db.get(id).unwrap().expect("按 id 取回");
    assert_eq!(got.bytes(), bytes.as_slice());
    let by_path = db.get_by_path("docs/a.sml").unwrap().expect("按 path 取回");
    assert_eq!(by_path, got, "同一个文档");
    assert!(db.get_by_path("nope").unwrap().is_none());
}

#[test]
fn tags_are_indexed_and_normalized() {
    let mut db = MemoryBackend::new();
    let id = db
        .insert(
            meta("docs/b.sml", &["beta", "alpha", "beta"]),
            Doc::from_ambr(ambr_bytes(1)).unwrap(),
        )
        .unwrap();
    assert_eq!(db.meta(id).unwrap().unwrap().tags, ["alpha", "beta"]);
    assert_eq!(db.find_by_tag("alpha").unwrap(), vec![id]);
    assert_eq!(db.find_by_tag("beta").unwrap(), vec![id]);
    assert!(db.find_by_tag("nope").unwrap().is_empty());
    assert_eq!(normalize_tags(&["z".into(), "a".into(), "z".into()]), ["a", "z"]);
}

#[test]
fn duplicate_path_and_empty_path_are_rejected() {
    let mut db = MemoryBackend::new();
    db.insert(meta("docs/a.sml", &[]), Doc::from_ambr(ambr_bytes(1)).unwrap())
        .unwrap();
    let dup = db.insert(meta("docs/a.sml", &[]), Doc::from_ambr(ambr_bytes(2)).unwrap());
    assert_eq!(dup, Err(Error::DuplicatePath("docs/a.sml".into())));
    let empty = db.insert(meta("", &[]), Doc::from_ambr(ambr_bytes(3)).unwrap());
    assert!(matches!(empty, Err(Error::BadInput(_))));
}

#[test]
fn non_ambr_bytes_never_enter_the_store() {
    let bad = Doc::from_ambr(b"not an ambr container".to_vec());
    assert!(matches!(bad, Err(Error::InvalidAmbr(_))));
    assert!(validate_bad_bytes_is_enough());
}

fn validate_bad_bytes_is_enough() -> bool {
    smldb::validate_ambr(b"nope").is_err()
}

#[test]
fn delete_clears_both_the_row_and_the_path_index() {
    let mut db = MemoryBackend::new();
    let id: DocId = db
        .insert(meta("docs/c.sml", &["x"]), Doc::from_ambr(ambr_bytes(9)).unwrap())
        .unwrap();
    assert!(db.delete(id).unwrap());
    assert!(!db.delete(id).unwrap(), "再删一次返回 false");
    assert!(db.get(id).unwrap().is_none());
    assert!(db.get_by_path("docs/c.sml").unwrap().is_none());
    assert!(db.find_by_tag("x").unwrap().is_empty());
    assert!(db.list().unwrap().is_empty());
}
