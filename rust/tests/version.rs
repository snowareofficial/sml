// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0
//! 验证 SML 版本演进语义：
//!  - V1 默认（裸词即字符串，向后兼容）
//!  - V3 严格（自由字符串必须引号，否则报错）
//!  - 版本范围限制（parse_allowed）

use sml::{parse, parse_allowed, parse_versioned, Value, Version};

#[test]
fn v1_default_bareword_is_string() {
    let v = parse("name: John\nage: 27\n").unwrap();
    assert_eq!(v.get("name"), Some(&Value::Str("John".into())));
    assert_eq!(v.get("age"), Some(&Value::Int(27)));
}

#[test]
fn v1_explicit_bareword_is_string() {
    let v = parse("@version v1\nname: John\n").unwrap();
    assert_eq!(v.get("name"), Some(&Value::Str("John".into())));
}

#[test]
fn v3_bareword_string_rejected() {
    let r = parse("@version v3\nname: John\n");
    assert!(r.is_err(), "v3 裸词字符串应被拒绝: {:?}", r);
    let msg = r.unwrap_err();
    assert!(msg.contains("引号") || msg.contains("\""), "错误信息应提示加引号: {msg}");
}

#[test]
fn v3_quoted_string_ok() {
    let v = parse("@version v3\nname: \"John\"\nage: 27\nactive: true\n").unwrap();
    assert_eq!(v.get("name"), Some(&Value::Str("John".into())));
    assert_eq!(v.get("age"), Some(&Value::Int(27)));
    assert_eq!(v.get("active"), Some(&Value::Bool(true)));
}

#[test]
fn v3_scalars_still_bareword() {
    let v = parse("@version v3\nn: 42\nf: 3.14\nb: true\nz: null\nref: &frag\n").unwrap();
    assert_eq!(v.get("n"), Some(&Value::Int(42)));
    assert_eq!(v.get("f"), Some(&Value::Float(3.14)));
    assert_eq!(v.get("b"), Some(&Value::Bool(true)));
    assert_eq!(v.get("z"), Some(&Value::Null));
    assert_eq!(v.get("ref"), Some(&Value::Str("&frag".into())));
}

#[test]
fn v3_array_bareword_rejected() {
    let r = parse("@version v3\ntags: [ a b c ]\n");
    assert!(r.is_err(), "v3 数组裸词元素应拒绝");
}

#[test]
fn v3_array_quoted_ok() {
    let v = parse("@version v3\ntags: [ \"a\" \"b\" \"c\" ]\n").unwrap();
    match v.get("tags") {
        Some(Value::Array(a)) => assert_eq!(a.len(), 3),
        other => panic!("期望数组, 得 {:?}", other),
    }
}

#[test]
fn v2_synonym_for_v3_strict() {
    let r = parse("@version v2\nname: John\n");
    assert!(r.is_err(), "v2 同 v3 语义，裸词字符串应拒绝");
}

#[test]
fn parse_versioned_reports_declared() {
    let (v, ver) = parse_versioned("@version v3\nx: 1\n").unwrap();
    assert_eq!(ver, Version::V3);
    assert_eq!(v.get("x"), Some(&Value::Int(1)));

    let (_, ver2) = parse_versioned("x: 1\n").unwrap();
    assert_eq!(ver2, Version::V1);
}

#[test]
fn parse_allowed_accepts_in_range() {
    let v = parse_allowed(
        "@version v3\nname: \"John\"\n",
        &[Version::V1, Version::V2, Version::V3],
    )
    .unwrap();
    assert_eq!(v.get("name"), Some(&Value::Str("John".into())));
}

#[test]
fn parse_allowed_rejects_out_of_range() {
    let r = parse_allowed("@version v3\nname: \"John\"\n", &[Version::V1]);
    assert!(r.is_err(), "超出版本范围应拒绝");
    let msg = r.unwrap_err();
    assert!(msg.contains("版本范围"), "应提示版本范围: {msg}");
}

#[test]
fn parse_allowed_default_is_v1() {
    let v = parse_allowed("name: John\n", &[Version::V1]).unwrap();
    assert_eq!(v.get("name"), Some(&Value::Str("John".into())));
}
