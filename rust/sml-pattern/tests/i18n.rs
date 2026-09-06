//! 关键字国际化验收：中 / 英 / 混写三种写法，语义必须完全一致。
//!
//! 同时验证扩展点：用户可实现 KeywordTable trait 自带任意语言。

use std::collections::BTreeMap;

use sml_pattern::{BILINGUAL, CHINESE, Concept, ENGLISH, KeywordTable, matches_with, StaticTable};

/// 从 SML 源码中取出 @规则名 片段作为规则表
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

// 同一条「日期」规则，三种书写形式
const CN: &str = r#"
@日期 {
    序列: [
        { 名: 年, 类: 数字, 次: 4 }
        { 字面: "-" }
        { 名: 月, 类: 数字, 次: 2 }
        { 字面: "-" }
        { 名: 日, 类: 数字, 次: 2 }
    ]
}
"#;

const EN: &str = r#"
@date {
    seq: [
        { name: year, class: digit, times: 4 }
        { lit: "-" }
        { name: month, class: digit, times: 2 }
        { lit: "-" }
        { name: day, class: digit, times: 2 }
    ]
}
"#;

const MIXED: &str = r#"
@日期 {
    seq: [
        { name: 年, class: 数字, times: 4 }
        { 字面: "-" }
        { 名: month, 类: digit, 次: 2 }
        { lit: "-" }
        { name: 日, class: 数字, times: 2 }
    ]
}
"#;

#[test]
fn 中文写法可匹配() {
    let r = rules_from(CN);
    assert!(!r.is_empty(), "规则表为空");
    assert!(matches_with(&r, "日期", "2026-09-06", &CHINESE).unwrap());
    assert!(!matches_with(&r, "日期", "2026-9-6", &CHINESE).unwrap());
}

#[test]
fn 英文写法可匹配() {
    let r = rules_from(EN);
    assert!(!r.is_empty(), "规则表为空");
    assert!(matches_with(&r, "date", "2026-09-06", &ENGLISH).unwrap());
    assert!(!matches_with(&r, "date", "2026-9-6", &ENGLISH).unwrap());
}

#[test]
fn 中英混写可匹配() {
    let r = rules_from(MIXED);
    assert!(!r.is_empty(), "规则表为空");
    assert!(matches_with(&r, "日期", "2026-09-06", &BILINGUAL).unwrap());
    assert!(!matches_with(&r, "日期", "2026-9-6", &BILINGUAL).unwrap());
}

#[test]
fn 三种写法语义完全一致() {
    // 同一批输入，在三套规则上必须得到相同判定
    let cn = rules_from(CN);
    let en = rules_from(EN);
    let mixed = rules_from(MIXED);
    let inputs = ["2026-09-06", "2026-9-6", "2026-09", "2026-09-06x", ""];

    for s in inputs {
        let a = matches_with(&cn, "日期", s, &CHINESE).unwrap();
        let b = matches_with(&en, "date", s, &ENGLISH).unwrap();
        let c = matches_with(&mixed, "日期", s, &BILINGUAL).unwrap();
        assert_eq!(a, b, "中英判定不一致，输入 `{s}`");
        assert_eq!(a, c, "中文与混写判定不一致，输入 `{s}`");
    }
}

#[test]
fn 单语表不认另一种语言() {
    let cn = rules_from(CN);
    // 中文规则用纯英文表编译 → 键名无法识别 → 报错（而不是静默匹配错）
    let r = matches_with(&cn, "日期", "2026-09-06", &ENGLISH);
    assert!(r.is_err(), "纯英文表不应识别中文键，实际: {r:?}");
}

// ---------------------------------------------------------------------------
// 扩展点：用户自带语言表
// ---------------------------------------------------------------------------

/// 一支虚构语言的表，用来证明 trait 扩展点真的可用
struct PigLatin;

impl KeywordTable for PigLatin {
    fn lookup(&self, word: &str) -> Option<Concept> {
        Some(match word {
            "eqes" => Concept::Seq,          // seq
            "asscl" => Concept::Class,       // class
            "igitday" => Concept::Digit,     // digit
            "imestay" => Concept::Times,     // times
            "itlay" => Concept::Lit,         // lit
            "amenay" => Concept::Name,       // name
            _ => return None,
        })
    }
}

#[test]
fn 自定义关键字表可用() {
    let src = r#"
@date {
    eqes: [
        { amenay: y, asscl: igitday, imestay: 4 }
        { itlay: "-" }
        { amenay: m, asscl: igitday, imestay: 2 }
    ]
}
"#;
    let r = rules_from(src);
    assert!(!r.is_empty(), "规则表为空");
    assert!(matches_with(&r, "date", "2026-09", &PigLatin).unwrap());
    assert!(!matches_with(&r, "date", "2026-9", &PigLatin).unwrap());
}

#[test]
fn 静态表可自定义构造() {
    use Concept::*;
    // 用 StaticTable 构造一张精简表
    const TINY: StaticTable = StaticTable::new(&[
        ("seq", Seq),
        ("class", Class),
        ("digit", Digit),
        ("times", Times),
        ("lit", Lit),
    ]);
    let src = r#"
@d {
    seq: [
        { class: digit, times: 2 }
        { lit: "-" }
        { class: digit, times: 2 }
    ]
}
"#;
    let r = rules_from(src);
    assert!(matches_with(&r, "d", "12-34", &TINY).unwrap());
    assert!(!matches_with(&r, "d", "1-34", &TINY).unwrap());
}
