# 文档对账清单（W22 · 第 1 步：仅出清单，不改文档）

- **日期**：2026-10-28
- **分支**：`docs/w22-reconcile-list`（在 `dev` / `b382ac4` 之上新建）
- **任务来源**：`TODO.md` §五 W22 —— 把 W17–W21 及之后新增的东西补进各处文档。原话要求**先出一份对账清单再动手**。
- **本棒范围**：**只产出本清单**，不修改任何既有文档、不重打 `.vsix`、不跑 site 构建、不动 `editors/`、不改代码、不 push。
- **铁律遵守**：本文件不出现内网地址 / 私库名 / 本机绝对路径 / 真实用户名 / 内部代号；涉及对外仓库一律用占位符（`<SML 公网仓库>`、`<发布镜像仓库>`）。

## 素材来源（已读）

| 素材 | 用途 |
|---|---|
| `CHANGELOG.md` 的 `## [未发布]` 段 + 最近版本段 | W22 本轮新增/变更项（隐私脱敏、`one_of`/`exactly_one`、`flatten` 行为变更、`parse_with_warnings` 等） |
| `errors/README.md`「码的落地进度」+「清点」+「静默清单」 | 错误码落地口径、码数（当前 139）、W20 给 Lua 补契约/include 等 |
| `docs/capability-matrix.md`（2026-10-27 调查稿） | 五端能力矩阵；**核查其中是否仍含过时结论** |
| `tests/conformance/known_divergences.md`（18 条分歧，W5 实测 2026-10-28） | 实测证据权威来源；含 F-01~F-05 顺带发现 |
| `README.md` / `README.en.md` / `llms.txt` | 根文档与 AI 指引现状 |
| `site/content/{zh,en}/book/ch14-capability-map.md` | 教科书「能力地图」（**两行过时结论的实际落点**） |
| `site/content/{zh,en}/errors.md`、`site/content/zh/_index.md` | 错误码页与落地页现状 |
| `editors/vscode/README*.md`、`editors/zed/{extension.toml,README.md}`、`editors/lsp/` | 编辑器文档现状 |

## 对账清单表

> 现状三档：**已有**（已存在且正确）/ **过时**（已存在但需修订）/ **缺失**（应出现但未出现，含"应复查/应新增"）。
> 行号均为写本清单时的版本，修订时请再次核对。

| # | 新增项 / 变更项 | 来源（提交或文件） | 该出现在哪个文件 | 现状 | 建议措辞要点 |
|---|---|---|---|---|---|
| A1 | 隐私脱敏（历史已重写）：内网私库地址 / 本机绝对路径 / 真实用户名 / 内部代号从全部提交抹掉；新增 `tools/check_private_assets.py` 守卫挂 CI | `CHANGELOG.md` `[未发布]`「仓库治理」+ `HANDOFF-2026-10-28.md` + `TODO.md` §八·二 | 全部文档（README / README.en / llms.txt / site/\*\*/editors/\*\*/docs/\*\*） | 缺失（需逐文件复查是否已无残留） | 全文复查不出现 `<内网私库地址>`/`<本机绝对路径>`/`<真实用户名>`/`<内部代号>`；旧 clone 须重新克隆；新提交过 `guards` job |
| A2 | 契约互斥字段组 `one_of [a b c]`（至多一个）/ `exactly_one [a b c]`（恰好一个），I-7 缺口① | `CHANGELOG.md` `[未发布]`「新增（契约扩展点：互斥字段组）」+ 提交 `b382ac4`（!10） | README.md（契约章）/ README.en.md / llms.txt / `site/content/{zh,en}/book/ch05-contract.md` | 缺失 | 跨字段约束；判据看**显式写出的键**（default 回填不算）；组成员名写错响亮报错；复用 `E-CONTRACT-011` 不新增码；外置 `Modifier::check_group`，不动 C/C++/JS/Lua |
| A3 | `#[sml(flatten)]` 键冲突：从静默覆盖改为报错 | `CHANGELOG.md` `[未发布]`「⚠️ 行为变更」 | `rust/derive/README.md`、`rust/STRUCT_SERDE.md`、`site` 教科书 serde 章、llms.txt | 缺失 | 两个方向都拦 `Err`（flatten 盖父块 / 父块盖 flatten）；依赖旧"后者胜"的代码会收 `Err`；需改名或去 `flatten` |
| A4 | `parse_with_warnings` / `parse_file_with_warnings` + `DiagnosticKind::DuplicateKey` / `KeyOverwritten` + `ParseOptions::warn_key_conflict`（默认 true） | `CHANGELOG.md` `[未发布]`「新增」 | `rust` crate README、`site` 教科书、llms.txt | 缺失 | 返回 `Result<(Value, Vec<Diagnostic>), SmlError>`；两类**此前完全静默**的告警（重复键提升为数组 / 片段覆盖父键已丢值）；值逐字不变 |
| A5 | `Diagnostic` 新增 `line: Option<usize>` 字段（1 起；跨文件合并恒 `None`） | `CHANGELOG.md` `[未发布]`「⚠️ 升级注意」 | `rust` API 文档 | 缺失 | 只读取诊断的代码不受影响；用结构体字面量构造 `Diagnostic` 需补 `line`；原有 `parse()` 入口恒不返回诊断 |
| A6 | 编辑器（VSCode / LSP）本轮回不同步告警（不标黄） | `CHANGELOG.md` `[未发布]`「⚠️ 升级注意」 | `editors/vscode/README.md`、`editors/vscode/README.en.md`、llms.txt、`editors/lsp/` | 已有（相关限制已写，但缺"本轮回不同步 key_conflict 告警"一句） | 编辑器诊断走 `js/sml.mjs` 的 `parseSafe` 非 Rust 库，本轮回只改库；需带行号重复键诊断请用 `smltools --lint`（`W-LINT-003`） |
| A7 | 本轮回告警不带错误码（`W-PARSE-*` 留待下一轮） | `CHANGELOG.md` `[未发布]`「⚠️ 升级注意」 | errors 相关文档 | 已有（`errors/codes.sml` 未动，文档未声称新增码） | 明确本轮不加码；码体系未变；语言层告警码下一轮再定（需解释与 `W-LINT-003` 分域） |
| B1 | ⚠️ **过时结论**：「`top-level-array` 只有 Rust + C」 | `tests/conformance/known_divergences.md` F-02（用例 `02`/`03` 五端全部 AGREE）+ `site/content/zh/book/ch14-capability-map.md:24`、`en:23`、`TODO.md` §排雷区（雷表 `top-level-array` 行） | `site/content/{zh,en}/book/ch14-capability-map.md`、`TODO.md` §排雷区 | 过时 | 五端**均已支持**顶层数组（C++/JS/Lua 现在都支持）；改为 ✅ 五端，删除"只有 Rust+C" |
| B2 | ⚠️ **过时结论**：「`fragment` 行级 `&base` 展开：C 不展开」 | `tests/conformance/known_divergences.md` F-03（`11_fragment_splice` 里 C 与 Rust 同组，只剩 C++）+ `README.md:77`、`llms.txt:27`、`site/content/{zh,en}/book/ch14-capability-map.md`（zh:21 / en:20）、`TODO.md` §排雷区 | README.md、README.en.md、llms.txt、`site/content/{zh,en}/book/ch14-capability-map.md`、`TODO.md` §排雷区 | 过时 | **C 已实现行级 `&base` 展开**（与 Rust 同组）；只剩 **C++** 未实现；把"C / C++"改为"C++" |
| B3 | ⚠️ **引用失效**：`known_divergences.md` F-02/F-03 把上述两结论归因到"能力矩阵 §1.1 / 三·十" | `tests/conformance/known_divergences.md` F-02/F-03 正文 | `tests/conformance/known_divergences.md` | 过时（引用错） | `docs/capability-matrix.md` 当前版本**已不含**这两行（调查稿 2026-10-27 未列顶层数组/`&base` 行）；修订 F-02/F-03 指向 `ch14` 与 `TODO.md` §排雷区，或注明矩阵已不含 |
| B4 | ⚠️ **矛盾**：README 称「Lua 是契约唯一未落地的一侧」，与 llms.txt / W20 事实冲突 | `errors/README.md`（W20 一阶段给 Lua 补契约）+ `llms.txt:40` 称 Lua 支持契约 vs `README.md:178` / `README.md:259` | README.md、README.en.md | 过时 | W20 已给 Lua 补契约（`@contract`/`@is` + 默认值回填）；README 契约支持表与多语言实现表应更新为"Lua 已落地"，与 llms.txt 对齐 |
| C1 | capability-matrix 缺「嵌套数组」行（Lua 缺陷） | `tests/conformance/known_divergences.md` F-04 + #1~#3（Lua 嵌套数组不支持；C 同类已由 W17 修，Lua 未同步） | `docs/capability-matrix.md` | 缺失 | 在矩阵加"嵌套数组"行：Lua ❌（键位置 `[`/`{` 报错）；C 已修，其余四端 ✅ |
| C2 | JS 组合契约子契约默认值不回填，未登记 | `tests/conformance/known_divergences.md` #9 JS 半（"顺带发现"） | `docs/capability-matrix.md`、`site/content/{zh,en}/book/ch05-contract.md` | 缺失 | 标注 JS 组合契约（字段类型是另一契约）默认值**不递归回填**为已知缺陷；矩阵原"契约五端都实现"需细分 |
| C3 | JS dump 数字形字符串不加引号（载体盲区 B-01）未登记 | `tests/conformance/known_divergences.md` B-01（JS 数字样字符串 `stringify` 不加引号→回读变数字，类型丢失） | `docs/capability-matrix.md` | 缺失 | 登记 B-01：与 #17（引号策略）同族；属数据保真级，根治需 JSON 直发通道（本轮刻意不做） |
| C4 | 教科书/文档未提及跨实现一致性套件（W5）与 18 条分歧台账 | `tests/conformance/known_divergences.md` 引言 + `CHANGELOG.md` W5（AGREE 15 / DIVERGE 18） | `site/content/{zh,en}/book/ch14-capability-map.md` 或 `docs/` | 缺失 | 加一小节：五端一致性套件（值树比对，非字节）、18 条分歧台账（缺陷/已知取舍）、载体盲区（B-01/B-02）；强调"两端都失败不算一致" |
| C5 | ⚠️ **错误码数量声明多处不一致** | `site/content/zh/errors.md:20`（141）/ `llms.txt:69`（144）/ `llms.txt:135`（143）/ `errors/README.md`（139，W20/W21 加 `E-INCLUDE-012`、`E-CLI-008`） | `site/content/zh/errors.md`、llms.txt（en/errors.md 无数字） | 过时（不一致） | 统一指向 `site/static/errors.json` 的 `count` 字段，勿在正文另记一份；或写明"以 errors.json 为准" |
| C6 | README 顶层数组示例本身解析不过（裸写带 `:` 的时间戳） | `tests/conformance/known_divergences.md` F-01（`README.md` 示例 `2026-08-30T10:00:00Z` 含 `:` → 五端 `E-PARSE-006`） | README.md（顶层形态示例，`README.md:85`） | 缺失（示例缺陷） | 给时间戳加引号或换值（`02_top_array_object.sml` 已刻意避开） |
| D1 | TODO §五 W22 原话「vsix 提供的也加」—— 指把 VSIX 已有能力补进 Zed 扩展，还是补进文档？ | `TODO.md` §五 W22 | `editors/zed/`、`editors/vscode/syntaxes/`、`docs/` | 缺失（待澄清，见下节①） | 待用户裁决后定；若要在 tmLanguage 补 `one_of`/`exactly_one` 修饰符高亮则需重打 `.vsix`（本棒不做） |
| D2 | Zed `extension.toml` + Tree-sitter grammar 是否需同步本轮新能力 | `editors/zed/extension.toml`、`editors/zed/README.md` | `editors/zed/` | 已有（纯 grammar，无解析语义；本轮新契约能力是解析层，无新语法 token，无需改） | 确认无新 top-level token 即可；grammar 与 `extension.toml` 的 `rev` 维持占位/发布镜像 |
| D3 | LSP（`editors/lsp/server.mjs`）能力说明 | `README.md:303`、`editors/vscode/README.md`「为什么不用 LSP」 | `editors/lsp/`、`README.md` | 已有 | 本轮回不同步告警已在 A6 覆盖；LSP 文档无需为本次库变更另写 |

## 按现状分组统计

### 缺失（11 条）
A1（隐私脱敏复查）、A2（`one_of`/`exactly_one`）、A3（`flatten` 行为变更）、A4（`parse_with_warnings`）、A5（`Diagnostic.line`）、C1（矩阵缺 Lua 嵌套数组行）、C2（JS 组合契约回填未登记）、C3（JS dump B-01 未登记）、C4（一致性套件未提及）、C6（README 示例解析不过）、D1（vsix→Zed/文档，待澄清）。

### 过时（5 条）
B1（`top-level-array` 只有 Rust+C）、B2（`&base` 展开 C 不展开）、B3（F-02/F-03 引用失效）、B4（README Lua 契约状态矛盾）、C5（错误码数量 141/144/143/139 不一致）。

### 已有（4 条）
A6（编辑器不同步告警，相关说明已存在，仅缺一句补充）、A7（本轮回不加码，与码表一致）、D2（Zed grammar 无需改）、D3（LSP 说明已存在）。

> 合计 **20 条**（缺失 11 / 过时 5 / 已有 4）。其中 B1、B2 正是任务点名的"至少两行已过时的能力矩阵结论"——它们实际落在教科书 `ch14` 能力地图与 `README`/`llms.txt`/`TODO.md` §排雷区，而 `docs/capability-matrix.md` 调查稿本身已不含这两行（见 B3）。

## 需要用户澄清

1. **TODO §五 W22 原话「vsix 提供的也加」—— 是指把 VSIX 已有的能力补进 Zed 扩展，还是补进文档？**
   涉及 `editors/zed/`（Zed 用 Tree-sitter grammar + `extension.toml`，与 VSIX 技术栈不通用，见 `README.md`「Zed 装不了 .vsix」）与 `editors/vscode/syntaxes/`（若要给 `one_of`/`exactly_one` 加高亮则需重打 `.vsix`）。需先定"加进哪"。

2. **B4 与 llms.txt 的 Lua 契约状态以谁为准？** README 仍写"Lua 唯一未落地"，llms.txt 已写 Lua 支持契约，且 `errors/README.md` 记录 W20 给 Lua 补契约。是否确认"Lua 契约已落地（W20）"，并据此修订 README（含 `README.en.md`）？

3. **C5 错误码数量以哪个数字为准？** zh/errors.md 写 141、llms.txt 写 144/143、`errors/README.md` 写 139。权威应为 `site/static/errors.json` 的 `count`。是否统一改为"以 errors.json 为准"、撤掉正文硬编码数字？

4. **`one_of`/`exactly_one`（A2）是否要进 llms.txt 与教科书 ch05？** 它是 W22 本轮主新增特性，建议在契约章与 AI 指引同步；但需注意它"外置形态、不动 C/C++/JS/Lua"，措辞要写清跨端可用性（仅 Rust 外置扩展点可用）。是否纳入本次文档补齐范围？

5. **C4 一致性套件（W5）是否要写进教科书/能力矩阵？** 目前仅在 `known_divergences.md` 与 `CHANGELOG` 提及，教科书读者看不到"五端一致性"这件事。是否补一小节？

---

## 验收自检（本棒产出）

- ① 清单表覆盖点名全部目标文件：README.md / README.en.md / llms.txt / `site/content/{zh,en}/**`（ch14、errors 页、ch05、_index）/ `editors/vscode/README*.md` / `editors/vscode/syntaxes/`（D1）/ `editors/zed/extension.toml`+grammar（D2）/ `editors/lsp`（D3）/ `docs/capability-matrix.md`（B3/C1~C3）✔
- ② 每条均有「来源」可追溯（提交 / 文件:行 / known_divergences 条目）✔
- ③ 已明确标出两行过时结论 B1（`top-level-array` 只有 Rust+C）、B2（`&base` 展开 C 不展开），并指明实际落点与 `docs/capability-matrix.md` 已不含（B3）✔
- ④ 有「需要用户澄清」一节，第一条固定为 vsix 歧义 ✔
- ⑤ 未修改任何既有文件（仅新增本文件）✔
- ⑥ 在分支 `docs/w22-reconcile-list` 上提交本文件 ✔

---

*下一步（第 2 步，不在本棒执行）：待清单与澄清确认后，逐处更新文档；重打 `.vsix`、跑 site 构建、改 `editors/` 均属后续，本棒一律未做。*
