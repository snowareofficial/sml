# eclog-rs 设计：Eclog-CPP 的 Rust 版

> 第 3 步设计结论，对应 [I-28](../../repo/ref/todo-ideas-tables.md) 的 eclog-rs 定位。
> 口径（用户裁决）：**完整语法对齐 C++ 全量行为**；**eclog-rs 独立、不依赖 smlite**；数值自建带 raw 的 Number，**字节级严格对拍**；amberice 对 eclog 的支持由 eclog-rs 自己的模块实现。
> 事实来源：Vallest/Eclog-CPP 源码（`_scratch/eclog-cpp`，MIT），行为重写而非代码翻译。
> 状态：3a / 3b 已落地，3c 的**契约与对拍工具**也已落地（`rust/eclog-rs`，2026-11-17，`cargo test -p eclog-rs` 与 `--features amberice` 全绿）；3c 的实依赖待 P16 / P17。

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
    pub delimiter: String,           // raw 的定界符 / heredoc 的 TAG，≤16 个 alnum/_
}
```

- 与 smlite 的区别：smlite 是 `Float(f64)` 无 raw、int/float 分型；eclog-rs 按 C++ 语义保留 raw 与记法，**不可**复用 smlite 值模型。
- `Number::as_i64()` / `as_u64()` / `as_f64()` 按需解析 raw；`Str` 保留记法以支撑 `typical` scheme 下的原样渲染。
- `delimiter` 是渲染必需而非可选：heredoc 必须用原 TAG 重发（C++ `RendererTests.cpp:314-334`），内容等于 TAG 时才降级为 quoted。
- `Object` 用插入序 `Vec<(Str, Value)>`，重复键语义照 C++ `insertForce`：**位置取首次出现，键与值取最后一次**（含键的记法）。键的唯一性由 `insert` 保证，`append` 撞键时整体不动。
- 相等语义有意不对称：`Str` 只比 `text`（记法属表现层），`Number` 比 `raw + info`（raw 就是身份，不做数值折算）。

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
- feature：`default = []`；`std`（错误 trait）；`amberice`（`.ambr` 对接**契约**，零依赖 —— 只建 trait，不建不生效的空 module）。
- 实际落地的文件：`lib.rs` / `error.rs` / `value.rs` / `parse.rs`（3a）+ `render.rs` / `merge.rs`（3b）+ `amberice.rs`（3c 契约）+ `tests/conformance.rs` / `tests/parse_rules.rs` / `tests/render_merge.rs` + `tests/golden/` + `tools/{oracle.cpp,edge.cpp,regen_render_golden.py}`（对拍工具）。
- 解析器是**状态机逐条移植**，不是重写的递归下降：分隔规则（逗号与换行等价、尾逗号、同一行必须逗号）与 `+` 拼接的跨行规则都藏在那几个 `state_*` 里。树与事件共用同一条解析路径（单一 `Sink` 双出口），因此两个出口不可能各自漂移。
- `ParseHandler` 不做错误通道（C++ 的 `pe_user_asked_for_termination` 不收），需要中途终止时用树 API 自行短路。

## 5. amberice 接力模块（当前只定契约）

- 目标：把 eclog 值树封装成 `.ambr`，反向解出后回 eclog 值树；调用的是 amberice 容器**字节面**（写 `dump_with` / `pack_with`、读 `load` / `get`，见 P17 评估 §6.1）。
- **已落地（3c）**：feature `amberice` 里的 `amberice::AmbericeBridge`（`dump(&Value) -> Vec<u8>` / `load(&[u8]) -> Value`，`type Error` 由实现方定）。零依赖、不接线，doctest 里用一个「载荷即 eclog 文本」的退化适配器证明接口可用。
- **不经 smlite**：amberice → eclog-rs 的对接在 eclog-rs 侧完成，smlite 值模型不参与；与 smlite 的 `ByteEnvelope` trait（仅草案、无实依赖）并列但互不调用。
- **不自造二进制格式**：容器不认识 eclog 文本语法（P17 评估 §6.3），值树的编码归适配器；在 `sml-value` 的 canonical 表示就绪前自定一套字节格式只会与将来那份重复。因此当前只定 trait，不定编码。
- 待办：P16（no_std `sml-value`）与 P17（no_std amberice 子集）落地后，由本 crate 的适配器实现该 trait。届时必须显式决定一处**信息损失**：`Number::raw` 与 `Str` 的 `notation` / `delimiter` 是 eclog 特有表现层，canonical 表示不承载 —— 往返后值相等，但渲染字节不再逐字节相同。

## 6. 对拍策略（字节级严格）

1. **素材**：把 `_scratch/eclog-cpp/Tests/Text/` 的 **14 组 / 23 对 / 46 个文件**（`.ecl` / `.json` 输入 + 同名 `.check` 期望）收为仓库内 golden 文件（放 `rust/eclog-rs/tests/golden/`，注明 MIT 来源）。字节必须取自 **git 索引的 LF**（`git show HEAD:Tests/Text/<file>`）——上游仓库是 `i/lf`，本地浅克隆因 `core.autocrlf=true` 会变成 CRLF，而 heredoc 内容会带着行尾进正文。仓库 `.gitattributes` 已钉 `*.ecl` / `*.check` 为 `text eol=lf`（`*.json` 原就有），对拍断言里也显式检查 `.check` 不含 `\r`。
2. **解析对拍**：Rust `parse_with` 产生的事件序列与 `.check` 逐事件比对；改一条规则即全量重跑。
3. **渲染对拍**：把 C++ `Compact/Eclog.cpp`（+ 一个 20 行的驱动）编成 oracle，对每份输入用 `typical` 与 `json_compatible` 两 scheme 渲染，产物收为 `tests/golden/render/<名>.<scheme>.render`（23 输入 × 2 = 46 份），Rust 端逐字节比对。另有 `edge.typical/compact.render` 两份，由另一个直接建 `ValueDesc` 的 oracle 生成，专门覆盖**记法降级**（raw 含控制字符 / `"`+定界符、heredoc 内容撞 TAG 或缩进后的 TAG）、compact、inline、注释 —— 这些内容无法从 `.ecl` 文本反推，只能构造。
   驱动与脚本已进仓库：`tools/oracle.cpp` / `tools/edge.cpp` / `tools/regen_render_golden.py`。复跑判据：`python rust/eclog-rs/tools/regen_render_golden.py --check`（重新编译 oracle → 重跑 48 份 → 逐字节比对，任何漂移非零退出；`--write` 才重写）。脚本需要 `g++` 与本地的 Eclog-CPP 浅克隆（默认 `_scratch/eclog-cpp`），故不进 CI。
4. **数值对拍**：解析侧 `Number.raw` 与 `NumberInfo` 逐字段比对（`+0/-0`、`1e100`、超 64 位整数、`±inf/nan`），不做数值等价折算；渲染侧**按数值重新格式化**（C++ 值树只存数值）——`+0`→`0`、`-0`→`0`、`-0.0`→`-0`、`3.14159265358979323846`→`3.141592653589793`。浮点数字取自 Rust 的最短往返格式化，排版规则照 C++（`[-4, 17)` 定点、其余 `e` 指数且不带 `+`）。
5. **Merge Patch 对拍**：移植 `DocumentTests.cpp:1230-1313` 的 11 个用例到 `tests/render_merge.rs`（树相等判据），含「补丁子树里的 `null` 成员先递归剔除」这条容易漏的规则。

## 7. 分轮施工

| 轮 | 内容 | 判据 | 状态 |
|---|---|---|---|
| 3a | 值模型（Number/Str 带 raw 与记法）+ 解析器（4 种字符串记法 + 拼接 + 数值状态机 + ast 树 + 树/事件双出口） | 14 组 / 23 个输入的解析事件与 `.check` 逐行一致；树序列化回事件亦一致 | **已完成（2026-11-17）** |
| 3b | 双 scheme 渲染器 + RFC 7396 merge | 两 scheme 渲染字节与 C++ oracle 一致（46 + 2 份 `.render`）；merge 11 个用例一致 | **已完成（2026-11-17）** |
| 3c | amberice 对接模块（契约，待 P16/P17 再接实依赖）+ 对拍工具固化 | `cargo test -p eclog-rs`（含 `--features amberice`）全绿；`python tools/regen_render_golden.py --check` 可重复跑出 48 份一致 | **契约与工具已完成（2026-11-17）**；实依赖待 P16 / P17 |

## 8. 文档治理

- 新增 owner `eclog`：改 `rust/smldocs/src/docs.rs` 的 `OWNERS` 与 `docs/meta.sml` 顶部注释；登记本文（status `current`，date/updated `2026-11-17`）。
- I-28 行同步 eclog-rs 依赖事实为本文口径；正文不写历史。
- 跑 `cargo run -p smldocs -- --index` 与 `--check`。

## 9. 边界与后续衔接

- 3a（值模型 + 解析器）、3b（双 scheme 渲染器 + RFC 7396 merge）已落地；3c 只剩**实依赖**（P16 / P17 落地后由本 crate 的适配器实现 `AmbericeBridge`）。
- oracle 的驱动与复跑脚本已进仓库（`tools/`），但 C++ 源码仍只在本机 `_scratch` 浅克隆里；仓库里只留它产出的 `.render` 期望字节。amberice 不接实依赖；不改 smlite。
- 不碰编辑器扩展（第 4 步：`editors/vscode` 的 eclog 分支、语法文件、vsix 构建）。
- 不记 `docs/changes.sml`（无搬迁/拆分/删除）；不改生成物 `docs/index.md` / `docs/report.md`（由工具刷新）。
- 第 4 步衔接：编辑器语法高亮须与本文 §2 语言规范一致（4 种字符串记法、`#` 注释、双 scheme 差异、inf/nan）。
- 已记的边界（照 C++ 行为保留，不"修正"）：`transferDecimalDigits` 先无条件吞一个字符（`1.e5` / `1.x` 都成了 number token）；raw 与 quoted 都不能跨行；heredoc 闭标记行尾必须是换行（EOF 处报 `unexpected end of file`）；递归无深度上限（与 C++ 一致）。
- 3b 的两处**已知未覆盖**（都不出现在正常文档里）：① per-value `fracDigits`（C++ `NumberDesc(v, n)` 的定点位数）未进值模型，构造值只能按最短往返渲染；② `1.x` 这类被 `transferDecimalDigits` 吞进 token 的退化字面量，渲染时经 Rust 的 `f64` 解析会得到 `nan`（C++ 的 `stringToDouble` 只吃前缀、得 `1`）。正常文档的 23 个 golden 输入不含这两种形态。
