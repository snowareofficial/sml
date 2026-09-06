//! audit_num.rs — 数字字面量边界审计 · Rust 实现侧（主观测点）
//!
//! 运行: cargo run -p swsml --features serde --example audit_num
//!
//! 目的：绕开 Playground 的 JS 观测层，直接检验 Rust 值模型
//! （Value::Int(i64) / Value::Float(f64) / Value::Str）的实际行为，
//! 并输出 to_sml round-trip 与 JSON 视图作三方对照。

use sml::{parse, to_sml, Value};

const CASES: &str = r#"a: 01234
f: 007
negzero7: -007
b: 1.0
g: 1.10
j: 9007199254740993
k: 12345678901234567890
negbig: -99999999999999999999
c: 1E5
d: 0x1F
m: -0
mf: -0.0
n: .5
o: 5.
p: 1_000
q: 123456789012345.67
e1: 1e10
e2: 1.5e-3
server web { port: 8080 }
config { version: 1 }
"#;

fn main() {
    println!("================ [1] Rust parse: 值模型 ================");
    let v = match parse(CASES) {
        Ok(v) => v,
        Err(e) => {
            println!("PARSE ERROR: {e}");
            return;
        }
    };
    if let Value::Object(map) = &v {
        for (k, val) in map {
            // {:?} 直接展示 enum 变体：Int(7) / Str("007") / Float(1.0) …
            println!("{k:>9} : {:?}", val);
        }
    }

    println!("\n================ [2] to_sml round-trip ================");
    println!("{}", to_sml(&v));

    // round-trip 再解析：形态是否二义
    let dumped = to_sml(&v);
    println!("---- re-parse dumped text ----");
    match parse(&dumped) {
        Ok(v2) => {
            if let Value::Object(map) = &v2 {
                for (k, val) in map {
                    println!("{k:>9} : {:?}", val);
                }
            }
            println!("round-trip equal: {}", v == v2);
        }
        Err(e) => println!("RE-PARSE ERROR: {e}"),
    }

    #[cfg(feature = "serde")]
    {
        println!("\n================ [3] JSON 视图 (serde_json) ================");
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
}
