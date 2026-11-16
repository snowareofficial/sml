# eclog-rs 设计：Eclog-CPP 的 Rust 版

> 第 3 步设计结论，对应 [I-28](../../repo/ref/todo-ideas-tables.md) 的 eclog-rs 定位。
> 口径（用户裁决）：**完整语法对齐 C++ 全量行为**；**eclog-rs 独立、不依赖 smlite**；数值自建带 raw 的 Number，**字节级严格对拍**；amberice 对 eclog 的支持由 eclog-rs 自己的模块实现。
> 事实来源：Vallest/Eclog-CPP 源码（`_scratch/eclog-cpp`，MIT），行为重写而非代码翻译。本步只出设计，不落实现代码。

## 0. 结论摘要

| 问题 | 结论 |
|---|---|
| 范围 | 对齐 C++ 全量语法：6 种值类型、4 种字符串记法 + `+` 拼接、数值 raw + `±inf/nan`、双渲染 scheme、RFC 7396 Merge Patch。 |
| 依赖 | 独立 crate `rust/eclog-rs`，**不依赖 smlite**；核心 `core + alloc`、零硬依赖。smlite 先行完成（第 0–2 步），二者并列。 |
| amberice 接力 | amberice 对 eclog 的支持放在 eclog-rs 自己的模块（feature `amberice`），自实现字节信封读写，**不经 smlite、不改 smlite**。 |
| 数值 | 自建 `Number`（raw lexeme + sign/frac/exp/inf/nan 标志 + 按需 i64/f64），对拍要求**字节级一致**，不改 smlite 的 `Float(f64)`（无 raw）决策。 |
| 对拍 | 把 C++ `Tests/Text/*.ecl|json` 收为 golden；编译 C++ `Compact/Eclog.cpp` 作 oracle，解析走 `.check` 事件期望、渲染逐字节比对。 |
| 许可 | C++ 版 MIT；Rust 版按行为重写，测试向量可引用并注明来源，不复制 C++ 源码。 |

## 1. 定位与边界

- eclog-rs 是 [`Vallest/Eclog-CPP`](https://github.com/Vallest/Eclog-CPP) 的 Rust 实现：JSON-like 文本格式的解析 / 渲染库，不是日志格式（README.md:3）。
- 独立发布（`publish = true`），与 smlite 并列；smlite 是轻量 SML 纯数据子集，二者语法目标不同，不互相依赖。
- I-28 原「`smlite → eclog-rs`」依赖链不再成立，本文与 I-28 同步为新事实：`eclog-rs` 独立，amberice 对 eclog 的覆盖由 eclog-rs 模块自实现。
- 编辑器侧 eclog 扩展属第 4 步，放 `editors/vscode/`，不在本文范围。

## 2. 语言规范（从 C++ 源码提炼）

> 行号相对 `_scratch/eclog-cpp`。无本地 EBNF，语法以源码为准。

### 2.1 值类型

- `ValueType` 6 种：`null / boolean / string / number / object / array`（`Include/Eclog/ValueType.h:11-18`）；**number 不分 int/float**，由 raw lexeme + `NumberInfo` 决定（`:42-48`、`Include/Eclog/NumberInfo.h:11-18`）。
- 根是 object；空输入合法（`Source/Parse.cpp:1485-1520`）。

### 2.2 字符串（4 种记法 + 拼接）

| 记法 | 规则 | 证据 |
|---|---|---|
| unquoted | 首字符 `[A-Za-z_]`；后续 `[A-Za-z0-9_.-]`；不含空格 | `Parse.cpp:389-412` |
| quoted | 双引号；转义 `\" \\ \/ \b \f \n \r \t \uXXXX \u{...}`（变长）+ 代理对；控制字符禁 | `Parse.cpp:240-366`、`414-490` |
| raw | `@delim"..."delim`，delim ≤16 位 alnum/_ | `Parse.cpp:492-600`、`StringNotation.h:11-16` |
| heredoc | `\|TAG ... TAG`，按首行缩进 dedent | `Parse.cpp:649-814` |
| 拼接 | `+` 连接，可跨行 | `Parse.cpp:815-854`、`1225-1255`、`Tests/Text/StringConcatenation.ecl` |

### 2.3 数值

- 状态机：可选 `±`；整数 `0` 或 `1-9` 起（禁前导零）；小数 `.digits`；指数 `e/E±`；无十六进制（`Parse.cpp:856-992`）。
- `±inf` / `±nan` 是 number 关键字（`Parse.cpp:909-928`、`1191-1215`）。
- 保留 raw bytes + `NumberInfo`（位记录 sign/neg/frac/exp/inf/nan），渲染按 `fracDigits` 控制（`Value.h:122-183`、`NumberDesc.h:15-22`）。

### 2.4 结构 / 注释 / key

- 注释只有 `#` 行注释（`Parse.cpp:164-219`）；无 `//` / `/* */`。
- 顶层 `{}` 可省；**嵌套对象必须显式 `{}`、数组必须 `[]`**（`Parse.cpp:1531-1584`、`1587-1603`）；无 SML 式具名块。
- key:value 只用 `:`（无 `=`）；字段 / 元素以逗号或换行分隔，允许尾逗号（`Parse.cpp:1042-1086`、`1170-1188`、`1384-1402`）。
- unquoted key 不得为 `true/false/inf/nan/null`，否则 `pe_invalid_key`（`Parse.cpp:1042-1052`、`1059-1071`）。

### 2.5 JSON 兼容与 Merge Patch

- 双 scheme（`RendererConfig.h:15-18,41-56`）：
  - `typical`：根无 `{}`、bare 串、无逗号、inf/nan 裸写、允许注释。
  - `json_compatible`：根有 `{}`、全 quoted、逗号分隔、inf/nan 写成带引号串、无注释。
- JSON 文本是 Eclog 的子集：`Tests/Text/*.json` 与 `.ecl` 成对作为 parse 输入（`ParseTests.cpp:23-48`）；无独立 `to_json/from_json` API。
- RFC 7396 Merge Patch 已实现：null 删键 / 对象递归 / 其它替换（`README.md:754-792`、`ObjectNodeImpl.h:740-784`、`DocumentTests.cpp:1230-1310`）。

## 3. 值模型（自建，带 raw）

```rust
pub enum Value {
    Null,
    Bool(bool),
    Object(Object),                  // 插入序；key 可为任意字符串记法解码结果
    Array(Vec<Value>),
    Number(Number),                  // 不分 int/float
    Str(Str),                        // 携带来源记法（unquoted/quoted/raw/heredoc）
}

pub struct Number {
    pub raw: String,                 // 原始字面量，字节级对拍用
    pub info: NumberInfo,            // sign/neg/frac/exp/inf/nan 位
}

pub struct Str {
    pub text: String,                // 解码后的值
    pub notation: StringNotation,    // 来源记法，渲染时按 scheme 决定保留/规范化
}
```

- 与 smlite 的区别：smlite 是 `Float(f64)` 无 raw、int/float 分型；eclog-rs 按 C++ 语义保留 raw 与记法，**不可**复用 smlite 值模型。
- `Number::as_i64()` / `as_f64()` 按需解析 raw；`Str` 保留记法以支撑 `typical` scheme 下的原样渲染。

## 4. crate 骨架与公开 API

```text
rust/eclog-rs/
  Cargo.toml      # publish=true, version.workspace, 核心零硬依赖 + feature
  README.md       # 双语（中文 + English）
  src/
    lib.rs        # SPDX + 双语 //! + 集中 pub use
    value.rs      # Value / Number / NumberInfo / Str / StringNotation / Object
    parse.rs      # parse(&[u8]) -> Document；事件流 + 树双入口
    render.rs     # Renderer + RendererConfig（双 scheme）
    merge.rs      # merge_patch（RFC 7396）
    error.rs      # EclogError + kind()
    amberice.rs   # #[cfg(feature="amberice")] 自实现字节信封对接
  tests/
    conformance.rs # golden 对拍（解析事件 + 渲染字节）
```

- 对照 C++ API（`Include/Eclog/Parse.h:16-56`、`Document.h:162-265`、`ObjectNode.h:203-235`）：
  - 事件流：`parse_with(&[u8], handler)`，handler 实现 `on_object_begin/end`、`on_array_begin/end`、`on_key`、`on_value`（对照 `Parse.h`）。
  - 树 API：`Document::parse(&[u8])`、`find/get/append/insert/remove`、`merge`（对照 `Document.h`/`ObjectNode.h`）。
  - 渲染：`Renderer::new(OutputStream, RendererConfig)` + `begin_root_object/end_root_object/render_comment/close`（对照 `Renderer.h:21-97`）。
- feature：`default = []`；`std`（错误 trait）、`amberice`（字节信封模块，不依赖 smlite）。

## 5. amberice 接力模块（自实现）

- eclog-rs 的 `amberice` feature 自实现：把 eclog 值树编码为字节、调用 amberice 容器字节面（`dump`/`pack`/`load`/`get`，见 P17 评估 §6.1）封装成 `.ambr`，反向解出后回 eclog 值树。
- **不经 smlite**：amberice → eclog-rs 的对接在 eclog-rs 侧完成，smlite 值模型不参与；与 smlite 的 `ByteEnvelope` trait（仅草案、无实依赖）并列但互不调用。
- 该模块待 P16/P17 的 no_std amberice 子集落地后再接实依赖；当前只定契约，不接线。

## 6. 对拍策略（字节级严格）

1. **素材**：把 `_scratch/eclog-cpp/Tests/Text/` 的 13 组 `.ecl` / `.json` / `.check` 收为仓库内 golden 文件（放 `rust/eclog-rs/tests/golden/`，注明 MIT 来源）。
2. **解析对拍**：Rust `parse_with` 产生的事件序列与 `.check` 逐事件比对；改一条规则即全量重跑。
3. **渲染对拍**：编译 C++ `Compact/Eclog.cpp` 为 oracle，对每份 `.ecl` 用 `typical` 与 `json_compatible` 两 scheme 渲染，Rust 端逐字节比对（含 `\u{...}`、heredoc dedent、inf/nan 表示）。
4. **数值对拍**：`Number.raw` 与 `NumberInfo` 逐字段比对（`+0/-0`、`1e100`、超 64 位整数、`±inf/nan`），不做数值等价折算。
5. **Merge Patch 对拍**：移植 `DocumentTests.cpp:1230-1310` 用例到 `tests/conformance.rs`。

## 7. 分轮施工（本步不落代码）

| 轮 | 内容 | 判据 |
|---|---|---|
| 3a | 值模型（Number/Str 带 raw 与记法）+ 解析器（4 种字符串记法 + 拼接 + 数值状态机 + ast 树） | 13 组 `.ecl` 解析事件与 `.check` 一致 |
| 3b | 双 scheme 渲染器 + RFC 7396 merge | 两 scheme 渲染字节与 C++ oracle 一致；merge 用例一致 |
| 3c | amberice 对接模块（契约，待 P16/P17 再接实依赖）+ 对拍工具固化 | `cargo test -p eclog-rs` 全绿、对拍脚本可重复跑 |

## 8. 文档治理

- 新增 owner `eclog`：改 `rust/smldocs/src/docs.rs` 的 `OWNERS` 与 `docs/meta.sml` 顶部注释；登记本文（status `current`，date/updated `2026-11-17`）。
- I-28 行同步 eclog-rs 依赖事实为本文口径；正文不写历史。
- 跑 `cargo run -p smldocs -- --index` 与 `--check`。

## 9. 本步不做 / 后续衔接

- 不创建 `rust/eclog-rs/` 实现代码；不编译 C++ oracle；不接 amberice 实依赖；不改 smlite。
- 不碰编辑器扩展（第 4 步：`editors/vscode` 的 eclog 分支、语法文件、vsix 构建）。
- 不记 `docs/changes.sml`（无搬迁/拆分/删除）；不改生成物 `docs/index.md` / `docs/report.md`（由工具刷新）。
- 第 4 步衔接：编辑器语法高亮须与本文 §2 语言规范一致（4 种字符串记法、`#` 注释、双 scheme 差异、inf/nan）。
