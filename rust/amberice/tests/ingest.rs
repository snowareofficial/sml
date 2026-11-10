// SPDX-License-Identifier: MulanPSL-2.0
//! D53：批量归档 —— N 帧合成一层、单写者、崩溃后截断恢复。

use std::path::PathBuf;

use amberice::{
    dump, layers, load, CanonicalValue, HashAlgo, History, IngestOptions, Ingestor, Map, Value,
};

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::ordered();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}

fn base_bytes() -> Vec<u8> {
    let v = obj(&[("a", Value::Int(0)), ("keep", s("base"))]);
    dump(&CanonicalValue::try_from(&v).expect("ordered")).expect("dump")
}

fn tmp(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "amberice-ingest-{}-{name}.ambr",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&p);
    p
}

fn write_base(p: &PathBuf) {
    std::fs::write(p, base_bytes()).expect("write base");
}

#[test]
fn batch_keep_writes_one_layer_per_batch() {
    let p = tmp("keep");
    write_base(&p);
    let opts = IngestOptions {
        frames_per_layer: 2,
        max_delay_ms: 0,
        fsync: false,
        history: History::Keep,
    };
    let mut ing = Ingestor::open(&p, opts).expect("open");

    let o1 = ing.push(obj(&[("a", Value::Int(1))])).expect("push 1");
    assert!(!o1.committed, "第一帧还不到阈值");
    let o2 = ing.push(obj(&[("a", Value::Int(2))])).expect("push 2");
    assert!(o2.committed, "第 2 帧应触发提交");

    ing.push(obj(&[("a", Value::Int(3))])).expect("push 3");
    let c = ing.commit().expect("commit");
    assert_eq!(c.layers, 2, "两批 = 两层");
    assert_eq!(ing.layer_count(), 2);

    let bytes = std::fs::read(&p).expect("read");
    let v = load(&bytes).expect("load");
    let Value::Object(m) = &v else { panic!("root") };
    assert_eq!(m.get("a"), Some(&Value::Int(3)));
    assert_eq!(m.get("keep"), Some(&s("base")), "未触及的键保持");

    let ls = layers(&bytes).expect("layers");
    assert!(ls.iter().all(|l| l.author == [0u8; 32] && l.sigs.is_empty()));
    let _ = std::fs::remove_file(&p);
}

#[test]
fn batch_drop_rewrites_and_leaves_no_layers() {
    let p = tmp("drop");
    write_base(&p);
    let opts = IngestOptions {
        frames_per_layer: 1,
        max_delay_ms: 0,
        fsync: false,
        history: History::Drop,
    };
    let mut ing = Ingestor::open(&p, opts).expect("open");
    ing.push(obj(&[("a", Value::Int(7))])).expect("push");
    let c = ing.commit().expect("commit");
    assert_eq!(c.layers, 0, "drop 不留层");

    let bytes = std::fs::read(&p).expect("read");
    assert!(layers(&bytes).expect("layers").is_empty());
    let v = load(&bytes).expect("load");
    let Value::Object(m) = &v else { panic!("root") };
    assert_eq!(m.get("a"), Some(&Value::Int(7)));
    let _ = std::fs::remove_file(&p);
}

#[test]
fn recover_truncates_a_partial_tail() {
    let p = tmp("recover");
    write_base(&p);
    let one = amberice::append_unsigned(
        &page(&p),
        Some(obj(&[("a", Value::Int(9))])),
        HashAlgo::Sha256,
    )
    .expect("append");
    std::fs::write(&p, &one).expect("write");
    // 模拟崩溃：尾部多出半截记录（不足一个 Trailer）
    let mut broken = one.clone();
    broken.extend_from_slice(&[0u8; 20]);
    std::fs::write(&p, &broken).expect("write broken");

    let mut ing = Ingestor::open(&p, IngestOptions::default()).expect("open");
    let r = ing.recover().expect("recover");
    assert_eq!(r.truncated_bytes, 20);
    assert!(r.dropped_tail);
    assert_eq!(ing.layer_count(), 1);

    let bytes = std::fs::read(&p).expect("read");
    let v = load(&bytes).expect("load");
    let Value::Object(m) = &v else { panic!("root") };
    assert_eq!(m.get("a"), Some(&Value::Int(9)));
    let _ = std::fs::remove_file(&p);
}

fn page(p: &PathBuf) -> Vec<u8> {
    std::fs::read(p).expect("read")
}
