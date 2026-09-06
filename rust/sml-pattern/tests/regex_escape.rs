//! regex 逃生舱验收
//!
//! 核心承诺：regex 是**另一种前端**，编译进同一个 IR，
//! 因此逃生舱不会逃掉「无回溯 / 免疫 ReDoS」的保证。

use std::collections::BTreeMap;

use sml_pattern::{matches, regex::parse_regex, Pat};

fn m(pat: &str, text: &str) -> bool {
    let p = parse_regex(pat).unwrap_or_else(|e| panic!("编译 `{pat}` 失败: {e}"));
    sml_pattern::is_match(&p, text).unwrap()
}

#[test]
fn 基础语法可用() {
    // 数字与量词
    assert!(m(r"^\d{11}$", "13800138000"));
    assert!(!m(r"^\d{11}$", "1380013800"));

    // 字符类与范围
    assert!(m(r"^[a-z]+$", "abc"));
    assert!(!m(r"^[a-z]+$", "ABC"));

    // 补集
    assert!(m(r"^\D+$", "abc"));
    assert!(!m(r"^\D+$", "ab1"));

    // 选择
    assert!(m(r"^(ERROR|WARN|INFO)$", "WARN"));
    assert!(!m(r"^(ERROR|WARN|INFO)$", "TRACE"));

    // 分组与量词组合（注意：(\d{4}-){2} 要求两组「4 位数字 + 横杠」，
    // 而日期第二组只有 2 位，故用直观写法）
    assert!(m(r"^(ab){2}$", "abab"));
    assert!(m(r"^\d{4}-\d{2}-\d{2}$", "2026-09-06"));
    assert!(!m(r"^\d{4}-\d{2}-\d{2}$", "2026-9-6"));
    assert!(!m(r"^(\d{4}-){2}\d{2}$", "2026-09-06"), "第二组位数不足，应为 false");

    // 点号与转义
    assert!(m(r"^a.c$", "abc"));
    assert!(m(r"^a\.c$", "a.c"));
    assert!(!m(r"^a\.c$", "abc"));

    // 次数区间
    assert!(m(r"^\d{2,4}$", "12"));
    assert!(m(r"^\d{2,4}$", "1234"));
    assert!(!m(r"^\d{2,4}$", "1"));
    assert!(!m(r"^\d{2,4}$", "12345"));

    // 无上界
    assert!(m(r"^\d{2,}$", "123456"));

    // \w \s
    assert!(m(r"^\w+$", "abc_123"));
    assert!(m(r"^\S+$", "abc"));
    assert!(!m(r"^\S+$", "a b"));
}

#[test]
fn 中文可直接匹配() {
    // Unicode 语义：无需 \p{Han} 之类
    assert!(m(r"^[一-龥]+$", "中文"));
    assert!(!m(r"^[一-龥]+$", "abc"));
}

/// 危险语法必须**报错**，而不是静默降级成能跑但语义错误的东西。
#[test]
fn 危险语法被明确拒绝() {
    // 反向引用
    let e = parse_regex(r"^(\w+)\1$").unwrap_err();
    assert!(e.contains("反向引用"), "实际: {e}");

    // lookahead
    let e = parse_regex(r"^(?=a)a$").unwrap_err();
    assert!(e.contains("不支持"), "实际: {e}");

    // 否定 lookahead
    let e = parse_regex(r"^(?!b)a$").unwrap_err();
    assert!(e.contains("不支持"), "实际: {e}");

    // lookbehind
    let e = parse_regex(r"^(?<=x)a$").unwrap_err();
    assert!(e.contains("不支持"), "实际: {e}");
}

#[test]
fn 懒惰量词被安全忽略() {
    // 无回溯引擎没有贪婪概念，`*?` 的 `?` 可忽略，语义仍正确
    assert!(m(r"^a.*?b$", "axxb"));
    assert!(m(r"^\d+?$", "123"));
}

/// 逃生舱同样免疫 ReDoS：病态模式在无回溯引擎上是多项式的。
#[test]
fn 逃生舱不引入redos() {
    let p = parse_regex(r"^(a+)+b$").unwrap();
    let text = "a".repeat(40);
    let t0 = std::time::Instant::now();
    let r = sml_pattern::is_match(&p, &text).unwrap();
    let dt = t0.elapsed();
    assert!(!r, "无结尾 b 应不匹配");
    assert!(dt.as_millis() < 500, "耗时 {dt:?} 过长，疑似引入回溯");
}

/// 在 SML 规则里使用逃生舱（中英键名皆可）
#[test]
fn 在规则中使用逃生舱() {
    let rules: BTreeMap<String, sml::Value> = rules_from(
        r#"
@手机号 {
    正则: "^1[3-9]\\d{9}$"
}

@date {
    regex: "^\\d{4}-\\d{2}-\\d{2}$"
}
"#,
    );
    assert!(matches(&rules, "手机号", "13800138000").unwrap());
    assert!(!matches(&rules, "手机号", "12800138000").unwrap(), "第二位不在 3-9");
    assert!(matches(&rules, "date", "2026-09-06").unwrap());
    assert!(!matches(&rules, "date", "2026-9-6").unwrap());
}

/// 逃生舱可与结构化写法混用
#[test]
fn 逃生舱与结构化写法混用() {
    let rules: BTreeMap<String, sml::Value> = rules_from(
        r#"
@混合 {
    序列: [
        { 名: 前缀, 正则: "^[A-Z]{2}" }
        { 字面: "-" }
        { 名: 编号, 类: 数字, 次: 4 }
    ]
}
"#,
    );
    assert!(matches(&rules, "混合", "AB-1234").unwrap());
    assert!(!matches(&rules, "混合", "ab-1234").unwrap(), "小写应拒绝");
    assert!(!matches(&rules, "混合", "AB-123").unwrap());
}

fn rules_from(src: &str) -> BTreeMap<String, sml::Value> {
    let mut out = BTreeMap::new();
    for line in src.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix('@') {
            if let Some(name) = rest.split_whitespace().next() {
                if name.contains(':') || name.is_empty() {
                    continue;
                }
                let probe = format!("{src}\n__p_{name}: &{name}\n");
                if let Ok(v) = sml::parse(&probe) {
                    if let Some(val) = v.get(&format!("__p_{name}")) {
                        out.insert(name.to_string(), val.clone());
                    }
                }
            }
        }
    }
    out
}

#[test]
fn 非法regex给出可读错误() {
    let e = parse_regex(r"^[a-z$").unwrap_err();
    assert!(!e.is_empty());
    let e = parse_regex(r"^\d{11$").unwrap_err();
    assert!(e.contains("量词") || e.contains("`}`"), "实际: {e}");
}
