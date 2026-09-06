//! audit_nan.rs — NaN / inf 能否绕过契约的 min/max 区间校验？
//!
//! 运行: cargo run -p swsml --example audit_nan
//!
//! 背景：sml-parse/src/parser.rs:parse_spec_number() 的注释已指出
//!   「Rust 的 "nan".parse::<f64>() == Ok(NaN)，而 NaN 的所有比较均为 false，
//!     会让 min/max 校验被静默绕过」——即**契约边界值**处已做防护。
//!
//! 本探针问的是另一件事：**普通数据值**写成 nan 时，区间校验是否同样失效。
//! NaN 的语义特性：NaN < x == false 且 NaN > x == false，
//! 因此 `if v < min || v > max { 报错 }` 这种写法对 NaN 必然放行。

use sml::parse;

const CONTRACT: &str = "@contract C {\n  w: num min 0 max 100\n}\n";

// (标签, 数据值)
const VALUES: &[(&str, &str)] = &[
    ("对照·正常值", "50"),
    ("对照·越上界", "200"),
    ("对照·越下界", "-5"),
    ("nan 小写", "nan"),
    ("NAN 大写", "NAN"),
    ("NaN 混合", "NaN"),
    ("inf", "inf"),
    ("INF 大写", "INF"),
    ("-inf 负无穷", "-inf"),
    ("1e309 溢出成 inf", "1e309"),
    ("-1e309 负溢出", "-1e309"),
];

fn main() {
    println!("契约: w: num min 0 max 100\n");
    println!("{:<18} {:<14} {}", "标签", "写入值", "校验结果");
    println!("{}", "-".repeat(72));

    let mut bypassed = 0;

    for (tag, val) in VALUES {
        let text = format!("{CONTRACT}item {{\n  @is C\n  w: {val}\n}}\n");
        let verdict = match parse(&text) {
            Ok(_) => {
                // 解析通过 = 校验放行
                "**通过（未被拦截）**".to_string()
            }
            Err(e) => {
                let msg = e.to_string();
                // 只留关键一句，避免刷屏
                let short = msg.lines().next().unwrap_or(&msg).to_string();
                format!("拦截: {}", truncate(&short, 38))
            }
        };
        if verdict.contains("通过") && (tag.contains("nan") || tag.contains("NaN")
            || tag.contains("inf") || tag.contains("溢出")) {
            bypassed += 1;
        }
        println!("{tag:<18} {val:<14} {verdict}");
    }

    println!("{}", "-".repeat(72));
    println!("非有限值绕过区间校验: {bypassed} 例");
}

fn truncate(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        return s.to_string();
    }
    chars[..n].iter().collect::<String>() + "…"
}
