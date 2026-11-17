// SPDX-License-Identifier: MulanPSL-2.0
//! 把 `memory.x` 放进 OUT_DIR，并告诉链接器去那里找（`cortex-m-rt` 的 `link.x` 会 include 它）。
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    // cortex-m-rt 的链接脚本：负责向量表 / .bss 清零 / 入口。少了它，
    // 链接器会报 `cannot find entry symbol _start` 并丢掉整个 `Reset`。
    println!("cargo:rustc-link-arg=-Tlink.x");
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
