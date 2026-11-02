# 基于渐进式心智模型的 API 二次封装 crate —— 分层草案

> **草案 v0.1，未批准，不代表定稿。** 仅为讨论稿，不含实现；文中函数签名均是**文档里的签名**，
> 不是已存在的代码。拍板前不要照此写代码（见 §6 的 10 条待决问题）。
>
> 对应 `HANDOFF-2026-11-02.md` 的 **R8**；关联 `IDEAS.md` I-12（改名未定，故暂不抢注 crate 名）。

## 0. 范围

在门面 crate `swsml`（lib 名 `sml`）之上再包一层，按新手心智模型渐进暴露能力，**每层只增加一个概念**。
本文先盘现状（只写读到的能力），再给分层与待决问题。

## 1. 现状：从「一段 SML 文本」到「一个字段」的最短路径

### 1.1 最短路径只需 4 样东西

| 需要知道 | 证据（`文件:符号`） | feature 门控 |
|---|---|---|
| `parse(&str) -> Result<Value, SmlError>` | `rust/sml-parse/src/api.rs:parse`，经 `rust/src/lib.rs` 的 `pub use sml_parse::{parse, …}` 出口 | 无（无条件编译） |
| `Value` 这个名字 | `rust/src/lib.rs:pub use sml_value::{describe_value, Map, Value}`；枚举定义见 `rust/sml-value/src/value.rs:enum Value` | 无 |
| `Value::get(&self, path: &str) -> Option<&Value>` | `rust/sml-value/src/value.rs:Value::get`（吃 `"."` 点路径 + `[idx]` 下标，**字面键名优先**） | 无 |
| 一个标量转换，如 `as_i64 / as_str / as_bool` | `rust/sml-value/src/value.rs:as_i64 / as_str / as_bool / as_object / as_array` | 无 |

错误类型：`parse` 返回 `sml_codes::SmlError`；`rust/sml-parse/src/error.rs:pub use sml_codes::SmlError as ParseError`
—— 即 `ParseError` 与 `SmlError` **是同一个类型**，只是别名。

### 1.2 真实可编译形态的最小示例（L0 现状对照）

```rust
// Cargo.toml: swsml = "0.6"      // 默认特性即可，无需任何额外 feature
use sml::{parse, Value};

fn main() {
    let text = "server web {\n  host: web.example\n  port: 8080\n}\n";
    let v: Value = parse(text).unwrap();
    // 一个字段：块被解析成对象，块内字段用点路径取
    assert_eq!(v.get("server.port").and_then(|x| x.as_i64()), Some(8080));
    // 块元数据也在这层对象里（__type / __name）
    assert_eq!(v.get("server.__type").and_then(|x| x.as_str()), Some("server"));
}
```

（`__type` / `__name` 的落点依据 `rust/src/lib.rs` 内 `parse_fragment` 测试对 `server.__type` / `server.__name` 的断言。）

结论：**最短路径本身零 feature 门控**，但新手在 `use sml::{` 的补全列表里会看到下面一整片与自己无关的东西。

## 2. 已经暴露、但新手不需要的概念

| 概念 | 现在出现在哪 | 新手为什么会撞上 |
|---|---|---|
| 契约 `sml-contract` | `rust/src/lib.rs:pub use sml_contract::{Contract, FieldSpec, TypeSpec}`、`infer`、`contract_ext` | ① `parse` **默认就应用文档内契约**（`rust/sml-parse/src/ext.rs:ParseOptions::apply_contracts` 默认 `true`），抄来的示例带 `@is X` 就会因 `E-CONTRACT-004` 未声明字段报错；② 想关掉只能走 `parse_with(ParseOptions::new().apply_contracts(false))`，而 `parse` 不接受 options |
| 模式 `sml-pattern` | `rust/src/lib.rs:pub use sml_pattern as pattern`（整模块挂到 `sml::pattern`） | 只在 `@type` / `TypeSpec::Pattern`（`rust/sml-contract/src/lib.rs:TypeSpec`）用得到；新手不写 `@type` 却先看到一整个模块名 |
| include `sml-include` | `rust/src/lib.rs:pub use sml_include::{IncludeTarget, MiniRegex, RegexError, compile_regex, parse_include_line, strip_line_comment, MAX_REGEX_LEN, …}` | 文档明说 `parse` 不做 IO、要 include 得用 `parse_file`（`rust/src/lib.rs` 模块文档「纯解析 vs 文件解析」）；而 `parse_file` 一旦被选，glob / regex / `ext-rewrite` / `implicit-ns` 这些 `@feature` 词就跟着出现（`rust/src/lib.rs` 测试 `glob_include_requires_feature` 等） |
| 多目标 emit | `rust/src/lib.rs:pub mod emit`（由 7 个 `emit-*` feature 任一开启）；它们**全在 `default`**（`rust/Cargo.toml [features].default`） | `cargo add swsml` 就编进 7 个后端；且根级 `pub use crate::emit::MAX_VALUE_DEPTH` 让内部常量出现在根命名空间 |
| 诊断 / 警告 | `rust/src/lib.rs` 根出口 `parse_with_warnings`、`parse_file_with_warnings`、`Diagnostic`、`DiagnosticKind` | 根命名空间平铺；`Diagnostic.line` 是 `Option<usize>`（`rust/sml-parse/src/ext.rs:Diagnostic`），文件入口有 include 时恒为 `None` —— 新手拿到 `None` 不知为何 |
| `preserve_order` feature | `rust/Cargo.toml [features].preserve_order` 转发到 7 个子 crate；`rust/sml-value/src/value.rs:pub type Map` 在该档换成 `crate::object::Object` | 一开就要**所有**吃 `sml-value` 的依赖同步开，否则 `Value` 裂成两个类型（`rust/Cargo.toml` 注释原文警告）；`Object` 类型在默认档不存在（`rust/sml-value/src/lib.rs` 对 `Object` 的 cfg 说明） |
| C-ABI | `rust/src/c_abi.rs:sml_parse` / `sml_dump` / `sml_free` 等 `pub unsafe extern "C" fn`，经 `rust/src/lib.rs:pub use c_abi::*` **平铺进 `sml::`** | 补全里与 Rust 端 `parse` 并列出现 `sml_parse`；`crate-type = ["rlib", "cdylib"]`（`rust/Cargo.toml [lib]`）也意味着每次都编动态库 |
| derive 宏 | `rust/src/lib.rs` 根出口 `SmlSerialize`、`SmlDeserialize`、`__private`、`from_str`、`to_string`；宏本体 `swsml_derive` | 默认开启（`derive` 在 `default`）；`from_str` / `to_string` 与 serde 桥同名，见 §3 |

## 3. 命名二义风险清单

1. **`parse` 一族（11 个近名入口）**：`parse`、`parse_versioned`、`parse_allowed`、`parse_with`、
   `parse_with_features`、`parse_with_features_env`、`parse_with_warnings`、`parse_file`、
   `parse_file_versioned`、`parse_file_features`、`parse_file_with_warnings`（均在
   `rust/sml-parse/src/api.rs`，全量平铺到 `sml::`）；另有 `loads` —— 它等于 `parse`，只是返回
   `ParseError` 别名（`rust/sml-parse/src/api.rs:loads`）。**`load` / `loads` 已被占用**，L0 不能再叫 `load`。
2. **`Value` 的取值方法**：`get`（点路径，且「字面键名优先」）、`as_str`、`as_i64`、`as_bool`、
   `as_float`、`as_array`、`as_object`、`as_object_mut`、`as_array_mut`、`len`、`is_empty`、`iter`、
   `eq_ordered`、`sorted_by_key`（`rust/sml-value/src/value.rs`）。
   **`get` 这个名字一旦被封装层复用、语义却不同（如返回 `Option<&str>`），会与 `Value::get` 直接打架。**
3. **`to_string` / `to_sml*` 一族（同一动作 6 种含义）**：
   - `impl fmt::Display for Value` → `.to_string()` 其实就是 `to_sml`（`rust/sml-value/src/dump.rs`，仅 `sml` feature）；
   - `sml::to_string`（`T: SmlSerialize`，`rust/src/derive_macro.rs:to_string`，需 `derive` + `sml`）；
   - `sml::serde::to_string` 与 `sml::serde::to_string_direct`（`rust/src/lib.rs:pub mod serde`，后者需 `sml`）；
   - `sml::to_sml` / `to_sml_canonical` / `to_sml_with` / `to_sml_checked`（`rust/sml-value/src/dump.rs`；
     `to_sml_checked` 的门面版在 `rust/src/lib.rs` 返回 `SmlError`）；
   - 再加 trait 方法 `SmlSerialize::to_sml()`（`rust/src/derive_macro.rs`）—— 同一路径名下方法名与自由函数同名。
   - 同理 `from_str` 有两套：`sml::from_str`（derive）与 `sml::serde::from_str`。

   **封装层任何「写回文本」的入口都不能叫 `to_string` / `to_sml` / `from_str`。**

## 4. 分层草案

### 4.1 L0 新手层（概念：**文档 → 字段**）

只增加 `Doc` 一个概念；`Value` 不强制出现。

```rust
// 模块 sml::easy  （设计名，待拍板 Q1/Q4）
pub struct Doc { /* 内部持有 parse 返回的 Value，不复制 */ }
pub struct Error { /* L0 统一错误，待拍板 Q8 */ }

pub fn read(text: &str) -> Result<Doc, Error>;

impl Doc {
    pub fn str(&self, path: &str) -> Option<&str>;
    pub fn int(&self, path: &str) -> Option<i64>;
    pub fn bool(&self, path: &str) -> Option<bool>;
    pub fn has(&self, path: &str) -> bool;
    pub fn raw(&self) -> &Value;   // 唯一逃生口；不用它也能跑通 L0
}
```

设计约束：`read` = `parse` 的零拷贝包装（`parse` 已返回 owned `Value`）；`str/int/bool` = `get` + `as_*`，
**不引入中间 `Value`、不做 clone**。L0 不提供写回、不提供文件、不提供诊断 —— 这三件事分别是 L1 / L2 的概念。

### 4.2 L1 进阶层（概念：**文件与契约**，两个独立闸门）

```rust
// 概念 A：从文件读（含 include 展开）
pub fn read_file(path: impl AsRef<std::path::Path>) -> Result<Doc, Error>;

// 概念 B：显式声明「我允许文档用契约」
pub enum Contracts { Off, On }          // 对应 apply_contracts(false/true)

pub fn read_file_with(path: impl AsRef<std::path::Path>, contracts: Contracts)
    -> Result<Doc, Error>;
pub fn read_with(text: &str, contracts: Contracts) -> Result<Doc, Error>;
```

⚠️ 现状硬事实（设计必须绕开）：**「文件」与「契约开关」今天无法同时使用** —— `parse_file*` 家族内部把
`ParseOptions::new()` 写死（`rust/sml-parse/src/api.rs:parse_file_with_warnings` 里
`parse_impl_tokens_ext(…, ParseOptions::new())`），不接受调用方选项；而 `parse_with(text, opts)` 只做
`tokenize_spans`、**不展开 include**（对比同文件 `parse_file` 用 `resolve_includes_spans`）。因此 L1 的
`read_file_with` 若只做转发，**当前无法实现** —— 这是本次草案最需要拍板的技术前提（Q5）。

契约相关的另一处缺口：门面只导出契约的**类型**（`Contract` / `FieldSpec` / `TypeSpec`），**没有**导出
`sml_contract::apply_contract` / `apply_contracts`（`rust/sml-contract/src/lib.rs` 有这两个 `pub fn`，
但 `rust/src/lib.rs` 的 re-export 列表里没有）。所以 L1 能给的契约能力，目前只能是「开 / 关文档内 `@is`」（Q6）。

### 4.3 L2 专家层（概念：**输出与诊断**）

```rust
pub struct Report { pub doc: Doc, pub diagnostics: Vec<Diagnostic> }
pub fn read_with_report(text: &str) -> Result<Report, Error>;               // 对应 parse_with_warnings
pub fn read_file_with_report(p: impl AsRef<Path>) -> Result<Report, Error>; // 对应 parse_file_with_warnings

// emit：薄转发，函数名避开 to_markdown / to_sml 一族
pub mod out {
    pub fn markdown(doc: &Doc) -> Result<String, Error>;
    // 其余后端按同一形状逐个开闸，各由对应 emit-* feature 门控
}
```

`Report` 与既有 `ParseOutput`（`rust/sml-parse/src/ext.rs:ParseOutput`：`value` / `diagnostics` /
`contract_map` / `errors`）字段重叠 —— 是复用还是新造，见 Q7。

### 4.4 进阶如何开闸：两个候选

| 候选 | 形态 | 优 | 劣 |
|---|---|---|---|
| **A. 构造器 + builder** | `read_with(Options::new().file(p).contracts(On))` | 入口只有 1 个，新手只需要记 `read` 与 `read_with`；加能力 = 加一个链式方法，符合「每层加一个概念」 | `Options` 会逐步长成上帝对象；引入 `read` 与 `read_with` 的**双入口**，L0 代码在 L1 之后要改写（除非 `read` 定义为 `read_with(Default)`） |
| **B. 独立模块 + 独立函数名**（本文暂取） | `easy::read` / `easy::read_file` / `easy::report::*` | 每个能力一个平铺函数，名字即文档；关掉某层不会互相牵连；L0 代码在 L1 / L2 后**逐字不变** | 入口数量随层数线性增长；命名要一直躲 §3 的占用名 |

取舍建议：**B 起步**（因为「每层只加一个概念」的直接读法是「加一个函数」，而不是「加一个开关字段」），
若日后判定 builder 更省心智成本，再在 L2 之上叠 A，不改 L0。

### 4.5 形态选择：① crate 内模块 vs ② 新建独立 crate

| 维度 | ① `swsml` 内加 `prelude` / `easy` 模块 | ② 新建独立 crate |
|---|---|---|
| 命名空间 | `sml::easy::*`，与根命名空间隔离，直接躲开 §3 全部占用名 | 同样可隔离，但要用新的 crate 名（受发布范围约束） |
| `preserve_order` 硬约束 | **不新增转发点**：新模块与 `Value` 同 crate，已随 `[features].preserve_order` 一起转发到 7 个依赖（`rust/Cargo.toml`） | **必须再加一处转发**：新 crate 也吃 `sml-value`，若它的 `preserve_order` 没把 `sml-value`（及 `sml-parse` 等间接项）一起打开，`Value` 就会裂成两个互不兼容的类型（`expected sml_value::Value, found Value`） |
| 发布范围硬约束 | 不变：仍只是 `swsml` 一个发布物 | 需新增发布物。允许范围是「swsml / swsml-derive / sml-* / smltools」（`rust/Cargo.toml` 注释「发布范围」），`qsm` / `flow` 一律 `publish = false`，所以新 crate **必须**落在 `swsml-*` / `sml-*` 命名族里 |
| 版本节奏 | 与主 crate 绑定，封装层一次破坏性改动就推 `swsml` 大版本 | 可独立发版；但要处理「依赖尚未发布的 dev 版本」的发布顺序问题（当前 workspace 版本是 `0.6.9-dev.0`） |
| L0 体积 / 依赖 | 与根 crate 同编译单元，L2 部分必须 `#[cfg]` 到 emit feature 后 | 可让新 crate 只依赖 `sml-parse`，物理上拿不到 emit 与 C-ABI |
| L0 用户仍会看到的东西 | 根命名空间照旧平铺 11 个 `parse*` + C-ABI + derive（只是不再推荐） | 用户只 `use swsml_easy::*`，看不到根命名空间 |

取舍建议：**① 起步**（少一处 `preserve_order` 转发点、少一个发布物、少一次发布顺序协调），
把 ② 作为「封装层长大到需要独立版本节奏时」的后续动作。

### 4.6 「每层只加一个概念」自检表

| 层 | 新增概念（个数） | 新增入口 | L0 代码是否逐字不变 | 关掉该层后 | 是否触碰新 feature |
|---|---|---|---|---|---|
| L0 | 文档 `Doc`（1） | `read` | — | 无 L0 即无封装 | 否（`parse` 无条件编译） |
| L1-A | 文件路径 / include（1） | `read_file` | 是 | 退回纯文本入口 | 否（`parse_file` 无条件编译） |
| L1-B | 契约开关（1） | `read_with` / `read_file_with` | 是 | `Contracts::Off` = 关 `apply_contracts` | 否 |
| L2-A | 诊断（1） | `read_with_report` 等 | 是 | 退回 `read` 的丢失诊断语义 | 否（`*_with_warnings` 无条件编译） |
| L2-B | 输出后端（1，每后端一枚） | `out::markdown` 等 | 是 | 对应 `emit-*` 关闭即整体不编译 | 是（`emit-*`） |

## 5. 风险清单

1. **与既有 `sml::*` 二义**：根命名空间已占用 `parse*`（11 个）、`loads`、`to_sml*`、`to_string`、
   `from_str`、`describe_value`、`depth_of`、`split_kv`、`scan_source_refs`、`feature_names`、`FEATURES`、
   `BUILTIN_DIRECTIVES`、`MAX_VALUE_DEPTH`、`SmlSerialize` / `SmlDeserialize`、C-ABI 的
   `sml_parse` / `sml_dump` / `sml_free`。封装层入口必须全部落在子模块内，且**不得复用 `get`、
   `to_string`、`load(s)`、`from_str`**。
2. **feature 传播**：`preserve_order` 的转发清单是 7 项（`rust/Cargo.toml`），任何新增「吃 `sml-value`
   的 crate」都会多一个漏点；`when` 只门控实现、枚举项常驻（同文件注释），封装层不得据 `when` 做条件编译假设。
3. **零依赖承诺**：文档承诺 `default-features = false` 可回到完全零依赖（`rust/src/lib.rs` 模块文档、
   `rust/Cargo.toml`）。但根级 `pub use crate::emit::MAX_VALUE_DEPTH`（`rust/src/lib.rs`）**没有 cfg**，
   而 `pub mod emit` 有 7 个 feature 的 cfg 门控 —— 零 feature 档能否编译需实测确认，
   **在确认前不要在草案里承诺任何零依赖行为**；L0 / L2 代码一律不得无条件引用 `emit::*`。
4. **性能**：`Doc` 必须持有 `parse` 已返回的 owned `Value`，`str/int/bool` 返回借用；**不得**引入第二次遍历、
   中间 `Value`、或 `to_string()` 文本中转。已知代价点：`to_sml_with(sort_keys=true)` 会先
   `sorted_by_key()`（深拷贝）再 `to_sml`（`rust/sml-value/src/dump.rs:to_sml_with`）—— L2 若暴露
   canonical 输出必须显式说明这次克隆。
5. **错误类型不统一**：`SmlError`（parse / emit / `to_sml_checked`）vs `String`（derive 的 `to_string` /
   `from_str`、serde 桥、`to_sml_checked` 的 sml-value 版）。L0 若包装成自己的 `Error`，会丢掉
   `SmlError::code()` 这个跨端稳定契约；不包则新手要面对带码文案（`文案 [E-XXX-NNN]`）。
6. **契约能力受限**：门面只导出契约类型，不导出 `apply_contract` / `apply_contracts`；且「文件 + 契约」
   组合在当前入口形状下不可达（§4.2）。L1-B 若无新入口，只能做「关掉契约」而不能做「按名字应用契约」。

## 6. 待用户拍板的问题清单

1. L0 入口叫什么？（`read` 可用；`load` / `loads` 已被 `sml::loads` 占用）
2. L0 是否引入新类型 `Doc`，还是直接返回 `Value` + 一层扩展 trait？
3. L0 是否保留 `raw() -> &Value` 逃生口，还是要求进阶者自己去用 `sml::parse`？
4. 承载形态取 ① `swsml` 内 `easy` 模块，还是 ② 新 crate（若 ②，命名必须在 `swsml-*` / `sml-*` 内
   且需新增一处 `preserve_order` 转发）？
5. 是否授权为「文件 + 契约开关」新增一个真正的 file+options 入口（等于往 `sml-parse` / 门面加 API），
   还是 L1 只做纯转发、接受这两个能力暂时不能同时用？
6. L1 的契约闸门只做「开 / 关文档内 `@is`」（`apply_contracts`），还是要把 `sml_contract::apply_contract`
   提到门面（后者需先决定 `Contract` 表由谁持有）？
7. L2 的诊断输出是复用既有 `ParseOutput`，还是新造 `Report { doc, diagnostics }`
   （前者省一次映射、但把 `contract_map` / `errors` 两个专家字段也带进 L2）？
8. L0 / L1 的错误类型统一用 `SmlError`（保住 `code()`）还是包成自有的 `Error`（更易读，但需维护码映射）？
9. 封装层是否需要任何「写回文本」入口？若需要，`to_string` / `to_sml` / `from_str` 三个名字都已被占用，
   需先定新名。
10. `preserve_order` 是否进 L0 推荐配置？（一旦推荐，所有下游依赖必须同步开启，否则 `Value` 裂型）

---

## 摘要

- 现状最短路径只需 `parse` / `Value` / `Value::get` / 一个 `as_*`，**零 feature 门控**；但根命名空间已平铺
  11 个 `parse*`、C-ABI 的 `sml_*`、derive 的 `to_string` / `from_str` 与 emit 常量。
- 三个命名雷区：`parse` / `parse_*` / `loads` 一族、`Value::get` 的点路径语义、
  `to_string` / `to_sml*` / `to_string_direct` 六种同义入口。
- 关键硬事实：`parse_file*` 不接受 `ParseOptions`（内部写死 `ParseOptions::new()`），`parse_with` 不展开
  include ⇒ **「文件 + 契约」今天无法同时用**；门面只导出契约类型、未导出 `apply_contract`。
- 分层：L0 `Doc` + `read`；L1 `read_file`（文件 / include）与 `read_with` / `read_file_with`（契约开关，
  两个独立闸门）；L2 `Report` + `*_with_report` 与 `out::*`（emit，按 feature 门控）。
- 形态建议先用 ① `swsml` 内 `easy` 模块（不新增 `preserve_order` 转发点、不新增发布物），② 独立 crate 留作后续。
- 待拍板问题共 **10 条**，核心四问：L0 入口名与是否引入 `Doc`（Q1–Q3）、形态 ①/②（Q4）、
  是否授权新增 file+options 入口以打通「文件+契约」（Q5）、L2 输出复用 `ParseOutput` 还是新造 `Report`（Q7）；
  其余问错误类型（Q8）、写回入口命名（Q9）、`preserve_order` 是否进 L0 推荐（Q10）。
