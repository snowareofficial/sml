// SPDX-License-Identifier: MulanPSL-2.0
//! serde 桥集成测试：任意 `serde::Serialize/Deserialize` 类型 <-> SML。
//!
//! 运行方式：`cargo test --features serde`（serde feature 默认关闭）。

#![cfg(feature = "serde")]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// 测试类型：与 toml-rs 示例同构，含嵌套、枚举、Option、Map、数组
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Server {
    host: String,
    port: u16,
    #[serde(rename = "tls-enabled", default)]
    tls_enabled: bool,
    #[serde(default)]
    upstream: Option<String>,
    #[serde(default)]
    labels: BTreeMap<String, String>,
    #[serde(default)]
    nodes: Vec<String>,
    #[serde(default)]
    status: Status,
}

#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Status {
    Active,
    #[default]
    StandBy,
    #[serde(rename = "in-maintenance")]
    Maintenance,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
enum Shape {
    Circle(f64),
    Point(f64, f64),
    Rect { w: f64, h: f64 },
}

// ---------------------------------------------------------------------------
// 一键反序列化：SML 文本 -> serde 类型（toml::from_str 的等价物）
// ---------------------------------------------------------------------------

#[test]
fn from_str_sml_text_to_serde_type() {
    let text = [
        "host: web.example",
        "port: 8080",
        "tls-enabled: true",
        "upstream: b.example",
        "labels:",
        "{",
        "  tier: backend",
        "}",
        "nodes: [n1, n2]",
        "status: active",
    ]
    .join("\n");
    let s: Server = sml::serde::from_str(&text).unwrap();
    assert_eq!(s.host, "web.example");
    assert_eq!(s.port, 8080);
    assert!(s.tls_enabled);
    assert_eq!(s.upstream.as_deref(), Some("b.example"));
    assert_eq!(s.labels.get("tier").map(String::as_str), Some("backend"));
    assert_eq!(s.nodes, vec!["n1".to_string(), "n2".to_string()]);
    assert_eq!(s.status, Status::Active);
}

#[test]
fn from_str_missing_option_fields_default() {
    // 缺失的可选字段（Option/default）由 serde derive 兜底，不报错
    let s: Server = sml::serde::from_str("host: web.example\nport: 80\n").unwrap();
    assert_eq!(s.host, "web.example");
    assert_eq!(s.upstream, None);
    assert_eq!(s.labels.len(), 0);
    assert_eq!(s.status, Status::StandBy);
}

#[test]
fn from_str_enum_variants() {
    // 单元变体：裸词（SML 顶层裸词包裹为 {"in-maintenance": "in-maintenance"}）
    let m: Status = sml::serde::from_str("in-maintenance").unwrap();
    assert_eq!(m, Status::Maintenance);
    // 带数据变体：__type 块
    let c: Shape = sml::serde::from_str("{ __type: Circle _value: 3 }").unwrap();
    assert_eq!(c, Shape::Circle(3.0));
    let p: Shape = sml::serde::from_str("{ __type: Point _value: [1, 2] }").unwrap();
    assert_eq!(p, Shape::Point(1.0, 2.0));
    let r: Shape = sml::serde::from_str("{ __type: Rect w: 4 h: 5 }").unwrap();
    assert_eq!(r, Shape::Rect { w: 4.0, h: 5.0 });
}

#[test]
fn from_str_type_error_is_clear() {
    let err = sml::serde::from_str::<Server>("host: 42\n").unwrap_err();
    assert!(err.contains("字符串"), "应指明期望类型: {err}");
}

// ---------------------------------------------------------------------------
// 一键序列化：serde 类型 -> SML 文本（toml::to_string 的等价物）
// ---------------------------------------------------------------------------

#[test]
fn to_string_serde_type_to_sml_text() {
    let s = Server {
        host: "web.example".into(),
        port: 8080,
        tls_enabled: true,
        upstream: Some("b.example".into()),
        labels: BTreeMap::from([("tier".to_string(), "backend".to_string())]),
        nodes: vec!["n1".into(), "n2".into()],
        status: Status::Active,
    };
    let text = sml::serde::to_string(&s).unwrap();
    assert!(text.contains("host: web.example"), "{text}");
    assert!(text.contains("tls-enabled: true"), "{text}"); // serde(rename)
    assert!(text.contains("status: active"), "{text}");
    // roundtrip
    let back: Server = sml::serde::from_str(&text).unwrap();
    assert_eq!(back, s);
}

#[test]
fn to_string_enum_shapes() {
    let text = sml::serde::to_string(&Shape::Rect { w: 4.0, h: 5.0 }).unwrap();
    assert!(text.contains("__type: Rect"), "{text}");
    let text = sml::serde::to_string(&Shape::Point(1.0, 2.0)).unwrap();
    assert!(text.contains("__type: Point"), "{text}");
}

#[test]
fn to_value_from_value_value_model() {
    let s = Server {
        host: "h".into(),
        port: 1,
        tls_enabled: false,
        upstream: None,
        labels: BTreeMap::new(),
        nodes: Vec::new(),
        status: Status::StandBy,
    };
    let v = sml::serde::to_value(&s).unwrap();
    match &v {
        sml::Value::Object(m) => {
            assert_eq!(m.get("host"), Some(&sml::Value::Str("h".into())));
            assert_eq!(m.get("port"), Some(&sml::Value::Int(1)));
        }
        other => panic!("期望块，实际为 {other:?}"),
    }
    // 原样 Value 可再反序列化
    let back: Server = sml::serde::from_value(v).unwrap();
    assert_eq!(back, s);
}

// ---------------------------------------------------------------------------
// 与 toml crate 双向互通（Value 实现 serde traits）
// ---------------------------------------------------------------------------

#[test]
fn interop_with_toml_roundtrip() {
    let text = "host: web.example\nport: 8080\ntls-enabled: true\nupstream: b.example\n";
    let v: sml::Value = sml::parse(text).unwrap();

    // sml::Value -> TOML 文本
    let toml_text = toml::to_string(&v).unwrap();
    assert!(toml_text.contains("host = \"web.example\""), "{toml_text}");

    // TOML 文本 -> sml::Value（形状一致）
    let back: sml::Value = toml::from_str(&toml_text).unwrap();
    assert_eq!(back, v);

    // sml::Value -> serde 类型
    let s: Server = sml::serde::from_value(back).unwrap();
    assert_eq!(s.host, "web.example");
    assert_eq!(s.port, 8080);
}

#[test]
fn interop_with_json_roundtrip() {
    let v: sml::Value = sml::parse("port: 8080\nnames: [a, b]\n").unwrap();
    let json = serde_json::to_string(&v).unwrap();
    let back: sml::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(back, v);
}
