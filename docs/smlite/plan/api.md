# smlite crate 骨架与公开 API 设计

> 第 1 步设计结论，对应 [I-28](../../repo/ref/todo-ideas-tables.md) 与 [P17 评估](../../amberice/plan/nostd-subset-eval.md) 的前置裁决。
> 本文只定骨架、公开 API 与值层互转契约；不创建 `rust/smlite/` 实现代码，不发明 eclog 具体词法语法（待第 3 步拿到 Eclog-CPP 源码对拍）。
> 已确认的两处地基决策：① 自建精简值模型（`Float(f64)` 无 raw）+ 互转层；② `amberice` 走可选 feature 接力、核心不硬依赖，可独立发布 crates.io。

## 0. 结论摘要

- **值模型**：自建 `Value` 七变体，去掉 Float raw / 无无符号整数 / 无 `__type`/`__name` 保留键；`Object` 默认用插入序 `Vec<Entry>`，`BTreeMap` 排序序作为互斥可选 feature，默认关（插入序对读写更友好，遵循「互斥 feature 默认选性能更优者」）。
- **解析器边界**：`parse(&[u8])` / `parse_str(&str)` 收字节出值；`to_string` / `to_vec` 出 JSON-compatible 文本；`merge_patch` 落 RFC 7396。本步只定签名与语义边界，不写具体词法。
- **feature**：`default = ["std"]`；`swsml-compat` 挂互转实现；`amberice` 先留 trait/接口不接实依赖；核心 `#![no_std] + extern crate alloc` 自洽。
- **互转**：在 `swsml-compat` 下于 smlite crate 内实现 `From` / `TryFrom`，逐变体映射并保证 `Map::Ordered` 对接可签 canonical。
- **发布**：纳入 workspace、`publish = true`、双语 README；本步不动 `rust/Cargo.toml`（第 2 步建 crate 时一并做）。

## 1. 定位与边界

- smlite 是主仓内**新 crate** `rust/smlite`，发 crates.io 作被依赖包；全新轻量解析器/运行时，**不复用 `sml-parse`**。
- 语法取 SML 纯数据子集，机器导向、类 JSON、对标 `serde_json`，面向嵌入式与高吞吐；**排除** featag / weak / `@type` / include / env / 片段 / 契约等文档友好特性（I-28）。
- 依赖链目标：`amberice`（规划中 no_std 子集）→ smlite → swsml → swml；smlite → eclog-rs。当前 amberice 尚无 no_std 实现且 `publish = false`（见 P17 评估），故本步**不接实依赖**，仅在接口层留接力点。
- 本地无 Eclog-CPP 源码（全仓 `*eclog*` 为 0 命中）；I-28 只给五个互操作点（无引号串、`#` 注释、顶层省略 `{}`、JSON-compatible 渲染、RFC 7396），具体词法规则不在本步发明。

## 2. 值模型

### 2.1 `Value` 定义（草案）

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),                 // 无无符号整数，与 sml-value 对齐
    Float(f64),               // 无 raw 字面量：解析后丢书写形式，渲染走 format_float(f, None)
    Str(String),
    Array(Vec<Value>),
    Object(Object),           // 插入序容器，见 2.2
}

pub const MAX_VALUE_DEPTH: usize = 128;   // 与 sml-value 对齐，保证互转无深度语义差
```

- **无 Float raw**：`f64` 无法表达「作者写的是科学计数法」；为对标 serde_json、让 `Value` 更小更快，不携带原始字面量。渲染统一走 `format_float(f, None)`。若 eclog 场景确需保留 `1e10` 这类书写形式，属 eclog 层职责，不进 smlite 值模型。
- **无 `__type` / `__name` 保留键**：这两项是 swsml 文档友好特性；smlite 对象键一律普通键。
- **错误类型自建**：`SmliteError` + 稳定 `kind()` 分类（仿 `ContainerError` / `CanonicalError`，见 [overview.md](../../amberice/spec/overview.md) 的「错误模型」）；**不占** `errors/codes.sml` 五端码位。仅在与 swsml 语言层对齐的转换点复用 `SmlError` / `E-*`。

### 2.2 `Object` 插入序容器（用户裁决：Vec<Entry>）

```rust
pub struct Object { entries: Vec<(String, Value)> }
```

- 默认插入序，读写都按写入顺序，保留源序；对标 serde_json 的 `preserve_order` 心智，也是 eclog / SML 场景的自然形态。
- **互斥 feature `object-sort`（默认关）**：切到 `BTreeMap<String, Value>` 排序序，换取确定性字节序输出与更快的键查找，代价是丢源序。两个后端互斥，遵循「互斥 feature 默认选性能更优者」——本场景默认 `Vec<Entry>`。
- 关键方法：`new` / `with_capacity` / `get(&str)` / `insert(String, Value)` / `iter()` / `eq_ordered`；`From<[(String, Value); N]>` 便捷构造。

### 2.3 便捷构造（对标 `json!` 体验）

- `From<i64>` / `From<f64>` / `From<&str>` / `From<String>` / `From<bool>` / `From<Vec<Value>>` / `From<Object>`。
- sml-value 没有这些 `From`（构造走 `Value::float` 等关联函数，见 `sml-value/src/value.rs`），属 smlite 自建表面 API。
- 宏 `smlite::json!` 列为后续候选，不进第 2 步最小切片。

## 3. 解析器边界（只定签名，不发明语法）

| 入口 | 签名草案 | 语义 |
|---|---|---|
| 字节解析 | `parse(bytes: &[u8]) -> Result<Value, ParseError>` | 无 std IO；输入即完整文档字节。 |
| 文本解析 | `parse_str(s: &str) -> Result<Value, ParseError>` | `parse` 的 UTF-8 便利包装。 |
| 文本渲染 | `to_string(&Value) -> String` | JSON-compatible；float 走 `format_float(f, None)`，非有限数写成带引号字符串。 |
| 字节渲染 | `to_vec(&Value) -> Vec<u8>` | `to_string` 的字节形式，供 `.ambr` / eclog 交换。 |
| Merge Patch | `merge_patch(base: &Value, patch: &Value) -> Result<Value, PatchError>` | RFC 7396：`null` 删键、对象递归、数组整体替换；标量有界限。 |
| 深度上限 | `MAX_VALUE_DEPTH = 128` | 与 sml-value 对齐，互转无语义差；超限报明确错误，不静默截断。 |

- **解析器内部规则（无引号串边界、`#` 注释、顶层省略 `{}`）待第 3 步**：I-28 给的五点是互操作目标，不是完整语法规范；自行补齐会与 Vallest/Eclog-CPP 实际实现偏离，故本步只留 API 口子。
- `ParseError` / `PatchError` 是 `SmliteError` 的分类子类型，均带 `kind()`。

## 4. feature 旗帜与 no_std 接力点

| feature | 默认 | 内容 |
|---|---|---|
| `std` | 开 | `std::error::Error` impl、`parse_str` 便利入口等；`default-features = false` 进嵌入式。 |
| `object-sort` | 关 | `Object` 切 `BTreeMap` 排序序（与默认 `Vec<Entry>` 互斥，见 2.2）。 |
| `swsml-compat` | 关 | 可选依赖 `sml-value`，放值层互转实现（见 §5）。 |
| `amberice` | 关 | **no_std 接力点**：先只定义 trait/接口，不接实依赖（P16/P17 已落地，接线仍未做）。核心不硬依赖 amberice，故可独立发 crates.io。 |
| `merge-patch` | — | RFC 7396 进核心 API（eclog 必用），不单开 feature。 |

- 核心 crate `#![no_std] + extern crate alloc`：smlite 自己就能 no_std，不必等 amberice。
- 「接力点」层次：smlite 提供字节面（`&[u8] -> Vec<u8>`，`parse` / `to_vec`），amberice 提供容器字节信封；两者通过接口对接而非硬编码依赖。草案 trait 形如：

```rust
pub trait ByteEnvelope {
    fn wrap(bytes: &[u8]) -> Result<Vec<u8>, EnvelopeError>;
    fn unwrap(envelope: &[u8]) -> Result<Vec<u8>, EnvelopeError>;
}
```

- 32 位风险：smlite 自建值模型不涉及 sml-value 的 `u64 as usize` 截断（`sml-value/src/lib.rs:40-44`），但 `swsml-compat` 打开会传递该风险，需在互转文档标注。

## 5. 与 swsml 值层的互转方案

### 5.1 放置位置与孤儿规则

- 互转实现放 **smlite crate**（`swsml-compat` feature 下），依赖 `sml-value`，不改 sml-value 源码。
- 孤儿规则允许：`impl From<smlite::Value> for sml_value::Value`（目标类型是外部 `sml_value::Value`，但 trait 参数 `smlite::Value` 是本地类型，且出现在第一个类型参数位置之前无裸类型参数，合法）；`impl TryFrom<sml_value::Value> for smlite::Value`（源类型外部、目标本地，合法）。若实装时编译器判定冲突，退路是定义本地新类型包装 `SmlValueConvert`，不阻塞方案。

### 5.2 逐变体映射

| smlite `Value` | sml-value `Value` | 处理要点 |
|---|---|---|
| `Null` / `Bool` / `Int(i64)` / `Str` | 同名变体 | 直转；`Array` 递归。 |
| `Float(f64)` | `Float(f64, Option<String>)` | smlite→sml-value 置 raw=`None`（`Value::float`）；sml-value→smlite 丢弃 raw。 |
| `Object`（插入序 `Vec<Entry>`） | `Map::Ordered`（源序 `Object`） | **默认对接 Ordered**：用 `Map::ordered()` 保留源序，使结果可签 canonical（否则 `CanonicalValue` 返回 `KeyOrderUnavailable`，见 `sml-value/src/canonical.rs:390-394`）。 |
| `Object`（`object-sort`） | `Map::Unordered`（`BTreeMap`） | 排序序对接无序态；字节序确定。 |
| — | `Map::Ordered` / `Unordered` ← smlite | 反向：取 Ordered 源序、Unordered 按 key 字节序，装成 smlite `Object`。 |
| 普通键 | `__type` / `__name` 保留键 | 作为普通字符串键直转，不做特殊语义解释（smlite 无保留键概念）。 |

- 两个方向各提供 owned 与 borrowed 路径：`From<Value> for sml_value::Value`、`From<sml_value::Value> for Value`（owned）；`TryFrom<&sml_value::Value> for Value`（borrow，内部 clone，失败只因深度超限）。
- 深度：`MAX_VALUE_DEPTH` 两边同值 128，转换不引入额外深度限制。

### 5.3 门面契约（本步只定，不改门面）

- swsml 侧后续加 feature：`smlite = ["dep:smlite"]`，门面 `#[cfg(feature = "smlite")] pub use smlite::...;`（照 `amberice` 开门惯例，见 `rust/Cargo.toml` 的 `amberice` / `amberice-compress` 分层）。
- 只在 `swsml-compat` 打开时才能走 `sml::` 与 `smlite::` 的互转；默认构建不会把 smlite 拖进依赖树。

## 6. crate 骨架草案（文档内，非实现）

```text
rust/smlite/
  Cargo.toml      # package: publish=true, version.workspace, edition 2021, 中文 description
  README.md       # 双语（中文 + .en.md 或同文件中英并列，照仓库 i18n 惯例）
  src/
    lib.rs        # SPDX + 双语 //! + 集中 pub use
    value.rs      # Value / Object / MAX_VALUE_DEPTH / From 实现
    error.rs      # SmliteError + ParseError / PatchError + kind()
    parse.rs      # parse / parse_str（词法语法第 3 步后填）
    emit.rs       # to_string / to_vec / format_float 复用（text_rules 同口径）
    patch.rs      # merge_patch（RFC 7396）
    compat.rs     # #[cfg(feature = "swsml-compat")] 互转 impl
    envelope.rs   # #[cfg(feature = "amberice")] ByteEnvelope trait 草案（接力点）
```

- `Cargo.toml` 骨架：`[package]` 用 `version.workspace = true`、license `MulanPSL-2.0`、`publish = true`；`[dependencies]` 仅 `sml-value` 可选（`swsml-compat`）、`amberice` 可选（接力点，当前不开实际版本）；`[features]` 如 §4，`default = ["std"]`。
- `lib.rs` 开头风格：第 1 行 SPDX、`//!` 中英并列、模块用成对 `/// 中文 / /// English` 说明、末尾集中 `pub use`（照 `sml-value/src/lib.rs` / `sml-lex/src/lib.rs`）。
- 接入 workspace：第 2 步在 `rust/Cargo.toml` 的 `members` 加 `"smlite"`；本步不改。

## 7. 文档治理

- 新增 `smlite` 产品线：改 `rust/smldocs/src/docs.rs:30` 的 `OWNERS` 常量（加 `"smlite"`），并更新 `docs/meta.sml` 顶部 owner 注释（`:11`）。这是 owner 枚举扩充，向后兼容。
- 本文登记进 `docs/meta.sml`（`owner: "smlite"`、`status: "current"`、date/updated `2026-11-17`），跑 `cargo run -p smldocs -- --index` 与 `--check`。
- 正文不写历史；本步无搬迁/拆分/删除，不记 `docs/changes.sml`。

## 8. 本步不做 / 后续衔接

- 不创建 `rust/smlite/` 任何源码或 `Cargo.toml`；不接 amberice 实依赖；不改 swsml 门面代码（只定 §5.3 契约）。
- 不发明 eclog 词法/语法规则；不碰 eclog-rs 与 `editors/vscode`。
- 不记 `docs/changes.sml`；不改生成物 `docs/index.md` / `docs/report.md`（由工具刷新）。
- **第 2 步衔接**：按本文骨架建 crate、接入 workspace、实现最小切片（值模型 + 基础解析/渲染 + merge_patch + 测试），性能口径见 `docs/swsml/ref/perf-baseline.md`。
- **第 3 步衔接**：拿到 Eclog-CPP 源码后定具体词法，补齐 `parse` / `to_string` 的 eclog 互操作。
