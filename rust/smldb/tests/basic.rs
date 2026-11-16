// SPDX-License-Identifier: MulanPSL-2.0
//! Turso 后端验收：`.ambr` 入库后按 path 精确取回、标签反查、删除清索引；
//! 坏 BLOB 与重复 path 必须响亮拒绝。

use std::path::{Path, PathBuf};

use sml_value::{CanonicalValue, Value};
use smldb::{normalize_tags, validate_ambr, Doc, Error, Meta, Store};

fn db_path(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("smldb_{}_{}.db", tag, std::process::id()))
}

fn cleanup(path: &Path) {
    let s = path.to_string_lossy().into_owned();
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{s}{suffix}"));
    }
}

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

#[tokio::test]
async fn insert_and_get_by_path_round_trip_the_ambr_bytes() {
    let path = db_path("roundtrip");
    let db = Store::open(&path).await.unwrap();
    let bytes = ambr_bytes(7);
    let doc = Doc::from_ambr(bytes.clone()).unwrap();
    let id = db.insert(&meta("docs/a.sml", &["api"]), &doc).await.unwrap();

    let got = db.get(id).await.unwrap().expect("按 id 取回");
    assert_eq!(got.bytes(), bytes.as_slice());
    let by_path = db.get_by_path("docs/a.sml").await.unwrap().expect("按 path 取回");
    assert_eq!(by_path, got);
    assert!(db.get_by_path("nope").await.unwrap().is_none());
    drop(db);
    cleanup(&path);
}

#[tokio::test]
async fn tags_are_indexed_and_normalized() {
    let path = db_path("tags");
    let db = Store::open(&path).await.unwrap();
    let id = db
        .insert(&meta("docs/b.sml", &["beta", "alpha", "beta"]), &Doc::from_ambr(ambr_bytes(1)).unwrap())
        .await.unwrap();
    assert_eq!(db.meta(id).await.unwrap().unwrap().tags, ["alpha", "beta"]);
    assert_eq!(db.find_by_tag("alpha").await.unwrap(), vec![id]);
    assert_eq!(db.find_by_tag("beta").await.unwrap(), vec![id]);
    assert!(db.find_by_tag("nope").await.unwrap().is_empty());
    assert_eq!(db.list().await.unwrap().len(), 1);
    assert_eq!(normalize_tags(&["z".into(), "a".into(), "z".into()]), ["a", "z"]);
    drop(db);
    cleanup(&path);
}

#[tokio::test]
async fn duplicate_path_and_empty_path_are_rejected() {
    let path = db_path("dup");
    let db = Store::open(&path).await.unwrap();
    db.insert(&meta("docs/a.sml", &[]), &Doc::from_ambr(ambr_bytes(1)).unwrap())
        .await.unwrap();
    let dup = db.insert(&meta("docs/a.sml", &[]), &Doc::from_ambr(ambr_bytes(2)).unwrap()).await;
    assert_eq!(dup, Err(Error::DuplicatePath("docs/a.sml".into())));
    let empty = db.insert(&meta("", &[]), &Doc::from_ambr(ambr_bytes(3)).unwrap()).await;
    assert!(matches!(empty, Err(Error::BadInput(_))));
    drop(db);
    cleanup(&path);
}

#[tokio::test]
async fn non_ambr_bytes_never_become_a_doc() {
    assert!(matches!(Doc::from_ambr(b"not an ambr".to_vec()), Err(Error::InvalidAmbr(_))));
    assert!(validate_ambr(b"nope").is_err());
}

#[tokio::test]
async fn delete_clears_the_row_and_the_tag_index() {
    let path = db_path("delete");
    let db = Store::open(&path).await.unwrap();
    let id = db
        .insert(&meta("docs/c.sml", &["x"]), &Doc::from_ambr(ambr_bytes(9)).unwrap())
        .await.unwrap();
    assert!(db.delete(id).await.unwrap());
    assert!(!db.delete(id).await.unwrap(), "再删返回 false");
    assert!(db.get(id).await.unwrap().is_none());
    assert!(db.get_by_path("docs/c.sml").await.unwrap().is_none());
    assert!(db.find_by_tag("x").await.unwrap().is_empty());
    assert!(db.list().await.unwrap().is_empty());
    drop(db);
    cleanup(&path);
}
