// SPDX-License-Identifier: MulanPSL-2.0
//! 构建期双语 warning（D46 / D50）：只提示，不改变编译结果。
//! Build-time bilingual warnings (D46 / D50); they never change the build result.

fn main() {
    if std::env::var_os("CARGO_FEATURE_SOURCE").is_some() {
        println!(
            "cargo:warning=amberice `source` feature enabled: `SOURCE` (0x50) stores the original \
text verbatim and is written only through explicit APIs. Treat the file as sensitive. \
｜ 已启用 `source` feature：`SOURCE`（0x50）会原样保存原文，且只经显式 API 写入；请把产物视为敏感。"
        );
    }

    // D50：wasm32 + zstd。`zstd-sys` 的公告只落 wasm32（allocator shim 内存破坏），
    // 且运行时压缩向量从未在 wasm 宿主里跑过 —— 编过 ≠ 跑对。
    let wasm = std::env::var("CARGO_CFG_TARGET_ARCH")
        .map(|arch| arch == "wasm32")
        .unwrap_or(false);
    if wasm && std::env::var_os("CARGO_FEATURE_COMPRESS").is_some() {
        println!(
            "cargo:warning=amberice: `wasm32` + `compress` enabled. `zstd-sys` carries a \
wasm32-only advisory (allocator-shim memory corruption), and the runtime compression vectors \
were never executed in a wasm host — building is not proof of correctness. Ship wasm \
uncompressed by default. \
｜ 已启用 `wasm32` + `compress`：`zstd-sys` 有仅落 wasm32 的内存破坏公告，且压缩向量从未在 \
wasm 宿主里跑过 —— 编译通过不等于运行正确；wasm 版请默认按不压缩发布。"
        );
    }
}
