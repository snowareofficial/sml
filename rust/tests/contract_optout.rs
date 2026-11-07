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

/// 文件入口 + 选项：`parse_file_with` 同时做到「展开 include」与「忽略文档内契约」。
///
/// 这是旧入口的组合缺口：`parse_file` 一族把 `ParseOptions::new()` 写死（关不掉契约），
/// `parse_with` 收选项但不展开 include。下面先钉住「旧入口确实做不到」，再钉新入口能做到。
#[test]
fn file_entry_with_options_expands_includes_and_can_disable_contracts() {
    let dir = std::env::temp_dir().join(format!("sml_optout_file_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("建临时目录");
    std::fs::write(dir.join("sub.sml"), "frag base { a: 1 }\n").expect("写子文件");
    let main = dir.join("main.sml");
    std::fs::write(
        &main,
        "@version v1\ninclude \"sub.sml\"\ndoc {\n  @is NoSuch\n  x: 1\n}\n",
    )
    .expect("写主文件");

    // ① 旧文件入口：契约默认开启 ⇒ 未知契约报错（证明它关不掉）
    let e = sml_parse::parse_file(&main).expect_err("旧文件入口应报未知契约");
    assert_eq!(e.code(), "E-CONTRACT-001", "{e}");

    // ② 新入口 + 空选项：契约仍是默认开启 ⇒ 与旧入口同码报错（行为不加不减）
    let e2 = sml_parse::parse_file_with(&main, ParseOptions::new())
        .expect_err("空选项下仍应报未知契约");
    assert_eq!(e2.code(), e.code(), "新入口空选项应与旧入口同码：{e2} vs {e}");

    // ③ 新入口 + 关闭契约：既展开了 include，也不因未知契约报错
    let out = sml_parse::parse_file_with(&main, ParseOptions::new().apply_contracts(false))
        .expect("关闭契约后应成功");
    assert_eq!(out.value.get("doc.x").and_then(|v| v.as_i64()), Some(1));
    // `frag base { a: 1 }` 是「裸块类型 + 名字」：键名取类型名 `frag`，名字进 `__name`
    assert_eq!(
        out.value.get("frag.a").and_then(|v| v.as_i64()),
        Some(1),
        "include 必须被展开：{:?}",
        out.value
    );

    let _ = std::fs::remove_dir_all(&dir);
}
