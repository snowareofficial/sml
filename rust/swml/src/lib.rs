// SPDX-License-Identifier: MulanPSL-2.0
//! # swml — the SML facade: everything at the root, four concepts in [`easy`].
//! # swml —— SML 门面：根上是完整能力，[`easy`] 里是「四个概念」的入门路径。
//!
//! Two audiences, one crate ｜ 两种人，一个 crate：
//!
//! - **Progressive path** — [`easy`]: document → field → value → error. Start here if you
//!   just want to read a config file. ｜ 入门路径：只想「读一份配置」就从这里开始。
//! - **Expert path** — the crate root re-exports **all** of `swsml`: `parse_*`, `Value`,
//!   `emit::*`, the C-ABI helpers and the contract internals. ｜ 专家路径：根上原样再导出
//!   `swsml` 的全部名字，需要控制每个旋钮时从这里取。
//!
//! ## Layering ｜ 分层
//!
//! `swml` implements nothing: it moves `swsml`'s public surface to this crate's root and
//! collects the beginner layer into [`easy`]. No new parser, no new value model, no wrapper
//! types — `swml::Value` **is** `sml::Value`, so values pass between both paths untouched.
//! ｜ `swml` 不实现任何东西：把 `swsml` 的公开面搬到根上，再把入门层收进 [`easy`]。
//! 没有第二套解析器、没有第二套值模型、没有包装类型 —— `swml::Value` 就是 `sml::Value`，
//! 两条路径之间传值不需要转换。
//!
//! ```no_run
//! // Progressive ｜ 入门
//! use swml::easy::Doc;
//! let doc = Doc::read("server { host: localhost port: 8080 }")?;
//! assert_eq!(doc.str("server.host"), Some("localhost"));
//!
//! // Expert ｜ 专家（同一个 crate 的根）
//! let value: swml::Value = swml::parse("a: 1")?;
//! assert!(value.get("a").is_some());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Feature flags ｜ 特性开关
//!
//! `emit-*` 只门控 [`easy::out`] 里的同名函数；`preserve_order` 转发给 `swsml`（进而是
//! 七个值层 crate）。**其余旋钮全部属于 `swsml`** —— 门面不复制一套 feature 名字，
//! 也不假装能关掉根上的 `emit` 模块：那是底层 crate 的职责。
//! ｜ The `emit-*` flags only gate the matching functions in [`easy::out`];
//! `preserve_order` is forwarded to `swsml`. Every other knob belongs to `swsml`.

/// The full SML surface, re-exported unchanged ｜ 原样再导出 SML 的完整公开面.
///
/// This is `swsml`'s root, not a curated subset ｜ 这是 `swsml` 的根，不是挑选过的子集.
pub use sml::*;

/// Progressive layer: four concepts only ｜ 入门层：只有四个概念.
///
/// See the [module docs](easy) for the API and for what this layer deliberately omits.
/// ｜ API 与「这一层故意不做什么」见[模块文档](easy)。
pub mod easy;
