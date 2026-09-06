# sml-contract

SML 契约系统：`@contract` 定义与 `@is` 应用、类型校验、默认值填充、min/max 与枚举约束。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`sml-contract` 为 SML 提供可选的 schema 层。它在纯数据格式之上增加结构约束：字段类型、必填/可选、默认值、枚举、数值区间、数组元素类型，以及自定义模式类型。

契约之间通过组合（而非继承）复用：字段类型可以是另一个契约名。

## 安装

```toml
[dependencies]
sml-contract = "0.1.0-alpha.1"
```

## 快速开始

```rust
use std::collections::BTreeMap;
use sml_contract::{Contract, FieldSpec, TypeSpec, apply_contract};
use sml_value::Value;

let mut fields = BTreeMap::new();
fields.insert(
    "port".into(),
    FieldSpec {
        ty: TypeSpec::Int,
        required: true,
        default: Some(Value::Int(8080)),
        min: None,
        max: None,
    },
);

let contract = Contract {
    name: "Server".into(),
    fields,
    allow_extra: false,
};

let mut node = BTreeMap::from([
    ("host".into(), Value::Str("db1.internal".into())),
]);

apply_contract(&contract, &mut node, &BTreeMap::new(), "")?;
assert_eq!(node.get("port"), Some(&Value::Int(8080)));
```

> 实际使用中，`@contract` 与 `@is` 由 `sml-parse` 解析并自动应用。

## 许可

MulanPSL-2.0

---

# English

`sml-contract` provides an optional schema layer for SML. On top of the pure data format it adds structural constraints: field types, required/optional, defaults, enums, numeric ranges, array element types, and custom pattern types.

Contracts reuse each other through composition, not inheritance: a field's type can simply be another contract name.

## Installation

```toml
[dependencies]
sml-contract = "0.1.0-alpha.1"
```

## Quick start

```rust
use std::collections::BTreeMap;
use sml_contract::{Contract, FieldSpec, TypeSpec, apply_contract};
use sml_value::Value;

let mut fields = BTreeMap::new();
fields.insert(
    "port".into(),
    FieldSpec {
        ty: TypeSpec::Int,
        required: true,
        default: Some(Value::Int(8080)),
        min: None,
        max: None,
    },
);

let contract = Contract {
    name: "Server".into(),
    fields,
    allow_extra: false,
};

let mut node = BTreeMap::from([
    ("host".into(), Value::Str("db1.internal".into())),
]);

apply_contract(&contract, &mut node, &BTreeMap::new(), "")?;
assert_eq!(node.get("port"), Some(&Value::Int(8080)));
```

> In practice, `@contract` and `@is` are parsed and applied automatically by `sml-parse`.

## License

MulanPSL-2.0
