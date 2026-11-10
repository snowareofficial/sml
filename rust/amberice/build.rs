// SPDX-License-Identifier: MulanPSL-2.0
//! D46：启用 `source` feature 时打双语 warning —— 原文入文件即敏感面。
//! Bilingual warning when `source` is enabled: original text in a file is sensitive by default.

fn main() {
    if std::env::var_os("CARGO_FEATURE_SOURCE").is_some() {
        println!(
            "cargo:warning=amberice `source` feature enabled: `SOURCE` (0x50) stores the original \
text verbatim and is written only through explicit APIs. Treat the file as sensitive. \
｜ 已启用 `source` feature：`SOURCE`（0x50）会原样保存原文，且只经显式 API 写入；请把产物视为敏感。"
        );
    }
}
