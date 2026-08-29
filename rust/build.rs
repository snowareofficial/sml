// 跨 edition 兼容：根据 Cargo.toml 的 [package] edition 字段，
// 在 edition = "2024" 时输出 cfg(edge2024)。
// rustc 1.85+ 起，edition 2024 要求 `#[unsafe(no_mangle)]`，
// 而 edition 2021 无法解析该写法；故在 lib.rs 用
// `#[cfg_attr(edge2024, unsafe(no_mangle))]`
// `#[cfg_attr(not(edge2024), no_mangle)]`
// 让同一份源码在两种 edition 下均可编译。
use std::path::Path;

fn main() {
    let manifest = Path::new("Cargo.toml");
    let text = std::fs::read_to_string(manifest).unwrap_or_default();
    let edition = text
        .lines()
        .find_map(|line| {
            let line = line.trim();
            line.strip_prefix("edition").map(|rest| {
                rest.trim()
                    .trim_start_matches('=')
                    .trim()
                    .trim_matches('"')
                    .to_string()
            })
        })
        .unwrap_or_else(|| "2021".to_string());
    if edition == "2024" {
        println!("cargo:rustc-cfg=edge2024");
    }
    println!("cargo:rerun-if-changed=Cargo.toml");
}
