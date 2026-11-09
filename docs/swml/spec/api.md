# `swml` 门面 API 规范

> **产品线** swml ｜ **状态** current ｜ **最后核对** 2026-11-06
>
> 门面 crate `swml` 的公开面规范：根 = `swsml` 全集，`easy` = 四个概念的入门路径。
> 尚未定稿的部分（命名二义、feature 传播、发布时机）在 `../plan/api-draft.md`；
> 发布状态在 `../status.md`；元数据在 `docs/meta.sml`。
> 代码对应：`rust/swml/src/lib.rs`（根再导出）、`rust/swml/src/easy.rs`（入口层）。

## 0. 一句话

`swml` **不实现任何东西**：把 `swsml` 的公开面搬到本 crate 根，再把入门层收进 `swml::easy`。
没有第二套解析器、没有第二套值模型、没有包装类型 —— `swml::Value` **就是** `sml::Value`，
两条路径之间传值不需要转换。

> 命名陷阱：包名是 `swsml`，库名是 `sml`（crates.io 上 `sml-rs` 已被无关项目占用）。
> 故 `Cargo.toml` 写 `sml = { package = "swsml" }`，而代码里 `use sml::…`。

## 1. 两条路径

| 路径 | 位置 | 给谁 | 公开名 |
|---|---|---|---|
| 入门（progressive） | `swml::easy` | 只想「读一份配置」的人 | `Doc` / `Value` / `Error` / `Contracts` |
| 专家（expert） | crate 根 | 要把 SML 嵌进产品、需要每个旋钮 | `swsml` 全集原样再导出 |

同一份安装覆盖两种人：`easy` 只是根的一个子模块，能力上限不变。

## 2. `swml::easy` 的公开面

### 2.1 L0 的四个概念

```rust
pub use sml::SmlError as Error;   // 换名不换类型，保住跨端稳定的 Error::code()
pub use sml::Value;               // 就是 sml::Value；Doc::list / Doc::raw 的返回类型

pub enum Contracts { Off, On }    // 文档里的 @is <name> 认不认
pub struct Doc { /* 持有 parse 返回的 Value，不复制 */ }
```

### 2.2 构造入口

```rust
impl Doc {
    pub fn read(text: &str) -> Result<Self, Error>;
    pub fn read_file(path: impl AsRef<Path>) -> Result<Self, Error>;              // 展开 include
    pub fn read_with(text: &str, contracts: Contracts) -> Result<Self, Error>;
    pub fn read_file_with(path: impl AsRef<Path>, contracts: Contracts) -> Result<Self, Error>;
    pub fn read_with_report(text: &str) -> Result<Self, Error>;                   // 挂非致命诊断
    pub fn read_file_with_report(path: impl AsRef<Path>) -> Result<Self, Error>;
}
```

两个独立闸门：**文件 / include** 与 **契约开关**。二者可同时用 —— 走库的
`parse_file_with(path, ParseOptions)`，不再需要新增库能力。

### 2.3 取值入口

```rust
impl Doc {
    pub fn has(&self, path: &str) -> bool;
    pub fn str(&self, path: &str) -> Option<&str>;
    pub fn int(&self, path: &str) -> Option<i64>;
    pub fn float(&self, path: &str) -> Option<f64>;
    pub fn bool(&self, path: &str) -> Option<bool>;
    pub fn list(&self, path: &str) -> Option<&[Value]>;
    pub fn raw(&self) -> &Value;        // 唯一逃生口；不用它也能跑通 L0
    pub fn to_sml(&self) -> String;     // 写回文本的唯一入口；L0 不暴露 to_sml 一族的其它名字
}
```

路径是 `.` 分隔的**对象**路径，与 `Value::get` 同源：

- **只能穿对象**：`items.0.name` 永远取不到（没有下标语法）；要数组先 `list` 再自己索引。
- 键名自带 `.` 的字段取不到。
- 取不到一律静默（`None` / `false`），**从不报错**；区分「不存在」与「类型不对」用 `raw()`。

### 2.4 诊断

```rust
impl Doc {
    pub fn diagnostics(&self) -> &[sml::Diagnostic];
    pub fn has_diagnostics(&self) -> bool;
}
```

只有 `*_with_report` 构造出来的 `Doc` 会带诊断；普通 `read` 丢弃非致命诊断（不是错误）。

### 2.5 输出后端

```rust
pub mod out {
    pub fn markdown(doc: &Doc) -> Result<String, Error>;   // 各由同名 emit-* feature 门控
    pub fn html(doc: &Doc) -> Result<String, Error>;
    pub fn latex(doc: &Doc) -> Result<String, Error>;
    pub fn xml(doc: &Doc) -> Result<String, Error>;
    pub fn svg(doc: &Doc) -> Result<String, Error>;
    pub fn slint(doc: &Doc) -> Result<String, Error>;
}
```

`emit-*` 只门控 `easy::out` 里的同名函数：**关掉即函数不存在**，不是运行时错误。
它不会裁剪 `swsml` 的 `emit` 模块 —— 那是底层 crate 的 feature。

### 2.6 这一层故意不做的

| 不做 | 想要就去用 |
|---|---|
| 契约**内容**（定义、类型检查、`contract_map` / `errors`） | `sml::parse_with` / `ParseOutput` |
| 逐后端**旋钮**（`math` / `standalone` / 自定义模板…） | `sml::emit` 的 options |
| 特性开关、版本白名单 | `sml::parse_with_features` / `parse_allowed` |
| 二进制持久化、查询 | `amberice`（`.ambr`）/ 未开工的 `smldb` |

再往下（按名字应用契约、`contract_map`、增量 emit）都回专家层。

## 3. 裁决记录（Q1–Q10，2026-11-03 用户逐条确认）

| # | 问题 | 裁决 |
|---|---|---|
| Q1 | L0 入口叫什么 | `read`（一眼看懂；`load` / `loads` 与解析层 `parse` 系列易混） |
| Q2 | 是否引入新类型 | 是，`Doc` |
| Q3 | 是否保留 `raw() -> &Value` 逃生口 | 保留 |
| Q4 | 承载形态：`swsml` 内模块 vs 独立 crate | **并入门面 crate `swml` 的 `easy` 模块**（2026-11-06 收口：两条发布线合成一条） |
| Q5 | 是否授权 file+options 入口 | 授权，已落地 `parse_file_with(path, ParseOptions)` |
| Q6 | L1 契约闸门范围 | 只做「开 / 关文档内 `@is`」 |
| Q7 | L2 诊断输出 | 复用既有 `ParseOutput`，不新造 `Report` 类型 |
| Q8 | 错误类型 | `SmlError`（保住 `code()`），本模块换名 `Error` |
| Q9 | 写回文本 | 只给 `Doc::to_sml()`，不开其它写入口 |
| Q10 | `preserve_order` 是否进 L0 推荐 | 不进 default，要用自己开 |

## 4. API 硬规则（全仓库适用）

> 用户 2026-11-05 定：**所有 API 都照这三条走**，新 crate 也先把约束立在这里，
> 免得长出另一套习惯。已落地的样板是 `amberice` 的加密接口。

### 规则 1 · 一个 enum 管一组选项

同一件事的多个来源 / 形态，**写在同一个 enum 里**，不要拆成「写版类型 + 读版类型」两个。

```rust
// ✅ 一个 enum，两个方向靠函数签名约束
pub enum KeySource<'a> {
    Bytes(&'a [u8; 32]),
    Passphrase { secret: &'a [u8], kdf: Option<KdfParams> },
}

// ❌ 两个类型：调用方要在两个名字之间做一次没有信息量的选择
pub enum SealKey<'a> { … }
pub enum OpenKey<'a> { … }
```

代价与对策：一个 enum 会多出「这个字段在写 / 读时该不该填」的语义 —— 用**运行期显式报错**解决，
不要为此再拆类型。

### 规则 2 · 命名：明确 > 简短

一个名字只表达一件事；同一动作不给第二个名字。名字要能单独看懂，不依赖上下文。

- 反例（本仓历史债务）：`to_string` / `to_sml` / `to_sml_canonical` /
  `to_string_direct` / `SmlSerialize::to_sml()` —— 同一动作的五种含义。
- 正例：`Contracts::{Off, On}`、`Doc::read_file_with_report`。

### 规则 3 · API 树扁平：一棵树，每层节点少一些

不要为了「看起来有层次」加深嵌套：**层数增加的是记忆成本，不是能力**。
一个模块下的公开名保持个位数；超了就该问「这些是不是同一件事的两个名字」。

### 与渐进式心模的关系（优先于上面三条）

「每层只增加一个概念」的判据是：**L0 会不会被迫看到它**。

| 层 | 允许出现 | 不许出现 |
|---|---|---|
| L0 最简路径 | `read` / `Doc` / `str` / `int` | 任何 feature、任何 `_with_*` 变体 |
| L1 进阶 | `Contracts` 这类**可选旋钮** | 必须理解才能读文件的**强制**概念 |
| L2 专家 | `out::*`、`crypto` / `compress` 各 feature | 默认构建里的第三方依赖 |

判定顺序永远是：先问「L0 会不会被迫看到它」，再问「命名与位置合不合上面三条」。
一个名字再好、位置再扁平，只要出现在 L0 的补全列表里，就是错的。
