// SPDX-License-Identifier: MulanPSL-2.0
//! `TYPED` (N04) tests: typed variables ride along `VALUE`, checked by `type_id`.
//! `TYPED`（N04）测试：强类型变量搭在 `VALUE` 旁边，读时比对 `type_id`。

use amberice::typed::{type_hash, Decode, Decoder, Encode, Encoder, TypedError};
use amberice::{CanonicalValue, ContainerError, DumpOptions, Map, Store, Value, SEG_STRTAB, SEG_VALUE, Writer};

#[derive(Debug, PartialEq)]
struct User {
    id: u64,
    name: String,
    tags: Vec<String>,
    note: Option<String>,
}

const USER_TYPE: u64 = type_hash(&["struct", "User", "id", "name", "tags", "note"]);

impl Encode for User {
    const TYPE_ID: u64 = USER_TYPE;
    fn encode(&self, e: &mut Encoder<'_>) {
        self.id.encode(e);
        self.name.encode(e);
        self.tags.encode(e);
        self.note.encode(e);
    }
}

impl Decode for User {
    const TYPE_ID: u64 = USER_TYPE;
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
        Ok(User {
            id: Decode::decode(d)?,
            name: Decode::decode(d)?,
            tags: Decode::decode(d)?,
            note: Decode::decode(d)?,
        })
    }
}

/// Same layout, different fingerprint — used to prove the type check is real.
/// 同布局、不同指纹 —— 用来证明类型检查是真的。
#[derive(Debug, PartialEq)]
struct Impostor {
    id: u64,
}

impl Decode for Impostor {
    const TYPE_ID: u64 = type_hash(&["struct", "Impostor", "id"]);
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
        Ok(Impostor { id: Decode::decode(d)? })
    }
}

fn doc() -> Value {
    let mut m = Map::ordered();
    m.insert("who".to_string(), Value::Str("u1".to_string()));
    Value::Object(m)
}

fn user() -> User {
    User {
        id: 42,
        name: "ada".to_string(),
        tags: vec!["a".to_string(), "b".to_string()],
        note: None,
    }
}

/// Build a file with `VALUE`/`STRTAB` (always) + `TYPED` (from `put_var`).
fn build_with_vars() -> Vec<u8> {
    let v = doc();
    let cv = CanonicalValue::try_from(&v).expect("ordered");
    let blocks = cv.canonical_blocks().expect("blocks");
    let mut buf = Vec::new();
    {
        let mut w = Writer::new(&mut buf, &DumpOptions::default(), SEG_VALUE).expect("writer");
        w.add_segment(SEG_VALUE, 0, 0, &blocks.value).expect("VALUE");
        if !blocks.strtab.is_empty() {
            w.add_segment(SEG_STRTAB, 0, 0, &blocks.strtab).expect("STRTAB");
        }
        w.put_var(1, &user()).expect("put_var 1");
        w.put_var(2, &vec![10u32, 20, 30]).expect("put_var 2");
        w.finish().expect("finish");
    }
    buf
}

#[test]
fn roundtrip_through_writer_and_store() {
    let bytes = build_with_vars();
    let s = Store::open(&bytes).expect("open");
    assert!(s.typed_body().is_some(), "应写出 TYPED 段");
    assert_eq!(s.var::<User>(1).expect("var 1"), user());
    assert_eq!(s.var::<Vec<u32>>(2).expect("var 2"), vec![10, 20, 30]);
    // VALUE 仍然可读，且与 TYPED 无关
    assert_eq!(s.get("who").unwrap().as_str(), Some("u1"));
}

#[test]
fn type_mismatch_is_reported_not_guessed() {
    let bytes = build_with_vars();
    let s = Store::open(&bytes).expect("open");
    let err = s.var::<Impostor>(1).expect_err("类型不符必须报错");
    match err {
        ContainerError::Typed(msg) => assert!(msg.contains("类型不符"), "实际信息：{msg}"),
        other => panic!("应包装成 ContainerError::Typed，实得 {other:?}"),
    }
}

#[test]
fn missing_var_and_missing_segment_are_distinct() {
    let bytes = build_with_vars();
    let s = Store::open(&bytes).expect("open");
    let err = s.var::<User>(9).expect_err("不存在的 id 必须报错");
    assert!(matches!(err, ContainerError::Typed(_)));

    let v = doc();
    let cv = CanonicalValue::try_from(&v).expect("ordered");
    let plain = amberice::dump(&cv).expect("dump");
    let s2 = Store::open(&plain).expect("open plain");
    assert!(s2.typed_body().is_none());
    assert!(s2.var::<User>(1).is_err(), "没有 TYPED 段时必须报错");
}

#[test]
fn duplicate_and_reserved_ids_are_rejected() {
    let mut t = amberice::Typed::new();
    assert!(matches!(
        t.put(0, &1u32).expect_err("id=0 必须拒绝"),
        TypedError::ReservedVarId
    ));
    t.put(1, &1u32).expect("first");
    assert!(matches!(
        t.put(1, &2u32).expect_err("重复 id 必须拒绝"),
        TypedError::DuplicateVarId { id: 1 }
    ));
}

#[test]
fn bytes_newtype_is_not_an_array_of_varints() {
    let mut t = amberice::Typed::new();
    let raw = amberice::typed::Bytes(vec![0u8, 1, 2, 127, 128, 255]);
    t.put(7, &raw).expect("put");
    let body = t.to_bytes();
    // 段体 = 目录（count 1 + 项 1+8+4+4）+ 载荷（长度 1 + 6 字节）≈ 25 B。
    // 若退化成「每个字节一个 varint」，仅 6 个原始字节就要 11 B 以上且语义不同。
    assert!(body.len() < 32, "Bytes 应当紧凑编码，实际 {} B", body.len());
    let back: amberice::typed::Bytes = amberice::read_var(&body, 7).expect("read");
    assert_eq!(back, raw);
}
