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
    assert!(msg.message().contains("引号") || msg.message().contains("\""), "错误信息应提示加引号: {msg}");
}

#[test]
fn v3_quoted_string_ok() {
    let v = parse("@version v3\nname: \"John\"\nage: 27\nactive: true\n").unwrap();
    assert_eq!(v.get("name"), Some(&Value::Str("John".into())));
    assert_eq!(v.get("age"), Some(&Value::Int(27)));
    assert_eq!(v.get("active"), Some(&Value::Bool(true)));
}

#[test]
// 这里的 `3.14` 是**测试数据**（裸词标量），不是圆周率近似值：
// 改成 `std::f64::consts::PI` 会改变断言对象，故就地豁免该 lint。
#[allow(clippy::approx_constant)]
fn v3_scalars_still_bareword() {
    let v = parse("@version v3\nn: 42\nf: 3.14\nb: true\nz: null\n").unwrap();
    assert_eq!(v.get("n"), Some(&Value::Int(42)));
    assert_eq!(v.get("f"), Some(&Value::float(3.14)));
    assert_eq!(v.get("b"), Some(&Value::Bool(true)));
    assert_eq!(v.get("z"), Some(&Value::Null));
}

#[test]
fn v3_undefined_fragment_is_error() {
    // v3 下未定义片段引用同样报错，不再静默降级为字符串
    let r = parse("@version v3\nref: &frag\n");
    assert!(r.is_err());
    assert!(r.unwrap_err().message().contains("未定义的片段引用"));
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
    assert!(msg.message().contains("版本范围"), "应提示版本范围: {msg}");
}

#[test]
fn parse_allowed_default_is_v1() {
    let v = parse_allowed("name: John\n", &[Version::V1]).unwrap();
    assert_eq!(v.get("name"), Some(&Value::Str("John".into())));
}

// ===========================================================================
// S0b：`@version` 的 edition 字面量（`docs/swsml/spec/version.md`）
//
// 目标语法 `@version 50 "camelia" dev`；旧写法一律仍是别名。
// 本组用例只钉**已发布的 1.0–4.0** 能正常认，以及**各种写错**的报错口径。
// ===========================================================================

#[test]
fn edition_literals_are_aliases_for_published_versions() {
    // 连写 / 点号 / 旧写法 指向同一代
    for lit in ["10", "1.0", "1", "v1"] {
        let (_, ver) = parse_versioned(&format!("@version {lit}\nx: 1\n"))
            .unwrap_or_else(|e| panic!("`@version {lit}` 应被接受：{e}"));
        assert_eq!(ver, Version::V1, "字面量 {lit}");
    }
    for lit in ["40", "4.0", "4", "v4", "\"4\""] {
        let (_, ver) = parse_versioned(&format!("@version {lit}\nx: 1\n"))
            .unwrap_or_else(|e| panic!("`@version {lit}` 应被接受：{e}"));
        assert_eq!(ver, Version::V4, "字面量 {lit}");
    }
}

#[test]
fn dev_edition_requires_the_dev_marker() {
    let e = parse("@version 50\nx: 1\n").unwrap_err();
    let msg = format!("{e}");
    assert!(msg.contains("dev"), "应提示须写 dev 标记：{msg}");
}

#[test]
fn dev_edition_is_not_enabled_yet_even_with_marker() {
    // 定稿前本实现还没有 5.0 的功能集 ⇒ 明确报「未启用」，而不是含糊的「不支持」
    let e = parse("@version 50 \"camelia\" dev\nx: 1\n").unwrap_err();
    let msg = format!("{e}");
    assert!(msg.contains("开发") || msg.contains("未启用"), "应说明尚未启用：{msg}");
    assert!(msg.contains("camelia"), "文案应带代号：{msg}");
}

#[test]
fn published_edition_rejects_dev_marker() {
    let e = parse("@version 40 dev\nx: 1\n").unwrap_err();
    let msg = format!("{e}");
    assert!(msg.contains("不需要") || msg.contains("dev"), "应提示不需要 dev：{msg}");
}

#[test]
fn codename_must_agree_with_the_number() {
    let e = parse("@version 40 \"camelia\"\nx: 1\n").unwrap_err();
    let msg = format!("{e}");
    assert!(msg.contains("同一代") || msg.contains("camlia") || msg.contains("camelia"), "{msg}");
}

#[test]
fn unknown_codename_and_three_digit_literal_are_rejected() {
    assert!(parse("@version \"nope\"\nx: 1\n").is_err(), "未知代号应报错");
    // 三位连写有歧义（10.0 还是 1.00）⇒ 必须写点号
    let e = parse("@version 100\nx: 1\n").unwrap_err();
    assert!(
        format!("{e}").contains("不支持"),
        "三位连写应报「不支持」而不是被当成 10.0：{e}"
    );
}

#[test]
fn dev_marker_must_be_last() {
    let e = parse("@version dev 40\nx: 1\n").unwrap_err();
    let msg = format!("{e}");
    assert!(msg.contains("最后") || msg.contains("dev"), "{msg}");
}
