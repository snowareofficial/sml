//! 政务示范样例守护测试。
//!
//! 官网 demo 页（/demo-gov/）的练习场预填内容来自 `tests/fixtures/gov_demo.sml`
//! （内容同时也存在于 `site/data/sml-lessons.json`）。
//! 该文件被两个引擎（Rust wasm / JS）同时在页面上解析，因此它必须：
//! 1. 能被 Rust 解析器无错接受；
//! 2. 关键语义（strict 契约、enum 白名单、片段展开、字符串保真）符合预期。
//! 样例改动 → 同步更新 `site/data/sml-lessons.json` 与 demo 页文案。
//!
//! ⚠️ **夹具位置是本轮（2026-09-18）挪进来的**：它原先叫 `_gov_demo.sml`、放在仓库根，
//! 而 `.gitignore` 有 `**/_*` ⇒ **这个夹具从未进过版本库**，于是 `cargo test` 只在
//! 「本机恰好有那个文件」时才过（换台机器 / CI 必红，且报的是「读取失败」这种与测试
//! 意图无关的错）。清理仓库里的 `_` 文件时一并改名挪进 `tests/fixtures/`，现在它是**已跟踪**的。

use sml::Value;

fn gov_src() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/gov_demo.sml");
    std::fs::read_to_string(path).expect("读取 tests/fixtures/gov_demo.sml 失败")
}

#[test]
fn gov_demo_parses_clean() {
    let v = sml::parse(&gov_src()).expect("政务示范样例应无错解析");
    // 两个窗口块都存在且通过了 strict 契约（契约本身不出现在树里）
    assert!(v.get("公安窗口").is_some(), "缺 公安窗口");
    assert!(v.get("人社窗口").is_some(), "缺 人社窗口");
}

#[test]
fn gov_demo_fragment_expanded_in_both_blocks() {
    let v = sml::parse(&gov_src()).unwrap();
    for k in ["公安窗口", "人社窗口"] {
        let blk = v.get(k).expect(k);
        assert_eq!(
            blk.get("办理地点"),
            Some(&Value::Str("区行政服务中心".into())),
            "{k}: &base 片段未展开"
        );
        // 区划代码是带引号的字符串 —— 保真，不会被当数字
        assert_eq!(
            blk.get("区划代码"),
            Some(&Value::Str("330106".into())),
            "{k}: 区划代码应为字符串（引号保真）"
        );
    }
}

#[test]
fn gov_demo_enum_and_strict_semantics() {
    // 密级在 enum 白名单内 → 通过并保留
    let v = sml::parse(&gov_src()).unwrap();
    assert_eq!(
        v.get("人社窗口").unwrap().get("密级"),
        Some(&Value::Str("内部".into()))
    );
    // default 生效：公安窗口未写密级 → 公开
    assert_eq!(
        v.get("公安窗口").unwrap().get("密级"),
        Some(&Value::Str("公开".into()))
    );

    // 越界密级（不在白名单）→ 拦截
    let bad = gov_src().replace("密级: 内部", "密级: 内部涉密");
    assert!(sml::parse(&bad).is_err(), "enum 白名单外的密级应被拦截");

    // 时限越界（> 90）→ 拦截
    let bad = gov_src().replace("承诺时限: 7", "承诺时限: 91");
    assert!(sml::parse(&bad).is_err(), "时限超过 max 90 应被拦截");

    // strict：块内出现未声明字段 → 拦截（拼错字段立即暴露）
    let bad = gov_src().replace("电话: \"0571-87654321\"", "电话: \"0571-87654321\"\n  备注: x");
    assert!(sml::parse(&bad).is_err(), "strict 契约应拒绝未声明字段");

    // 同名行为对齐 JS 引擎（_test_gov_demo.mjs 同款断言）：
    // &base 在前 + 显式同名在后 → 数组提升。用无契约变体验证
    // （demo 样例的 strict 契约会把 array 拦下——那本身也是两端一致的行为）
    let v = sml::parse(
        "@base { 区划代码: \"330106\" }\nw { &base\n  区划代码: \"330102\" }",
    )
    .unwrap();
    assert_eq!(
        v.get("w").unwrap().get("区划代码"),
        Some(&Value::Array(vec![
            Value::Str("330106".into()),
            Value::Str("330102".into()),
        ])),
        "片段在前+显式同名应为数组（对齐 JS）"
    );

    // 契约约束下同名数组被拦（两端一致）
    let bad = gov_src().replace("&base\n}", "&base\n  区划代码: \"330102\"\n}");
    assert!(
        sml::parse(&bad).is_err(),
        "str 契约字段得到 array 应被拦截"
    );
}
