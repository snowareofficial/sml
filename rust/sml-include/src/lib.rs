/// 指令解析：`include` / `import` 行的识别与路径解析。
pub mod parse;

/// 跨文件展开：把 include 指令替换为被包含文件的内容。
pub mod expand;

pub use expand::{expand_includes, resolve_includes, expand_file_tokens};
pub use parse::{
    IncludeTarget, parse_include_line, resolve_target_paths, strip_line_comment,
    MAX_INCLUDE_DEPTH, MAX_INCLUDE_EXPANSIONS,
};

// glob / regex 匹配转发自 sml-regex，保持 `sml_include::` 单一入口
// （`*_checked` 是 W16 的显式失败入口：过长 / 非法 / 超预算都报原因，
//   由 `parse.rs` 的 `map_regex_err` 映射成 E-LIMIT-007 / E-PARSE-025 / E-LIMIT-002）
pub use sml_regex::{
    compile_regex, compile_regex_checked, regex_matches, regex_matches_checked, MiniRegex,
    RegexError, MAX_REGEX_LEN, MAX_REGEX_STEPS,
};
