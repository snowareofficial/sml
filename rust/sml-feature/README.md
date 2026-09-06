# sml-feature

SML 版本与特性系统（纯数据层）：`Version`、`Feature` / `FeatureSet` 及名字注册表。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`sml-feature` 只定义 SML 的版本与能力位：

- `Version`：v1 / v2 / v3 / v4；
- `Feature` / `FeatureSet`：特性枚举与位掩码集合；
- `FEATURES` / `feature_names`：名字 ↔ 位的注册表。

它刻意不包含 `@version` / `@feature` 的扫描：那部分依赖 `Tok`，在 `sml-lex` / `sml-parse` 中完成，以避免循环依赖。

## 安装

```toml
[dependencies]
sml-feature = "0.1.0-alpha.1"
```

## 快速开始

```rust
use sml_feature::{Version, Feature, FeatureSet};

let v = Version::V4;
assert!(v.strict_strings());

let mut fs = FeatureSet::for_version(v);
fs = fs.with(Feature::Contract);
assert!(fs.has(Feature::Contract));
println!("enabled: {}", fs);
```

## 许可

MulanPSL-2.0

---

# English

`sml-feature` only defines SML versions and capability bits:

- `Version`: v1 / v2 / v3 / v4;
- `Feature` / `FeatureSet`: feature enum and bit-mask set;
- `FEATURES` / `feature_names`: name ↔ bit registry.

It deliberately does **not** include `@version` / `@feature` scanning, which lives in `sml-lex` / `sml-parse` to avoid circular dependencies.

## Installation

```toml
[dependencies]
sml-feature = "0.1.0-alpha.1"
```

## Quick start

```rust
use sml_feature::{Version, Feature, FeatureSet};

let v = Version::V4;
assert!(v.strict_strings());

let mut fs = FeatureSet::for_version(v);
fs = fs.with(Feature::Contract);
assert!(fs.has(Feature::Contract));
println!("enabled: {}", fs);
```

## License

MulanPSL-2.0
