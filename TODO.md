# SML 待办（跨实现一致性）

本文档记录 SML 多语言实现的**已知缺陷与未完成项**，以及**新特性的跨实现的落地计划**。

## 编辑器插件

已交付 VSCode 扩展：`editors/vscode/`（高亮 + 诊断 + 补全 + 悬浮 + 格式化）。
桥接层 `src/sml-parse.mjs` 复用仓库的 `js/sml.mjs`，保证与语言实现行为一致。

依赖的 JS 侧增强（已完成）：token 记录字符偏移 `pos`、错误携带位置、
新增 `offsetToPosition(text, offset)`、`parseSafe` 返回 `pos` / `position`。

- [ ] **契约校验接入编辑器**：契约仅 Rust 支持，JS 侧只做语法解析，
      故字段类型/枚举/区间等**语义**错误在编辑器中不会报出（已在扩展 README 说明）。
      契约移植到 JS 后（见下节）即可自动获得语义诊断
- [ ] **一次报出多条错误**：当前解析器遇错即停，编辑器只显示第一条
- [ ] **LSP server**：抽一层 LSP 以支持 Neovim / JetBrains 等编辑器
- [ ] **跳转到契约定义**：`@is Server` → 跳到 `@contract Server`
- [ ] **悬停显示契约展开结果**：展示默认值填充后的最终结构

## 新特性：契约（Contract）

契约是可选 schema 层，为块提供结构体约束、枚举、默认值、取值区间。
语法与语义见 `README.md` 的「契约」章节与 `showcase_contract.sml`。

| 实现 | 状态 | 说明 |
|---|---|---|
| Rust | ✅ **已实现** | `@contract` / `@is` + 校验 + 默认值 + **组合** + **严格模式/loose**；22 个测试通过 |
| C | ✅ **已实现** | `c/sml.c` 有 `ccontract` / `apply_contract_rec` / `parse_contract_body`（此前误标为"待实现"，2026-09-18 更正） |
| C++ | ✅ **已实现** | `cpp/sml.cpp` 封装同一套能力 |
| JS | ✅ **已实现** | `js/sml.mjs` 有契约表、`@is` 处理与默认值填充（此前误标为"待实现"，2026-09-18 更正） |
| Lua | ⏳ 待实现 | **唯一未落地的一侧** |

**已在 resender 中落地使用**：`AppConfig` 的 SML 持久化应用了契约
（`src/config.rs` 的 `CONFIG_CONTRACT`），读取时校验字段类型并补齐缺失默认值。
选择 `loose` 是刻意的——避免未来新增配置项后旧配置文件被拒绝。

移植时需保持一致的行为（以 Rust 为参考）：
- 契约定义不进解析结果
- `@is` 前必须已定义契约，否则报「未定义的契约」
- 字段缺失：有 default 则填充，无 default 且 required 则报错，optional 则字段不出现
- 校验顺序：类型 → 枚举 → 数值区间；数组逐元素校验
- 不使用契约时行为完全不变（向后兼容）

**已定稿的设计决策**（用户 2026-08-30 确认）：
- ✅ **用组合，不用继承**：契约之间不共享字段定义，而是「字段的类型是另一个契约」。
  语法上复用裸词写契约名，不引入新 token；可多层嵌套、递归校验并回填默认值
- ✅ **严格为默认，放宽须显式**：未声明字段默认报错（可捕获拼写错误），
  确需允许额外字段时必须在契约名后写 `loose`；`loose` 只放宽未声明字段，
  已声明字段照样校验
- ✅ **少用 token**：不新增关键字/符号。类型名复用裸词，`loose` 复用裸词，
  组合引用复用裸词，分隔符沿用既有的 `{ } [ ] :`

**待确认的剩余设计项**：
- [ ] 契约级「至少一个字段」/互斥字段（`oneOf`）等高级约束是否需要
- [ ] 数组长度约束（如 `min_items` / `max_items`）是否需要
- [ ] 契约是否支持跨文件（配合 `include` 复用契约库）

背景：在 resender 中大规模使用 SML 时，发现 Rust 实现有两个阻断性缺陷
（顶层数组无法解析、词中 `@` 导致邮箱被截断），修复过程中同步暴露出
各语言实现之间的行为不一致。

> 状态更新：2026-08-30

## 一、两个已定位的核心缺陷（Rust 已修，其他实现待对齐）

| 缺陷 | 说明 | Rust | Lua | C | JS |
|---|---|---|---|---|---|
| 顶层数组无法解析 | `dump`/`stringify` 能输出顶层数组，但 `parse` 只认键值块，导致「能写不能读」（如发信历史这类对象数组） | ✅ 已修 | ✅ 已修 | ✅ 已修 | ✅ 已修 |
| 词中 `@` 截断邮箱 | `a@b.c` 被切成 `a` + `@` + `b.c`，后半段丢失，邮箱静默损坏为 `a` | ✅ 已修 | ✅ 已修 | ✅ 已修 | ✅ 已修（原本即正常） |
| `@version` 未处理 | `@version v1` 被当片段名，吞掉后续内容，解析结果为空对象 | ✅ 原生支持 | ✅ 已修 | ✅ 已修 | ✅ 已修 |
| 顶层数组 dump 格式 | Lua 的 `dump` 顶层数组输出 `1: { }` 键值形式，而非 `[ ]`，与 load 不对称 | — | ✅ 已修 | — | — |

## 二、未完成项（按优先级）

### P0 — 阻断性

- [x] **Lua：`@version v1` 未处理**
  - 现象：`Sml.load('@version v1\naddress { state: NY }')` 返回**空表**，内容全丢
  - 参照：C 实现已在 `parse_block` 的 `T_AT` 分支加特判（`strcmp(fname,"version")==0`）；
    JS 实现已在 `@` 分支加版本校验。Lua 已同样处理
  - 影响：`showcase.sml`（含 `@version v1`）在 Lua 下解析不出 `address` 字段
  - 验证：`soupx lua/main.lua showcase.sml` 实测 `address` 块完整解析；`@version v2` 报错（对齐 C/Rust）

### P1 — 一致性

- [ ] **建立跨实现一致性测试套件（conformance suite）**
  - 现状：各实现的验证脚本是临时文件（`_probe_c.c`、`_probe_lua.lua`、`_probe_js.mjs`、
    `_verify_showcase.*`），未纳入仓库、未进 CI
  - 目标：一份共享的用例集（SML 文本 → 期望值），四实现各自跑一遍并比对
  - 建议位置：`tests/conformance.{sml,json}` + 各语言一个 runner

- [ ] **系统性核对各实现的特性支持矩阵**，至少覆盖：
  - `include "path"` 指令（Rust 有 `parse_file`/`resolve_includes`；C/JS/Lua 支持情况待确认）
  - 转义集（Rust 与 C 支持 `\0 \n \t \r \" \\ \uXXXX`；Lua 额外支持 `\a \b \f \v \'`）
  - 数字格式（整数/浮点/科学计数法的识别是否一致）
  - `$env.VAR` 内联（已验证 JS 正常；其余实现待测）
  - 错误信息文本与失败行为是否一致

- [ ] **顶层标量的行为未统一**
  - Rust：顶层标量不可往返（SML 顶层需为容器），已在代码注释与 README 说明
  - 其余实现：行为未定义，可能静默产生异常结果
  - 待办：统一为「显式报错」并写入文档

### P2 — 健壮性

- [ ] **错误边界行为不一致**，需逐一核对齐：同名键冲突（Rust 提升为数组）、
      未闭合括号、空文档、只有注释的文档、重复片段名
- [ ] **Lua/JS 缺少单元测试**（Rust 有 32 个测试；Lua 与 JS 目前只有临时探针）
- [ ] **C 实现的 `sml_dump` 输出风格**与 Rust `to_sml` 是否逐字节对齐，待比对

## 三、如何验证（当前可用的临时命令）

```bash
# Rust
cd rust && cargo test                      # 32 passed

# C
gcc -O1 -std=c99 -Wall -Ic -o _probe_c.exe c/sml.c _probe_c.c && ./_probe_c.exe

# JS
node _verify_showcase.mjs

# Lua（需 luajit 或 soupx）
luajit _verify_showcase.lua
```

上述 `_probe_*` / `_verify_showcase.*` / `_t*.c` / `_t*.lua` / `_t*.mjs`
均为**临时文件**，待一致性测试套件（P1）落地后应清理或正式化。

## 三·五、外置扩展机制（0.6.1 新增，进行中）

目标：让下游注册自定义 `@指令` 与自定义契约类型，**不必改动 crate 源码**，
也不必把方言名字写进 SML 规范层。根因见下「方言为什么会诞生」。

### 已落地（Rust）

- [x] `sml-parse::ext`：`Directive` trait / `Outcome`（Discard 元数据块 · Emit 展开）/
      `Diagnostic`（弃用警告）/ `DirectiveTable` / `ParseOptions` / `parse_with()`
- [x] `sml-contract::ext`：`TypeCheck` trait / `ContractExt` / `TypeSpec::Ext` /
      `FieldSpec.ext`（校验器随字段规格走，校验期不查全局表）
- [x] `swsml` 门面导出 `sml::ext` / `sml::contract_ext`
- [x] 版本：`sml-parse` / `sml-contract` → `0.1.0-alpha.2`（后者含不兼容改动：
      `TypeSpec` 加 `Ext` 变体、`FieldSpec` 加 `ext` 字段）

### 待办（按序）

- [x] **`Modifier` 扩展点**：`items_max`（数组元素数量上限）之类。
      必须**新起名字**，不能复用 `max` —— `max` 在 Rust 侧是数值上界，
      `sml-contract/src/lib.rs` 明写"数组不参与 min/max 校验"；PVACIS 把 `max` 当数量上限，
      同形不同义，复用会造成两端语义分叉。
      （已实现：解析期 `apply` 改写规格 + 校验期 `check` 回调；数组字段同样会跑回调）
- [x] **JS 侧同步**：`js/sml.mjs` 加 `parse(text, { directives, types, warnings })`，
      与 Rust 侧同样的语义（外置类型判定次序 `@type` > 外置 > 契约引用）。
      已同步三处副本：`site/static/`、`site/public/`、`editors/vscode/src/vendor/`。

### 同步时暴露的既有跨实现差异（记下，不在扩展点里擅自改）

- **未注册指令**：Rust 侧**报错**；JS 侧落进「片段定义」分支**静默收下**（`@form F { }`
  变成名为 `form` 的片段，不进主树也不报错）。两端行为不同 —— 收敛前先决定以哪侧为准。
- **数组类型写法**：Rust 侧 `[str]`；JS 侧 `parseFieldSpec` 要求首 token 是 word，
  故 `[str]` 会报"字段类型期望标识符"，只认 `array [ str ]`。
- **数组元素的逐元素校验**：Rust 侧对 `[image]` 逐元素调外置校验器；
  JS 侧 `valueMatchesType` 的 array 分支未逐元素校验外置类型。
- [ ] 清理 `sml-parse` 的 8 个既有 unused import 与 `when` 关闭时的 2 个 dead_code
- [ ] 教科书补一节「外置扩展」+ `CHANGELOG.md`

### 方言为什么会诞生（根因，勿忘）

PVACIS 想要的是「**给文档挂带类型的元数据块，且不进主数据树**」。这是**通用需求**，
不是 PVACIS 独有。因为 swsml 没有这个口子，它才 fork 出自己的 Go 子集解析器
（`Backend/internal/pkg/smlform/smlform.go`），于是方言诞生、两端文档互不相通
（`@form Name { }` 在 swsml 里会被判为"缺少片段体"而整篇失败）。

另注：`@form Name { }` 这类**位置参数形式**自 v4 起已被刻意废弃
（`sml-parse/src/parser.rs` 的注释详述了原因：与"拼错的指令"同形，会静默吞掉块内容）。
外置指令可显式开启 `positional()` 兼容既有方言文档，但会产出弃用诊断。

### 其它实现的处置（2026-09-18 定）

| 侧 | 是否需要改 | 结论 |
|---|---|---|
| **Go（PVACIS `smlform`）** | **暂不需要** | 它是独立子集解析器，继续可用。若要将同一份方言定义两端共用，有两条路：<br>① 在 Go 侧实现**同样的注册接口**（对称但重复）<br>② 走 C-ABI —— **不可行**：`Directive`/`TypeCheck` 是 trait 对象，C-ABI 传不了<br>③ （备选）给 C-ABI 加一层"按名注册的函数指针表"，复杂度高，暂不做 |
| **C（纯 C99 `c/sml.c`）** | **不建议改** | 该版本已知"带名块堆损坏"，此前已建议弃用；再给它加扩展点不划算。新能力应走 `c/sml_rs.h`（桥接 Rust cdylib） |
| **C（`c/sml_rs.h` 桥接）** | **受限于 C-ABI** | 无法暴露外置扩展。C 调用方只能用**无扩展**路径；方言文档需注明"需 Rust 侧 + 注册扩展" |
| **C++（`cpp/sml.cpp` / `sml_rs.cpp`）** | 同 C | 同上 |
| **Lua** | 契约都还没实现 | 优先级最低 |

### 从 Go 解析器可并入 swsml 的项（对比结论，2026-09-18）

| 能力 | Go 侧 | 处置 |
|---|---|---|
| token 携带行号 | `smlform.go:63-67` | **高价值**，待办：给 `Tok` 加 `line`（注意 include 跨文件行号归属） |
| 标量/列表双形态归一 | `toStrSlice` :829-849 | 下沉到 `sml-value`，零风险 |
| 未知指令跳过整行 | :583-588 | 仅作 **opt-in**，默认保持严格（防静默丢内容） |
| 未知 `{}` 块整块跳过 | :591-608 | 同上 |
| 同行多裸词聚合 | :226-237 | 同上 |
| 未闭合字符串容错 / `min`/`max` 静默忽略 | :136-138 / :442-453 | **不并入**（与既有审计项冲突） |

---

## 三·六、安全审计（2026-09-18，三轮 agent 扫描 + 修复）

**已修（全部已提交）**

- 严重：JS 原型污染（**5 处入口** —— include 命名空间路径 / 文档键 / 片段合并 /
  契约默认值 / 部分引用）、C include 路径穿越、C 带名块 use-after-free
- 高：C 与 C++ 的递归深度守卫（与 Rust `MAX_VALUE_DEPTH=128` 同口径）、
  C 的 `json_to_value` 深度、JS 内联正则源长度与量词上界、C include 规范化改动态分配
- 中：C/C++ 的 include 越界校验改 **fail-closed**（原先规范化失败即跳过校验）、
  LSP `Content-Length` 上限（16MiB）、C 侧 include **全局展开次数**上限
  `MAX_INC_EXPANSIONS=256`（挡菱形包含的指数级文件读取）
- 凭据：`site/.baidu.env.example` 曾含**真实**百度翻译 APPID/KEY，已改占位符，
  并用 `git filter-repo --replace-text` 从**全部历史**清除。
  ⚠️ **那对密钥仍需在百度控制台作废轮换**（已泄露，清历史不等于失效）
- PII：`site/serve_local.py`、`rust/test_smlconv.py`、`rust/osv_check.py`、
  `examples/slint/slint_check/Cargo.toml` 里硬编码的本机绝对路径
  （`C:\Users\<用户名>\...`）已改为基于 `__file__` 推导
- dead code 信号（都查实并处理）：`rust/tests/emit.rs` 那条陈旧 `#[ignore]`
  （各后端其实早已加深度保护，放开后直接通过）、`sml-include` 的 `MAX_VALUE_DEPTH`
  死导入、JS 的 `PATTERN_MAX_LEN` 死常量（定义在 `parse()` 内而校验处写死 4096）

**未修（低危，留观）**

- Lua 侧无深度守卫（Lua 栈溢出由 `pcall` 接住，不崩溃）
- `lua/lib/sml.soup` 无危险键概念（Lua 无原型链，不构成同类漏洞）

## 三·七、徽章与跨站调用（2026-09-18）

- `tools/gen_badge.py`：**手写**徽章 SVG 生成器（零第三方依赖，走官方 API）
  - `badge/swsml.svg` —— crates.io **多数据合一**（版本 + 下载量 + 版本数，带雪花图标）
  - `badge/gitee.svg` —— Gitee **stars + forks 合一**
  - 同时同步到 `site/static/badge/` 供官网引用；数据是**快照**，刷新时重跑脚本
- **跨站调用**：`https://sml.swebase.cn/lib/sml.mjs`（连同 `lib/sml.wasm`、
  `lib/sml-rs.js`、`lib/sml-verify.js`），CORS 由 `site/static/_headers` 放行，
  `/lib/` 入口由 `build_site.py` 每次构建从 `js/sml.mjs` 同步。
  注意：只暴露 `sml.mjs` 的话调用方拿不到 wasm 对照能力，故四个文件必须一起暴露

---

## 四、已完成

- [x] Rust：顶层数组解析（`parse_impl` 支持 `[`/`{`/键值三种顶层形态）
- [x] Rust：词中 `@` 保留（仅词首为片段标记）
- [x] Rust：新增 4 个回归测试（顶层数组、标量数组、顶层对象、空数组）
- [x] Rust：新增 3 个回归测试（邮箱裸词、邮箱往返、片段定义）
- [x] C：同 Rust 两处修复 + `@version` 支持
- [x] JS：顶层数组解析 + `@` token 化（此前完全不识别 `@`，片段定义失效）+ `@version` 支持
- [x] Lua：词中 `@` 保留 + 顶层数组解析 + `dump` 顶层数组格式对齐
- [x] README：修正错误的片段示例（块内裸写 `&base` 不展开，正确写法是 `key: &base`）
- [x] README：补充顶层形态与词中 `@` 规则说明
- [x] `showcase.sml`：可解析的 SML 优势展示文件（已用 JS/C 验证通过）
- [x] Rust：**契约（Contract）**实现 —— `@contract` 定义、`@is` 应用、
      类型/枚举/区间/数组元素校验、默认值填充、未知契约报错
- [x] Rust：契约测试 12 项 + showcase 验证 1 项（`tests/contract.rs`、`tests/contract_showcase.rs`）
- [x] README：新增契约章节
- [x] `showcase_contract.sml`：契约能力展示（已用 Rust 验证通过）
