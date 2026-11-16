// SPDX-License-Identifier: MulanPSL-2.0
//! Integration smoke tests for smlite: value / parse / emit / roundtrip / patch.
//! smlite 集成冒烟测试：值 / 解析 / 渲染 / roundtrip / patch。

use smlite::{merge_patch, parse, parse_str, to_string, to_vec, ErrorKind, Value};

#[test]
fn value_from_and_accessors() {
    let v = Value::from(42i64);
    assert_eq!(v.as_int(), Some(42));
    assert_eq!(Value::from(1.5f64).as_float(), Some(1.5));
    assert_eq!(Value::from(true).as_bool(), Some(true));
    assert_eq!(Value::from("hi").as_str(), Some("hi"));
    assert_eq!(Value::Null.get("x"), None);
}

#[test]
fn parse_document_forms() {
    let implicit = parse_str("a: 1 b: two").unwrap();
    let explicit = parse_str("{a: 1 b: two}").unwrap();
    assert_eq!(implicit, explicit);

    let arr = parse(b"[1 2 3]").unwrap();
    assert_eq!(arr.as_array().unwrap().len(), 3);

    // SML 具名块 `key { ... }`：无需冒号，整体作为隐式对象的条目。
    let named = parse_str("b0 { x: 1 y: s }").unwrap();
    let obj = named.get("b0").and_then(Value::as_object).unwrap();
    assert_eq!(obj.get("x").and_then(Value::as_int), Some(1));
    assert_eq!(obj.get("y").and_then(Value::as_str), Some("s"));
}

#[test]
fn parse_errors_have_kinds() {
    let e = parse_str("a: 1 b 2").unwrap_err();
    assert_eq!(e.kind(), ErrorKind::ExpectedColon);
    assert!(e.pos() <= 8);
    assert_eq!(parse_str("a: \"\\q\"").unwrap_err().kind(), ErrorKind::InvalidEscape);
}

#[test]
fn duplicate_keys_last_wins_first_position() {
    let v = parse_str("a: 1 b: 2 a: 3").unwrap();
    let obj = v.as_object().unwrap();
    assert_eq!(obj.len(), 2);
    assert_eq!(obj.get("a").and_then(Value::as_int), Some(3));
    let order: Vec<&str> = obj.iter().map(|(k, _)| k).collect();
    assert_eq!(order, ["a", "b"]);
}

#[test]
fn roundtrip_preserves_value() {
    let src = "name: John age: 27 active: true tags: [a b c] note: \"hello world\" nested: { x: 1 y: z }";
    let v = parse_str(src).unwrap();
    let text = to_string(&v);
    let back = parse_str(&text).unwrap();
    assert_eq!(v, back, "roundtrip mismatch: {text}");
    assert_eq!(to_vec(&v), text.into_bytes());
}

#[test]
fn merge_patch_rfc7396() {
    let base = parse_str("title: old author: { first: A last: B } tags: [x y]").unwrap();
    let patch = parse_str("title: null author: { last: C } tags: [z] extra: 1").unwrap();
    let out = merge_patch(&base, &patch).unwrap();

    assert_eq!(out.get("title"), None);
    let author = out.get("author").and_then(Value::as_object).unwrap();
    assert_eq!(author.get("first").and_then(Value::as_str), Some("A"));
    assert_eq!(author.get("last").and_then(Value::as_str), Some("C"));
    assert_eq!(out.get("tags").and_then(Value::as_array).unwrap().len(), 1);
    assert_eq!(out.get("extra").and_then(Value::as_int), Some(1));
}

#[cfg(feature = "swsml-compat")]
#[test]
fn interop_with_sml_value() {
    let v = parse_str("a: 1 b: [x y] c: 1.5").unwrap();
    let sv: sml_value::Value = v.clone().into();
    assert!(matches!(sv, sml_value::Value::Object(_)));
    let back = Value::try_from(sv).unwrap();
    assert_eq!(v, back);
}

#[cfg(feature = "object-sort")]
#[test]
fn object_sort_backend_roundtrip() {
    let v = parse_str("z: 1 a: 2 m: 3").unwrap();
    let text = to_string(&v);
    assert_eq!(text, "{a: 2 m: 3 z: 1}");
    assert_eq!(parse_str(&text).unwrap(), v);
}
