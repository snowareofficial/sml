//! IDEAS I-2：解析期契约 **opt-out**（`ParseOptions::apply_contracts(false)`）。
//!
//! 目的：允许 Rust 侧用原生 struct/enum 当契约来源、**无视**文件里写的
//! `@contract` / `@is`。关闭后：不校验、不填默认值、未知契约也不报错；
//! 但 `@contract` 定义仍照常收集、**不进入结果树**（两种模式都不进）。

use sml_parse::{parse_with, ParseOptions};
use sml::Value;

fn parse_optout(text: &str) -> Result<Value, String> {
    parse_with(text, ParseOptions::new().apply_contracts(false))
        .map(|out| out.value)
        .map_err(|e| e.to_string())
}

/// 常开时未知契约报 `E-CONTRACT-001`；关闭后应直接放行。
#[test]
fn optout_skips_unknown_contract_error() {
    let text = "@version v1\ndoc {\n  @is NoSuch\n  x: 1\n}\n";
    let v = parse_optout(text).expect("关闭后未知契约不应报错");
    assert_eq!(v.get("doc.x").and_then(|x| x.as_i64()), Some(1));
}

/// 关闭后：类型不符不报错、也不回填默认值（`Value` 与原文逐字一致）。
#[test]
fn optout_skips_type_check_and_default_fill() {
    let text = "@version v1\n@contract P { n: str role: str default guest }\n\
                doc {\n  @is P\n  n: 42\n}\n";
    let v = parse_optout(text).expect("关闭后类型不符不应报错");
    assert_eq!(v.get("doc.role"), None, "关闭后不应回填默认值");
    assert_eq!(v.get("doc.n").and_then(|x| x.as_i64()), Some(42));
}

/// 默认（开启）行为不变：同一份文本仍报错。
#[test]
fn default_still_enforces_contracts() {
    let text = "@version v1\ndoc {\n  @is NoSuch\n  x: 1\n}\n";
    let e = sml::parse(text).expect_err("默认应报未知契约");
    assert_eq!(e.code(), "E-CONTRACT-001", "{e}");
}

/// `@contract` 定义在**两种模式**下都不进入结果树。
#[test]
fn contract_definitions_never_enter_tree_in_either_mode() {
    let text = "@version v1\n@contract P { a: int }\ndoc {\n  x: 1\n}\n";
    for (label, v) in [
        ("on", sml::parse(text).expect("on 应成功")),
        ("off", parse_optout(text).expect("off 应成功")),
    ] {
        let s = format!("{v:?}");
        assert!(!s.contains("P"), "{label}: 契约定义不该进树: {s}");
    }
}
