//! 量词上限 / 编译规模的回归测试（安全：防病态规则 OOM / 卡死）。
//!
//! 背景：有上界的量词在 `nfa` 编译时被**物化成 n 份指令**。若无上限，恶意
//! `@type`（如 `次: 1000000000`）会让编译物化天文数字指令 → OOM / 卡死（DoS）。
//! 现有两道闸：单量词 `QUANT_MAX`（对齐 JS 的 1000）+ 编译指令总数 `MAX_PROG`
//! （挡**嵌套**量词相乘膨胀）。这些测试钉住：超限必须**快速报错**而非 OOM。

use std::collections::BTreeMap;

use sml_pattern::{compile_rule, is_match, QUANT_MAX};
use sml::Value;

/// 从 SML 源码里取出全部 `@规则名` 片段作为规则表（与 l1_basic 同一手法）。
fn rules_from(src: &str) -> BTreeMap<String, Value> {
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

#[test]
fn huge_single_quantifier_is_rejected_not_oom() {
    // 单个量词写成天文数字：必须快速报「量词过大」，而不是物化到 OOM/卡死。
    let rules = rules_from("@Evil { 序列: [ { 类: 数字, 次: 1000000000 } ] }\n");
    let e = compile_rule(rules.get("Evil").unwrap(), &rules).unwrap_err();
    assert!(e.contains("量词过大"), "应报量词过大，实得: {e}");
}

#[test]
fn quantifier_boundary_rejects_over_allows_within() {
    // QUANT_MAX+1 拒；QUANT_MAX 之内照常编译（不误伤合法规则）。
    let over = rules_from(&format!(
        "@T {{ 序列: [ {{ 类: 数字, 次: {} }} ] }}\n",
        QUANT_MAX + 1
    ));
    assert!(
        compile_rule(over.get("T").unwrap(), &over).unwrap_err().contains("量词过大"),
        "QUANT_MAX+1 应被拒"
    );
    let at = rules_from(&format!(
        "@T {{ 序列: [ {{ 类: 数字, 次: {} }} ] }}\n",
        QUANT_MAX
    ));
    assert!(compile_rule(at.get("T").unwrap(), &at).is_ok(), "QUANT_MAX 之内应编译通过");
}

#[test]
fn nested_quantifier_explosion_capped_by_max_prog() {
    // 单个量词都在 QUANT_MAX 内、compile_rule 建 IR 成功；但物化发生在 nfa::compile
    // （is_match 内）：1000×1000=1e6 > MAX_PROG(1e5) ⇒ 报「程序过大」，不 OOM。
    let rules = rules_from(
        "@Boom { 序列: [ { 组: { 序列: [ { 类: 数字, 次: 1000 } ] }, 次: 1000 } ] }\n",
    );
    let pat = compile_rule(rules.get("Boom").unwrap(), &rules)
        .expect("IR 编译应通过（各量词未超 QUANT_MAX）");
    let e = is_match(&pat, "1").unwrap_err();
    assert!(e.contains("程序过大"), "应由 MAX_PROG 闸住，实得: {e}");
}

#[test]
fn normal_patterns_still_compile_and_match() {
    // 真实模式（semver）不受影响：照常编译 + 匹配。
    let rules = rules_from(
        "@Semver { 序列: [ { 类: 数字, 次: \"+\" } { 字面: \".\" } { 类: 数字, 次: \"+\" } { 字面: \".\" } { 类: 数字, 次: \"+\" } ] }\n",
    );
    let pat = compile_rule(rules.get("Semver").unwrap(), &rules).unwrap();
    assert!(is_match(&pat, "1.4.2").unwrap());
    assert!(!is_match(&pat, "1.4").unwrap());
}
