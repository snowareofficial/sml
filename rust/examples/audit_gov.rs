//! audit_gov.rs — 政务数据实测：SML 对真实政务字段的保真性
//!
//! 运行: cargo run -p swsml --features serde --example audit_gov
//!
//! 背景：交接手册 3.4 曾列出「政务数据风险清单」，但标注为 ⚠️ 推测、未实测。
//! 本探针用真实形态的政务数据实测，确认 P1/P3/P4 修复后的实际表现。
//!
//! 关注两类风险：
//!   1. 前导零丢失（行政区划/机构代码以 0 开头）
//!   2. 精度丢失（18 位身份证/信用代码、大额金额超出 2^53 或 2^63）

use sml::{parse, to_sml, Value};

const GOV: &str = r#"行政区划代码: 330106
机构代码: 0571
行政区划前导零: 010101
电话号码: 0571-87052651
邮政编码: 310000
版本号: 1.0
身份证号: 330106201503071234
统一社会信用代码: 91330100MA2XXXXX7B
金额: 123456789012345.67
大额编号: 12345678901234567890
密级: 机密
"#;

/// 是否为「应保持原样」的政务字段（有前导零 / 超精度 / 非纯数字形态）
fn should_stay_text(literal: &str) -> bool {
    let d = literal.trim_start_matches(['+', '-']);
    let has_leading_zero = d.starts_with('0') && d.len() > 1 && !d.contains('.');
    let all_digits = !d.is_empty() && d.chars().all(|c| c.is_ascii_digit());
    // 纯数字且位数 >= 16：超出 JS 安全整数范围（2^53 ~ 16 位）
    let too_big_for_js = all_digits && d.len() >= 16;
    has_leading_zero || too_big_for_js || !all_digits
}

fn main() {
    let v = match parse(GOV) {
        Ok(v) => v,
        Err(e) => return println!("PARSE ERROR: {e}"),
    };

    // 键 -> 原字面量
    let lit_of: Vec<(&str, &str)> = GOV
        .lines()
        .filter_map(|l| l.split_once(": ").map(|(k, v)| (k, v)))
        .collect();

    println!("{:<18} {:<22} {:<24} {}", "字段", "原字面量", "Rust 解析为", "判定");
    println!("{}", "-".repeat(88));

    let mut issues = 0;
    let map = match &v {
        Value::Object(m) => m,
        _ => return,
    };

    for (k, lit) in &lit_of {
        let val = map.get(*k).cloned().unwrap_or(Value::Null);
        let shown = match &val {
            Value::Float(f, raw) => match raw {
                Some(r) => format!("Float({f}) raw={r}"),
                None => format!("Float({f})"),
            },
            other => format!("{other:?}"),
        };

        // 风险评估
        let expect_text = should_stay_text(lit);
        let is_text = matches!(val, Value::Str(_));
        let verdict = if expect_text && !is_text {
            issues += 1;
            "⚠ 风险：应保持文本却成了数字".to_string()
        } else if is_text {
            "保持文本".to_string()
        } else {
            "数值".to_string()
        };

        println!("{k:<18} {lit:<22} {shown:<24} {verdict}");
    }

    println!("{}", "-".repeat(88));
    println!("风险字段数: {issues}");

    println!("\n---- to_sml round-trip ----\n{}", to_sml(&v));

    #[cfg(feature = "serde")]
    {
        println!("\n---- JSON 视图（传给 JS 消费者时的形态）----");
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
}
