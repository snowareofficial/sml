// SPDX-License-Identifier: MulanPSL-2.0
//! `TYPED` (N04) tests: typed variables ride along `VALUE`, checked by `type_id`.
//! `TYPED`（N04）测试：强类型变量搭在 `VALUE` 旁边，读时比对 `type_id`。

use amberice::typed::{type_hash, Decode, Decoder, Encode, Encoder, TypedError};
use amberice::container::{SEG_STRTAB, SEG_VALUE};
use amberice::{CanonicalValue, ContainerError, DumpOptions, Map, Store, Value, Writer};

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

/// `User` 的内嵌 schema（字段顺序必须与 `Encode` 一致）。
/// Embedded schema for `User`; field order must match `Encode`.
fn user_schema() -> Value {
    let mut id = Map::ordered();
    id.insert("id".to_string(), Value::Int(1));
    id.insert("name".to_string(), Value::Str("id".to_string()));
    id.insert("ty".to_string(), Value::Str("u64".to_string()));

    let mut name = Map::ordered();
    name.insert("id".to_string(), Value::Int(2));
    name.insert("name".to_string(), Value::Str("name".to_string()));
    name.insert("ty".to_string(), Value::Str("str".to_string()));

    let mut vec_str = Map::ordered();
    vec_str.insert("vec".to_string(), Value::Str("str".to_string()));
    let mut tags = Map::ordered();
    tags.insert("id".to_string(), Value::Int(3));
    tags.insert("name".to_string(), Value::Str("tags".to_string()));
    tags.insert("ty".to_string(), Value::Object(vec_str));

    let mut opt_str = Map::ordered();
    opt_str.insert("opt".to_string(), Value::Str("str".to_string()));
    let mut note = Map::ordered();
    note.insert("id".to_string(), Value::Int(4));
    note.insert("name".to_string(), Value::Str("note".to_string()));
    note.insert("ty".to_string(), Value::Object(opt_str));

    let mut ty = Map::ordered();
    ty.insert(
        "id".to_string(),
        Value::Str(format!("{USER_TYPE:016x}")),
    );
    ty.insert("name".to_string(), Value::Str("User".to_string()));
    ty.insert("kind".to_string(), Value::Str("struct".to_string()));
    ty.insert(
        "fields".to_string(),
        Value::Array(vec![
            Value::Object(id),
            Value::Object(name),
            Value::Object(tags),
            Value::Object(note),
        ]),
    );

    let mut top = Map::ordered();
    top.insert("schema_version".to_string(), Value::Int(1));
    top.insert("types".to_string(), Value::Array(vec![Value::Object(ty)]));
    Value::Object(top)
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
    let mut t = amberice::typed::Typed::new();
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
    let mut t = amberice::typed::Typed::new();
    let raw = amberice::typed::Bytes(vec![0u8, 1, 2, 127, 128, 255]);
    t.put(7, &raw).expect("put");
    let body = t.to_bytes();
    // 段体 = 目录（count 1 + 项 1+8+4+4）+ 载荷（长度 1 + 6 字节）≈ 25 B。
    // 若退化成「每个字节一个 varint」，仅 6 个原始字节就要 11 B 以上且语义不同。
    assert!(body.len() < 32, "Bytes 应当紧凑编码，实际 {} B", body.len());
    let back: amberice::typed::Bytes = amberice::typed::read_var(&body, 7).expect("read");
    assert_eq!(back, raw);
}

/// D45：`SCHEMA` 是可选附加段；写入后可读回，且不影响 `TYPED` / `VALUE`。
#[test]
fn schema_segment_is_optional_and_round_trips() {
    let v = doc();
    let cv = CanonicalValue::try_from(&v).expect("ordered");
    let blocks = cv.canonical_blocks().expect("blocks");
    let schema = {
        let mut ty = Map::ordered();
        ty.insert("id".to_string(), Value::Str("1a2b3c4d5e6f7788".to_string()));
        ty.insert("name".to_string(), Value::Str("User".to_string()));
        ty.insert("kind".to_string(), Value::Str("struct".to_string()));
        let mut field = Map::ordered();
        field.insert("id".to_string(), Value::Int(1));
        field.insert("name".to_string(), Value::Str("id".to_string()));
        field.insert("ty".to_string(), Value::Str("u64".to_string()));
        ty.insert("fields".to_string(), Value::Array(vec![Value::Object(field)]));
        let mut top = Map::ordered();
        top.insert("schema_version".to_string(), Value::Int(1));
        top.insert("types".to_string(), Value::Array(vec![Value::Object(ty)]));
        Value::Object(top)
    };

    let mut buf = Vec::new();
    {
        let mut w = Writer::new(&mut buf, &DumpOptions::default(), SEG_VALUE).expect("writer");
        w.add_segment(SEG_VALUE, 0, 0, &blocks.value).expect("VALUE");
        if !blocks.strtab.is_empty() {
            w.add_segment(SEG_STRTAB, 0, 0, &blocks.strtab).expect("STRTAB");
        }
        w.put_var(1, &user()).expect("put_var");
        w.put_schema(&schema).expect("put_schema");
        w.finish().expect("finish");
    }

    let s = Store::open(&buf).expect("open");
    assert_eq!(s.schema().expect("schema"), Some(schema));
    assert!(s.schema_body().is_some());
    assert_eq!(s.var::<User>(1).expect("var"), user());
    assert!(amberice::check(&buf).expect("check").ok);

    // 不写 schema 时，读侧必须给 None（不是空对象）。
    let plain = build_with_vars();
    let s2 = Store::open(&plain).expect("open plain");
    assert!(s2.schema().expect("schema").is_none());
}

/// D45：schema 驱动的动态渲染要覆盖标量 / 字符串 / vec / opt。
#[test]
fn documented_renders_nested_fields() {
    let v = doc();
    let cv = CanonicalValue::try_from(&v).expect("ordered");
    let blocks = cv.canonical_blocks().expect("blocks");
    let schema = user_schema();

    let mut buf = Vec::new();
    {
        let mut w = Writer::new(&mut buf, &DumpOptions::default(), SEG_VALUE).expect("writer");
        w.add_segment(SEG_VALUE, 0, 0, &blocks.value).expect("VALUE");
        if !blocks.strtab.is_empty() {
            w.add_segment(SEG_STRTAB, 0, 0, &blocks.strtab).expect("STRTAB");
        }
        w.put_var(1, &user()).expect("put_var");
        w.put_schema(&schema).expect("put_schema");
        w.finish().expect("finish");
    }

    let store = Store::open(&buf).expect("open");
    let vars = amberice::typed::documented(store.typed_body().expect("typed"), &schema)
        .expect("documented");
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0].id, 1);
    assert_eq!(vars[0].name, "User");

    let Value::Object(m) = &vars[0].value else {
        panic!("应为对象");
    };
    assert_eq!(m.get("id"), Some(&Value::Int(42)));
    let want_name = Value::Str("ada".to_string());
    assert_eq!(m.get("name"), Some(&want_name));
    assert_eq!(m.get("note"), Some(&Value::Null));
    let Some(Value::Array(tags)) = m.get("tags") else {
        panic!("tags 应为数组");
    };
    assert_eq!(tags.len(), 2);
}

