//! probe_sign.rs — 符号处理探针：当前改动的实际影响面
use sml::{parse, Value};

fn scalar_of(literal: &str) -> Value {
    let text = format!("v: {literal}\n");
    let root = parse(&text).unwrap_or_else(|e| panic!("解析失败 `{literal}`: {e}"));
    root.get("v").cloned().unwrap_or(Value::Null)
}

fn main() {
    // (期望仍为数字?, 字面量)
    let cases: &[(bool, &str)] = &[
        (true, "27"), (true, "-3"), (true, "+3"), (true, "0"),
        (true, "1.5"), (true, "-1.5"), (true, "0.5"), (true, "-0.5"),
        (true, "1e10"), (true, "-1e10"), (true, "1e-10"), (true, "-1.5e-3"),
        (true, ".5"), (true, "-.5"), (true, "5."), (true, "-5."),
        (true, "1e309"), (true, "-1e309"),
        (true, "-0"), (true, "-0.0"),
        // 前导零：期望保留为字符串
        (false, "007"), (false, "-007"), (false, "+007"), (false, "-0755"),
        // 大整数：期望保留为字符串
        (false, "12345678901234567890"), (false, "-12345678901234567890"),
        (false, "-99999999999999999999"),
        // 非数字裸词：期望字符串
        (false, "inf"), (false, "nan"), (false, "-inf"), (false, "hello"),
    ];

    let mut wrong = 0;
    println!("{:<14} {:<22} {}", "字面量", "实际解析为", "判定");
    println!("{}", "-".repeat(64));
    for (want_num, lit) in cases {
        let got = scalar_of(lit);
        let is_num = matches!(got, Value::Int(_) | Value::Float(_, _));
        let ok = is_num == *want_num;
        if !ok { wrong += 1; }
        let got_str = format!("{got:?}");
        let verdict = if ok {
            "正确".to_string()
        } else {
            format!("错误 (期望{})", if *want_num { "数字" } else { "字符串" })
        };
        println!("{lit:<14} {got_str:<22} {verdict}");
    }
    println!("{}", "-".repeat(64));
    println!("不符合预期: {wrong} / {}", cases.len());
}
