# swsml

**SWE Serial `<< 19 * 99 >>`** — 1999

> **In memory of the Chinese victims of the NATO bombing of the Chinese Embassy
> in Yugoslavia on 7 May 1999.** \
> 谨以此编号纪念 1999 年 5 月 7 日（贝尔格莱德时间）北约轰炸中国驻南斯拉夫联盟
> 大使馆中遇难的三位中国记者：邵云环、许杏虎、朱颖。

**SML — SNOWARE Markup Language** for Rust: a declarative data/configuration format, an alternative to JSON and YAML. Features: optional quotes, fragments, contracts (schema layer with enums, defaults and composition), `include` directive, environment-variable inlining, zero dependencies (optional serde).

**English below** ｜ 中文在上方，English 在下方

> 包名是 `swsml` 而非 `sml-rs`——后者已被无关项目占用
> （Smart Message Language 智能电表协议解析器）。
> lib 名仍为 `sml`，因此 `use sml::{...}` 不受影响。
>
> The crate is named `swsml` because `sml-rs` was taken by an unrelated project
> (a smart-meter protocol parser). The **lib name is still `sml`**,
> so `use sml::{...}` is unaffected.

---

# 中文

SML（SNOWARE Markup Language）的 Rust 实现：声明式数据/配置格式，JSON/YAML 的替代品。

Logo：黑花括号 `{}` 表示语法骨架（块的边界），蓝色雪花 `❄` 表示精确的取值点。

## 安装

```toml
[dependencies]
swsml = "0.1"

# 需要 serde 互操作时：
# swsml = { version = "0.1", features = ["serde"] }

# 不需要 derive 宏时可关闭默认 feature，回到完全零依赖：
# swsml = { version = "0.1", default-features = false }
```

## 快速开始

```rust
use sml::{parse, to_sml};

let v = parse("name: John\nage: 27")?;
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
assert_eq!(v.get("age"), Some(&sml::Value::Int(27)));

// 序列化回 SML（可 round-trip）
println!("{}", to_sml(&v));
```

## 语法一览

```sml
@version v1

# 引号可选：裸词即字符串
firstName: John
age: 27

# 块冒号可省：address { } 等价于 address: { }
address {
    streetAddress: "21 2nd Street"   # 含空格才需要引号
    state: NY                         # 裸词
}

# 数组：逗号可选
phoneNumbers: [ { type: home } { type: office } ]

# 片段：定义 + 以「值」形式引用
@base { region: cn-north-1 }
region: &base

# 环境变量内联
apiKey: $env.RESEND_API_KEY

# 词中 @ 无需转义（仅词首的 @ 才是片段标记）
contact {
    to: a@b.c
    from: "sal <sal@mail.swebase.cn>"
}
```

## 特性

| 特性 | 说明 |
|---|---|
| 引号可选 | 裸词即字符串（`state: NY`） |
| 块冒号可省 | `address { }` ≡ `address: { }` |
| 数组分隔灵活 | `[ a b c ]`、每行一个、逗号可选 |
| 片段 | `@name { }` 定义，`&name` 以**值**形式引用并展开 |
| **契约** | 可选 schema 层：字段类型、枚举、默认值、区间、组合 |
| include 指令 | 拆分配置，可嵌套、可在块内注入字段 |
| 版本声明 | `@version v1`，便于将来演进不破坏旧文档 |
| 环境变量内联 | `$env.VAR` |
| 类型自识别 | `true/false/null` / 数字 / 字符串 |

## 数据类型

SML 是**纯数据格式**，值模型与 JSON 同构，共 7 种：

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}
```

顶层支持三种形态（与 `to_sml` 输出对称）：键值块、`{ ... }` 对象块、`[ ... ]` 数组。

```rust
// 对象数组（如「历史记录」这类列表数据）
let v = parse("[ { ts: \"2026-01-01\" to: \"a@b.c\" } ]")?;
```

> 顶层**标量**（如单独的 `42`）不可往返——SML 顶层需为容器。

## 契约（Contract）

SML 本身无类型系统。契约是**可选**的 schema 层，为块提供结构体约束、枚举、默认值、取值区间。

```rust
use sml::parse;

let text = r#"
@contract Server {
    host: str                              # 必填（默认 required）
    port: int default 5432                 # 缺失时填充
    tls: bool default false
    tags: [str] optional                   # 可选，元素须为字符串
    status: enum [ active standby retired ]
    weight: num min 0 max 100
}
db {
    @is Server
    host: db1.internal
    status: active
    weight: 80
}
"#;
let v = parse(text)?;
// port / tls 由 default 填充
assert_eq!(v.get("db.port"), Some(&sml::Value::Int(5432)));
```

- `@contract Name { }` 定义契约（**不进解析结果**）；`@is Name` 在块内应用
- 应用时**填充 default**，并校验必填、类型、枚举、数值区间、数组元素类型
- 校验发生在**解析期**，违反即返回错误（而非留到应用侧）
- 契约须在 `@is` 之前定义（顺序依赖）
- 不使用契约时行为完全不变 —— **向后兼容**

类型：`str` / `int` / `num` / `bool` / `any` / `[T]` / `enum [ ... ]`
修饰符：`required`（默认）/ `optional` / `default <值>` / `min <数>` / `max <数>`

### 组合（而非继承）

契约之间不共享字段定义，而是「字段的类型是另一个契约」——直接写契约名，可多层嵌套：

```sml
@contract Address {
    city: str
    country: str default CN
}
@contract Server {
    host: str
    address: Address        # 组合：该字段的值须符合 Address 契约
}
db {
    @is Server
    host: db1.internal
    address { city: Beijing }   # country 缺 -> 自动填 CN
}
```

嵌套块会递归校验并回填默认值；被引用契约可在之后定义。

### 严格模式（默认严格）

未声明字段**默认被拒绝**（能立即发现 `prot` 这类拼写错误）。确需放宽须显式写 `loose`：

```sml
@contract Metrics loose {   # 允许额外字段
    latency: num min 0
}
```

`loose` 只放宽「未声明字段」，已声明字段照样校验。

## include 指令

```sml
# app.sml
app: resender
database {
    include "conf.d/db.sml"   # 在块内注入一组字段
    pool: 16
}
```

```rust
let v = sml::parse_file("app.sml")?;
```

- 相对路径按**被包含文件自身所在目录**解析（同 C 预处理器）
- 语义是**文本内联**而非对象合并，因此可出现在块内部
- 循环引用、文件缺失均返回错误，不静默跳过；嵌套上限 32 层

> `parse()` 是纯函数（不做 IO），include 由 `parse_file()` / `resolve_includes()` 处理，
> 因此在无文件系统的环境（WASM / 沙箱）中仍可安全嵌入 `parse()`。

## 版本声明

```rust
use sml::{parse_versioned, Version};

let (v, ver) = parse_versioned("@version v1\nname: John")?;
assert_eq!(ver, Version::V1);
```

- 未声明时默认按当前版本处理，**既有文档不受影响**
- 声明了不支持的版本会报错，而非静默按错误语法解析
- `version` 是保留字，不可作为片段名

## serde 支持（可选）

启用 `serde` feature 后，`Value` 实现 `Serialize`/`Deserialize`，可与任意 serde 后端互操作：

```rust
let v = parse("name: John\nage: 27")?;
let json = serde_json::to_string(&v)?;          // {"name":"John","age":27}
let back: sml::Value = serde_json::from_str(&json)?;
```

采用**手写实现**而非 `#[derive]`，以保证数据形状自然：
`Value::Int(27)` 序列化为 `27`，而非 derive 会产生的 `{"Int":27}`。

### serde 桥：任意 serde 类型一键反序列化

`serde` feature 还提供 `sml::serde::{from_str, from_value, to_value, to_string}`，
任何 `#[derive(serde::Deserialize / Serialize)]` 类型都能像 toml-rs 一样直接与 SML 互转
（枚举沿用 `__type` 约定，也兼容 `{ VariantName: ... }` 外部标签）：

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Server { host: String, port: u16 }

let s: Server = sml::serde::from_str("host: web.example\nport: 8080\n").unwrap();
let text = sml::serde::to_string(&s).unwrap();
```

`Value` 也因此可与 toml/serde_json 双向互通：

```rust
let v: sml::Value = sml::parse("host: web.example\nport: 8080\n").unwrap();
let toml_text = toml::to_string(&v).unwrap();            // SML -> TOML
let back: sml::Value = toml::from_str(&toml_text).unwrap(); // TOML -> SML
```

不启用该 feature 时，本 crate 为**零依赖**。

## 自然序列化宏（derive，默认开启）

`derive` feature（默认开启）提供 `#[derive(SmlSerialize, SmlDeserialize)]`，
把自定义结构体/枚举「自然地」映射为 SML，无需引入 serde：

```rust
use sml::{SmlDeserialize, SmlSerialize};

#[derive(SmlSerialize, SmlDeserialize, Debug)]
struct Server {
    host: String,
    #[sml(default)]
    port: i32,
    #[sml(rename = "tls-enabled")]
    tls_enabled: bool,
    #[sml(skip)]
    secret: String,
}

#[derive(SmlSerialize, SmlDeserialize, Debug)]
enum Status {
    Active,
    #[sml(rename = "stand-by")]
    StandBy,
}

let s = Server {
    host: "web.example".into(),
    port: 8080,
    tls_enabled: true,
    secret: "hunter2".into(),
};
let text = s.to_sml();   // host: web.example / port: 8080 / tls-enabled: true
let back = Server::from_sml(&text)?;   // secret 由 skip 重置为 Default

// 也支持 toml-rs 风格的顶层函数（derive 默认开启时可用）：
let text = sml::to_string(&s);
let back: Server = sml::from_str(&text)?;
```

映射规则（形状与 `to_sml` 输出对称）：
- **结构体 → 块**：字段名即键；`Option` 字段为 `None` 时省略
- **枚举单元变体 → 裸词**（`status: Active`）
- **枚举带数据变体 → 带 `__type` 的块**（`{ __type: Circle _value: 3 }`）
- **单元结构体 → 裸词**；**newtype → 透明**；**tuple 结构体 → 数组**

属性：
`#[sml(rename = "...")]` 改名、`#[sml(skip)]` 跳过（反序列化时重置为 `Default`）、
`#[sml(default)]` 缺失时用 `Default`、`#[sml(flatten)]` 并入子块，
容器级 `#[sml(rename_all = "kebab-case")]` 批量改名。
泛型结构体/枚举也支持（自动补充 `T: SmlSerialize` 等约束）。

## 运行示例

```bash
cargo run --example include_demo
cargo run --example include_demo --features serde   # 额外打印 JSON
cargo run --example derive_demo                     # derive 宏「自然」序列化
```

## 多语言实现

| 语言 | 位置 |
|---|---|
| Soup / Lua | `../lua/`（`lib/sml.soup`） |
| Rust | 本目录 |
| C | `../c/sml.h` |
| JavaScript | `../js/sml.mjs` |

## License

MulanPSL-2.0

---

# English

Rust implementation of **SML — SNOWARE Markup Language**: a declarative
data/configuration format, an alternative to JSON and YAML.

## Installation

```toml
[dependencies]
swsml = "0.1"

# With serde interop:
# swsml = { version = "0.1", features = ["serde"] }
```

## Quick start

```rust
use sml::{parse, to_sml};

let v = parse("name: John\nage: 27")?;
assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
assert_eq!(v.get("age"), Some(&sml::Value::Int(27)));

// Serialize back to SML (round-trip safe)
println!("{}", to_sml(&v));
```

## Syntax at a glance

```sml
@version v1

# Quotes optional: bare words are strings
firstName: John
age: 27

# Block colon optional
address {
    streetAddress: "21 2nd Street"
    state: NY
}

# Arrays: commas optional
phoneNumbers: [ { type: home } { type: office } ]

# Fragment: defined then referenced as a *value*
@base { region: cn-north-1 }
region: &base

# Environment variable inlining
apiKey: $env.RESEND_API_KEY

# `@` inside a word needs no escaping (only a leading `@` is a fragment marker)
contact {
    to: a@b.c
    from: "sal <sal@mail.swebase.cn>"
}
```

## Features

| Feature | Description |
|---|---|
| Optional quotes | Bare words are strings (`state: NY`) |
| Optional block colon | `address { }` ≡ `address: { }` |
| Flexible array separators | `[ a b c ]`, one per line, commas optional |
| Fragments | `@name { }` defines, `&name` references as a **value** |
| **Contracts** | Optional schema layer: types, enums, defaults, ranges, composition |
| `include` directive | Split config files; nestable, injectable inside blocks |
| Version declaration | `@version v1` for forward-compatible evolution |
| Env var inlining | `$env.VAR` |
| Type inference | `true/false/null` / numbers / strings |

## Data types

SML is a **pure data format**; its value model is isomorphic to JSON with 7 variants:

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}
```

Three top-level forms are supported (symmetric with `to_sml` output):
key-value blocks, `{ ... }` object blocks, and `[ ... ]` arrays.

```rust
let v = parse("[ { ts: \"2026-01-01\" to: \"a@b.c\" } ]")?;
```

> A top-level **scalar** (e.g. a lone `42`) cannot round-trip —
> the top level must be a container.

## Contracts

SML has no type system of its own. Contracts are an **optional** schema layer
providing struct-like constraints, enums, defaults and numeric ranges.

```rust
use sml::parse;

let text = r#"
@contract Server {
    host: str                              # required (default)
    port: int default 5432                 # filled when missing
    tls: bool default false
    tags: [str] optional                   # optional, items must be strings
    status: enum [ active standby retired ]
    weight: num min 0 max 100
}
db {
    @is Server
    host: db1.internal
    status: active
    weight: 80
}
"#;
let v = parse(text)?;
// port / tls come from `default`
assert_eq!(v.get("db.port"), Some(&sml::Value::Int(5432)));
```

- `@contract Name { }` defines a contract (**not included in the parse result**);
  `@is Name` applies it inside a block
- Applying one **fills defaults** and validates required fields, types, enums,
  numeric ranges and array item types
- Validation happens at **parse time** — violations return errors immediately
- A contract must be defined **before** the `@is` that uses it
- Without contracts, behaviour is unchanged — **fully backward compatible**

Types: `str` / `int` / `num` / `bool` / `any` / `[T]` / `enum [ ... ]`
Modifiers: `required` (default) / `optional` / `default <value>` / `min <n>` / `max <n>`

### Composition (not inheritance)

Contracts do not share field definitions; instead, *a field's type can be another
contract*. Just write the contract name — nesting works to any depth:

```sml
@contract Address {
    city: str
    country: str default CN
}
@contract Server {
    host: str
    address: Address        # the value must satisfy the Address contract
}
db {
    @is Server
    host: db1.internal
    address { city: Beijing }   # country missing -> filled with CN
}
```

Nested blocks are validated recursively and defaults are filled.
A referenced contract may be defined later.

### Strict mode (strict by default)

Fields not declared in the contract are **rejected by default** (this catches
typos like `prot`). To allow extras, write `loose` explicitly:

```sml
@contract Metrics loose {   # allow undeclared fields
    latency: num min 0
}
```

`loose` only relaxes *undeclared* fields; declared ones are still validated.

## The `include` directive

```sml
# app.sml
app: resender
database {
    include "conf.d/db.sml"   # injects a set of fields into this block
    pool: 16
}
```

```rust
let v = sml::parse_file("app.sml")?;
```

- Relative paths resolve against **the including file's own directory**
  (like the C preprocessor)
- Semantics are **text inlining**, not object merging, so it works inside blocks
- Cycles and missing files return errors (never silently skipped); depth limit 32

> `parse()` is a pure function (no I/O); `include` is handled by `parse_file()` /
> `resolve_includes()`. This makes `parse()` safe to embed in environments
> without a filesystem (WASM / sandboxes).

## Version declaration

```rust
use sml::{parse_versioned, Version};

let (v, ver) = parse_versioned("@version v1\nname: John")?;
assert_eq!(ver, Version::V1);
```

- Undeclared documents use the current version — **existing documents are unaffected**
- An unsupported declared version errors instead of parsing with wrong grammar
- `version` is a reserved word and cannot be a fragment name

## Serde support (optional)

With the `serde` feature, `Value` implements `Serialize`/`Deserialize` and
interoperates with any serde backend:

```rust
let v = parse("name: John\nage: 27")?;
let json = serde_json::to_string(&v)?;          // {"name":"John","age":27}
let back: sml::Value = serde_json::from_str(&json)?;
```

This uses a **hand-written** implementation rather than `#[derive]` to keep the
data shape natural: `Value::Int(27)` serializes to `27`, not `{"Int":27}`.

### Serde bridge: one-shot deserialization for any serde type

The `serde` feature also provides `sml::serde::{from_str, from_value, to_value,
to_string}`. Any `#[derive(serde::Deserialize / Serialize)]` type can go back and
forth with SML just like toml-rs (enums use the `__type` convention and also
accept `{ VariantName: ... }` external tags):

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Server { host: String, port: u16 }

let s: Server = sml::serde::from_str("host: web.example\nport: 8080\n").unwrap();
let text = sml::serde::to_string(&s).unwrap();
```

`Value` interoperates bidirectionally with toml / serde_json as well:

```rust
let v: sml::Value = sml::parse("host: web.example\nport: 8080\n").unwrap();
let toml_text = toml::to_string(&v).unwrap();              // SML -> TOML
let back: sml::Value = toml::from_str(&toml_text).unwrap(); // TOML -> SML
```

Without this feature the crate is **dependency-free**.

## Natural derive macros (default on)

The `derive` feature (enabled by default) provides
`#[derive(SmlSerialize, SmlDeserialize)]` to map your own structs/enums to SML
"naturally", without pulling in serde:

```rust
use sml::{SmlDeserialize, SmlSerialize};

#[derive(SmlSerialize, SmlDeserialize, Debug)]
struct Server {
    host: String,
    #[sml(default)]
    port: i32,
    #[sml(rename = "tls-enabled")]
    tls_enabled: bool,
    #[sml(skip)]
    secret: String,
}

#[derive(SmlSerialize, SmlDeserialize, Debug)]
enum Status {
    Active,
    #[sml(rename = "stand-by")]
    StandBy,
}

let s = Server { host: "web.example".into(), port: 8080, tls_enabled: true, secret: "x".into() };
let text = s.to_sml();             // host: web.example / port: 8080 / tls-enabled: true
let back = Server::from_sml(&text)?;   // `secret` is reset to Default (skip)

// toml-rs style top-level functions are also provided (with the default `derive` feature):
let text = sml::to_string(&s);
let back: Server = sml::from_str(&text)?;
```

Mapping rules (symmetric with `to_sml` output):
- **struct → block**: field names are the keys; `Option` fields are omitted when `None`
- **enum unit variants → bare words** (`status: Active`)
- **enum variants with data → `__type` block** (`{ __type: Circle _value: 3 }`)
- **unit struct → bare word**; **newtype → transparent**; **tuple struct → array**

Attributes: `#[sml(rename = "...")]`, `#[sml(skip)]`, `#[sml(default)]`,
`#[sml(flatten)]`, and container-level `#[sml(rename_all = "kebab-case")]`.
Generic types are supported (bounds such as `T: SmlSerialize` are added automatically).

## Examples

```bash
cargo run --example include_demo
cargo run --example include_demo --features serde   # also prints JSON
cargo run --example derive_demo                     # derive macros
```

## Other language implementations

| Language | Location |
|---|---|
| Soup / Lua | `../lua/` (`lib/sml.soup`) |
| Rust | this directory |
| C | `../c/sml.h` |
| JavaScript | `../js/sml.mjs` |

## License

MulanPSL-2.0
