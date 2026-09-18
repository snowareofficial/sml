# SML 待办（跨实现一致性）

本文档记录 SML 多语言实现的**已知缺陷与未完成项**，以及**新特性的跨实现的落地计划**。

## 编辑器插件

已交付 VSCode 扩展：`editors/vscode/`（高亮 + 诊断 + 补全 + 悬浮 + 格式化）。
桥接层 `src/sml-parse.mjs` 复用仓库的 `js/sml.mjs`，保证与语言实现行为一致。

依赖的 JS 侧增强（已完成）：token 记录字符偏移 `pos`、错误携带位置、
新增 `offsetToPosition(text, offset)`、`parseSafe` 返回 `pos` / `position`。

- [x] **契约校验接入编辑器**：JS 侧契约**已实现**（`js/sml.mjs` 有契约表 / `@is` 处理 /
      默认值填充），故语义诊断的**前置条件已具备** —— 此处原写「契约仅 Rust 支持」，
      2026-09-18 更正。**仍未验证**：编辑器是否已把语义错误呈现出来（属 VSCode 侧接线）
- [ ] **一次报出多条错误**：当前解析器遇错即停，编辑器只显示第一条
- [x] **LSP server**：`editors/lsp/server.mjs` 已实现 diagnostics / completion /
      definition / hover —— 2026-09-18 更正勾选状态（此前未勾但实际已做）
- [x] **跳转到契约定义（VSCode 侧已补齐，2026-09-18）**：`editors/vscode/src/sml-parse.mjs`
      新增 `findDefinition(text, name, kind)`，`extension.js` 注册 `DefinitionProvider` ——
      `@is Server` → `@contract Server`，`&base` → `@base { }`；中文名可跳（`\p{L}` 系列），
      邮箱 `a@b.c` 不会被误判成指令（后顾断言）。顺带修 `collectFragmentNames` 保留名单不全
      （`@when` / `@for` / `@feature` / `@type` 曾被当成片段名塞进补全列表）
- [ ] **悬停显示契约展开结果**：展示默认值填充后的最终结构（现有 hover 只显示关键字说明）

## 新特性：契约（Contract）

契约是可选 schema 层，为块提供结构体约束、枚举、默认值、取值区间。
语法与语义见 `README.md` 的「契约」章节与 `showcase_contract.sml`。

| 实现 | 状态 | 说明 |
|---|---|---|
| Rust | ✅ **已实现** | `@contract` / `@is` + 校验 + 默认值 + **组合** + **严格模式/loose**；22 个测试通过 |
| C | ✅ **已实现** | `c/sml.c` 有 `ccontract` / `apply_contract_rec` / `parse_contract_body`（此前误标为"待实现"，2026-09-18 更正） |
| C++ | ✅ **已实现** | `cpp/sml.cpp` 封装同一套能力 |
| JS | ✅ **已实现** | `js/sml.mjs` 有契约表、`@is` 处理与默认值填充（此前误标为"待实现"，2026-09-18 更正） |
| Lua | ⏳ 待实现 | **唯一未落地的一侧**。2026-09-18 用户决定：**允许引入 native** —— 即允许 Lua 侧走 C-ABI 绑定 Rust 的契约实现，而不要求在纯 Lua 里再重写一遍。选型时优先走绑定（`lua/` 侧已有链接 C 的现成先例），移植只在绑定不可行时才考虑 |

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
- 凭据：站点曾把**第三方翻译 API 的 APPID/KEY 明文**写进凭据模板并入库。
  处置：删除凭据模板与全部调用该 API 的脚本（6 个），
  并用 `git filter-repo`（`--path` 删文件历史 + `--replace-text` 替换明文
  + `--replace-message` 净提交信息）从**全部历史**清除。
  ⚠️ **那对密钥仍需去服务商控制台作废轮换**（已泄露，清历史不等于失效）
- PII：`site/serve_local.py`、`rust/test_smltools.py`、`rust/osv_check.py`、
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

## 三·八、清仓库欠账（2026-09-18 完成）

- **警告清零**：workspace **19 → 0**
  - `cargo fix` 清 unused import / unused mut（sml-value / sml-lex / sml-include / sml-parse / c_abi）
  - 3 处「赋值后未读」：sml-regex 量词初值（改延迟初始化）、sml-include `parse` 的 `rest` 死赋值
  - 2 处死代码：smltools 的 `Format::name` **改为用起来**（`--to` 报错里的格式列表原先手写，
    与 `name()` 两处维护 —— 0.6.1 加 `html` 时就得人工同步两处；现由 `Format::ALL` 生成）；
    `rust/src/lib.rs` 的 `tmpdir` 恢复（见下）
- ⚠️ **根因/教训**：`rust/src/lib.rs` 的 `mod tests` **漏写 `#[cfg(test)]`**，
  导致非测试构建下模块仍被编译，其内部 import 与辅助函数被误报 unused/dead_code；
  `cargo fix` 据此删掉了 `tmpdir` —— 而它被 10+ 处测试调用，删完 `cargo test` 直接编译失败。
  **今后：看到 `mod tests` 缺 cfg 要先补，再动 cargo fix。**
- **smltools 标题推断**（旧注释与实现各说各话）：改为 `--title` > 文档顶层 `title` 字段 >
  文件名 stem > `"doc"`，并传入真实解析结果（原先两个调用点都传 `&Value::Null`，
  文档标题永远读不到）；删除永不触发的 `__name` 分支与两处错误注释；
  `sanitize_filename` / `sanitize_section` 保留 Unicode 字母数字
  （原先只认 ASCII，`我的长篇小说` → `______.md`）
- **测试栈溢出**：`emit_depth_limit_does_not_overflow` / `value_deep_drop_does_not_overflow`
  改在 256MB 栈线程里跑 —— Rust 测试线程默认仅 2MB，构造 5 万层嵌套时**测试自身**先溢出，
  会掩盖真正要验证的后端行为。改用大栈后可区分「测试资源不足」与「实现漏保护」，
  实测后端深度保护到位（43 passed / 0 ignored）；全量测试退出码 `0xC00000FD` 已消除
- **残留清理**：`rust/tests/_tmp_probe.rs`、`rust/tests/_ec_out.txt` 已删
- **ISSUE 第一节判据更正**：原判据有误，已按源码核对结果改写

**剩余待办（按建议顺序）**

1. 丙 · 跨实现一致性套件（先小范围：`typed-block` / 括号词法 / `@is type(X)` / 量词对象式 / 数字字面量）
2. 丁 · 编辑器接契约语义诊断（前置条件早已具备，属"白捡的果子"）
3. 乙 · 功能缺口（EPUB 直出、章节分页 + TOC、语义部件 figure/admonition/footnote…）

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
- [x] JS（2026-09-18）：**契约 `[T]` 数组类型简写**曾未实现（照 README 写
      `tags: [str] optional` 会在编辑器里**假报错**）；**嵌套数组值**曾把
      `m: [ [ a ] ]` 静默解析成 `{"m":[],"[":"a"}`（数据损坏、不报错）。
      两处均已修，并与 Rust 对齐；探针 `js/_probe_contract_types.mjs`（19 个用例）
      是 W5 一致性用例集的种子。副本同步：扩展 `vendor/` + 站点 `static/`、`public/`
- [x] VSCode（2026-09-18）：跳转到定义（`@is X` → `@contract X`、`&f` → `@f { }`）
      + 悬浮显示**applyDefaults 之后**的契约结构（取不到实例时明说「未找到」，不编）

---

## 五、剩余工作分解（2026-09-18）

按「**能独立验证**」切分，每条都写清文件范围（并行时不互相踩）、验收标准、依赖。
✋ = 需要人工判断（动公开语义 / 跨多实现）；🤖 = 范围封闭、验收可自动化，适合派 agent。

| # | 任务 | 文件范围 | 验收标准 | 依赖 | 并行 |
|---|---|---|---|---|---|
| ~~**W1**~~ ✅ | ~~`sml-regex` 量词 off-by-one（`+`/`?`/`*` 全部：原子被强制消费一次）~~ **已完成 2026-09-18**：另发现并修掉 `^...$` 锚点松判（`^conf\.sml$` 会匹配 `conf.sml.bak`）；`sml-regex` 升 `0.1.0-alpha.3`；workspace 467 通过 / 0 失败 | `rust/sml-regex/src/lib.rs`、`rust/tests/security.rs`、`rust/AUDIT_REPORT.md`、`CHANGELOG.md` | ✅ 9 个新单测 + `security.rs` 改按正确语义断言 + ReDoS 预算测试仍通过 | 无 | ✅ |
| ~~**W2**~~ ✅ | ~~hover 显示**默认值填充后**的契约结构~~ **已完成 2026-09-18**：`contractHoverMarkdown` 组装两段（声明 + 解析器应用契约后的实例），无实例时明说「未找到」；顺带修 JS 契约 `[T]` 简写（曾对合法 SML 假报错） | `editors/vscode/src/{extension.js,sml-parse.mjs}`、`js/sml.mjs`、`editors/vscode/README.md` | ✅ node 探针实测：`port: 5432`/`tls: false` 确实被填进实例；非契约名返回 null；嵌套块正确退化为「未找到」 | 无 | ✅ |
| **W3** | 顶层标量行为四实现统一为**显式报错** | `c/`、`cpp/`、`js/sml.mjs`、`lua/`、`rust/` + 文档 | **口径已定（2026-09-18）**：四端一律显式报 `E-PARSE-008`，**码必须一致、文案不要求逐字一致**；禁止静默（当前 JS/C/Lua 行为未定义）。各端各配一条单测 | 需先调研取证 | 🤖 |
| **W4** | C `sml_dump` 与 Rust `to_sml` 逐字节比对 | `c/`、`rust/tests/` | 一批用例输出逐字节相同；有差异则逐条列明并判定是否可接受 | W3 之后（同批文件） | 🤖 |
| **W5** | 跨实现一致性套件（conformance） | 新 `tests/conformance/`（用例集 + 各语言 runner） | 一份共享用例被 Rust/C/C++/JS/Lua 各跑一遍，结果一致；临时探针脚本（`_probe_*`/`_verify_showcase.*`）收编后删除 | W3/W4 之后 | 🤖 |
| **W6** | Lua 侧契约（走 native 绑定） | `lua/`、`c/` 的导出面 | Lua 能校验 `@contract`/`@is` 并回填默认值，与 Rust 行为一致（复用 W5 用例集） | W5 的用例集 | ✋ |
| **W7** | 解析器一次报多条错误 | `rust/sml-parse/src`（错误收集）、`js/sml.mjs`、`editors/vscode/src` | 同一文档的多个错误一次全部返回；旧 `parse()` 行为不变（只加新 API） | 无 | ✋ |
| **W8** | Zed：填 `extension.toml` + 编译验证 grammar | `editors/zed/` | `tree-sitter generate && tree-sitter parse test/parse/*.sml` 无 `ERROR`；`extension.toml` 指向可用 grammar | 需 tree-sitter CLI（联网下载） | 🤖 |
| **W9** | 残余风险：Miri / 安全门禁 / 非 Rust 实现扫描进 CI | CI 配置、`rust/{miri_check,osv_check}.py` | CI 里跑得起来，失败能挡住合并 | 无 | 🤖 |
| **W10** | 错误码**落地到五端**：`errors/codes.sml` 已定 **137 条**码（W11 已录全）。**Rust ✅ / JS ✅ / C ✅ / C++ ✅ / C-ABI ✅ 已全部带码**（见 `errors/README.md` 的「码的落地进度」）；**Lua ❌ 本仓库做不了**（源码不在本仓库，见下）；`smltools` 的部分输出仍只有文案 | 剩余：Lua（须回 Soup 工程）、`smltools` 的部分输出 | 四端各有一份「触发条件 → 期望码」用例，**交集部分逐一同码**：`rust/tests/error_codes.rs`、`js/probe-error-codes.mjs`、`c/test_codes.c`、`cpp/test_codes.cpp`；生成链路 `errors/gen_codes.py`（含「手写码字面量必须在表里」的反向校验） | 无 | ✋ |

> **Lua 为什么卡住（2026-09-18 查证）**：`lua/` 下只有 `main.lua`（demo 入口）与
> `lua/lib/sml.soup`。**`lib/sml.soup` 是编译产物，本仓库里没有它的源码**，
> `lua/MANIFEST.json` 也没写源在哪。记忆里那个 Soup 工程路径
> （`~/Downloads/lua-5.5.1/lua`）**已不存在**。
> 所以 Lua 带码必须先回到 Soup 工程拿到 sml 的 `.tl` 源码、用 `soupc` 重编再回填 `.soup`，
> **不是本仓库内能完成的事**。要动它之前先去找 Soup 工程的实际位置。
| ~~**W11**~~ ✅ | ~~错误码总表**录全**（46 / 约 120）~~ **已完成 2026-09-18**：按语义条件清点五端 + `smltools`（CLI / 迁入格式 / lint / 定制）+ 编辑器，**46 → 135 条 / 14 个领域**，分四层（语言层 / 宿主绑定层 / 工具层 / 编辑器层）；新增 `DERIVE` / `MIGRATE` / `CLI` / `LINT` / `EDITOR` 五域；`coverage` 改「全量」；`status` 的语义（**行为是否已实现**，与是否带码无关）在表头明确定义；清点范围与「该报错却静默」清单落进 `errors/README.md` | `errors/codes.sml`、`errors/README.md`、`site/content/{zh,en}/errors.md`、`site/static/site-tools.js` | ✅ 135 条 id 唯一 / 领域已声明 / 级别与码前缀一致 / 字段无缺 / **0 处转义或插值损坏**；`gen_json.py` 通过并重生成 `errors.json`；官网领域筛选按数据生成，无需改模板 | 无 | ✅ |
| **W12** | 错误码查询工具接上**搜索**：官网 `/errors` 目前是关键词过滤，教科书 `/search` 已可搜 | `site/static/site-tools.js` | 两处都能按码与前缀（`E-CONTRACT-*`）检索；结果可深链（`/errors/#E-PARSE-008`） | 无 | 🤖 |
| ~~**W13**~~ ✅ | ~~**安全**：C++ 深度守卫被绕过 + C 错误路径越界写~~ **已完成 2026-09-18**，且**比原描述更严重**：① C++ 子块直接递归、不增长深度计数；② **修①时发现光补「走受限入口」不够** —— 守卫超限后把深度复位为 0，而复位不会让栈帧退回，外层又从 0 往下钻（每 128 层一轮反复压栈），10 万层照样崩；③ **C 有同一个问题**（审计曾据「parse_block 是带守卫的 wrapper」判定 C 无此洞，实测 10 万层块嵌套段错误）；④ C 的越界写不止那两处 —— `sml.h` 明写 `err` 可为 `NULL`，而所有 `snprintf(errbuf, ...)` 在 `NULL` 时都是空指针写（共 21 处 + 2 处 dummy 缓冲）。**修法**：C++ 加 `aborted` 中止标志、各层循环 break（与 Rust 当年靠 `Result` 传播 `?` 同一思路）；C 复用既有 `ps->failed` 同样 break；C 的错误写入全部收敛到 `set_err()` 助手（缓冲区为空则一个字节不写） | `cpp/sml.cpp`、`c/sml.c`、`cpp/test_limits.cpp`、`c/test_limits.c`、`cpp/build_verify.py`、`c/build_check.py`、`c/Makefile` | ✅ 新增两侧回归用例：10 万层块/数组/交替嵌套**报错返回而不崩**、100 层照常解析、`err=NULL`/`errsz=0` 四条路径一个字节都不写。`c/build_check.py --run` 与 `cpp/build_verify.py` 全绿（后者含 contract/comments/rs-bridge 对照） | 清点记录（`errors/README.md`） | ✅ |
| **W17** | C 的**嵌套数组被静默丢弃**（数据正确性） | `c/sml.c`（`parse_array`） | `parse_array` 不递归：元素只处理块/字符串/裸词，遇到 `[` 直接跳过 → `a: [[1]]` 之类被静默错解（与 JS 早先修掉的「嵌套数组静默截断」同类）。验收：嵌套数组与 Rust/JS 行为一致，并配回归用例；修好后可把深度用例补回 `c/test_limits.c`（现在那里写了一行注释说明为何缺席） | 无 | ✋ |
| **W14** | JS 空键列表抛 `ReferenceError` | `js/sml.mjs`（及四份副本） | 报告函数是 `parse` 的**局部量**，空键列表分支引用不到它 → 用户看到宿主异常而非 `E-INCLUDE-005`。验收：该分支抛带码的正常错误 + 一条回归用例 | 无（但与 W3 同改 `js/sml.mjs`，需串行） | 🤖 |
| **W15** | Lua 补深度上限，与 `E-LIMIT-001` 对齐 | `lua/lib/sml.soup` | 深嵌套不再耗尽 C 栈，改为显式报码；上限与其它端一致（128 层）。验收：深嵌套用例返回错误而非崩溃 | 无（但与 W3 同改 `lua/`，需串行） | 🤖 |
| **W16** | 「静默清单」逐条判定并回填码表 | `errors/README.md` 的静默清单、各实现、`errors/codes.sml` | 清单里每条判定为「改成报错（给码）」或「写进规范、明确允许静默」，判定结果回填码表的 `status` 与规范文档。**先出判定表再动实现** —— W3 只是其中的顶层标量一条 | W11 的清单 | ✋ |
| **W18** | ⚠️ **C++ `@include` 展开会毁掉文档**（既有缺陷，非 W10 引入）：`@include "b.sml"` 的目标文件字段**全部丢失**，只留一个垃圾键（形如 `include = "include"`）。根因已定位到行：把目标 token **插到路径 token 之前**之后又执行 `st.i++`，恰好跳过插入段的**首个 token**（嵌套的 `@`），于是 `include` 退化成裸块键，把 includer 后面的字段全当参数吞掉。**附带**：环检测的 `include_stack` 是 push 完立刻 pop，**永远是空的** → 自包含/互包含根本检测不到（`E-INCLUDE-002` 实际不可能触发） | `cpp/sml.cpp` | ① `@include` 链式包含后，两侧字段都在；② 自包含 / 互包含给出明确错误而不是死循环或静默。**⚠️ 修索引前必须先定环检测语义**：off-by-one 恰好压住了无限展开，只改索引会把它变成**真死循环**；而「严格见过即拒」会误伤合法的菱形包含，正确做法是维护**链栈**（只判当前路径） | 无（但它是 W10 期间实测发现的，与 W10 无关） | ✋ |

**建议顺序**：W13（安全，最优先）→ W16（静默清单判定，是 W3 的前置）→ W3 → W4 → W5 → W6 → W7 → W9。
W14 / W15 范围封闭，但与 W3 改同一批文件（`js/sml.mjs`、`lua/`），**必须与 W3 串行**；
W8 等 tree-sitter CLI（联网），W12 完全独立。
W10 的剩余部分（C/C++ 带码）与 W4（C `sml_dump` 比对）改同一批 `c/` 文件，**建议串在一起做**；
Lua 那一份要动 `lua/lib/sml.soup`（编译产物）之前，先解决「源码根本不在这仓库」这个前提
（见 W10 那一行下面的说明）。
W3/W4/W5 共享同一批文件（`c/`、`js/`、`lua/`），**必须串行**；其余两两之间无文件重叠，可并行。

---

## 六、发布纪律（2026-09-18 用户定，开工/发版前先看这一节）

- **任何对外发布前必须先告知用户**（crates.io `cargo publish`、站点发布、
  仓库打 tag/release 都算）。不要自行发版 —— 用户要「发布前说一声」。
  落地含义：`CHANGELOG.md` 的 `## [未发布]` 可以随时累积，但**把它变成版本号那一步
  必须等用户点头**；`Cargo.toml` 里的版本号也不要在没通知的情况下改。
- **VSCode 扩展（VSIX）上架市场：暂缓**。理由：上架流程麻烦（publisher 资质、
  人工审核、每次发版都要重走一遍）。现状保持不变 —— `editors/vscode/` 用
  `npm run package` 本地打包 `.vsix` 后手动安装即可；README 里对「未上架」的说明
  是**有意为之**，不是待补的短板。
- 相关：Zed 扩展的 grammar 要拆独立仓库才能正式发布（见 `editors/zed/README.md`），
  该拆分同属「发布」动作，同样先告知用户再动。
