//! C 验收（TASK-agent-w24 §5）：契约叠加。
//!
//! 11 条判据（§2 D1–D4）：
//! `@is a, b` ≡ `@is a b` ≡ 块级 `conta contb item { }`；strict 优先；
//! 字段并集、顺序无关；冲突 ⇒ `E-CONTRACT-016`；多条 `@is` 全部生效（行为变更）。
//!
//! 回归红线：单契约路径**逐字保持旧行为**（含 P1-3 的 `server web prod { }`）。

use sml::parse;
use sml_parse::{parse_with, ParseOptions, PathSeg};

fn ok(text: &str) -> sml::Value {
    match parse(text) {
        Ok(v) => v,
        Err(e) => panic!("应解析成功，实际失败: {e}\n---\n{text}\n---"),
    }
}

fn err(text: &str) -> String {
    match parse(text) {
        Ok(_) => panic!("应校验失败，实际通过了:\n{text}"),
        Err(e) => e.to_string(),
    }
}

fn k(s: &str) -> PathSeg {
    PathSeg::Key(s.into())
}

/// #1 逗号形式：两份契约都生效（各自的默认值都回填）。
#[test]
fn comma_form_applies_both() {
    let v = ok(
        "@contract A { x: int default 1 }\n\
         @contract B { y: int default 2 }\n\
         d { @is A, B }",
    );
    assert_eq!(v.get("d.x"), Some(&sml::Value::Int(1)));
    assert_eq!(v.get("d.y"), Some(&sml::Value::Int(2)));
}

/// #2 省逗号 ≡ 逗号：同一棵树。
#[test]
fn space_form_equals_comma_form() {
    let head = "@contract A { x: int default 1 }\n@contract B { y: int default 2 }\n";
    let comma = ok(&format!("{head}d {{ @is A, B }}"));
    let space = ok(&format!("{head}d {{ @is A B }}"));
    assert_eq!(comma, space, "两种写法必须得到同一棵树");
}

/// #3 顺序无关：交换书写顺序，结果同树（D3）。
#[test]
fn order_independent() {
    let head = "@contract A { x: int default 1 }\n@contract B { y: int default 2 }\n";
    let ab = ok(&format!("{head}d {{ @is A B }}"));
    let ba = ok(&format!("{head}d {{ @is B A }}"));
    assert_eq!(ab, ba);
}

/// #3b required 顺序无关（D3 违返修复）：同字段 `x`，A 必填 / B 可选。
/// 缺 `x` 时 `@is A B` 与 `@is B A` 必须**同样报「必填缺失」**（合并 required 取**或**），
/// 而非旧版 `insert` 后者覆盖导致的「前者放过、后者报错」（required 随书写顺序变）。
#[test]
fn order_independent_when_required_differs() {
    let head = "@contract A loose { x: str }\n@contract B loose { x: str optional }\n";
    let ab = err(&format!("{head}d {{ @is A B }}"));
    let ba = err(&format!("{head}d {{ @is B A }}"));
    assert!(ab.contains("E-CONTRACT-003"), "@is A B 缺 x 应报必填缺失: {ab}");
    assert!(ba.contains("E-CONTRACT-003"), "@is B A 缺 x 应报必填缺失: {ba}");
    // 提供 x：两种顺序都通过且同树
    assert_eq!(
        ok(&format!("{head}d {{ @is A B x: v }}")),
        ok(&format!("{head}d {{ @is B A x: v }}"))
    );
}

/// #4 多条 `@is` 全部生效（**行为变更**：旧版后一条覆盖前一条）。
#[test]
fn multiple_is_lines_all_apply() {
    let v = ok(
        "@contract A { x: int default 1 }\n\
         @contract B { y: int default 2 }\n\
         d {\n\
         \x20 @is A\n\
         \x20 @is B\n\
         }",
    );
    assert_eq!(v.get("d.x"), Some(&sml::Value::Int(1)));
    assert_eq!(v.get("d.y"), Some(&sml::Value::Int(2)));
}

/// #5 strict 优先：任一严格 ⇒ 整体严格，额外字段被拒（E-CONTRACT-004）。
#[test]
fn strict_wins_over_loose() {
    let e = err(
        "@contract A loose { x: int }\n\
         @contract B { y: int }\n\
         d { @is A, B\n\
         \x20 x: 1\n\
         \x20 y: 2\n\
         \x20 extra: 9 }",
    );
    assert!(
        e.contains("E-CONTRACT-004") && e.contains("未在契约"),
        "got: {e}"
    );
}

/// #6 冲突：同字段类型不同 ⇒ E-CONTRACT-016；默认值不同同理。
#[test]
fn conflicts_rejected_with_new_code() {
    let e = err("@contract A { port: int }\n@contract B { port: str }\nd { @is A, B }");
    assert!(e.contains("E-CONTRACT-016"), "got: {e}");
    assert!(e.contains("叠加冲突"), "got: {e}");

    let e = err("@contract A { port: int default 80 }\n@contract B { port: int default 443 }\nd { @is A B }");
    assert!(e.contains("E-CONTRACT-016"), "got: {e}");

    // 区间不同同样冲突
    let e = err("@contract A { n: int min 0 }\n@contract B { n: int min 5 }\nd { @is A B }");
    assert!(e.contains("E-CONTRACT-016"), "got: {e}");
}

/// #7 多契约里的未定义名：E-CONTRACT-001，**位置指向违规的那个词**。
#[test]
fn unknown_name_reports_with_position() {
    let e = err("@contract A { x: int }\nd { @is A Nope\n  x: 1 }");
    assert!(e.contains("E-CONTRACT-001"), "got: {e}");
    assert!(e.contains("Nope"), "got: {e}");
    assert!(e.contains("第 2 行"), "位置应指向 Nope 所在行: {e}");
}

/// #8 同行键值不被吞（D1 补充判据：裸词后跟 `:` 的是键）。
#[test]
fn same_line_key_value_not_swallowed() {
    // host 声明在契约里，`@is A` 与键值同写一行
    let v = ok(
        "@contract A { x: int default 1  host: str }\n\
         d { @is A host: h1 }",
    );
    assert_eq!(v.get("d.host"), Some(&sml::Value::Str("h1".into())));
    assert_eq!(v.get("d.x"), Some(&sml::Value::Int(1)));
    // 换行即停：下一行的键同样不被吞
    let v = ok(
        "@contract A { x: int default 1  host: str }\n\
         d {\n\
         \x20 @is A\n\
         \x20 host: h1\n\
         }",
    );
    assert_eq!(v.get("d.host"), Some(&sml::Value::Str("h1".into())));
}

/// #9 块级多契约：`conta contb item { }`，两份都生效、`__name == "item"`。
#[test]
fn block_level_multi_contract() {
    let v = ok(
        "@feature enable typed-block\n@feature enable block-name\n\
         @contract Conta { a: int default 1 }\n\
         @contract Contb { b: int default 2 }\n\
         Conta Contb item { }",
    );
    assert_eq!(v.get("Conta.a"), Some(&sml::Value::Int(1)));
    assert_eq!(v.get("Conta.b"), Some(&sml::Value::Int(2)));
    assert_eq!(v.get("Conta.__name"), Some(&sml::Value::Str("item".into())));
    assert_eq!(v.get("Conta.__args"), None, "item 是块名，后面没有 args");
}

/// #10 **P1-3 回归红线**：单契约词时块级行为逐字不变。
#[test]
fn block_level_single_contract_unchanged() {
    let v = ok(
        "@feature enable typed-block\n@feature enable block-name\n\
         @contract Server { host: str }\n\
         Server web prod { host: h }",
    );
    assert_eq!(v.get("Server.__name"), Some(&sml::Value::Str("web".into())));
    assert_eq!(
        v.get("Server.__args"),
        Some(&sml::Value::Array(vec![sml::Value::Str("prod".into())]))
    );
    assert_eq!(v.get("Server.host"), Some(&sml::Value::Str("h".into())));
    // 首词根本不是契约 ⇒ 纯裸块元数据，更不变
    // `server web prod { }` 首词未命中契约表 ⇒ 普通裸块，无需 block-name（回归红线）
    let v = ok("@feature enable typed-block\nserver web prod { a: 1 }");
    assert_eq!(v.get("server.__name"), Some(&sml::Value::Str("web".into())));
    assert_eq!(v.get("server.a"), Some(&sml::Value::Int(1)));
}

/// #11 契约映射：叠加后 `names` 为多项（B 铺好的记录点在此兑现）。
#[test]
fn contract_map_lists_all_names() {
    let out = parse_with(
        "@contract A { x: int default 1 }\n\
         @contract B { y: int default 2 }\n\
         d { @is A, B }",
        ParseOptions::new().trace_contracts(true),
    )
    .unwrap();
    let m = out.contract_map.as_ref().unwrap();
    assert_eq!(m.names_at(&[k("d")]), ["A", "B"], "按应用顺序列出全部契约");

    // 块级形式同样记录
    let out = parse_with(
        "@feature enable typed-block\n@feature enable block-name\n\
         @contract Conta { a: int default 1 }\n\
         @contract Contb { b: int default 2 }\n\
         Conta Contb item { }",
        ParseOptions::new().trace_contracts(true),
    )
    .unwrap();
    let m = out.contract_map.as_ref().unwrap();
    assert_eq!(m.names_at(&[k("Conta")]), ["Conta", "Contb"]);
}
