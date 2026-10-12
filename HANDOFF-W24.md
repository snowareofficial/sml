# HANDOFF-W24：契约叠加 + 契约映射 API + 错误定位

> 给下一个 AI / 会话：**读完本文件即可接手**，细节判据都在 `TASK-agent-w24.md`（唯一事实来源）。
> 仓库级背景（纪律、坑、规格）在 `HANDOFF.md` 与 `TODO.md`，本文件不重复。
> 写于 2026-10-12，任务 A 提交 `dae3be1` 之后。

## 一句话目标

契约从「一个块一份」变成「一个块多份」—— `@is a, b` / `@is a b`（逗号可省）/
块级 `conta contb item { }` 三写法等价；并新增「契约映射 API」让程序能问
「**这个实体应用了哪些契约**」（映射本身用一份 SML 表示）。

## 当前进度

| 任务 | 状态 | 证据 / 备注 |
|---|---|---|
| **A 错误定位** | ✅ 完成，已提交 `dae3be1` | 全量 `EXIT=0 / SUITES=47 / PASSED=564 / FAILED=0`；判别实验 HEAD 版 2 failed；`X { a: B::C }` 报 `（第 1 行第 9 列）`，前置两行指令仍落第 3 行 |
| **B 契约映射 API** | ✅ 完成，已提交 `57a9655`（feat）+ `37a96ff`（fix: CLI 大栈） | 全量 `EXIT=0 / SUITES=48 / PASSED=574 / FAILED=0`；判别实验 HEAD 版编译红。**顺带修了 CLI 爆栈**（main 挪 64MB 线程；解析器承诺 128 层但主线程 1MB 栈 70 层就溢）——详见 TASK-agent-w24 §4 执行记录 |
| **C 契约叠加** | ✅ 完成（Rust 侧） | 全量 `EXIT=0 / SUITES=49 / PASSED=585 / FAILED=0`；`contract_mixin` 11 条全绿；全仓扫描 30 个 .sml **零误伤**（唯一差异 = codes.sml 自身 JSON）。判据全序与四处执行差异见 TASK-agent-w24 §5 |
| **D 其它端响亮拒绝** | ✅ 完成（已提交） | 四端「响亮拒绝」多契约写法，绝不静默给树；`_rust_test.bat` `EXIT=0/SUITES=49/PASSED=585/FAILED=0`（零回归）；JS 副本已 `check_js_copies.py --fix` 同步且四份一致；C `build_check` 全绿、C++ `build_verify`（RS-BRIDGE 跳过）contract/comments/limits/codes 全 rc=0、Lua `run_check` 151/0；单契约路径逐字未变 |
| **D 其它端响亮拒绝** | ✅ 同上 | 新码 `E-CONTRACT-017` 已入码表（143 条）；js / c / cpp / lua 只加拒绝，绝不静默；改 js 后已跑 `tools/check_js_copies.py --fix` 且一致 |
| **E 契约推断** | ⬜ **下一个** | `smltools --infer-contract`；不猜 enum/default/min/max；golden 确定性 |
| **F 编辑器 / G 教科书** | ⬜ 后置 | 需用户放行 `editors/**` / `site/**` |
| **H README 对照示例** | ⬜ 随时可做 | 纯新增；JSON/YAML/SML 三栏；不许夸大 |

**建议顺序**：B → C → D（与 C 同期）→ E；F/G 等用户放行；H 穿插。

## 已定裁决（用户拍板，不许翻案；全文见 TASK-agent-w24 §2）

- **D1** `@is` 后连续取裸词，遇 `:` / `{` / `]` / 行尾即停；逗号可省；每词必须命中契约表（否则 `E-CONTRACT-001` 带位置）。
- **D2** 块级：从首词起**连续命中契约表**的词都是契约，第一个未命中的是块名；契约词 ≥2 才叠加，单契约**逐字保持旧行为**（P1-3 红线：`server web prod { }`）。
- **D3** strict 优先；字段按名并集（顺序无关）；同字段 ty/default/min/max 不同 ⇒ `E-CONTRACT-016`。
- **D4** 同块多条 `@is` 由「后者覆盖」改为**全部生效**（行为变更，记 CHANGELOG）。
- **D5** 映射：opt-in；`path: [Seg…]` Int 段=索引 / Str 段=键；顶层 `path: []`；无契约不出现；按 path 排序；无时间戳；记 resolve 后的名字。
- **D6** 错误只加位置不改语义；`Display` 保持纯文案，出口统一换算行列。
- **范围**：**Rust 先行**（其它端只做响亮拒绝）；教科书等总体功能完成；映射用 SML 文本表示；`trace_contracts` / `contract_map` 命名已确认。

## 环境事实（本机实测，与旧文档不符处以此为准）

- `Cargo.toml` 在 **`rust/`** 下，不在仓库根。
- `CARGO_TARGET_DIR=<target-dir>`（**E 盘只剩 1.3 GB，链接 `sml.dll` 会 LNK1104**；D 盘 11 GB）。
- **必须 `-j 1`**：`swsml` 同产 rlib+cdylib，`--workspace` 下不同 feature 变体并发链接同一个
  `deps\sml.dll` ⇒ LNK1104。`cargo build --lib` 单线程能过是假象。
- **PowerShell 偶发 AMSI 崩溃**（`AccessViolationException`，命令不执行）⇒ 长命令写成
  `.bat`（**纯 ASCII**，中文会被 cmd 按 ANSI 读坏）再跑。Rust 测试直接用仓库根
  **`_rust_test.bat`**（`_` 开头不入库）。
- **测试基线**：`_rust_test.bat` ⇒ `EXIT=0 / SUITES=47 / PASSED=564 / FAILED=0`。
  每步之后条数只增不减、FAILED 恒 0。

## 铁律（全文见 TASK-agent-w24 §0，最常犯的几条）

- 不许自己发挥 / 不重构；每步先跑验证、贴证据再进下一步。
- 不 `git add -A`；只 add 明确改过的文件；不 `git push`。
- 任何一步实测与任务单不符 ⇒ **停下贴命令+输出问人**，不猜。
- `site/**`、`editors/**`（除 `editors/vscode/src/vendor/sml.mjs` 强制同步副本）本轮未授权。
- 码表唯一事实来源是 `errors/codes.sml`；加码后必须重跑
  `python errors/gen_codes.py` + `python errors/gen_json.py` + `--check`。

## 本轮登记未改（待用户裁决）

1. 词法器 `--` 行注释**不冲裸词**：`tokenize("a--b\nc: 2")` ⇒ `Word("ac")`（跨行吞词，既有 quirk）。
2. `E-LEX-*` 词法错误尚无位置（本轮只接了解析层；接法：lexer 内 `.at(pos)`）。
3. 文件入口（`parse_file` 等）不带位置：include 展开是跨文件 token 流，无单一原文 —— 有意为之（宁缺勿错）。

## A 留下的可用设施（B/C 直接受益）

- `sml_lex::tokenize_spans(text) -> (Vec<Tok>, Vec<Span>)`（`Span` = 字节区间，与 toks 一一对应）。
- `SmlError::at(pos)` / `.pos()` / `.suffix()`；`scan.rs` 的 `line_spans()` / `blank_of()`。
- `Parser::err_at(code, msg, idx)` —— C 的新错误码必须走它（自动带位置）。
