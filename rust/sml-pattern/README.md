# sml-pattern

SML 模式语言：L1 无回溯匹配引擎，免疫 ReDoS。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`sml-pattern` 把规则写成普通 SML 数据，用 Thompson NFA 并行推进匹配，**结构上不存在回溯**，因此免疫 ReDoS。它默认支持中英双语关键字，也可通过 `KeywordTable` trait 扩展为任意语言。

典型用途：为 SML 契约系统提供 `@type` 自定义类型校验（手机号、身份证号、日期格式等）。

## 安装

```toml
[dependencies]
sml-pattern = "0.1.0-alpha.1"
```

## 快速开始

```rust
use std::collections::BTreeMap;
use sml_value::Value;
use sml_pattern::{compile_rule, is_match};

let rule = Value::Array(vec![
    Value::Object(BTreeMap::from([
        ("class".into(), Value::Str("digit".into())),
        ("times".into(), Value::Int(4)),
    ])),
    Value::Object(BTreeMap::from([
        ("lit".into(), Value::Str("-".into())),
    ])),
    Value::Object(BTreeMap::from([
        ("class".into(), Value::Str("digit".into())),
        ("times".into(), Value::Int(2)),
    ])),
]);

let mut rules = BTreeMap::new();
rules.insert("date".into(), rule);

let pat = compile_rule(&rules["date"], &rules).unwrap();
assert!(is_match(&pat, "2024-09-06").unwrap());
assert!(!is_match(&pat, "2024-9-6").unwrap());
```

## 关键字

中英双语等价，可混用。例如 `class` / `类`、`digit` / `数字`、`times` / `次`、`lit` / `字面`、`name` / `名` 等。

## 许可

MulanPSL-2.0

---

# English

`sml-pattern` expresses rules as ordinary SML data and matches them with a Thompson NFA, so it is **structurally backtracking-free** and immune to ReDoS. It supports bilingual Chinese/English keywords by default and can be extended to any language via the `KeywordTable` trait.

Typical use: providing `@type` custom type validation for the SML contract system (phone numbers, IDs, date formats, etc.).

## Installation

```toml
[dependencies]
sml-pattern = "0.1.0-alpha.1"
```

## Quick start

```rust
use std::collections::BTreeMap;
use sml_value::Value;
use sml_pattern::{compile_rule, is_match};

let rule = Value::Array(vec![
    Value::Object(BTreeMap::from([
        ("class".into(), Value::Str("digit".into())),
        ("times".into(), Value::Int(4)),
    ])),
    Value::Object(BTreeMap::from([
        ("lit".into(), Value::Str("-".into())),
    ])),
    Value::Object(BTreeMap::from([
        ("class".into(), Value::Str("digit".into())),
        ("times".into(), Value::Int(2)),
    ])),
]);

let mut rules = BTreeMap::new();
rules.insert("date".into(), rule);

let pat = compile_rule(&rules["date"], &rules).unwrap();
assert!(is_match(&pat, "2024-09-06").unwrap());
assert!(!is_match(&pat, "2024-9-6").unwrap());
```

## Keywords

Chinese and English keywords are equivalent and can be mixed, e.g. `class` / `类`, `digit` / `数字`, `times` / `次`, `lit` / `字面`, `name` / `名`.

## License

MulanPSL-2.0
