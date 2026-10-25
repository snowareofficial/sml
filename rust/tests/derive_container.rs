// SPDX-License-Identifier: MulanPSL-2.0
//! 容器级 `#[sml(...)]` 属性：`path` / `file` / `default` / `deny_unknown_fields` / `rename`。
//!
//! 这些都是「配置场景」要的能力：一个标签打在 struct 上，直接从 SML 拿到结构体数据。
//! 运行：`cargo test --test derive_container`

use sml::{SmlDeserialize, SmlSerialize, Value};

// ---- path：把类型绑到文档里的某个子块 ----

#[derive(SmlDeserialize, Debug, PartialEq)]
#[sml(path = "app.server")]
struct Server {
    host: String,
    port: u16,
}

// `path` 与容器级 `default` 叠加
#[derive(SmlDeserialize, Debug, PartialEq)]
#[sml(path = "app.server", default)]
struct ServerLenient {
    host: String,
    port: u16,
}

// ---- default（容器级）：所有字段缺失时回落 Default ----

#[derive(SmlDeserialize, Debug, PartialEq)]
#[sml(default)]
struct WithDefault {
    a: i32,
    b: String,
}

// ---- deny_unknown_fields：块里出现未声明的键即报错 ----

#[derive(SmlDeserialize, Debug, PartialEq)]
#[sml(deny_unknown_fields)]
struct Strict {
    host: String,
}

// ---- rename（容器级）：只对 unit struct 生效 ----

#[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
#[sml(rename = "svc")]
struct Unit;

// ---- file：生成 from_file() ----

#[derive(SmlDeserialize, Debug, PartialEq)]
#[sml(file = "tests/.tmp_container_file.sml")]
struct FromFile {
    x: i32,
}

/// `dump.rs` 输出的形态，解析必然接受（见 STRUCT_SERDE.md §1）。
const NESTED: &str = "app:\n{\n  server:\n  {\n    host: web\n    port: 8080\n  }\n}\n";

#[test]
fn path_locates_sub_block() {
    let v = sml::parse(NESTED).expect("解析失败");
    let got = Server::from_sml_value(&v).expect("path 定位失败");
    assert_eq!(
        got,
        Server {
            host: "web".into(),
            port: 8080
        }
    );
}

#[test]
fn path_missing_is_error() {
    let v = sml::parse("app:\n{\n  other: 1\n}\n").expect("解析失败");
    let err = Server::from_sml_value(&v).expect_err("路径不存在应报错");
    assert!(err.contains("app.server"), "错误信息应含路径，实际: {err}");
}

#[test]
fn path_applies_to_from_sml_text_entry() {
    // 走 `from_sml`（先 parse 再 from_sml_value），path 同样生效
    let got = Server::from_sml(NESTED).expect("from_sml 失败");
    assert_eq!(got.host, "web");
    assert_eq!(got.port, 8080);
}

#[test]
fn path_with_container_default_fills_missing() {
    let v = sml::parse("app:\n{\n  server:\n  {\n    host: web\n  }\n}\n").expect("解析失败");
    // 没有容器级 default 时 port 会缺失报错
    assert!(Server::from_sml_value(&v).is_err());
    let got = ServerLenient::from_sml_value(&v).expect("容器级 default 应补上 port");
    assert_eq!(got.port, 0, "缺失字段应回落 Default::default()");
    assert_eq!(got.host, "web");
}

#[test]
fn container_default_covers_all_fields() {
    let v = sml::parse("a: 7\n").expect("解析失败");
    let got = WithDefault::from_sml_value(&v).expect("容器级 default 失败");
    assert_eq!(got.a, 7);
    assert_eq!(got.b, "", "未出现的字段应回落 Default");
}

#[test]
fn deny_unknown_fields_accepts_declared() {
    let v = sml::parse("host: web\n").expect("解析失败");
    let got = Strict::from_sml_value(&v).expect("声明过的键应放行");
    assert_eq!(got.host, "web");
}

#[test]
fn deny_unknown_fields_rejects_extra() {
    let v = sml::parse("host: web\nextra: 1\n").expect("解析失败");
    let err = Strict::from_sml_value(&v).expect_err("未知键应报错");
    assert!(err.contains("extra"), "错误信息应含未知键名，实际: {err}");
    assert!(err.contains("deny_unknown_fields"), "实际: {err}");
}

#[test]
fn container_rename_affects_unit_struct_both_ways() {
    // 写：裸词用 rename 后的名字
    assert_eq!(Unit.to_sml_value().unwrap(), Value::Str("svc".into()));
    // 读：只认 rename 后的名字
    assert_eq!(
        Unit::from_sml_value(&Value::Str("svc".into())).expect("rename 后的裸词应匹配"),
        Unit
    );
    let err = Unit::from_sml_value(&Value::Str("Unit".into())).expect_err("原名应失配");
    assert!(err.contains("svc"), "错误信息应含期望的裸词，实际: {err}");
}

#[test]
fn file_generates_from_file() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/.tmp_container_file.sml");
    std::fs::write(&path, "x: 42\n").expect("写临时文件失败");
    let got = FromFile::from_file();
    let _ = std::fs::remove_file(&path);
    let got = got.expect("from_file 失败");
    assert_eq!(got, FromFile { x: 42 });
}
