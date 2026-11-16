// SPDX-License-Identifier: MulanPSL-2.0
//! A lightweight runtime for the pure-data subset of SML — JSON-like, no_std-ready.
//! 轻量 SML 纯数据子集运行时 —— 类 JSON、no_std 就绪。
//!
//! `smlite` is a new parser and value model: it does not reuse `sml-parse`, drops the
//! document-friendly features (featag / weak / `@type` / include), and targets machines
//! rather than humans. The core is `#![no_std] + alloc` with no hard dependencies, so it
//! can be published to crates.io on its own.
//! `smlite` 是全新解析器与值模型：不复用 `sml-parse`，去掉文档友好特性
//! （featag / weak / `@type` / include），面向机器而非人类。核心是 `#![no_std] + alloc`、
//! 零硬依赖，可独立发布到 crates.io。
//!
//! # Quick start / 快速开始
//!
//! ```
//! use smlite::{parse, to_string, merge_patch};
//!
//! let v = parse(b"name: John age: 27").unwrap();
//! assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
//! let merged = merge_patch(&v, &parse(b"age: null city: SZ").unwrap()).unwrap();
//! assert!(to_string(&merged).contains("city: SZ"));
//! ```
//!
//! # Features / 特性开关
//!
//! - `std` (default): `std::error::Error` impls. ｜ 默认档：`std::error::Error` 实现。
//! - `object-sort`: objects use `BTreeMap` (mutually exclusive with the default
//!   insertion-order `Vec`). ｜ 对象改用 `BTreeMap`（与默认插入序 `Vec` 互斥）。
//! - `swsml-compat`: value interop with `sml_value::Value`. ｜ 与 `sml_value::Value` 互转。
//! - `amberice`: the `ByteEnvelope` relay trait; no real dependency yet.
//!   ｜ `ByteEnvelope` 接力 trait；尚未接实依赖。

#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

mod emit;
pub mod error;
mod parse;
mod patch;
mod value;

#[cfg(feature = "swsml-compat")]
mod compat;
#[cfg(feature = "amberice")]
pub mod envelope;

pub use emit::{to_string, to_vec};
pub use error::{ConversionError, ErrorKind, ParseError, PatchError, SmliteError};
pub use parse::{parse, parse_str};
pub use patch::merge_patch;
pub use value::{Object, Value, MAX_VALUE_DEPTH};
