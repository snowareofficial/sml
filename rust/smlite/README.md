# smlite

轻量 SML 纯数据子集运行时：类 JSON、机器导向、`no_std` 就绪，对标 `serde_json`。全新解析器，不复用 `sml-parse`。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`smlite` 取 SML 的纯数据子集（无 featag / weak / 文档友好特性），提供值模型、解析器、JSON-compatible 渲染与 RFC 7396 Merge Patch。核心 `#![no_std] + alloc`，零硬依赖，可独立发布到 crates.io。

## 安装

```toml
[dependencies]
smlite = "0.6.9-dev.0"

# 需要值层互转时启用 swsml-compat
# smlite = { version = "0.6.9-dev.0", features = ["swsml-compat"] }
# 嵌入式（去掉 std::error::Error impl）
# smlite = { version = "0.6.9-dev.0", default-features = false }
```

## 快速开始

```rust
use smlite::{parse, to_string, Value};

let v = parse(b"name: John age: 27 tags: [a b c]").unwrap();
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
println!("{}", to_string(&v));

let base = parse(b"a: 1 b: 2").unwrap();
let patch = parse(b"a: null c: 3").unwrap();
let merged = smlite::merge_patch(&base, &patch).unwrap();
assert_eq!(merged.get("a"), None);
assert_eq!(merged.get("c").and_then(|x| x.as_int()), Some(3));
```

## 许可

MulanPSL-2.0

---

# English

`smlite` is a lightweight runtime for the pure-data subset of SML — no featag / weak / document-friendly features. It provides a value model, a parser, JSON-compatible rendering, and RFC 7396 Merge Patch. The core is `#![no_std] + alloc` with no hard dependencies, publishable to crates.io on its own.

## Installation

```toml
[dependencies]
smlite = "0.6.9-dev.0"

# Enable swsml-compat for value-layer interop
# smlite = { version = "0.6.9-dev.0", features = ["swsml-compat"] }
# Embedded (drop std::error::Error impls)
# smlite = { version = "0.6.9-dev.0", default-features = false }
```

## Quick start

```rust
use smlite::{parse, to_string, Value};

let v = parse(b"name: John age: 27 tags: [a b c]").unwrap();
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
println!("{}", to_string(&v));

let base = parse(b"a: 1 b: 2").unwrap();
let patch = parse(b"a: null c: 3").unwrap();
let merged = smlite::merge_patch(&base, &patch).unwrap();
assert_eq!(merged.get("a"), None);
assert_eq!(merged.get("c").and_then(|x| x.as_int()), Some(3));
```

## License

MulanPSL-2.0
