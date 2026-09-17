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
/// 解析期条件/循环原语（`@when` / `@for`）。
#[cfg(feature = "when")]
mod cond;
/// 外置扩展点：下游注册自定义 `@指令`，无需修改本 crate 源码。
pub mod ext;

pub use api::{
    loads, parse, parse_allowed, parse_file, parse_file_features, parse_file_versioned,
    parse_versioned, parse_with, parse_with_features, parse_with_features_env,
};
pub use ext::{ParseOptions, ParseOutput};
pub use error::ParseError;

// 解析器游标需对外可见：`sml_parse::Parser` 被 cond 与测试复用。
pub use parser::Parser;
pub use scan::strip_version;

// 转发依赖 crate 的公共类型，使下游只需 `sml_parse::`
pub use sml_feature::{Feature, FeatureSet, Version};
pub use sml_include::{IncludeTarget, compile_regex, regex_matches};
pub use sml_lex::{Tok, coerce_word, tokenize};
pub use sml_value::Value;
