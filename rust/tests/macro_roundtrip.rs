// SPDX-License-Identifier: MulanPSL-2.0
//! `SmlSerialize` / `SmlDeserialize` derive 宏的 roundtrip 集成测试。

use sml::{SmlDeserialize, SmlSerialize};

// ---------------------------------------------------------------------------
// 结构体：rename / default / skip / Option
// ---------------------------------------------------------------------------

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct Server {
    host: String,
    #[sml(default)]
    port: i32,
    #[sml(rename = "tls-enabled")]
    tls_enabled: bool,
    #[sml(skip)]
    secret: String,
    upstream: Option<String>,
}

#[test]
fn struct_roundtrip() {
    let s = Server {
        host: "a.example".into(),
        port: 8080,
        tls_enabled: true,
        secret: "hunter2".into(),
        upstream: Some("b.example".into()),
    };
    let v = s.to_sml_value();
    match &v {
        sml::Value::Object(m) => {
            assert_eq!(m.get("host"), Some(&sml::Value::Str("a.example".into())));
            assert_eq!(m.get("port"), Some(&sml::Value::Int(8080)));
            assert_eq!(m.get("tls-enabled"), Some(&sml::Value::Bool(true)));
            assert!(!m.contains_key("secret"), "skip 字段不应出现在序列化结果");
            assert_eq!(
                m.get("upstream"),
                Some(&sml::Value::Str("b.example".into()))
            );
        }
        other => panic!("期望块，实际为 {other:?}"),
    }
    // skip 字段：序列化忽略，反序列化时重置为 Default
    let back = Server::from_sml_value(&v).unwrap();
    assert_eq!(back.host, s.host);
    assert_eq!(back.port, s.port);
    assert_eq!(back.tls_enabled, s.tls_enabled);
    assert_eq!(back.upstream, s.upstream);
    assert_eq!(back.secret, "", "skip 字段反序列化后应为 Default");
}

#[test]
fn option_none_is_omitted() {
    let s = Server {
        host: "a.example".into(),
        port: 8080,
        tls_enabled: false,
        secret: "x".into(),
        upstream: None,
    };
    let v = s.to_sml_value();
    match &v {
        sml::Value::Object(m) => assert!(!m.contains_key("upstream")),
        other => panic!("期望块，实际为 {other:?}"),
    }
    let back = Server::from_sml_value(&v).unwrap();
    assert_eq!(back.host, s.host);
    assert_eq!(back.upstream, None);
    assert_eq!(back.secret, "");
}

#[test]
fn missing_required_field_is_error() {
    let v = sml::Value::Object(std::collections::BTreeMap::new());
    let err = Server::from_sml_value(&v).unwrap_err();
    assert!(err.contains("host"), "错误应指出缺失字段: {err}");
}

// ---------------------------------------------------------------------------
// 枚举：单元变体 → 裸词；带数据变体 → __type 块
// ---------------------------------------------------------------------------

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
enum Status {
    Active,
    #[sml(rename = "stand-by")]
    StandBy,
}

#[test]
fn enum_word_roundtrip() {
    assert_eq!(Status::Active.to_sml_value(), sml::Value::Str("Active".into()));
    assert_eq!(
        Status::StandBy.to_sml_value(),
        sml::Value::Str("stand-by".into())
    );
    assert_eq!(
        Status::from_sml_value(&sml::Value::Str("Active".into())).unwrap(),
        Status::Active
    );
    assert_eq!(
        Status::from_sml_value(&sml::Value::Str("stand-by".into())).unwrap(),
        Status::StandBy
    );
    assert!(Status::from_sml_value(&sml::Value::Str("nope".into())).is_err());
}

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
enum Shape {
    Circle(f64),
    Point(f64, f64),
    Rect { w: f64, h: f64 },
    Unit,
}

#[test]
fn enum_data_roundtrip() {
    let c = Shape::Circle(3.0);
    match &c.to_sml_value() {
        sml::Value::Object(m) => {
            assert_eq!(m.get("__type"), Some(&sml::Value::Str("Circle".into())));
            assert_eq!(m.get("_value"), Some(&sml::Value::Float(3.0)));
        }
        other => panic!("期望块，实际为 {other:?}"),
    }
    assert_eq!(Shape::from_sml_value(&c.to_sml_value()).unwrap(), c);

    let p = Shape::Point(1.0, 2.0);
    match &p.to_sml_value() {
        sml::Value::Object(m) => {
            assert_eq!(m.get("__type"), Some(&sml::Value::Str("Point".into())));
            assert_eq!(
                m.get("_value"),
                Some(&sml::Value::Array(vec![
                    sml::Value::Float(1.0),
                    sml::Value::Float(2.0)
                ]))
            );
        }
        other => panic!("期望块，实际为 {other:?}"),
    }
    assert_eq!(Shape::from_sml_value(&p.to_sml_value()).unwrap(), p);

    let r = Shape::Rect { w: 4.0, h: 5.0 };
    match &r.to_sml_value() {
        sml::Value::Object(m) => {
            assert_eq!(m.get("__type"), Some(&sml::Value::Str("Rect".into())));
            assert_eq!(m.get("w"), Some(&sml::Value::Float(4.0)));
            assert_eq!(m.get("h"), Some(&sml::Value::Float(5.0)));
        }
        other => panic!("期望块，实际为 {other:?}"),
    }
    assert_eq!(Shape::from_sml_value(&r.to_sml_value()).unwrap(), r);

    // 单元变体既可以是裸词，也可以是带 __type 的块
    assert_eq!(Shape::Unit.to_sml_value(), sml::Value::Str("Unit".into()));
    assert_eq!(
        Shape::from_sml_value(&sml::Value::Str("Unit".into())).unwrap(),
        Shape::Unit
    );
    let as_block = sml::Value::Object(
        [("__type".to_string(), sml::Value::Str("Unit".into()))]
            .into_iter()
            .collect(),
    );
    assert_eq!(Shape::from_sml_value(&as_block).unwrap(), Shape::Unit);
}

#[test]
fn enum_data_text_roundtrip() {
    // 带数据变体序列化为文本后，__type 元数据应保留，可完整反序列化
    let c = Shape::Circle(3.0);
    let text = c.to_sml();
    assert!(text.contains("__type: Circle"), "应保留 __type: {text}");
    assert_eq!(Shape::from_sml(&text).unwrap(), c);

    let r = Shape::Rect { w: 4.0, h: 5.0 };
    let text = r.to_sml();
    assert!(text.contains("__type: Rect"), "应保留 __type: {text}");
    assert!(text.contains("w: 4"), "字段应输出: {text}");
    assert_eq!(Shape::from_sml(&text).unwrap(), r);
}

// ---------------------------------------------------------------------------
// 结构体形态：newtype / tuple / unit
// ---------------------------------------------------------------------------

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct Id(u64);

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct Pair(i32, String);

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct Marker;

#[test]
fn struct_shapes_roundtrip() {
    let id = Id(42);
    assert_eq!(id.to_sml_value(), sml::Value::Int(42));
    assert_eq!(Id::from_sml_value(&id.to_sml_value()).unwrap(), id);

    let pair = Pair(7, "sml".into());
    assert_eq!(
        pair.to_sml_value(),
        sml::Value::Array(vec![sml::Value::Int(7), sml::Value::Str("sml".into())])
    );
    assert_eq!(Pair::from_sml_value(&pair.to_sml_value()).unwrap(), pair);

    let m = Marker;
    assert_eq!(m.to_sml_value(), sml::Value::Str("Marker".into()));
    assert_eq!(Marker::from_sml_value(&m.to_sml_value()).unwrap(), m);
}

// ---------------------------------------------------------------------------
// rename_all 批量改名 + 泛型
// ---------------------------------------------------------------------------

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
#[sml(rename_all = "kebab-case")]
struct Kebab {
    first_name: String,
    is_ready: bool,
}

#[test]
fn rename_all_kebab() {
    let k = Kebab {
        first_name: "张三".into(),
        is_ready: true,
    };
    let text = k.to_sml();
    assert!(text.contains("first-name:"), "应输出 kebab-case 键: {text}");
    assert!(text.contains("is-ready:"), "应输出 kebab-case 键: {text}");
    assert_eq!(Kebab::from_sml(&text).unwrap(), k);
}

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct Wrap<T> {
    inner: T,
    extra: Option<T>,
}

#[test]
fn generic_roundtrip() {
    let w = Wrap {
        inner: 42i32,
        extra: Some(7),
    };
    let v = w.to_sml_value();
    assert_eq!(Wrap::<i32>::from_sml_value(&v).unwrap(), w);
}

// ---------------------------------------------------------------------------
// flatten 并入子块
// ---------------------------------------------------------------------------

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct Base {
    region: String,
    zone: String,
}

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct App {
    name: String,
    #[sml(flatten)]
    base: Base,
}

#[test]
fn flatten_roundtrip() {
    let a = App {
        name: "web".into(),
        base: Base {
            region: "cn-north-1".into(),
            zone: "a".into(),
        },
    };
    let v = a.to_sml_value();
    match &v {
        sml::Value::Object(m) => {
            assert_eq!(m.get("name"), Some(&sml::Value::Str("web".into())));
            assert_eq!(m.get("region"), Some(&sml::Value::Str("cn-north-1".into())));
            assert_eq!(m.get("zone"), Some(&sml::Value::Str("a".into())));
        }
        other => panic!("期望块，实际为 {other:?}"),
    }
    assert_eq!(App::from_sml_value(&v).unwrap(), a);
}

// ---------------------------------------------------------------------------
// 文本往返（to_sml / from_sml）+ 嵌套
// ---------------------------------------------------------------------------

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
struct Service {
    name: String,
    replicas: u32,
    labels: std::collections::BTreeMap<String, String>,
    nodes: Vec<String>,
    status: Status,
}

#[test]
fn text_roundtrip_and_nesting() {
    let svc = Service {
        name: "auth".into(),
        replicas: 3,
        labels: [("tier".to_string(), "backend".to_string())]
            .into_iter()
            .collect(),
        nodes: vec!["n1".into(), "n2".into()],
        status: Status::Active,
    };
    let text = svc.to_sml();
    assert!(text.contains("replicas: 3"), "数字应保持为 SML 整数: {text}");
    assert!(text.contains("status: Active"), "枚举应输出裸词: {text}");
    let back = Service::from_sml(&text).unwrap();
    assert_eq!(back, svc);
}

#[test]
fn type_error_message_is_informative() {
    let bad = sml::Value::Object(
        [(
            "host".to_string(),
            sml::Value::Array(vec![sml::Value::Int(1)]),
        )]
        .into_iter()
        .collect(),
    );
    let err = Server::from_sml_value(&bad).unwrap_err();
    assert!(err.contains("host"), "错误应定位到字段: {err}");
    assert!(err.contains("字符串"), "错误应说明期望类型: {err}");
}

#[test]
fn toml_rs_style_top_level_functions() {
    let s = Server {
        host: "web.example".into(),
        port: 8080,
        tls_enabled: true,
        secret: "hunter2".into(),
        upstream: Some("b.example".into()),
    };
    // 与 toml-rs 相同的心智模型：to_string / from_str
    let text = sml::to_string(&s);
    assert!(text.contains("host: web.example"), "{text}");
    assert!(text.contains("tls-enabled: true"), "{text}");
    let back: Server = sml::from_str(&text).unwrap();
    assert_eq!(back, Server {
        host: "web.example".into(),
        port: 8080,
        tls_enabled: true,
        secret: String::new(), // skip 重置 Default
        upstream: Some("b.example".into()),
    });
    // 顶层函数与 trait 方法输出一致
    assert_eq!(sml::to_string(&s), s.to_sml());
    // 泛型 &str（内置实现）与 trait 方法一致
    assert_eq!(sml::to_string("hello"), "hello");
    assert_eq!(sml::to_string("hello"), "hello".to_sml());
}
