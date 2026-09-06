# sml-regex

SML 受限正则引擎：供 include 路径匹配使用，带步数上限以杜绝回溯爆炸。零依赖。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`sml-regex` 是一个刻意极简的正则实现，只服务于 SML 的 `include /re/` 文件名匹配。它不引入 `regex` crate，避免为大而全的功能付出体积代价，并通过 `MAX_REGEX_LEN` 与 `MAX_REGEX_STEPS` 两道硬限制防止 ReDoS。

## 安装

```toml
[dependencies]
sml-regex = "0.1.0-alpha.1"
```

## 快速开始

```rust
use sml_regex::{compile_regex, regex_matches};

let re = compile_regex(r"^widget_.*\.sml$");
assert!(regex_matches(&re, "widget_login.sml"));
assert!(!regex_matches(&re, "widget_login.txt"));
```

## 支持语法

- 字面字符、`.` 通配、`\\` 转义
- `*` `+` `?` 量词
- `[a-z]` / `[^a-z]` 字符类
- `^` / `$` 锚点

## 许可

MulanPSL-2.0

---

# English

`sml-regex` is a deliberately minimal regex engine, only serving SML's `include /re/` filename matching. It avoids pulling in the full `regex` crate and uses `MAX_REGEX_LEN` plus `MAX_REGEX_STEPS` hard limits to prevent ReDoS.

## Installation

```toml
[dependencies]
sml-regex = "0.1.0-alpha.1"
```

## Quick start

```rust
use sml_regex::{compile_regex, regex_matches};

let re = compile_regex(r"^widget_.*\.sml$");
assert!(regex_matches(&re, "widget_login.sml"));
assert!(!regex_matches(&re, "widget_login.txt"));
```

## Supported syntax

- Literal characters, `.` wildcard, `\\` escaping
- `*` `+` `?` quantifiers
- `[a-z]` / `[^a-z]` character classes
- `^` / `$` anchors

## License

MulanPSL-2.0
