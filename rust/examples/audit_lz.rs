//! audit_lz.rs — 前导零保护的符号对称性探针
//!
//! 运行: cargo run -p swsml --example audit_lz
//!
//! sml-lex/src/lib.rs:323 的前导零保护写的是 `w.starts_with('0')`，
//! 即只认「以 0 开头」的形態。`-007` / `+007` 以符号开头，
//! 落不进该分支 → 走 `parse::<i64>()` 变成 -7 / 7，前导零被吃掉。
//! 本探针实测确认该不对称是否真实存在。

use sml::{parse, to_sml, Value};

const CASES: &str = r#"p007: 007
n007: -007
s007: +007
p0755: 0755
n0755: -0755
p0: 0
n0: -0
pf: 0.5
nf: -0.5
p00: 00
n00: -00
pbig: 0123456789012345678901234567890
nbig: -0123456789012345678901234567890
"#;

fn main() {
    let v = match parse(CASES) {
        Ok(v) => v,
        Err(e) => return println!("PARSE ERROR: {e}"),
    };
    // 键 -> 原字面量，用于判断「原文到底有没有前导零」
    let literal_of: std::collections::HashMap<&str, &str> = CASES
        .lines()
        .filter_map(|line| {
            let (k, lit) = line.split_once(": ")?;
            Some((k, lit))
        })
        .collect();

    if let Value::Object(map) = &v {
        for (k, val) in map {
            // 判据：剥掉符号后，长度 > 1 且首字符为 '0' —— 这才是真前导零。
            // 只看「解析成了数字」会把 0 / -0 / 0.5 / -0.5 误报成丢失。
            let lit = literal_of.get(k.as_str()).copied().unwrap_or("");
            let digits = lit.trim_start_matches(['+', '-']);
            let has_leading_zero =
                digits.starts_with('0') && digits.len() > 1 && !digits.contains('.');
            let lost = if has_leading_zero && !matches!(val, Value::Str(_)) {
                "  <== 前导零丢失"
            } else {
                ""
            };
            println!("{k:>6} : {:?}{lost}", val);
        }
    }
    println!("\n---- to_sml round-trip ----\n{}", to_sml(&v));
}
