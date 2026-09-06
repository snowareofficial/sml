//! audit_repr.rs — 数字「表示方式」审计：同一数值的不同写法，round-trip 后还在不在？
//!
//! 运行: cargo run -p swsml --example audit_repr
//!
//! 与 audit_num.rs 的区别：
//!   audit_num 检的是「值对不对」（数值/类型是否保真）
//!   audit_repr 检的是「写法还在不在」（表示方式是否保真）
//!
//! 每个 case 单独 parse，一个失败不影响其余。

use sml::{parse, to_sml, Value};

// (标签, 字面量)
const CASES: &[(&str, &str)] = &[
    // ---- 科学计数法家族 ----
    ("sci_lower", "1e10"),
    ("sci_upper", "1E10"),
    ("sci_plus", "1e+10"),
    ("sci_minus", "1e-10"),
    ("sci_e2", "1e2"),
    ("sci_big", "1e100"),
    ("sci_tiny", "1e-320"),
    ("sci_huge", "1e309"), // 溢出 f64
    // ---- 进制前缀 ----
    ("hex_upper", "0x1F"),
    ("hex_lower", "0xff"),
    ("oct_legacy", "017"), // C 风格八进制
    ("oct_modern", "0o17"),
    ("bin", "0b101"),
    // ---- 小数点位置 ----
    ("lead_dot", ".5"),
    ("trail_dot", "5."),
    ("zero_lead", "0.5"),
    // ---- 零的书写 ----
    ("tz1", "1.0"),
    ("tz2", "1.10"),
    ("tz3", "1.000"),
    ("lz1", "007"),
    ("lz2", "01234"),
    // ---- 分隔符 ----
    ("sep", "1_000"),
    // ---- 疑似被误判为数字的裸词（挪威问题同类）----
    ("w_inf", "inf"),
    ("w_INF", "INF"),
    ("w_infinity", "infinity"),
    ("w_neg_inf", "-inf"),
    ("w_nan", "nan"),
    ("w_NAN", "NAN"),
    // ---- 对照：正常数值 ----
    ("plain_int", "27"),
    ("plain_neg", "-3"),
    ("plain_float", "1.5"),
];

fn main() {
    println!("{:<13} {:<12} {:<26} {:<26} {}",
             "标签", "字面量", "解析为", "dump 出的写法", "判定");
    println!("{}", "-".repeat(96));

    let mut repr_changed = 0;
    let mut unstable = 0;
    let mut mistyped = 0;

    for (tag, lit) in CASES {
        let text = format!("v: {lit}\n");
        let v1 = match parse(&text) {
            Ok(v) => v,
            Err(e) => {
                println!("{tag:<13} {lit:<12} PARSE ERROR: {e}");
                continue;
            }
        };

        // 取标量值
        let scalar = v1.get("v").cloned().unwrap_or(Value::Null);

        // dump 出的字面量（去掉 "v: " 前缀）
        let dumped = to_sml(&v1);
        let out_lit = dumped
            .trim()
            .strip_prefix("v: ")
            .unwrap_or_else(|| dumped.trim())
            .trim()
            .to_string();

        // 表示方式是否保持
        let kept = out_lit == *lit;

        // 值是否稳定：再解析一次 dump 文本
        let v2 = parse(&dumped).ok();
        let stable = match &v2 {
            Some(v2) => v2.get("v") == Some(&scalar),
            None => false,
        };

        // 疑似误判：原字面量不是纯数字形态，却被解析成了 Int/Float
        let is_num_literal = lit.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
            || lit.starts_with('.')
            || lit.starts_with('-')
            || lit.starts_with('+');
        let became_num = matches!(scalar, Value::Int(_) | Value::Float(_, _));
        let suspicious = !is_num_literal && became_num;

        let mut marks = Vec::new();
        if !kept {
            repr_changed += 1;
            marks.push("写法变".to_string());
        }
        if !stable {
            unstable += 1;
            marks.push("值不稳".to_string());
        }
        if suspicious {
            mistyped += 1;
            marks.push("**误判为数**".to_string());
        }

        // 浮点值打印完整精度，便于看真实数值
        let val_str = match &scalar {
            Value::Float(f, _) => format!("Float({f:e})"),
            other => format!("{other:?}"),
        };

        println!("{tag:<13} {lit:<12} {val_str:<26} {out_lit:<26} {}",
                 if marks.is_empty() { "保持".to_string() } else { marks.join(",") });
    }

    println!("{}", "-".repeat(96));
    println!("写法改变: {repr_changed} 例 | 值不稳定(再解析即变): {unstable} 例 | 疑似误判为数字: {mistyped} 例");
}
