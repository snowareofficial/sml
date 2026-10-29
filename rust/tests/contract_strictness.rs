// SPDX-License-Identifier: MulanPSL-2.0
//! I-7 缺口④⑤：契约严格性三态（strict / warn / loose）+ 块级覆盖 + 诊断 `Warning` 档。
//!
//! 所有判断都**同时断言「值树」与「诊断」**（照 `key_conflict.rs` 的风格）：
//! 违约放行时值树必须与 strict 下的成功分支逐字一致（不改数据形状）。
//!
//! 钉住的规则：
//! ① strict 仍 Err（回归，不许改坏）；
//! ② warn 放行 + 产出 `Warning` 诊断；
//! ③ loose 静默无诊断（回归）；
//! ④ 块级覆盖优先于契约级（各方向一条）；
//! ⑤ 多契约叠加 strict 优先且顺序无关（两条）；
//! ⑥ warn 态**不放行**类型不符 / 必填缺失（各一条，钉住 D1 边界）；
//! ⑦ `parse()` 看不到 warn 告警、`parse_with_warnings` 看得到（各一条）。
//!
//! ⚠️ 本文件只通过 `parse` / `parse_with_warnings` / `parse_with` 进入，不改任何实现。

use sml::{parse, parse_with_warnings, DiagnosticKind, Value};

/// 用 `parse` 跑；成功返回 `Value`，失败 panic。
fn ok(text: &str) -> Value {
    match parse(text) {
        Ok(v) => v,
        Err(e) => panic!("应解析成功，实际失败: {e}\n---\n{text}\n---"),
    }
}

/// 用 `parse` 跑；应失败，返回错误文案。
fn err(text: &str) -> String {
    match parse(text) {
        Ok(_) => panic!("应校验失败，实际通过了:\n{text}"),
        Err(e) => e.to_string(),
    }
}

/// 用 `parse_with_warnings` 跑；返回 (值, 诊断)。
fn with_warns(text: &str) -> (Value, Vec<sml::Diagnostic>) {
    parse_with_warnings(text).unwrap_or_else(|e| panic!("应解析成功（warn 态放行），实际失败: {e}\n---\n{text}\n---"))
}

const C: &str = "@contract Cfg {\n  host: str\n  port: int default 8080\n}\n";

// ---------- ① strict 仍 Err（回归，不许改坏）----------

#[test]
fn strict_rejects_undeclared_field() {
    // 不写修饰符 ⇒ strict（默认）；未声明字段 ⇒ E-CONTRACT-004。
    let e = err(&format!("{C}doc {{\n  @is Cfg\n  host: db1\n  extra: 1\n}}\n"));
    assert!(
        e.contains("E-CONTRACT-004") && e.contains("未在契约"),
        "strict 应拒绝未声明字段，got: {e}"
    );
}

// ---------- ② warn 放行 + 产出 Warning 诊断 ----------

#[test]
fn warn_allows_undeclared_field_and_warns() {
    let src = format!("@contract Cfg warn {{\n  host: str\n  port: int default 8080\n}}\ndoc {{\n  @is Cfg\n  host: db1\n  extra: 1\n}}\n");
    let (v, warns) = with_warns(&src);
    // 值树与 strict 成功分支逐字一致：声明字段原样、未声明字段也保留
    assert_eq!(v.get("doc.host").unwrap().as_str(), Some("db1"));
    assert_eq!(v.get("doc.port"), Some(&Value::Int(8080)), "default 仍回填");
    assert!(v.get("doc.extra").is_some(), "warn 应放行额外字段");
    // 一条 Warning 诊断
    assert_eq!(warns.len(), 1, "应恰好 1 条告警，实际 {warns:?}");
    assert_eq!(warns[0].kind, DiagnosticKind::Warning);
    assert!(
        warns[0].message.contains("extra") && warns[0].message.contains("Cfg"),
        "告警须点名字段与契约：{}",
        warns[0].message
    );
}

// ---------- ③ loose 静默无诊断（回归）----------

#[test]
fn loose_allows_undeclared_field_silently() {
    let src = format!("{C}doc {{\n  @is Cfg loose\n  host: db1\n  extra: 1\n}}\n");
    // 既有 `parse()` 链路本就看不到诊断；loose 也不应产出任何诊断
    let (v, warns) = with_warns(&src);
    assert!(v.get("doc.extra").is_some(), "loose 应放行额外字段");
    assert!(
        warns.is_empty(),
        "loose 应静默放行、无诊断，实际 {warns:?}"
    );
}

// ---------- ④ 块级覆盖优先于契约级（各方向一条）----------

/// 契约级 strict，块级 `@is X loose` ⇒ loose 胜：静默放行。
#[test]
fn block_loose_overrides_contract_strict() {
    let src = format!("{C}doc {{\n  @is Cfg loose\n  host: db1\n  extra: 1\n}}\n");
    let (v, warns) = with_warns(&src);
    assert!(v.get("doc.extra").is_some(), "块级 loose 应放行额外字段");
    assert!(warns.is_empty(), "块级 loose 应静默，实际 {warns:?}");
}

/// 契约级 strict，块级 `@is X warn` ⇒ warn 胜：放行 + Warning。
#[test]
fn block_warn_overrides_contract_strict() {
    let src = format!("{C}doc {{\n  @is Cfg warn\n  host: db1\n  extra: 1\n}}\n");
    let (v, warns) = with_warns(&src);
    assert!(v.get("doc.extra").is_some(), "块级 warn 应放行额外字段");
    assert_eq!(warns.len(), 1, "块级 warn 应产出 1 条告警，实际 {warns:?}");
    assert_eq!(warns[0].kind, DiagnosticKind::Warning);
}

/// 契约级 loose，块级 `@is X warn` ⇒ warn 胜（块级显式 > 契约级）：放行 + Warning。
#[test]
fn block_warn_overrides_contract_loose() {
    let src = format!("@contract Cfg loose {{\n  host: str\n}}\ndoc {{\n  @is Cfg warn\n  host: db1\n  extra: 1\n}}\n");
    let (_v, warns) = with_warns(&src);
    assert_eq!(warns.len(), 1, "块级 warn 应压倒契约级 loose，实际 {warns:?}");
    assert_eq!(warns[0].kind, DiagnosticKind::Warning);
}

/// 契约级 warn，块级 `@is X loose` ⇒ loose 胜：静默放行（无诊断）。
#[test]
fn block_loose_overrides_contract_warn() {
    let src = format!("@contract Cfg warn {{\n  host: str\n}}\ndoc {{\n  @is Cfg loose\n  host: db1\n  extra: 1\n}}\n");
    let (_v, warns) = with_warns(&src);
    assert!(
        warns.is_empty(),
        "块级 loose 应压倒契约级 warn，静默放行，实际 {warns:?}"
    );
}

// ---------- ⑤ 多契约叠加 strict 优先且顺序无关（两条）----------

/// A 宽松(warn) + B 严格(strict)，任一严格 ⇒ 整体严格，额外字段被拒（顺序无关）。
fn mixed_is_rejected(head: &str) -> bool {
    let e = err(&format!(
        "{head}doc {{\n  @is A B\n  a: 1\n  b: 2\n  extra: 9\n}}\n"
    ));
    e.contains("E-CONTRACT-004") && e.contains("未在契约")
}

#[test]
fn strict_wins_over_warn_order_ab() {
    let head = "@contract A warn { a: int }\n@contract B { b: int }\n";
    assert!(mixed_is_rejected(head), "A warn + B strict 应整体严格");
}

#[test]
fn strict_wins_over_warn_order_ba() {
    let head = "@contract A warn { a: int }\n@contract B { b: int }\n";
    // 顺序无关：@is B A 也应整体严格
    let e = err(&format!(
        "{head}doc {{\n  @is B A\n  a: 1\n  b: 2\n  extra: 9\n}}\n"
    ));
    assert!(
        e.contains("E-CONTRACT-004") && e.contains("未在契约"),
        "@is B A 也应整体严格（顺序无关），got: {e}"
    );
}

/// 两个都 warn ⇒ 整体 warn：放行 + 各一条 Warning（钉住 warn 叠加也走告警，而非静默）。
#[test]
fn warn_plus_warn_is_warn_with_warnings() {
    let head = "@contract A warn { a: int }\n@contract B warn { b: int }\n";
    let (v, warns) = with_warns(&format!(
        "{head}doc {{\n  @is A B\n  a: 1\n  b: 2\n  extra: 9\n}}\n"
    ));
    assert!(v.get("doc.extra").is_some(), "双 warn 应放行额外字段");
    assert_eq!(warns.len(), 1, "未声明字段 `extra` 只应产出 1 条告警，实际 {warns:?}");
    assert_eq!(warns[0].kind, DiagnosticKind::Warning);
}

// ---------- ⑥ warn 态不放行 类型不符 / 必填缺失（钉住 D1 边界）----------

/// 类型不符：warn 态下**仍 Err**（错类型数据放行出去是灾难）。
#[test]
fn warn_does_not_pardon_type_mismatch() {
    // port 是 int，写成字符串 ⇒ 类型错误，warn 也不放行
    let src = format!("{C}doc {{\n  @is Cfg warn\n  host: db1\n  port: not-a-number\n}}\n");
    let e = err(&src);
    assert!(
        e.contains("E-CONTRACT-002") || e.contains("类型应为"),
        "warn 不应放过类型不符，got: {e}"
    );
}

/// 必填缺失：warn 态下**仍 Err**（缺字段不是形状宽容问题）。
#[test]
fn warn_does_not_pardon_missing_required() {
    // host 必填但缺失 ⇒ 必填错误，warn 也不放行
    let src = format!("{C}doc {{\n  @is Cfg warn\n  port: 8080\n}}\n");
    let e = err(&src);
    assert!(
        e.contains("E-CONTRACT-003") || e.contains("必填"),
        "warn 不应放过必填缺失，got: {e}"
    );
}

// ---------- ⑦ parse() 看不到 warn 告警；parse_with_warnings 看得到 ----------

#[test]
fn plain_parse_hides_warning_diagnostics() {
    // `parse()` 在 api.rs 里把诊断丢掉 —— 拿到的是值，没有诊断通道。
    // 这里验证：调用方用 `parse()` 时，即便文档触发了 warn，也不会因为「看不见」而误判；
    // 值照常放行（与 with_warnings 同树）。
    let src = format!("{C}doc {{\n  @is Cfg warn\n  host: db1\n  extra: 1\n}}\n");
    let v = ok(&src);
    assert!(v.get("doc.extra").is_some(), "parse() 下 warn 也应放行（值不变）");
    // 用 with_warnings 反证：同文档确实产出了告警，只是 parse() 那条路不返回
    let (_v2, warns) = with_warns(&src);
    assert_eq!(warns.len(), 1, "对照：with_warnings 应看得到这条告警");
    assert_eq!(warns[0].kind, DiagnosticKind::Warning);
}

#[test]
fn with_warnings_sees_warning_diagnostics() {
    let src = format!("{C}doc {{\n  @is Cfg warn\n  host: db1\n  extra: 1\n}}\n");
    let (_v, warns) = with_warns(&src);
    assert_eq!(warns.len(), 1);
    assert_eq!(warns[0].kind, DiagnosticKind::Warning);
    assert!(warns[0].message.contains("extra"));
}
