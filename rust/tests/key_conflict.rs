// SPDX-License-Identifier: MulanPSL-2.0
//! 雷 5 · A 组 4-6：「键被静默改写 / 覆盖」的告警出口。
//!
//! # 判据
//!
//! 不是「不 panic」，而是**告警真的产生了** —— 每条正例都断言
//! `warns` 的条数、kind、文案要点与行号；并且每条正例都配一条
//! **反例**（不该告警的场景必须 0 条），否则「永远告警」也能骗过测试。
//!
//! # 语义不变是硬约束
//!
//! 本轮**只加告警、不改语义**：重复键仍提升为数组、片段值仍覆盖父块键。
//! 每条正例都同时断言 `Value` 与旧版一致 —— 否则就是行为变更，要另走流程。
//!
//! 唯一的行为变更在 `#[sml(flatten)]`（见 `macro_roundtrip.rs` 的
//! `flatten_key_clash_*` 两条）：那里是**丢值**，故改为报错。

use sml::{
    parse, parse_file_with_warnings, parse_with_warnings, DiagnosticKind, ParseOptions, Value,
};

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("sml_key_conflict_{tag}"));
    std::fs::create_dir_all(&d).expect("临时目录应可建");
    d
}

// ---------- 1. 重复键 ⇒ DuplicateKey（值仍提升为数组） ----------

#[test]
fn duplicate_key_warns_and_still_promotes_to_array() {
    let (v, warns) = parse_with_warnings("k: 1\nk: 2\n").unwrap();

    assert_eq!(warns.len(), 1, "重复键应产生 1 条告警，实际 {warns:?}");
    assert_eq!(warns[0].kind, DiagnosticKind::DuplicateKey);
    assert!(warns[0].message.contains("k"), "文案须点名键：{}", warns[0].message);
    assert!(warns[0].message.contains("数组"), "文案须说明改成了什么：{}", warns[0].message);
    assert_eq!(warns[0].line, Some(2), "第二次出现应在第 2 行：{warns:?}");

    // 语义未变：仍是数组（本轮只加告警，不改语义）
    match v.get("k") {
        Some(Value::Array(a)) => assert_eq!(a.len(), 2, "应得 [1, 2]，实际 {a:?}"),
        other => panic!("重复键仍应提升为数组，实际 {other:?}"),
    }
}

/// 反例：互不相同的键不得告警。
#[test]
fn distinct_keys_never_warn() {
    let (_, warns) = parse_with_warnings("a: 1\nb: 2\nc { d: 3 }\n").unwrap();
    assert!(warns.is_empty(), "无重复键不该告警，实际 {warns:?}");
}

/// 反例：**不同**块里的同名键互不冲突，不得告警。
#[test]
fn same_key_in_different_blocks_never_warns() {
    let (_, warns) = parse_with_warnings("a { k: 1 }\nb { k: 2 }\n").unwrap();
    assert!(warns.is_empty(), "不同块的同名键不该告警，实际 {warns:?}");
}

/// 反例：键是数组字面量、只写一次不得告警（不是重复）。
#[test]
fn single_array_literal_never_warns() {
    let (_, warns) = parse_with_warnings("k: [ 1 2 3 ]\n").unwrap();
    assert!(warns.is_empty(), "单次数组字面量不该告警，实际 {warns:?}");
}

// ---------- 2. 片段展开覆盖 ⇒ KeyOverwritten（值仍是片段值） ----------

#[test]
fn fragment_splice_overwrite_warns_and_names_the_key() {
    // `w` 已有 `x: 9`，`&base` 又带来 `x` ⇒ 父块的 9 被静默丢掉
    let src = "@base { x: 1 y: 2 }\nw {\n  x: 9\n  &base\n}\n";
    let (v, warns) = parse_with_warnings(src).unwrap();

    assert_eq!(warns.len(), 1, "片段覆盖应产生 1 条告警，实际 {warns:?}");
    assert_eq!(warns[0].kind, DiagnosticKind::KeyOverwritten);
    assert!(
        warns[0].message.contains("x"),
        "文案须点名被覆盖的键：{}",
        warns[0].message
    );
    assert!(
        warns[0].message.contains("base"),
        "文案须点名是哪个片段覆盖的：{}",
        warns[0].message
    );
    assert_eq!(warns[0].line, Some(4), "`&base` 在第 4 行：{warns:?}");

    // 语义未变：片段值胜（ch03/ch08 既定），父块的 9 确实丢了 —— 这正是要告警的原因
    let w = v.get("w").expect("缺 w");
    assert!(
        matches!(w.get("x"), Some(Value::Int(1))),
        "片段值仍应覆盖（语义未变），实际 {:?}",
        w.get("x")
    );
    assert!(matches!(w.get("y"), Some(Value::Int(2))));
}

/// 反例：片段带来的键与父块**不**重名 ⇒ 不得告警。
#[test]
fn fragment_splice_without_clash_never_warns() {
    let src = "@base { y: 2 }\nw {\n  x: 9\n  &base\n}\n";
    let (_, warns) = parse_with_warnings(src).unwrap();
    assert!(warns.is_empty(), "不撞键的片段展开不该告警，实际 {warns:?}");
}

// ---------- 3. 门控：warn_key_conflict(false) 可关 ----------

#[test]
fn key_conflict_warnings_can_be_switched_off() {
    let out = sml::parse_with(
        "k: 1\nk: 2\n",
        ParseOptions::new().warn_key_conflict(false),
    )
    .unwrap();
    assert!(
        out.diagnostics.is_empty(),
        "关掉开关后不该再收集键冲突告警，实际 {:?}",
        out.diagnostics
    );
    // 值不受开关影响
    assert!(matches!(out.value.get("k"), Some(Value::Array(_))));
}

/// 默认开关是开着的（`ParseOptions::new()` 即收）。
#[test]
fn key_conflict_warnings_are_on_by_default() {
    let out = sml::parse_with("k: 1\nk: 2\n", ParseOptions::new()).unwrap();
    assert_eq!(out.diagnostics.len(), 1, "默认应收集，实际 {:?}", out.diagnostics);
}

// ---------- 4. 零影响钉子：parse() 恒不返回诊断，值也不变 ----------

#[test]
fn plain_parse_is_untouched() {
    // `parse()` 在 api.rs 里就把诊断丢掉了 —— 这是「既有调用方零影响」的钉子：
    // 它拿不到告警（想要告警请用 `parse_with_warnings`），但值也与旧版逐字一致。
    let v = parse("k: 1\nk: 2\n").unwrap();
    assert!(matches!(v.get("k"), Some(Value::Array(_))), "值应与旧版一致：{v:?}");
}

// ---------- 5. 文件入口（片段常定义在子文件里，文本入口会漏） ----------

#[test]
fn file_entry_warns_without_line_numbers() {
    let d = tmp_dir("file");
    let p = d.join("dup.sml");
    std::fs::write(&p, "k: 1\nk: 2\n").unwrap();

    let (v, warns) = parse_file_with_warnings(&p).unwrap();
    assert_eq!(warns.len(), 1, "文件入口也应告警，实际 {warns:?}");
    assert_eq!(warns[0].kind, DiagnosticKind::DuplicateKey);
    assert_eq!(
        warns[0].line,
        None,
        "文件入口的 token 流跨文件合并，拿不到行号：{warns:?}"
    );
    assert!(
        warns[0].message.contains("smltools --lint"),
        "无行号时文案应指引拿行号的方式：{}",
        warns[0].message
    );
    assert!(matches!(v.get("k"), Some(Value::Array(_))));

    std::fs::remove_file(&p).ok();
}

/// 片段定义在**被 include 的子文件**里 —— 只有文本入口会漏掉这一类。
#[test]
fn file_entry_catches_fragment_overwrite_across_include() {
    let d = tmp_dir("include");
    std::fs::write(d.join("base.sml"), "@base { x: 1 }\n").unwrap();
    let main = d.join("main.sml");
    std::fs::write(&main, "include \"base.sml\"\nw {\n  x: 9\n  &base\n}\n").unwrap();

    let (v, warns) = parse_file_with_warnings(&main).unwrap();
    assert_eq!(warns.len(), 1, "跨 include 的片段覆盖应被抓到，实际 {warns:?}");
    assert_eq!(warns[0].kind, DiagnosticKind::KeyOverwritten);
    assert!(
        warns[0].message.contains("x"),
        "文案须点名被覆盖的键：{}",
        warns[0].message
    );

    let w = v.get("w").expect("缺 w");
    assert!(
        matches!(w.get("x"), Some(Value::Int(1))),
        "片段值仍应覆盖（语义未变），实际 {:?}",
        w.get("x")
    );
}
