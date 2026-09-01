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
| C | ⏳ 待实现 | 需移植 `Contract` / `FieldSpec` / `TypeSpec` + `apply_contract` |
| JS | ⏳ 待实现 | 同上 |
| Lua | ⏳ 待实现 | 同上 |

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
