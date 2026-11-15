// SPDX-License-Identifier: MulanPSL-2.0
//! 一次性生成器：把 Rust `char::is_numeric` / `is_alphabetic` / `is_whitespace`
//! 的码位区间导出成 C / Lua 表，供 C / C++ / Lua 三端的模式匹配器使用。
//!
//! 为什么要生成而不是手写：`Class::Digit` 的语义是 **Unicode**（不止 `0-9`，
//! 全角与罗马数字也算）。三端若各自手写区间，边界必然漂移；由 Rust 导出同一份
//! 数据，五端的字符类判定就是同一张表查出来的。
//!
//! 用法：`cargo run -q -p sml-pattern --example gen_unicode_tables -- c`
//!       `cargo run -q -p sml-pattern --example gen_unicode_tables -- lua`

fn segs(pred: fn(char) -> bool) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    let mut cur: Option<(u32, u32)> = None;
    for cp in 0u32..0x11_0000 {
        let ch = match char::from_u32(cp) {
            Some(c) => c,
            None => continue,
        };
        if pred(ch) {
            match cur {
                Some((lo, hi)) if hi + 1 == cp => cur = Some((lo, cp)),
                Some((lo, hi)) => {
                    out.push((lo, hi));
                    cur = Some((cp, cp));
                }
                None => cur = Some((cp, cp)),
            }
        }
    }
    if let Some(s) = cur {
        out.push(s);
    }
    out
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "c".to_string());
    let numeric = segs(|c| c.is_numeric());
    let alpha = segs(|c| c.is_alphabetic());
    let space = segs(|c| c.is_whitespace());

    if which == "lua" {
        println!("-- 由 rust/sml-pattern/examples/gen_unicode_tables.rs 生成，请勿手改。");
        println!("-- 语义与 Rust 的 char::is_numeric / is_alphabetic / is_whitespace 同源。");
        println!("-- 每张表是**扁平**的 lo,hi 对：{{ lo1, hi1, lo2, hi2, ... }}。");
        println!("return {{");
        for (name, t) in [("numeric", &numeric), ("alphabetic", &alpha), ("whitespace", &space)] {
            println!("  {} = {{", name);
            let mut line = String::from("    ");
            for (i, (lo, hi)) in t.iter().enumerate() {
                line.push_str(&format!("0x{:X}, 0x{:X}, ", lo, hi));
                if i % 4 == 3 {
                    line.push_str("\n    ");
                }
            }
            println!("{}", line.trim_end());
            println!("  }},");
        }
        println!("}}");
        return;
    }

    println!("/* 由 rust/sml-pattern/examples/gen_unicode_tables.rs 生成，请勿手改。 */");
    println!("/* 语义与 Rust 的 char::is_numeric / is_alphabetic / is_whitespace 同源： */");
    println!("/* 三端（C / C++ / Lua）查同一份区间，字符类判定不可能各自漂移。 */");
    println!("#ifndef SML_PATTERN_UNICODE_H");
    println!("#define SML_PATTERN_UNICODE_H");
    println!();
    for (name, t) in [("numeric", &numeric), ("alphabetic", &alpha), ("whitespace", &space)] {
        let upper = name.to_uppercase();
        println!("/* {} —— {} 段 */", name, t.len());
        println!("static const unsigned int SML_PAT_{}[][2] = {{", upper);
        for chunk in t.chunks(4) {
            let mut line = String::from("    ");
            for (lo, hi) in chunk {
                line.push_str(&format!("{{0x{:X}, 0x{:X}}}, ", lo, hi));
            }
            println!("{}", line.trim_end());
        }
        println!("}};");
        println!(
            "#define SML_PAT_{}_N {}u",
            upper,
            t.len()
        );
        println!();
    }
    println!("/* 码位是否落在区间表内（二分；表按 lo 升序）。 */");
    println!("static int sml_pat_cp_in(unsigned int cp, const unsigned int (*t)[2], unsigned int n) {{");
    println!("    unsigned int lo = 0, hi = n;");
    println!("    while (lo < hi) {{");
    println!("        unsigned int mid = lo + (hi - lo) / 2;");
    println!("        if (cp < t[mid][0]) hi = mid;");
    println!("        else if (cp > t[mid][1]) lo = mid + 1;");
    println!("        else return 1;");
    println!("    }}");
    println!("    return 0;");
    println!("}}");
    println!();
    println!("#endif /* SML_PATTERN_UNICODE_H */");
}
