//! L1 引擎验收：真实场景的模式能否匹配，以及是否免疫 ReDoS。

use std::collections::BTreeMap;

use sml_pattern::{matches, Class, Pat};

/// 从 SML 源码里取出全部 @规则名 片段，作为规则表。
fn rules_from(src: &str) -> BTreeMap<String, Value> {
    // 规则定义本身不进主树，故解析时把每条规则额外挂一个引用取出
    let mut out = BTreeMap::new();
    for line in src.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix('@') {
            if let Some(name) = rest.split_whitespace().next() {
                if name.contains(':') || name.is_empty() {
                    continue;
                }
                let probe = format!("{src}\n__probe_{name}: &{name}\n");
                if let Ok(v) = sml::parse(&probe) {
                    let key = format!("__probe_{name}");
                    if let Some(val) = v.get(&key) {
                        out.insert(name.to_string(), val.clone());
                    }
                }
            }
        }
    }
    out
}

use sml::Value;

const RULES: &str = r#"
@手机号 {
    序列: [
        { 名: 号, 类: 数字, 次: 11 }
    ]
}

@身份证分组 {
    序列: [
        { 名: 一, 类: 数字, 次: 6 }
        { 字面: " " }
        { 名: 二, 类: 数字, 次: 4 }
        { 字面: " " }
        { 名: 三, 类: 数字, 次: 4 }
        { 字面: " " }
        { 名: 四, 类: 数字, 次: 4 }
    ]
}

@区号电话 {
    序列: [
        { 名: 区号, 类: 数字, 次: 4 }
        { 字面: "-" }
        { 名: 号码, 类: 数字, 次: 8 }
    ]
}

@日期ISO {
    序列: [
        { 名: 年, 类: 数字, 次: 4 }
        { 字面: "-" }
        { 名: 月, 类: 数字, 次: 2 }
        { 字面: "-" }
        { 名: 日, 类: 数字, 次: 2 }
    ]
}

@日志行 {
    序列: [
        { 名: 日期, 用: 日期ISO }
        { 类: 空白, 次: "+" }
        { 名: 级别, 任一: [ ERROR WARN INFO ] }
        { 类: 空白, 次: "+" }
        { 名: 消息, 直到: 行尾 }
    ]
}
"#;

#[test]
fn 真实号码可匹配() {
    let rules = rules_from(RULES);
    assert!(!rules.is_empty(), "规则表不应为空，实际: {:?}", rules.keys());

    assert!(matches(&rules, "手机号", "13800138000").unwrap());
    assert!(!matches(&rules, "手机号", "1380013800").unwrap(), "10 位应拒绝");
    assert!(!matches(&rules, "手机号", "13800138000x").unwrap(), "含字母应拒绝");

    assert!(matches(&rules, "身份证分组", "221099 1988 0987 1211").unwrap());
    assert!(!matches(&rules, "身份证分组", "221099 1988 0987 121").unwrap());

    assert!(matches(&rules, "区号电话", "0571-23116789").unwrap());
    assert!(!matches(&rules, "区号电话", "057123116789").unwrap(), "缺连字符应拒绝");
}

#[test]
fn 组合与引用可工作() {
    let rules = rules_from(RULES);
    // 日志行引用了 日期ISO
    assert!(matches(&rules, "日志行", "2026-09-06 INFO 系统启动").unwrap());
    assert!(matches(&rules, "日志行", "2026-09-06   WARN   磁盘不足").unwrap());
    assert!(!matches(&rules, "日志行", "2026-9-6 INFO 月份不足两位").unwrap());
    assert!(!matches(&rules, "日志行", "2026-09-06 TRACE 级别不在白名单").unwrap());
}

#[test]
fn 循环引用被拒绝() {
    let mut rules = rules_from("@甲 { 序列: [ { 用: 乙 } ] }\n@乙 { 序列: [ { 用: 甲 } ] }\n");
    // 两个规则互相引用
    if rules.contains_key("甲") && rules.contains_key("乙") {
        let r = matches(&rules, "甲", "x");
        assert!(r.is_err(), "循环引用应报错而非栈溢出，实际: {r:?}");
    }
    let _ = &mut rules;
}

#[test]
fn 量词与选择的基本语义() {
    // 精确次数
    let p = Pat::Repeat { pat: Box::new(Pat::Class(Class::Digit)), min: 3, max: Some(3) };
    assert!(sml_pattern::is_match(&p, "123").unwrap());
    assert!(!sml_pattern::is_match(&p, "12").unwrap());
    assert!(!sml_pattern::is_match(&p, "1234").unwrap());

    // 无上界 +
    let p = Pat::Repeat { pat: Box::new(Pat::Class(Class::Digit)), min: 1, max: None };
    assert!(sml_pattern::is_match(&p, "1").unwrap());
    assert!(sml_pattern::is_match(&p, "123456").unwrap());
    assert!(!sml_pattern::is_match(&p, "").unwrap());

    // 区间 2-4
    let p = Pat::Repeat { pat: Box::new(Pat::Class(Class::Digit)), min: 2, max: Some(4) };
    assert!(sml_pattern::is_match(&p, "12").unwrap());
    assert!(sml_pattern::is_match(&p, "1234").unwrap());
    assert!(!sml_pattern::is_match(&p, "1").unwrap());
    assert!(!sml_pattern::is_match(&p, "12345").unwrap());

    // 选择
    let p = Pat::Alt(vec![Pat::Lit("ERROR".into()), Pat::Lit("WARN".into())]);
    assert!(sml_pattern::is_match(&p, "ERROR").unwrap());
    assert!(sml_pattern::is_match(&p, "WARN").unwrap());
    assert!(!sml_pattern::is_match(&p, "INFO").unwrap());

    // until
    let p = Pat::Seq(vec![
        Pat::Lit("[".into()),
        Pat::Until { pat: Box::new(Pat::Lit("]".into())) },
    ]);
    assert!(sml_pattern::is_match(&p, "[abc]").unwrap());
}

#[test]
fn 量词最小最大可工作() {
    // 1) 嵌套对象形式：次: { 最小, 最大 }
    let rules = rules_from(
        r#"
@码_nested {
    序列: [ { 类: 数字, 次: { 最小: 4, 最大: 4 } } ]
}
"#,
    );
    assert!(matches(&rules, "码_nested", "1234").unwrap());
    assert!(!matches(&rules, "码_nested", "123").unwrap(), "3 位应拒绝");
    assert!(!matches(&rules, "码_nested", "12345").unwrap(), "5 位应拒绝");

    // 2) 平铺形式：最小 / 最大 直接写在元素上（机翻等价直觉写法）
    let rules = rules_from(
        r#"
@码_flat {
    序列: [ { 类: 数字, 最小: 6, 最大: 6 } ]
}
"#,
    );
    assert!(matches(&rules, "码_flat", "123456").unwrap());
    assert!(!matches(&rules, "码_flat", "12345").unwrap(), "5 位应拒绝");

    // 3) 仅给出 最大：等价于 0..max
    let rules = rules_from(
        r#"
@码_open {
    序列: [ { 类: 数字, 最大: 2 } ]
}
"#,
    );
    assert!(matches(&rules, "码_open", "").unwrap(), "0 位应满足 0..2");
    assert!(matches(&rules, "码_open", "12").unwrap());
    assert!(!matches(&rules, "码_open", "123").unwrap(), "3 位应拒绝");
}

#[test]
fn 量词非法输入报错而非静默() {
    // max < min 应报错
    let rules = rules_from(
        r#"
@坏 {
    序列: [ { 类: 数字, 次: { 最小: 5, 最大: 2 } } ]
}
"#,
    );
    let r = matches(&rules, "坏", "12345");
    assert!(r.is_err(), "max<min 应报错，实际: {r:?}");

    // 负的最小应报错
    let rules = rules_from(
        r#"
@负 {
    序列: [ { 类: 数字, 最小: -1 } ]
}
"#,
    );
    let r = matches(&rules, "负", "1");
    assert!(r.is_err(), "负最小应报错，实际: {r:?}");
}

/// ReDoS 免疫：经典病态输入 `(a+)+b` 在回溯引擎上指数爆炸。
/// 本引擎无回溯，故应在毫秒级返回 false。
#[test]
fn redos_免疫() {
    // 等价 (a+)+b 的结构：Repeat(Repeat(a,1,None),1,None) 后接 b
    let inner = Pat::Repeat { pat: Box::new(Pat::Class(Class::One('a'))), min: 1, max: None };
    let p = Pat::Seq(vec![
        Pat::Repeat { pat: Box::new(inner), min: 1, max: None },
        Pat::Lit("b".into()),
    ]);

    let text = "a".repeat(40); // 无结尾 b，回溯引擎在此指数爆炸
    let t0 = std::time::Instant::now();
    let r = sml_pattern::is_match(&p, &text).unwrap();
    let dt = t0.elapsed();
    assert!(!r, "无结尾 b 应不匹配");
    assert!(dt.as_millis() < 500, "耗时 {dt:?} 过长，疑似存在回溯");
}
