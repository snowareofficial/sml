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
pub use sml_regex::{MiniRegex, compile_regex, regex_matches};
