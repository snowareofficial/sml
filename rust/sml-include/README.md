# sml-include

SML 模块化：`include` / `import` 指令解析、glob 与正则匹配、跨文件展开。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`sml-include` 负责 SML 的跨文件复用：

- 解析 `include "x.sml"` / `import x` 等指令；
- 支持 `as` 命名空间、挑键 `{a,b}`、glob 通配、正则匹配；
- 把 include 递归展开为 token 流，带深度与展开次数上限。

## 安装

```toml
[dependencies]
sml-include = "0.1.0-alpha.1"
```

## 快速开始

```rust
use std::path::Path;
use sml_feature::FeatureSet;
use sml_include::resolve_includes;

let features = FeatureSet::baseline();
let toks = resolve_includes(
    "app: demo\ninclude \"part.sml\"",
    Path::new("app.sml"),
    features,
)?;
```

## 许可

MulanPSL-2.0

---

# English

`sml-include` handles SML cross-file reuse:

- Parse directives such as `include "x.sml"` / `import x`;
- Support `as` namespace, key selection `{a,b}`, glob wildcards, and regex matching;
- Recursively expand includes into a token stream with depth and expansion-count limits.

## Installation

```toml
[dependencies]
sml-include = "0.1.0-alpha.1"
```

## Quick start

```rust
use std::path::Path;
use sml_feature::FeatureSet;
use sml_include::resolve_includes;

let features = FeatureSet::baseline();
let toks = resolve_includes(
    "app: demo\ninclude \"part.sml\"",
    Path::new("app.sml"),
    features,
)?;
```

## License

MulanPSL-2.0
