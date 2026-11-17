// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! Eclog 的 Rust 实现：JSON-like 文本格式的解析与渲染 —— 独立 crate，零硬依赖。
//! A Rust implementation of the Eclog text format — standalone, no hard dependencies.
//!
//! 核心是 `#![no_std]` + `alloc`。feature 只有两个：`std`（额外实现 `std::error::Error`）
//! 与 `amberice`（`.ambr` 对接契约，零依赖）。
//! The core is `#![no_std]` + `alloc`. Two features: `std` (adds `std::error::Error`)
//! and `amberice` (the `.ambr` contract, zero dependencies).
//!
//! # Quick start / 快速开始
//!
//! ```
//! use eclog::parse;
//!
//! let doc = parse(b"name: John\nage: 27\ntags: [ a, b, c ]").unwrap();
//! assert_eq!(doc.get("name").and_then(|v| v.as_text()), Some("John"));
//! assert_eq!(doc.get("age").and_then(|v| v.as_number()).map(|n| n.raw()), Some("27"));
//! assert!(doc.get("tags").and_then(|v| v.as_array()).is_some());
//! ```
//!
//! 事件流（不建树）：handler 的 `on_object_begin` / `on_key` / `on_value` / `on_object_end`
//! 与上游 `.check` 期望文件的顺序一一对应。
//! Event streaming without a tree: the handler callbacks map one-to-one onto the
//! upstream `.check` expectation files.
//!
//! # 值模型的取舍 ｜ Value model choices
//!
//! - [`Number`] 保留**原始字面量**与位标志，`+0` / `-0` / `1e100` / `+inf` / `-nan`
//!   在解析侧逐字节可辨（`.check` 对拍靠它）；渲染侧按 C++ 语义**重新格式化数值**，
//!   因此 `+0` 输出 `0`、`-0` 输出 `0`、`-0.0` 输出 `-0`。
//!   ｜ keeps the raw lexeme and flags for parsing; rendering reformats numerically
//!   (C++ semantics), so `+0` → `0`, `-0` → `0`, `-0.0` → `-0`.
//! - [`Str`] 保留来源记法与定界符，渲染阶段据此还原 heredoc 的 TAG 与 raw 的定界符。
//!   ｜ keeps the source notation and delimiter.
//!
//! # 渲染 ｜ Rendering
//!
//! 两种 scheme 与 C++ 一致：`typical`（根无 `{}`、裸词、无逗号）与 `json_compatible`
//! （根有 `{}`、全 quoted、逗号、`inf`/`nan` 写成字符串）。对拍判据是与 C++ oracle
//! 逐字节相同。
//! Both schemes match C++: `typical` and `json_compatible`; the criterion is byte
//! equality with the C++ oracle.
//!
//! ```
//! use eclog::{parse, to_json_compatible, to_typical};
//!
//! let doc = parse(b"name: John\ntags: [a, b]").unwrap();
//! assert_eq!(to_typical(&doc), "name: John\ntags:\n[\n\ta\n\tb\n]\n");
//! assert_eq!(to_json_compatible(&doc), "{\n\t\"name\": \"John\",\n\t\"tags\":\n\t[\n\t\t\"a\",\n\t\t\"b\"\n\t]\n}\n");
//! ```
//!
//! # 边界 ｜ Boundaries
//!
//! - 根必须是对象；空输入合法（得到空对象）。
//! - 只有 `#` 行注释；无 `//` 与 `/* */`。
//! - 本 crate 不依赖 `swsml` / `smlite`，也不复用它们的值模型。
//!   ｜ no dependency on `swsml` / `smlite`, and no reuse of their value models.
//! - `.ambr` 对接只有**契约**（feature `amberice` 的 [`amberice::AmbericeBridge`]），
//!   不引依赖、不自造二进制格式；实依赖等 P16 / P17 落地。
//!   ｜ `.ambr` support is a contract only (feature `amberice`), no dependency and no
//!   private binary format.

#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod error;
mod merge;
mod parse;
mod render;
mod value;

#[cfg(feature = "amberice")]
pub mod amberice;

pub use error::{Error, ErrorKind, ParseErrorKind};
pub use merge::merge_patch;
pub use parse::{parse, parse_str, parse_str_with, parse_with, ParseHandler};
pub use render::{
    render, to_json_compatible, to_typical, Formatting, IndentCharacter, Linebreak, Renderer,
    RendererConfig, Scheme,
};
pub use value::{Number, NumberInfo, Object, Str, StringNotation, Value};
