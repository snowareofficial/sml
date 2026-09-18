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
    // 单元变体：`{"in-maintenance": "in-maintenance"}`。
    //
    // ⚠️ **W16 起顶层裸词不再是合法 SML**（`E-PARSE-008`：顶层标量不可往返，
    // 改前 `in-maintenance` 会被静默造键成 `{"in-maintenance": "in-maintenance"}`），
    // 故必须写出等价的键值形式。本用例此前一直是「靠那个造键行为过的」——
    // 而这条套件只在 `--features serde` 下编译，全量 `cargo test --workspace`
    // 跑不到它，W16 改完后它坏了两个月没人发现（W16 的 A 批一并修正）。
    let m: Status = sml::serde::from_str("in-maintenance: in-maintenance").unwrap();
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

/// W16：超出 `i64` 的 `u64` **不再静默降级为 f64**（丢精度），改为报错。
///
/// 改前两条路径都是 `i64::try_from(v).map(Int).unwrap_or_else(|_| Value::float(v as f64))`：
/// f64 只有 53 位精度，`18446744073709551615` 一转过去就不再相等（回读也不是原值）。
/// 码是 `E-DERIVE-002`，但本 crate 刻意零依赖（不引 `sml-codes`），故码以
/// `[E-DERIVE-002]` 后缀出现在文案里，而不是结构化字段 —— 与 C/C++/Lua 的「码作
/// 消息前缀」是同一类取舍（见该条码的 note）。
#[test]
fn u64_beyond_i64_is_reported_not_lossy() {
    // ① 反序列化方向：JSON 的大整数不再变成 Float
    let e = serde_json::from_str::<sml::Value>("18446744073709551615")
        .expect_err("超出 i64 的 u64 应报错");
    assert!(e.to_string().contains("E-DERIVE-002"), "实得：{e}");

    // ② 序列化方向：Rust 结构里的 u64 字段同理
    let e2 = sml::serde::to_value(&u64::MAX).expect_err("u64::MAX 应报错");
    assert!(e2.to_string().contains("E-DERIVE-002"), "实得：{e2}");

    // ③ 正对照：i64 范围内照常落 Int（含边界值本身）
    assert_eq!(
        serde_json::from_str::<sml::Value>("9223372036854775807").unwrap(),
        sml::Value::Int(i64::MAX)
    );
    assert_eq!(sml::serde::to_value(&123u64).unwrap(), sml::Value::Int(123));
    assert_eq!(
        sml::serde::to_value(&(i64::MAX as u64)).unwrap(),
        sml::Value::Int(i64::MAX)
    );
    // 边界外一格就已经报错（不是"到 u64::MAX 才报"）
    assert!(sml::serde::to_value(&(i64::MAX as u64 + 1)).is_err());
}
