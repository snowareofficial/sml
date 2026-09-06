# sml-value

SML 值模型：`Value` 类型、SML 文本序列化与 serde 桥接。零依赖。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`sml-value` 是整个 SML 生态的地基 crate，只负责三件事：

- `Value`：7 种值类型的枚举（`Null` / `Bool` / `Int` / `Float` / `Str` / `Array` / `Object`）；
- `to_sml`（feature `sml`）：把 `Value` 渲染回 SML 文本；
- serde 桥接（feature `serde`）：`Value` 的 `Serialize` / `Deserialize` 实现。

把它独立出来，是为了让只需要值模型或解析能力的场景不必拖入过程宏、C-ABI 与转译后端。

## 安装

```toml
[dependencies]
sml-value = "0.1.0-alpha.1"

# 需要序列化回 SML 文本时启用 sml feature
# sml-value = { version = "0.1.0-alpha.1", features = ["sml"] }
```

## 快速开始

```rust
use sml_value::{Value, to_sml};

let mut m = std::collections::BTreeMap::new();
m.insert("name".into(), Value::Str("John".into()));
m.insert("age".into(), Value::Int(27));

let v = Value::Object(m);
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
println!("{}", to_sml(&v));
```

## 许可

MulanPSL-2.0

---

# English

`sml-value` is the foundation crate of the SML ecosystem. It does only three things:

- `Value`: the 7-variant value enum (`Null` / `Bool` / `Int` / `Float` / `Str` / `Array` / `Object`);
- `to_sml` (feature `sml`): render `Value` back to SML text;
- serde bridge (feature `serde`): `Serialize` / `Deserialize` for `Value`.

It is split out so that consumers who only need the value model or parsing do not pull in proc-macros, C-ABI, or translation backends.

## Installation

```toml
[dependencies]
sml-value = "0.1.0-alpha.1"

# Enable the sml feature to serialize back to SML text
# sml-value = { version = "0.1.0-alpha.1", features = ["sml"] }
```

## Quick start

```rust
use sml_value::{Value, to_sml};

let mut m = std::collections::BTreeMap::new();
m.insert("name".into(), Value::Str("John".into()));
m.insert("age".into(), Value::Int(27));

let v = Value::Object(m);
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
println!("{}", to_sml(&v));
```

## License

MulanPSL-2.0
