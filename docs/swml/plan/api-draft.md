# `swml` 门面 API · 未定部分与风险

> **产品线** swml ｜ **状态** draft ｜ **最后核对** 2026-11-06
>
> 已定稿的公开面见 `../spec/api.md`。本文只收**还没收敛**的东西：
> 根命名空间的历史债务、命名二义、feature 传播、以及仍卡在裁决上的项。
> **本文不得当规范引用**；条目收敛后应改写进 `../spec/api.md`，本文相应删除。
> 元数据在 `docs/meta.sml`。

---

## 1. 根命名空间：已暴露、但新手不需要的概念

`easy` 之所以存在，是因为 `use sml::{` 的补全列表里有一整片与「读一份配置」无关的东西：

| 概念 | 现在出现在哪 | 新手为什么会撞上 |
|---|---|---|
| 契约 `sml-contract` | 根 `pub use sml_contract::{Contract, FieldSpec, TypeSpec}`、`infer`、`contract_ext` | `parse` **默认就应用文档内契约**（`ParseOptions::apply_contracts` 默认 `true`），抄来的示例带 `@is X` 就会因 `E-CONTRACT-004` 未声明字段报错；想关掉只能走 `parse_with(ParseOptions::new().apply_contracts(false))`，而 `parse` 不接受 options |
| 模式 `sml-pattern` | 根 `pub use sml_pattern as pattern`（整模块） | 只在 `@type` / `TypeSpec::Pattern` 用得到；新手不写 `@type` 却先看到一整个模块名 |
| include `sml-include` | 根 `pub use sml_include::{IncludeTarget, MiniRegex, RegexError, compile_regex, parse_include_line, …}` | 文档明说 `parse` 不做 IO、要 include 得用 `parse_file`；而一旦选 `parse_file`，glob / regex / `ext-rewrite` / `implicit-ns` 这些 `@feature` 词就跟着出现 |
| 多目标 emit | 根 `pub mod emit`（由 7 个 `emit-*` feature 任一开启），**全在 `default`** | `cargo add swsml` 就编进 7 个后端；根级 `pub use crate::emit::MAX_VALUE_DEPTH` 让内部常量出现在根命名空间 |
| 诊断 / 警告 | 根出口 `parse_with_warnings`、`parse_file_with_warnings`、`Diagnostic`、`DiagnosticKind` | 根命名空间平铺；`Diagnostic.line` 是 `Option<usize>`，文件入口带 include 时恒为 `None` —— 新手拿到 `None` 不知为何 |
| `preserve_order` | `[features].preserve_order` 转发到 7 个子 crate；`sml-value` 的 `Map` 在该档换成 `Object` | 一开就要**所有**吃 `sml-value` 的依赖同步开，否则 `Value` 裂成两个类型；`Object` 在默认档不存在 |
| C-ABI | `c_abi.rs` 的 `sml_parse` / `sml_dump` / `sml_free` 等经 `pub use c_abi::*` **平铺进 `sml::`** | 补全里与 Rust 端 `parse` 并列出现 `sml_parse`；`crate-type = ["rlib", "cdylib"]` 也意味着每次都编动态库 |
| derive 宏 | 根出口 `SmlSerialize`、`SmlDeserialize`、`__private`、`from_str`、`to_string` | 默认开启；`from_str` / `to_string` 与 serde 桥同名 |

> 收敛方向：**不动 `swsml` 根**（已发 18 版，破坏面太大），把「新手看到的集合」交给
> `swml::easy` 的四名字。上面这些在门面里仍然可达，只是不再是推荐路径。

## 2. 命名二义风险清单

1. **`parse` 一族（11 个近名入口）**：`parse`、`parse_versioned`、`parse_allowed`、`parse_with`、
   `parse_with_features`、`parse_with_features_env`、`parse_with_warnings`、`parse_file`、
   `parse_file_versioned`、`parse_file_features`、`parse_file_with_warnings`（`sml-parse/src/api.rs`，
   全量平铺到 `sml::`）；另有 `loads` —— 它等于 `parse`，只是返回别名。**`load` / `loads` 已被占用**，
   L0 不能再叫 `load`。
2. **`Value` 的取值方法**：`get`（点路径，且「字面键名优先」）、`as_str`、`as_i64`、`as_bool`、
   `as_float`、`as_array`、`as_object`、`as_object_mut`、`as_array_mut`、`len`、`is_empty`、`iter`、
   `eq_ordered`、`sorted_by_key`。**`get` 一旦被封装层复用、语义却不同（如返回 `Option<&str>`），
   会与 `Value::get` 直接打架** —— 故 `easy` 用 `str` / `int` / `bool` / `list`，不用 `get`。
3. **`to_string` / `to_sml*` 一族（同一动作六种含义）**：`impl Display for Value` 的 `.to_string()`、
   `sml::to_string`（derive 的 `T: SmlSerialize`）、`sml::serde::to_string` 与
   `sml::serde::to_string_direct`、`sml::to_sml` / `to_sml_canonical` / `to_sml_with` /
   `to_sml_checked`、以及 trait 方法 `SmlSerialize::to_sml()`。
   同理 `from_str` 有两套（`sml::from_str` 与 `sml::serde::from_str`）。
   ⇒ **封装层任何「写回文本」的入口都不能叫 `to_string` / `to_sml` / `from_str`**，
   `easy` 的对应物是 `Doc::to_sml()`（方法，不是自由函数）。

## 3. 现行约束

1. **入口必须落在子模块内**，且不得复用 `get`、`to_string`、`load(s)`、`from_str`。
2. **feature 传播**：`preserve_order` 的转发清单是 7 项，任何新增「吃 `sml-value` 的 crate」
   都会多一个漏点；`when` 只门控实现、枚举项常驻，封装层不得据 `when` 做条件编译假设。
3. **零依赖档**：`default-features = false` 必须能编译通过（承诺真实成立）。
   L0 / L2 代码不得无条件引用 `emit::*` —— `pub mod emit` 仍被 7 个 `emit-*` 门控。
4. **性能**：`Doc` 必须持有 `parse` 已返回的 owned `Value`，取值返回借用；
   **不得**引入第二次遍历、中间 `Value`、或 `to_string()` 文本中转。
   已知代价点：`to_sml_with(sort_keys=true)` 会先 `sorted_by_key()`（深拷贝）再 `to_sml`
   —— 暴露 canonical 输出时必须显式说明这次克隆。
5. **错误类型**：`SmlError`（parse / emit / `to_sml_checked`）vs `String`（derive 的 `to_string` /
   `from_str`、serde 桥）。`easy` 包装成自己的 `Error` 会丢掉 `SmlError::code()` 这个跨端稳定契约，
   故只换名不换类型。
6. **契约能力受限**：门面只导出契约类型，**没有**导出 `sml_contract::apply_contract` /
   `apply_contracts`。L1 能给的只有「开 / 关文档内 `@is`」；「按名字把某份契约应用到已有树」
   仍做不到。

## 4. 未定项（卡在裁决或前置）

| # | 项 | 现状 | 卡在哪 |
|---|---|---|---|
| U1 | 发布时机 | `swml` 0.1.0，`publish = false` | 与 `amberice` / v5 一起发；不单独发 |
| U2 | 门面是否进本仓 workspace | `rust/Cargo.toml` 的 `members` **已含** `swml`（在 workspace 内编译与测试） | `TODO.md:1494` 写「门面**不进**本仓 workspace（独立仓库、依赖 crates.io）」⇒ 两处口径相反，**待裁决** |
| U3 | 防御性别名 `swmltool` / `swmltools` | 未做 | 前置是 `smltools` 抽 lib（现为 bin-only）；不抽就不占 |
| U4 | `apply_contract` / `apply_contracts` 是否上提到门面 | 未做 | 需先定 `Contract` 表由谁持有 |
| U5 | `easy` 的公开面是否冻结 | 目前 4 个名字 + 3 个附加能力 | 0.1.0 发布前定一次，之后按语义化版本走 |
