# TASK-agent-w24：契约叠加 + 契约映射 API + Rust 错误定位（Rust 先行，其它端后置）

> **这份文档是给「不需要判断力」的执行者写的。** 每一条都给了：**改哪个文件的哪一行**、
> **改成什么**、**跑什么命令**、**期望看到什么**、**不符怎么办**。
>
> 文档里的行号是 **2026-10-12** 的实测值；如果你读到的行对不上，**立刻停下**
> （说明文件已被人改过），不要凭猜。
>
> 目标一句话：**契约从「一个块一份」变成「一个块多份」** —— `@is a, b` / `@is a b` /
> 块级 `conta contb item { }` 三写法等价；并且程序终于能问「**这个实体应用了哪些契约**」
> （契约映射 API；映射本身也是一份 SML 数据）。
>
> 范围纪律（用户 2026-10-12 裁决）：**先把 Rust 做好，其它端放后面**；
> 教科书 / 官网内容**等总体功能完成再说**；本轮另加一条纯新增的 README 对照示例（任务 H）。

---

## 0. 铁律（违反就是失败，不需要讨论）

1. **不许自己发挥**：不重构、不「顺便优化」、不改与本任务无关的行、不调整格式。
   §2 的「设计裁决记录」是**写死的判据**，实现必须逐条符合，不许另创。
2. **不许 `git add -A` / `git add .`**：只 `git add <你明确改过的文件>`。临时探针文件（`_` 开头）**不要提交**。
3. **不许 `git push`**：提交完就停，推送由用户本人做。
4. **不许动**：`.codebuddy/`、`rust/qsm/**`、`Desktop\<private-repo>\`（私有资产库）、
   `.gitignore` 里「私有报送件 / 内部报告」那一段（安全护栏）；
   `site/**` 与 `editors/**` 的**功能改造**不在本轮授权（教科书后置；编辑器改造在任务 F，后置）。
   ⚠️ 唯一例外：`editors/vscode/src/vendor/sml.mjs` 是 `js/sml.mjs` 的**强制同步副本**
   （`tools/check_js_copies.py` 管着），任务 D 改 JS 后必须同步它，否则闸门红。
5. **一次只做一条**，做完一条就跑验证、贴证据，再进下一条。**不要攒着一起改**。
6. 任何一步的**实际输出与本文档不符** ⇒ 停下、把命令 + 输出贴出来问人，**不要猜着往下做**。
7. 临时探针文件一律以 `_` 开头（`.gitignore` 的 `**/_*` 会挡住它们）。
8. **不许改错误语义**：本轮只加「位置」，不改任何既有错误码的含义与触发条件
   （`E-PARSE-006` 的文案 / 跨端对齐是已登记的另一项议题，见 TODO.md「P1 — 一致性」）。
9. Rust 构建的**三个本机事实**（2026-10-12 实测，与 TASK-agent.md 记的旧值不符，以本节为准）：
   - **`Cargo.toml` 在 `rust/` 下**，不在仓库根 ⇒ 所有 cargo 命令先 `cd c:\Users\<user>\Desktop\sml\rust`。
   - **目标目录用 `<target-dir>`**（D 盘空闲 11.1 GB）。⚠️ 旧文档写的 `<target-dir>`
     已装不下：E 盘只剩 1.3 GB，链接 `sml.dll` 会 `LNK1104`。
   - **必须加 `-j 1`**：`swsml` 同时产 `rlib` 与 `cdylib`（`rust/Cargo.toml:73`），
     `--workspace` 下不同 feature 变体会并发链接到**同一个** `deps\sml.dll` ⇒ `LNK1104`。
     串行即绕开（`cargo build --lib` 不并发，所以它能过 —— 别被这个假象误导）。
   - **PowerShell 会偶发 AMSI 崩溃**（`AccessViolationException`，命令根本不执行）：
     长命令写成 `.bat`（**纯 ASCII**，中文会被 cmd 按 ANSI 读坏）再跑，见 §1 的 `_rust_test.bat`。

---

## 1. 开工前检查（照抄）

```bash
cd c:\Users\<user>\Desktop\sml

git status --short                  # 期望：空。不空 ⇒ 停下问人
python tools/check_private_assets.py    # 期望：私有资产未进入主库
python errors/gen_codes.py --check  # 期望：码表与生成物一致
python tools/check_js_copies.py     # 期望：四份副本一致（sha 与 js/sml.mjs 相同）
```

Rust 基线走仓库根的 `_rust_test.bat`（**纯 ASCII** 的临时批处理，`_` 开头不入库）：

```bash
c:\Users\<user>\Desktop\sml\_rust_test.bat
```

**实测基线（2026-10-12）**：`EXIT=0`、`SUITES=46 PASSED=549 FAILED=0`、无 `FAILED/panicked/error` 行。
后续每一步的 `cargo test` 都要与它比对：**条数只增不减、FAILED 恒为 0**。

**黄金参考（判据只能从这里抄，不许自己发明）**：

| 用途 | 位置 |
|---|---|
| 契约应用（单契约，照抄改造对象） | `rust/sml-contract/src/lib.rs:404` `apply_contract` |
| `@is` 读取 / 块结束应用 / 裸块 | `rust/sml-parse/src/parser.rs:823-846` / `:1057` / `:1091-1153` |
| `ParseOutput` 定义 | `rust/sml-parse/src/ext.rs:201` |
| `SmlError` 定义 | `rust/sml-codes/src/lib.rs:45-48` |
| 词法 `Tok` 定义 | `rust/sml-lex/src/lib.rs:23-39` |
| 码表（唯一事实来源） | `errors/codes.sml`（`E-CONTRACT-001..015` 已占用 ⇒ **新码从 016 起**） |
| 码生成器 | `python errors/gen_codes.py`（→ `rust/sml-codes/src/codes.rs`）与 `python errors/gen_json.py` |

---

## 2. 设计裁决记录（2026-10-12 用户裁决，**写死**）

**D1 — `@is` 多契约判据**：`@is` 后**连续取裸词**，遇到 `:` / `{` / `]` / **行尾**即停止；
逗号作分隔（**可省**：`@is a, b` ≡ `@is a b`）；**每个词都必须命中契约表**
（先按裸名查、再按当前 ns 前缀 qualify，与 `parser.rs:841-845` 同一判据），
否则报 `E-CONTRACT-001`（**带位置**，见任务 A）。`@is type(X)` 的括号形式对**每个词**
先做 `unwrap_type_call`（复用 `parser.rs:836` 的既有逻辑）。

**D2 — 块级多契约判据**：裸块首词（`parse_bare_block` 的 `key`）与其后的 args 中，
**从首词起连续命中契约表的词都是契约**；**第一个未命中契约表的词是块名**（写 `__name`），
其余词照旧进 `__args`。仅当**契约词 ≥ 2** 时才走叠加；**只有 1 个契约词时逐字保持旧行为**。
回归红线：P1-3 的 `server web prod { }` 在 typed-block 开启时仍得到
`__type: server` + `__name: web` + `__args: [prod]`。仅在 `typed-block` + `contract` 都开启时生效。

**D3 — 叠加合并语义**：
- **`strict` 优先**：参与叠加的契约里**任一**严格（`!allow_extra`）⇒ 整体按严格校验；
- **顺序无关**：字段按名取并集（`BTreeMap` 天然有序），结果不得依赖书写顺序；
- **冲突即报错**：同一字段名在两份契约里 `ty` 不同、`default` 值不同、或 `min`/`max` 不同
  ⇒ **`E-CONTRACT-016 契约叠加冲突`**（文案列出字段名与冲突的两个契约名）；
- 实现路径：构造**合并视图**（临时 `Contract { name: "a+b", fields: 并集,
  allow_extra: 全部允许才允许 }`），复跑既有 `apply_contract`
  ⇒ 回填 / 类型 / 组合递归全走老路，顺序无关性由并集预检保证。

**D4 — 多条 `@is` 也走叠加**：同一块内多条 `@is`，从「后一条覆盖前一条」
（TODO.md §契约叠加的实测记录）改为**全部生效**（与 `@is a b` 同义）。**行为变更**，
必须写进 CHANGELOG「变更」段并配判别实验。

**D5 — 契约映射 API**：
- `ParseOptions::trace_contracts(bool)` 开启；**不开启时 `ParseOutput` 与旧版逐字一致**
  （`contract_map: None`）—— 这条有测试钉着；
- 结构化：`AppliedContract { path: Vec<PathSeg>, names: Vec<String> }`，
  `PathSeg::Key(String) | Index(usize)`（**Int 段 = 数组索引，Str 段 = 键**，靠 SML 类型自识别区分）；
  `ContractMap::names_at(&[PathSeg]) -> &[String]`；
- **交换表示就是一份 SML**（`ContractMap::to_sml()`）：顶层数组，每条
  `{ path: [ database primary ] contracts: [ Server ] }`；**顶层实体 `path: []`**；
  **没有应用契约的实体不出现**；按 `path` 排序（Int 段比数值、Str 段按字典序）；
  **输出不含时间戳**（同输入必须逐字节相同，与 `gen_json.py` 同一纪律）；
- 记录 `resolve` 后的契约名（与 `contracts` 表的键一致，下游可直接反查）；
- `include` 进来的块按**结果树里的位置**记；片段 `&base` 展开进来的字段**不单独记**。

**D6 — 错误定位**：`Tok` **不动**（不加字段，把改动面压到最小）；
`tokenize` 增加**旁路** `spans: Vec<Span>`（byte offset）；
`SmlError` 加 `pos: Option<usize>`；`Display` **保持纯文案**（不附位置），
由 `sml-parse` 的出口（`api.rs` 的 `parse` 系列）统一把 offset 换算成
`[第 N 行第 M 列]` 追加进 message。旧构造点不传 pos ⇒ 文案与现在**逐字一致**。

**D7 — 命名**：选项名 `trace_contracts`、字段名 `contract_map`、类型名 `ContractMap` /
`AppliedContract` / `PathSeg`。（用户已确认。）

---

## 3. 任务 A：错误定位基础（**前置**，B / C 的报错都要靠它）

> **✅ 已完成（2026-10-12）**，证据：
> - 全量 `EXIT=0 / SUITES=47 / PASSED=564 / FAILED=0`（基线 46 套 549 条 ⇒ +15 条新增，零回归）；
> - 判别实验：`git stash` 押回 HEAD 版后 `error_position` **2 failed / 1 passed**，还原后全绿。
>
> 执行中的三处设计与文档差异（均已按实情落地）：
> 1. **`scan.rs` 的 `strip_version` / `strip_features` 是「删行」**，比原文短 ⇒ 行列必然漂移。
>    已改为**等长替换**（指令行换成同长度空格、行尾与 CRLF 原样保留、BOM 换 3 个空格），
>    偏移与原文 1:1 —— 与 JS 侧 `stripFeatureLines` 的既有修法对齐（scan.rs 新增 4 条测试钉住）。
> 2. **末行无换行时仍补 `'\n'`**（历史行为，`tests/error_codes.rs::lex_escape_at_eof_needs_the_lexer`
>    钉着它的可观察后果）；补在文末，不影响任何既有偏移。
> 3. **文件入口（`parse_file` 等）不带位置**：include 展开是跨文件合并的 token 流，
>    没有可映射的单一原文 ⇒ 传空 spans，`err_at` 退回「不带位置」（宁缺勿错）。
>
> 本轮新登记（未改，待裁决）：
> - 词法器 `--` 行注释**不先冲掉裸词**：`tokenize("a--b\nc: 2")` 中注释连 `\n` 一起吞掉，
>   下一行的 `c` 被并进同一个词 ⇒ `Word("ac")`。既有行为，影响面小但真实存在；
> - `sml-lex` 的词法错误（`E-LEX-*`）**尚无位置**（本轮只接了解析层）；

### A1. `rust/sml-lex`：给 `tokenize` 加旁路 spans

- **位置**：`rust/sml-lex/src/lib.rs:41` `pub fn tokenize(text: &str) -> Result<Vec<Tok>, SmlError>`
- **改法**（不动 `Tok` 枚举）：
  1. 新增 `pub struct Span { pub start: usize, pub end: usize }`（byte offset）；
  2. 新增 `pub fn tokenize_spans(text: &str) -> Result<(Vec<Tok>, Vec<Span>), SmlError>`，
     把现有扫描逻辑搬进去，在每个 token 入列时同步 `spans.push(Span { start, end })`；
     ⚠️ **注释要一并搬**：现有分支的注释解释的是「为什么这么切」（如孤立 `@` 的 `BareAt`），
     搬丢等于删掉判据的理由；
  3. `tokenize` 改成 `tokenize_spans(text).map(|(t, _)| t)` —— **对外行为逐字不变**。
- **验证**：`cargo test -p sml-lex` + `cargo test --workspace` 全绿（条数与基线相同）。

### A2. `rust/sml-codes`：`SmlError` 加可选 pos

- **位置**：`rust/sml-codes/src/lib.rs:45-48`
  ```rust
  pub struct SmlError { code: &'static str, message: String }
  ```
- **改法**：加 `pos: Option<usize>` 字段；`new()` 置 `None`；
  新增链式 `pub fn at(mut self, pos: usize) -> Self`；
  新增 `pub fn pos(&self) -> Option<usize>`；
  **`Display` 不改**（保持纯文案，见 D6）。
- **验证**：`cargo test -p sml-codes`；且 `cargo test --workspace` 与基线**条数相同、全绿**
  （这是「旧构造点逐字一致」的证据）。

### A3. `rust/sml-parse`：出口把 offset 换算成行列

- **位置**：`rust/sml-parse/src/api.rs:96` `parse_with` 及同文件的 `parse` / `parse_file` 系列
- **改法**：
  1. `tokenize` 改调 `tokenize_spans`，把 `spans` 存进 `Parser`（新字段，仅用于报错换算）；
  2. 新增私有 `fn line_col(text: &str, pos: usize) -> (usize, usize)`：
     `line = text[..pos].matches('\n').count() + 1`，
     `col = pos - text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0) + 1`；
  3. 在**返回给调用方之前**统一包一层：若 `e.pos()` 有值，把
     `[第 N 行第 M 列]` 追加到 message 尾部（追加，不改写原句）。
- **验证**：新增 `rust/tests/error_position.rs`：
  - `X { a: B::C }` ⇒ 仍是 `E-PARSE-006`，且位置**不再落到下一行**
    （断言行号 = `B::C` 所在行；**不要求文案提到 `::`**，那是另一项议题）；
  - `k: "abc\n`（未闭合字符串）⇒ `E-LEX-001`，行号 = 该行；
  - `@contract C { a: str }\nx { @is Nope }\n` ⇒ `E-CONTRACT-001`，行号 = 3。

### A4. 判别实验（A 组必做）

```bash
git stash push -- rust/sml-lex/src/lib.rs rust/sml-codes/src/lib.rs rust/sml-parse/src/api.rs
$env:CARGO_TARGET_DIR="<target-dir>"; cargo test --test error_position  # 期望：红（无 pos / 位置不对）
git stash pop
```

**期望**：HEAD 版 `error_position` 红；还原后再跑 `cargo test --workspace` 与基线一致。

---

## 4. 任务 B：契约映射 API（不开启时逐字一致）

1. **新文件** `rust/sml-parse/src/contract_map.rs`：
   `PathSeg` / `AppliedContract` / `ContractMap` / `names_at()` / `to_sml()`（按 D5）。
2. **`rust/sml-parse/src/ext.rs:201`**：`ParseOutput` 加 `pub contract_map: Option<ContractMap>`；
   构造处置 `None`（未开启时）。
   ⚠️ **这是公开结构体加字段**，属 API 变更 ⇒ CHANGELOG「升级注意」要写一句
   （照 0.6.2 的写法：加字段对**只读取**的代码无破坏，只有结构体字面量构造会受影响）。
3. **`ParseOptions`**（`rust/sml-parse/src/ext.rs` 或 `api.rs` 里，就近照抄既有
   `directive()` 的写法）：加 `pub fn trace_contracts(mut self, on: bool) -> Self`。
4. **`Parser`**（`rust/sml-parse/src/parser.rs`）：
   - 把现有 `ns_stack: Vec<String>` 升级为 **`path_stack: Vec<PathSeg>`**；
     `ns_prefix()` 从它派生（只取 `Key` 段、用 `.` 拼），
     **保证 `qualify()` 与错误前缀行为逐字不变**；
   - 进块 / 进数组元素时 push，退出时 pop（数组元素 push `Index(i)`）；
   - 在**两处应用契约的地方**记录：`parser.rs:1057`（`@is`）与 `:1132`（块级标注）；
     仅当 `trace` 开启时记，否则零开销。
5. **测试** `rust/tests/contract_map.rs`：
   - 不开启 ⇒ `contract_map.is_none()`（**显式断言**，这是向后兼容的钉子）；
   - 开启 ⇒ `names_at(&[Key("database"), Key("primary")]) == ["Server"]`；
   - 数组元素路径带 `Index(0)`；
   - `to_sml()` 输出与 golden 字符串**逐字节相等**（写死期望值，含排序与 `path: []` 的顶层形态）；
   - 叠加后 `names` 为多项（这条依赖任务 C，C 做完再补断言亦可）。

---

## 5. 任务 C：契约叠加（本轮主体）

### C1. 新增码 `E-CONTRACT-016`

- **位置**：`errors/codes.sml`（照 `E-CONTRACT-015` 那一节的字段写：
  `id / domain / severity / title / msg / impls / status / note`）
- `title: "契约叠加冲突"`；`msg` 说明「同一字段在参与叠加的两份契约里类型 / 默认值 / 区间不一致」；
  `impls` 先只填 `rust`；`status` 按码表既有取值填（照抄邻条的写法，不要自创）。
- 跑：
  ```bash
  python errors/gen_codes.py        # → rust/sml-codes/src/codes.rs 等生成物
  python errors/gen_json.py         # → site/static/errors.json
  python errors/gen_codes.py --check
  git status --short                # 期望：生成物确实有 diff（否则说明没生效）
  ```

### C2. 契约存储：单值 → 列表

- `rust/sml-parse/src/parser.rs:612`：`let mut applied_contract: Option<String> = None;`
  ⇒ `let mut applied_contracts: Vec<String> = Vec::new();`
- `:823-846`（`@is` 分支）：按 **D1** 循环读词 —— 连续 `Tok::Word` / `Tok::Str`；
  遇 `Tok::Colon` / `Tok::LBrace` / `Tok::RBrack` / 行尾停止；`Tok::Comma` 跳过（可省）；
  每个词做 `unwrap_type_call` → 裸名/qualify 查找 → **找不到即 `E-CONTRACT-001`（带 pos）**
  → 找到则 push。
- `:1057`（块结束应用）：改成对每个名字取契约后**一次性调用** `apply_contracts`（见 C3）。

### C3. 合并语义：`rust/sml-contract/src/lib.rs`

- 新增 `pub fn apply_contracts(names: &[String], node: &mut BTreeMap<String, Value>,
  contracts: &BTreeMap<String, Contract>, path: &str) -> Result<(), SmlError>`：
  1. 逐个取契约，取不到 ⇒ `E-CONTRACT-001`（既有文案形态照 `:1063` 写）；
  2. **两两预检冲突**（同字段名：`ty` 不同 / `default` 值不同 / `min` 或 `max` 不同）
     ⇒ `E-CONTRACT-016`（文案含字段名与两份契约名）；
  3. 构造合并视图 `Contract { name: "a+b", fields: 并集, allow_extra: 全部 allow_extra }`；
  4. 调既有 `apply_contract(&merged, node, contracts, path)?`。
- ⚠️ 冲突预检必须**按字段名遍历**而非按书写顺序 ⇒ 天然满足 D3 的「顺序无关」。

### C4. 块级多契约：`rust/sml-parse/src/parser.rs:1091` `parse_bare_block`

按 **D2**：在 `:1132` 的 typed-block 分支里，把「首词 `key` 是否命中契约表」
扩展为「`key` + args 前缀里**连续命中契约表**的词都算契约」；
第一个未命中的词是块名（写 `__name`），其余照旧进 `__args`（`:1158-1163` 现有逻辑不动）；
契约词 ≥ 2 ⇒ 走 `apply_contracts`，否则走现有单契约路径。
**P1-3 回归红线必测**：`server web prod { }`（`server` 是契约、`web`/`prod` 不是）
⇒ 结果与改动前**逐字相同**。

### C5. 测试 `rust/tests/contract_mixin.rs`（每条都要有「改动前」的对照记录）

| # | 输入 | 期望 |
|---|---|---|
| 1 | `@is a, b`（逗号） | 两份契约都生效 |
| 2 | `@is a b`（省逗号） | 与 #1 **同一棵树** |
| 3 | `@is b a`（交换顺序） | 与 #1 同一棵树（**顺序无关**） |
| 4 | 同块两条 `@is` | 全部生效（**行为变更**，见 D4） |
| 5 | `a` loose + `b` strict | **strict 优先**：额外字段被拒 ⇒ `E-CONTRACT-004` |
| 6 | 两份契约同字段类型不同 | `E-CONTRACT-016` |
| 7 | `@is a Nope` | `E-CONTRACT-001`，且**位置指向 `Nope`** |
| 8 | `@is a b host: x`（同行键值） | `host: x` 是数据键值，不被吞 |
| 9 | 块级 `conta contb item { }` | 两份都生效，`__name == "item"` |
| 10 | 块级 `server web prod { }`（P1-3） | 与改动前逐字相同 |
| 11 | 映射 API（trace 开启） | `names == ["a","b"]` |

### C6. 判别实验 + 全仓扫描（**必做**，照 TASK-agent §2.6 / §2.4 的做法）

- 判别实验：`git stash` 只押 `rust/sml-parse/src/parser.rs` + `rust/sml-contract/src/lib.rs`
  ⇒ `cargo test --test contract_mixin` **必须红**；还原后全绿，条数 = 基线 + 新增。
- 全仓扫描：用 `smltools` 对仓库全部 `.sml` 各跑一遍（改前 / 改后各存一份输出，逐条比对）。
  ⚠️ 判据照旧：**改动前那棵树是错的 ⇒ 可接受（静默变响亮）；
  改动前是对的 ⇒ 误伤，停下报告**。

---

## 6. 任务 D：其它端「响亮拒绝」（与 C 同期，每端改动极小）

**为什么必须做**：叠加**不能 opt-in**（C / C++ / Lua 没有 `@feature`），
只做 Rust 会重演 `typed-block`「其余端静默退化」的坑（教科书 §5.2.2 / ch10 已写过这个教训）。

- **做法**：在 `js/sml.mjs`、`c/sml.c`、`cpp/sml.cpp`、`lua/lib/sml.soup` 四端，
  对「`@is` 后出现第 2 个裸词」或「块级 args 里出现第 2 个已注册契约」⇒ **报明确错误**，
  绝不静默给树。
- **用哪个码**：新增 **`E-CONTRACT-017 契约叠加未实现`**（`impls` 记 `js`/`c`/`cpp`/`lua`，
  `rust` 不记）。某端将来实现了 ⇒ 从 `impls` 移除并更新 `status`。
  ⚠️ 加码流程同 C1（码表 → 两个生成器 → `--check`）。
- **JS 的具体收益**：TODO.md §契约叠记着 `@is tls, retry` 现在给的是**错树 +
  `E-CONTRACT-003`**（误导：用户看不出是契约名问题）⇒ D 把它改成 `E-CONTRACT-017`。
- **同步**：改完 `js/sml.mjs` 后跑
  `python tools/check_js_copies.py --fix`（会同步 site 三份 + `editors/vscode/src/vendor/sml.mjs`），
  再跑 `python tools/check_js_copies.py` 确认四份一致。
- **各端验收**：照 TASK-agent §2 的做法写 `_` 开头探针，跑各端既有自检
  （`python c/build_check.py --run` / `python lua/run_check.py` / `python cpp/build_verify.py`）。

---

## 7. 任务 E：契约推断（**数据量大，独立验收，排在 C/D 之后**）

- **落点**：`rust/sml-contract` 新增 infer 模块 + `rust/smltools` 新子命令 `--infer-contract`
  （或 `--to contract`，二选一待定；先按 `--infer-contract` 做）。
- **映射表**：`Int→int`、`Float→num`、`Bool→bool`、`Str→str`、`Null→any`、
  `Object→子契约（组合）`、`Array→[T]`（元素合一：全 int ⇒ `[int]`；混合 ⇒ `[any]` 并告警；
  全对象同形 ⇒ `[Sub]`）。
- **`optional` 只在多实例合并时判**（某字段在部分实例里缺失）；单实例一律 `required`
  并提示「单实例无法判定 optional」。
- **不猜**：`default` / `min` / `max` / `enum` / 自定义 `@type`。
  ⚠️ 尤其 `enum` 绝不能猜 —— 从少数样本猜枚举会**静默收紧合法取值**。
- **输出确定性**：字段排序、无时间戳、同输入逐字节相同（golden 测试）。
- **生成的契约默认 `loose` 或字段 `optional`**：否则用户下次新增字段立刻被严格模式拒绝。

---

## 8. 任务 F / G：编辑器改造、教科书（**后置，本轮只登记依赖**）

- **F（编辑器接 API）**：`editors/vscode/src/sml-parse.mjs` 的
  `contractOfBlock` / `contractInstance` / `blockHoverMarkdown`（`:602`、`:862`、`:901`）
  现在是**按行扫文本的单名正则**，多契约一落地就把 `a,` 当契约名 ⇒ 悬浮 / 跳转 / 实例展开全失效。
  改造方向：JS 侧先有映射 API（D 之后补），编辑器改读它，悬浮列出**全部**契约、F12 支持多目标，
  并补 `_verify_ext.mjs` 断言。需用户放行 `editors/**` 后开工。
- **G（教科书）**：用户裁决**等总体功能完成再做**。届时两件事：
  ① `ch05` 加「叠加（提案 / 已落地）」页（TODO.md 已规划，且要与 §5.4「组合」交叉说明
  「为什么不能互相替代」）；② 加一页「**能力地图**」（每个能力：是否 opt-in / 五端支持 /
  落点章节）—— 起因是实测发现 `typed-block` / `@when` / `@for` 等只在第 10 章出现，
  ch01–ch09 一处都没有。

---

## 9. 任务 H：README 开头加「30 秒看懂」对照示例（**纯新增**）

> 用户 2026-10-12 追加：**不改动既有内容**，只在**最前端**插入一节短小精悍的对照。

- **位置**：`README.md` 与 `README.en.md`，插在「声明式数据/配置格式，JSON/YAML 的替代品…」那段
  简介**之后、`## 特性` 之前**（即最前端）。**只新增一节，不动任何既有段落**。
- **内容**：同一份配置，**JSON / YAML / SML** 三栏并列，控制在各 8–10 行内；必须体现：
  注释（JSON 无）、引号可省、逗号可省、块冒号可省、无缩进语义、`@片段` 复用、可选 `@contract` 校验。
- **示例草稿（照抄，可微调措辞，但别加长）**：

  ```json
  {
    "name": "web",
    "image": "nginx:1.25",
    "ports": [80, 443],
    "deploy": { "region": "cn-north-1", "replicas": 2 }
  }
  ```

  ```yaml
  name: web
  image: "nginx:1.25"   # 缩进定结构；不加引号的 1.10 会变成浮点
  ports: [80, 443]
  deploy:
    region: cn-north-1
    replicas: 2
  ```

  ```sml
  @base { region: cn-north-1 replicas: 2 }   # 片段：一处定义，多处复用
  name: web
  image: "nginx:1.25"      # 引号可省；版本号要保真才加
  ports: [ 80 443 ]        # 逗号可省
  deploy: &base            # 块冒号可省；&base 展开为片段内容
  ```

  末尾一行点题（中英各一句）：**同样的信息，SML 少了引号、逗号和缩进规则，还多了注释、复用与可选契约校验。**
- ⚠️ **不许夸大**：涉及「未加引号的数字字符串会被重新归类」这类**五端有差异**的点，
  一律按 README「跨实现差异」第 2 条的既有口径写，或干脆回避；不要写 SML 独有的优点。
- **验收**：三份示例都要**真能解析**（`smltools -i _tmp.sml --to json` 跑通），
  并把 JSON 那段的事实核对一遍（别写出 JSON 里其实合法的东西）。

---

## 10. 验收清单（逐项打勾，缺一不可）

- [ ] §1 四项基线全绿，`git status --short` 开工前为空
- [ ] A：`error_position` 全绿；HEAD 版判别实验红；`cargo test --workspace` 与基线**条数相同**
- [ ] B：`contract_map` 未开启时 `is_none()`；`to_sml()` golden 逐字节相等；排序 / 无时间戳
- [ ] C：新增 `E-CONTRACT-016`；两个生成器重跑且生成物**有 diff**；`--check` 通过
- [ ] C：`contract_mixin` 11 条全绿（含 P1-3 回归红线与顺序无关）
- [ ] C：判别实验 HEAD 版红 + 还原后全绿；全仓扫描差集**逐条判定**、无误伤
- [ ] D：四端（js / c / cpp / lua）对多契约写法报 `E-CONTRACT-017`，**无静默**；
      `tools/check_js_copies.py` 一致
- [ ] H：README 中英各加一节；三份示例真能解析；既有段落零改动
- [ ] CHANGELOG：`## [未发布]` 下写「变更」（多 `@is` 行为变更、公开结构体加字段）
      与「新增」（多契约三写法、映射 API、E-CONTRACT-016/017）
- [ ] `python tools/check_private_assets.py` 仍 OK
- [ ] 提交**按主题分笔**（A 一笔、B+C 一笔、D 一笔、H 一笔），每笔**逐文件** `git add`
- [ ] **没有**做 `git push`

---

## 11. 提交规范（照抄现有习惯）

```bash
git add rust/sml-lex/src/lib.rs rust/sml-codes/src/lib.rs rust/sml-parse/src/api.rs
git commit -m "feat(rust): 错误带位置（词法 spans 旁路 + SmlError.pos + 出口换算行列）

<正文：改前后报错形态对照；判别实验 HEAD 版 N 条红；为什么不动 Tok 枚举>"
```

提交信息末尾**必须**带证据（判别实验条数、扫描判定结论）。**不许** `--no-verify`、**不许** `-A`、**不许** push。

---

## 12. 什么时候必须停手问人

1. 本文档任何一条的**实测结果与文档不符**（行号对不上、期望码不符、条数不符）。
2. 全仓扫描出现「**改动前是正确文档 / 改动后报错**」⇒ **误伤，绝对不许自己糊过去**。
3. 某个用例修了 **3 次**还是红。
4. 需要改**公开 API 的既有签名**（不是新增字段）⇒ 用户拍板。
5. 发现要动 `site/**`（教科书）或 `editors/**`（编辑器功能）才能继续 ⇒ 停（本轮未授权）。
6. 磁盘空间不足、构建要写 `C:\`（Rust 记得 `CARGO_TARGET_DIR=<target-dir>`）。

停下来时报告格式：**「我在做 X 几 + 命令 + 完整输出 + 我卡住的判断点」**，不要只写「失败了」。

---

## 13. 附：为什么是这些判据（一句话一条，供你判断合理性）

| 判据 | 理由 |
|---|---|
| `@is` 遇 `:` / `{` / `]` / 行尾即停 | SML 一行可写多个键值，无脑收到行尾会把 `host:` 当契约名 |
| 每个词必须命中契约表 | 报错即 `E-CONTRACT-001`；也给将来「`@is` 后加非契约参数」留了扩展位 |
| 逗号可有可无 | 逗号五端已是独立 token 且已是 include 多目标分隔符；可省是为了少敲符号 |
| 块级「连续命中契约表的词都是契约」 | 与既有 `__name` / `__args`（P1-3）共存，单契约时逐字不变 |
| strict 优先 | 否则 `loose` 会成为绕过校验的后门（TODO 已裁决） |
| 冲突即报错（016） | 只有不允许「后者覆盖」才能真正做到**顺序无关** |
| 映射用 SML 表示 | 五端共享一份表示、可再解析、可 golden；C-ABI 一个 `char*` 就能跨边界 |
| Int 段 = 索引 / Str 段 = 键 | 借 SML 自己的类型自识别区分，不另造 DSL |
| 其它端先只加「响亮拒绝」 | 叠加不能 opt-in；先守「同条件同码」，能力后补 |
