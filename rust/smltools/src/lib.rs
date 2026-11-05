// SPDX-License-Identifier: MulanPSL-2.0
//! Library surface of `smltools` — the same conversions the CLI offers, callable from code.
//! `smltools` 的库接口 —— 与 CLI 同一套转换，代码里可直接调用。
//!
//! 为什么要有它（用户 2026-11-05）：`.smlb` 重新定位为**通用二进制信封**后，最常见的一步
//! 是「任意字节 ⇄ 信封」与「SML 文本 ⇄ 信封」。这两件事只做成命令行，会把「嵌进别人程序」
//! 这条最自然的路堵死。故 CLI 与库共用同一份实现：CLI 只是它的一个调用者。
//! ｜ The CLI is just one caller of this module — not the other way round.
//!
//! # 命名（全仓库 API 三规则，见 `docs/api-progressive-draft.md` 附录）
//!
//! 动作沿用既有词根，变体只加一个尾缀词：
//!
//! | 做什么 | 函数 |
//! |---|---|
//! | 任意字节装进信封 | [`wrap`] |
//! | 从信封取出那个载荷 | [`unwrap`] |
//! | 列出信封里的载荷 | [`list`] |
//! | SML 文本 → 信封（`VALUE` 段） | [`sml_to_smlb`] |
//! | 信封 → SML 文本 | [`smlb_to_sml`] |

pub mod envelope;

pub use envelope::{list, sml_to_smlb, smlb_to_sml, unwrap, wrap, EnvelopeError, Payload};
