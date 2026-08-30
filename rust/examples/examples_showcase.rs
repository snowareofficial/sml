// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0
//! 验证 examples/ 目录下示例文件能否被完整解析（全部语言特性）。
//! 运行：cargo run --example examples_showcase --features serde

use sml::{parse_file, to_sml};

fn check(path: &str) {
    println!("========================================");
    println!("FILE: {path}");
    println!("========================================");
    match parse_file(path) {
        Ok(v) => {
            println!("{}", to_sml(&v));
            #[cfg(feature = "serde")]
            println!("--- json ---\n{}", serde_json::to_string_pretty(&v).unwrap());
        }
        Err(e) => {
            eprintln!("PARSE ERROR: {e}");
            std::process::exit(1);
        }
    }
}

fn main() {
    let base = concat!(env!("CARGO_MANIFEST_DIR"), "/../examples");
    check(&format!("{base}/full.sml"));
    check(&format!("{base}/app.sml"));
    check(&format!("{base}/secrets.sml"));
    println!("\nALL EXAMPLES PARSED OK");
}
