//! SML syntax analysis: a recursive-descent parser plus the public parse API (`parse` / `parse_file` / `loads` and their `_with_*` variants). ｜ SML 语法分析：递归下降解析器与公开 parse API（`parse` / `parse_file` / `loads` 及各自的 `_with_*` 变体）。
//!
//! Errors are coded (`sml_codes::SmlError`, aliased as `ParseError`). `ext` lets downstream crates register custom `@directives` via `ParseOptions` / `parse_with`, and `contract_map` records which contracts were applied to which entity. ｜ 错误带码（`sml_codes::SmlError`，别名 `ParseError`）。`ext` 让下游用 `ParseOptions` / `parse_with` 注册自定义 `@指令`，`contract_map` 记录「哪个实体应用了哪些契约」。
//!
//! Public types from the lower layers are re-exported so downstream only needs `sml_parse::`: `Value`, `Tok` / `tokenize`, `Edition` / `Version` (alias) / `Feature` / `FeatureSet`, `IncludeTarget`. The `when` feature enables the parse-time `@when` / `@for` primitives. ｜ 下层公共类型在此转发，下游只需 `sml_parse::`：`Value`、`Tok` / `tokenize`、`Edition` / `Version`（别名）/ `Feature` / `FeatureSet`、`IncludeTarget`。`when` 特性启用解析期 `@when` / `@for` 原语。

/// 语法分析器（内部实现）。
mod parser;
/// `@version` / `@feature` 指令扫描与剥离。
mod scan;
/// 公开解析 API。
mod api;
/// 错误类型。
mod error;
/// 契约桥接：把 sml-contract 的校验接到解析流程上。
mod contract_bridge;
/// Contract map: records which contracts were applied to which entity at parse time (opt-in, see `ParseOptions::trace_contracts`). ｜ 契约映射：解析期记录「哪个实体应用了哪些契约」（opt-in，见 `ParseOptions::trace_contracts`）。
pub mod contract_map;
/// 解析期条件/循环原语（`@when` / `@for`）。
#[cfg(feature = "when")]
mod cond;
/// Out-of-tree extension point: downstream registers custom `@directives` without touching this crate's source. ｜ 外置扩展点：下游注册自定义 `@指令`，无需修改本 crate 源码。
pub mod ext;

pub use api::{
    loads, parse, parse_allowed, parse_file, parse_file_features, parse_file_ordered,
    parse_file_versioned, parse_ordered,
    parse_file_with, parse_file_with_warnings, parse_versioned, parse_with, parse_with_features,
    parse_with_features_env, parse_with_warnings,
};
pub use contract_map::{AppliedContract, ContractMap, PathSeg};
pub use ext::{Diagnostic, DiagnosticKind, ParseOptions, ParseOutput, WeakBind, WeakData, WeakDoc};
pub use error::ParseError;

// 解析器游标需对外可见：`sml_parse::Parser` 被 cond 与测试复用。
pub use parser::Parser;
pub use scan::strip_version;

// 转发依赖 crate 的公共类型，使下游只需 `sml_parse::`
// `Version` 是 `Edition` 的别名（2026-11-04 改名，`docs/version-naming.md` V1）
pub use sml_feature::{Edition, Feature, FeatureSet, Version};
pub use sml_include::{IncludeTarget, compile_regex, regex_matches};
pub use sml_lex::{Tok, coerce_word, tokenize};
pub use sml_value::Value;
