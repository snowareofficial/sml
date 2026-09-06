# sml-parse

SML 语法分析：递归下降解析器与公开 parse API（`parse` / `parse_file` / `loads`）。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`sml-parse` 把词法 token 流解析为 `sml_value::Value`，并提供 `parse`、`parse_file`、`parse_versioned` 等公开 API。它会按需调用 `sml-include` 展开 include、调用 `sml-contract` 应用契约。

## 安装

```toml
[dependencies]
sml-parse = "0.1.0-alpha.1"
```

## 快速开始

```rust
use sml_parse::parse;

let v = parse("name: John\nage: 27")?;
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
```

## 许可

MulanPSL-2.0

---

# English

`sml-parse` parses the token stream into `sml_value::Value` and provides public APIs such as `parse`, `parse_file`, and `parse_versioned`. It invokes `sml-include` to expand includes and `sml-contract` to apply contracts as needed.

## Installation

```toml
[dependencies]
sml-parse = "0.1.0-alpha.1"
```

## Quick start

```rust
use sml_parse::parse;

let v = parse("name: John\nage: 27")?;
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
```

## License

MulanPSL-2.0
