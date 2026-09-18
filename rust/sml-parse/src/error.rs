// SPDX-License-Identifier: MulanPSL-2.0
//! 解析错误类型。
//!
//! 历史上这里是 `ParseError(pub String)` —— **只有文案、没有码**，于是
//! 「同一个错误在不同实现里是三句话」这件事无法被机器判定（见 `errors/README.md`）。
//!
//! 现在它只是 [`sml_codes::SmlError`] 的一个别名：`sml-parse` 的每个错误都带
//! **错误码**，码本身由 `errors/gen_codes.py` 从唯一事实来源 `errors/codes.sml`
//! 生成。`Display` 会把码缀在文案之后（`文案 [E-PARSE-008]`）。
//!
//! 保留 `ParseError` 这个名字是为了别让调用方跟着改：
//! 它同时表达「语法的错」与「带码的错」两件事，而后者才是新的重点。

pub use sml_codes::SmlError as ParseError;
