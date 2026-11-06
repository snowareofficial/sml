# TODO / IDEAS 两张表（2026-11-06）

> **本文件只做表**：不勾选、不改代码、不删原文件、不跑长测试。
> **权威仍是 `TODO.md` / `IDEAS.md`**（用户维护）；本表是镜像 + 状态纠偏，不替代原文。
> 规约来源：`docs/unfinished.md §11`。

状态用词：`未做` · `部分完成` · `已完成(未勾)` · `待裁决` · `待用户动作` · `挂起` · `Done` · `常驻约束`（不可勾，不是任务）。
优先级用词：**P0** = 不解决就发不出去/别人开不了工；**P1** = 主线内必做；**P2** = 可延后或外部条件阻塞；`—` = 非任务。

---

## 0. 数字与取数命令（先看这个）

| 维度 | 实测值 | 复核命令 |
|---|---|---|
| TODO 未勾 | **42** | `Select-String TODO.md '^\s*- \[ \]'` |
| TODO 已勾 | **70** | `Select-String TODO.md '^\s*- \[ x\]'` |
| TODO 非复选框欠账 | **8** | 人工取自 `TODO.md` 表格行 / `⬜` 标记 / `⏳ 待修`（本文 T43–T50，逐行注来源） |
| IDEAS 条目 | **23**（I-1…I-23，其中 I-12 = `Rejected`） | `Select-String IDEAS.md '^## I-\d+'` |
| 码表条目 | **166** | `Select-String errors/codes.sml '\{ id: [EWI]-'` |
| 仅 Rust 实现（`impls: [ rust ]`） | **47**；语言层 **39**（PARSE 16 / CONTRACT 10 / FEATURE 4 / INCLUDE 3 / EXT 3 / LEX 2 / LIMIT 1）+ DERIVE 8（宿主绑定层，Rust 专属，不是跨端欠账） | `Select-String errors/codes.sml '^\s*impls:\s*\[\s*rust\s*\]'`，按 `domain` 分组 |
| 一致性套件 | **41 例 / AGREE 34 / DIVERGE 7** | `python tests/conformance/run_conformance.py` |
| `.vsix` 产物 vs 源码 | 产物最新 **0.4.3**，`package.json` 已是 **0.5.0** | `Get-ChildItem editors/vscode -Filter *.vsix` |

**三处与旧文档不一致（以本表为准，别照抄旧数字）**

1. `docs/unfinished.md §0` 写「42 条 `impls: [rust]`」——实测 **47 条**（其中语言层 39 条才是四端欠账）。
2. `TODO.md:812` 写「146 条码」——实测 **166 条**。
3. `TODO.md:166`「顶层标量未统一」、`TODO.md:149`「一致性套件本轮不做」都是**过期未勾**：前者五端已同报 `E-PARSE-008`，后者套件已建（41 例）。

---

## 表 1 · TODO（未完成 42 + 非复选框 8）

| 编号 | 主题 | 状态 | 优先级 | 依赖 | 完成判据 | 来源(file:行) | 卡在谁 |
|---|---|---|---|---|---|---|---|
| T01 | 跨实现一致性套件（conformance） | 部分完成：41 例已建，AGREE 34 / DIVERGE 7 | P1 | 无 | `run_conformance.py` 输出 DIVERGE=0 且五端无 SKIP | `TODO.md:149`、`tests/conformance/known_divergences.md:6` | 实现者（剩 7 条跨端缺陷） |
| T02 | 顶层标量行为统一 | 已完成(未勾)：五端同报 `E-PARSE-008` | P2 | 无 | 五端各跑 `42` 均得 `E-PARSE-008`（码表该条 `impls: [ rust js c cpp lua ] status: done`） | `TODO.md:166`、对照 `TODO.md:1178` | 无；只需回填勾选 |
| T03 | 错误边界不一致（同名键 / 未闭合括号 / 空文档 / 仅注释 / 重复片段名） | 部分完成：同名键已改告警 `W-PARSE-001/002`；空文档与仅注释两例已五端 AGREE | P1 | T01 | 五类各一条 conformance 用例全 AGREE | `TODO.md:201`、`tests/conformance/known_divergences.md:15` | 实现者 |
| T04 | Lua/JS 缺单元测试 | 部分完成：Lua 有 `test_codes.lua` 184 条 + `run_check.py` 120 条；JS 有 `probe-error-codes.mjs` 52 条（均为脚本，非测试框架） | P2 | T01 | 两端各有进 CI 的测试目标，且 `0 passed` 不算通过 | `TODO.md:203` | 实现者 |
| T05 | C `sml_dump` 与 Rust `to_sml` 逐字节对齐 | 部分完成：行数一致 19/19、总行差 0；剩「引号策略」「键序」两类未改 | P2 | 无 | `python tools/check_dump_parity.py` 逐字节一致，或两类取舍写进规范并登记 | `TODO.md:204`、`TODO.md:1180` | 用户（两类是数据保真级取舍） |
| T06 | Lua 补契约数组 `[契约名]` | 未做（用户已定「先不动 Lua」） | P2 | 无 | `41_contract_array` 五端 AGREE | `TODO.md:318` | 用户 |
| T07 | 清理 DeepSeek 开发痕迹 | 未做（发布前） | P0 | 无 | 代码/文档/历史关键词 grep 归零，或产出「无法抹除」清单 | `TODO.md:802` | 用户（敏感部分自行处理） |
| T08 | 国际化：README / crate 文档 / CLI 帮助中英双语（范围：`swml` / `swsml` / `sml-*`，**`smltools` 不算**；`amberice` / `smlkit-easy` 待问） | 未做；`smlb` 已双语，其余 crate 与 CLI 待核对 | P1 | T28 / T40 / T41 / T42（与文档治理同批） | 逐 crate 清单勾完；缺英文时回退中文且不静默 | `TODO.md:804`、`docs/amberice-plan.md:2.1` | 实现者；**对外措辞动前问用户** |
| T09 | 双授权（MIT 或 Apache-2.0 + MulanPSL-2.0） | 未做；`swsml` / `smlb` 现仅 MulanPSL-2.0 | P0 | 无 | 两 crate 的 `license` 字段、LICENSE 文件、README 三处一致 | `TODO.md:806` | 用户（授权口径） |
| T10 | 各 crate `repository` 补 GitHub 地址 | 未做；现只有 `gitee.com/snoware/sml` | P1 | T30 | 每个 `Cargo.toml` 指 GitHub，README 链接 GitHub 在前 | `TODO.md:808` | 用户（等迁 GitHub） |
| T11 | Lua 嵌套数组 | 未做（用户已定「先不动 Lua」） | P2 | 无 | `05`/`06`/`07` 五端 AGREE | `TODO.md:967` | 用户 |
| T12 | 清理临时探针清单（`_*` 约 40 个） | 未做（等套件全 AGREE） | P2 | T01 | 台账末节清单清零，且 `git grep` 无工具链引用 | `TODO.md:969` | 实现者 |
| T13 | `@feature base` 只认版本号，文档无法声明书名 | 未做；CLI 按顶层 `title:` 推断（已是事实约定） | P2 | 无 | 一个正式承载位（字段或指令）+ 规范页 + 五端同判据 | `TODO.md:971`、`IDEAS.md:788` | 用户裁决 |
| T14 | 重打 `.vsix` | 未做：源码 0.5.0，入库产物最新 0.4.3 | P1 | 无 | `sml-lang-0.5.0.vsix` 入库 + 复制到 `site/static/dl/` + 下载页同步 | `TODO.md:1003` | 用户（需网络 + 定版本号） |
| T15 | weak* 落地后同步扩展 | 未做 | P1 | I-5 / I-15 / I-17 | 语法文件 + 补全 + `package.json` `languages` 齐备，`_prepublish.mjs` 全绿 | `TODO.md:1007` | 实现者 |
| T16 | 表示层优化（`Tok` 借用 / 键 interning / inline 小 map） | 未做（需设计决策） | P2 | I-18、`docs/value-layout.md` | 同负载 bench ≥15%，`eq_ordered` / fuzz 不回归 | `TODO.md:1042` | 用户（动表示层） |
| T17 | 先想清「哪一层能编译掉」 | 未做 | P2 | 无 | 一张逐项表：cargo feature vs SML 特性（`when`/`for`/`contract` 同名）各能否编译掉 | `TODO.md:1047` | 实现者 |
| T18 | 重定发布节奏 | 未做；原「等 I-18/I-19」前提已不成立 | P0 | T29 / T31 / T35 | 写死发布顺序与门槛（谁先发、什么条件） | `TODO.md:1080` | 用户裁决 |
| T19 | 发布前占 GitHub org / 域名 / docs 子域 | 未做 | P0 | T30 | 三处已注册可解析，并记进发布清单 | `TODO.md:1086` | 用户 |
| T20 | 命名陷阱（包名 `swsml` / 库名 `sml`） | 常驻约束（不可勾） | — | 无 | 无；写进 `Cargo.toml` 注释与 README 即可 | `TODO.md:1087` | 无 |
| T21 | `k: "abc\`（转义符后 EOF）三端三个码 | 待裁决（Rust 词法 `E-LEX-006` / Rust `parse` 入口 `E-LEX-004` / C·JS·Lua `E-LEX-001`） | P1 | T01 | 三端同码，或分域写清且码表 `impls` / note 同步 | `TODO.md:1093` | 用户（006 / 001 / 分域 三选一） |
| T22 | 两台远端分支保护 | 未做（2026-10-26 暂缓、2026-11-02 再「先等等」） | P1 | T30 | `main`/`dev` 禁强推 + required checks 生效（用一次直推验证被挡） | `TODO.md:1271`、`TODO.md:1188` | 用户（仓库设置） |
| T23 | PR 默认目标指 `dev` | 未做 | P2 | 无 | 两远端各设一次，建 PR 时默认目标为 `dev` | `TODO.md:1274` | 用户 |
| T24 | 收掉存量长期分支（4 条） | 未做；只在 GitHub，GitHub 通了才能删 | P2 | T30 | 四条各走 PR → `dev` 后删除，`git ls-remote` 不再出现 | `TODO.md:1275` | 用户 / 外部（GitHub 连通） |
| T25 | 删 `origin/feat/i9-contract-typo-hint` | 未做（内容已全在 `dev`） | P2 | 无 | `git push origin --delete …` 成功 | `TODO.md:1279` | 用户 |
| T26 | 删 `origin/master` | 未做（实测落后 `main` 223 笔、完全包含） | P2 | 无 | `git push origin --delete master` 成功 | `TODO.md:1281` | 用户 |
| T27 | 双远端同步口径 | 常驻约束（不可勾） | — | 无 | 无；`--delete` 不同步、工作流不带 `--prune` 是刻意的 | `TODO.md:1324` | 无 |
| T28 | 全仓文档对齐 + 可重跑机制 | 未做（§九 五条症状，执行方案在 §14） | P0 | T40 / T41 / T42 | §九 的 4 条「做完的定义」全部满足 | `TODO.md:1414` | 实现者 |
| T29 | 裁决 `amberice` 定位 | 待裁决（三注册表均 404） | P0 | 无 | 一句话定层（格式品牌 / 产品线 / 引擎），并回答「是否取代 smlkit、CLI 是否双名、语言是否仍叫 SML」 | `TODO.md:1440`、`TODO.md:1533` | 用户 |
| T30 | 主仓迁 GitHub（Gitee 降镜像） | 待用户动作（前置：Gitee 先拿完） | P0 | T29 | `origin`=GitHub；同步方向反转；CI 绿；Pages/域名/`repository` 就位；发布公告 | `TODO.md:1465` | 用户 |
| T31 | 门面 `smlkit` → `swml` | 已定、未动手（crate 未建，零迁移） | P1 | T18 / T29 | `swml` 0.1.0 建好（独立仓、`pub use swsml::*`）并按发布纪律发出 | `TODO.md:1488` | 用户（发布动作） |
| T32 | 防御别名 `swmltool` / `swmltools` | 待做，有硬前提 | P2 | T31 | `smltools` 抽出 `lib`，别名包同版本、README 写明是别名（否则违反 crates.io 反占坑） | `TODO.md:1497` | 实现者（抽 lib）+ 用户（发布） |
| T33 | 裁决 `.swl` 后缀策略 | 待裁决（`.sml` 被 516 个受控文件引用；与 `.smld` 冲突） | P1 | T29 | 二选一（`.swl` 或 `.smld`）+ 读取端别名；改默认留到 v5 | `TODO.md:1510` | 用户 |
| T34 | 官网重写 | 未做（顺序绑命名） | P1 | T31 / T33 | 五区信息架构；版本与下载项由构建生成；中英同源；旧链接 301 | `TODO.md:1525` | 实现者（排期在命名后） |
| T35 | 发布工作流 `release.yml` | 未做 | P1 | T30 / T31 | tag 触发 + `environment: release` 人工审批 + 发布前闸门 + 拓扑序 `cargo publish` 先 `--dry-run` 全绿 | `TODO.md:1549` | 实现者 |
| T36 | 定时部署 `deploy.yml` | 未做；部署对象一半未知（Cloudflare 是 Pages 不是 Worker；VPS 跑什么仓库无记录） | P2 | T30 + 用户补事实 | Ruby 脚本 + 系统定时器：构建→Pages→VPS→健康检查→失败回滚，退出码可观测 | `TODO.md:1572` | 用户（补 VPS 部署内容） |
| T37 | 13.1 存档（tag + 四份基线） | 未做（不改一行代码，可立即做） | P1 | 无 | `git tag -a pre-deep-2026-11-06`；两份 bench + smlb 测试 + fuzz 语料快照；从 tag 能重建同机同数字 | `TODO.md:1603` | 实现者 |
| T38 | 13.2 优化（只做能测出收益的） | 未做 | P2 | T37 | 同负载 bench ≥15%，`eq_ordered` / fuzz 不回归，语义零改动 | `TODO.md:1611` | 实现者 |
| T39 | 13.3 漏洞 / 缺陷检查 | 未做 | P1 | T37 | 每条给「位置 / 复现 / 影响 / 修法 / 回归测试」；无复现的降级为待验证并留在清单 | `TODO.md:1621` | 实现者 |
| T40 | 14.1 文件归档一刀切 | 未做（受控 `_*` 18 个，磁盘约 231 个） | P1 | 无 | 同一批提交内：新目录 + 全部内容 + 引用改路径 + 旧位置清空；例外在 `meta.sml` 记 `status: keep` | `TODO.md:1645` | 实现者 |
| T41 | 14.2 文档元数据 `docs/meta.sml` | 未做 | P1 | T40 | `meta.sml` + `changes.sml` 存在；`docs/index.md` / `report.md` 由前者生成 | `TODO.md:1658` | 实现者 |
| T42 | 14.3 `smltools docs` 扫描器 | 未做（当前无 `docs` 子命令） | P1 | T41 | `--check` 进 CI：未登记 / 断链 / `stale` 无替代者即红；`--fix` 只做机械补全 | `TODO.md:1681` | 实现者 |
| T43 | W4-片段行级展开（C++ 缺行级 `&frag` splice） | 未做（C 已与 Rust 同组） | P1 | T01 | `@base {a:1}` + `w { c:3` + `&base }` 在 C++ 与 Rust 同树；`10`/`11` 转 AGREE | `TODO.md:1181` | 实现者（✋ 需拍板是否动 C++） |
| T44 | W6 Lua 走 native 绑定做契约 | 挂起（用户已定「native 可先不做」） | P2 | 无 | 无需完成；保持口径「能力已由纯 Lua 实现，本条只是性能/一致性收益」 | `TODO.md:1184` | 用户（已裁决不做） |
| T45 | W7 JS / 编辑器侧一次报多条错误 | 未做（Rust 已做 `collect_errors`） | P2 | T01 | JS `parse` 返回多错 + 编辑器一次显示；旧 `parse()` 行为不变 | `TODO.md:1185` | 实现者 |
| T46 | W22 文档与扩展对账 | 未做（对账清单未出） | P1 | T14 | 先出「新增项 → 该出现在哪个文件」清单，再逐处更新，`_prepublish.mjs` 全绿 | `TODO.md:1211` | 实现者 |
| T47 | W23 JS include 架构（不带子文件片段/契约/类型表） | 未做 | P1 | T01 | `examples/app.sml` 与「契约在被包含文件里」两例在 JS 与 Rust 同树，既有 `E-INCLUDE-*` 不回归 | `TODO.md:1212` | 实现者 |
| T48 | Lua 三引号丢键 + `@feature … # 注释` 吞下一行 | 未做（用户已定「先不动 Lua」） | P2 | 无 | 两条用例在 Lua 下输出含 `x`，与 Rust/JS 同树 | `TODO.md:915`、`TODO.md:917` | 用户 |
| T49 | 私库非交互推送（SSH / GCM / Token） | 未做（现 HTTP 明文 + 无凭据助手） | P2 | 无 | 配 SSH key（22 端口已开）或 token 后 `git push` 无交互成功 | `TODO.md:1123` | 用户 |
| T50 | v5 定稿（`5.0` 启用 + 表 A） | 部分完成：实现已先行（`type.featag` 33 处、`__featags` 31、`from-featag` 37、`strict-paths` 28），文档表 A 仍写草案 | P0 | T33 / I-6 | `@version 50 "camelia" dev` 不再报 `E-FEATURE-004`；表 A 11 条逐条实测通过；B6–B10 文档回填 | `docs/v5-syntax.md:34`、`docs/unfinished.md:58` | 用户（v5 何时定稿/开放） |

---

## 表 1（续）· TODO 已完成 70 条（Done，别再当欠账）

| 编号 | 主题 | 状态 | 优先级 | 依赖 | 完成判据 | 来源(file:行) | 卡在谁 |
|---|---|---|---|---|---|---|---|
| D01 | 契约校验接入编辑器 | Done（含尾巴：编辑器是否呈现语义错误仍未验证） | — | 无 | JS 侧契约表 / `@is` / 默认值填充在位；编辑器呈现另算 | `TODO.md:13` | 无 |
| D02 | 一次报出多条错误 | Done | — | 无 | `collect_errors(true)` + `rust/tests/parse_multi_errors.rs` 5 条；`@for` 已并入 | `TODO.md:16` | 无 |
| D03 | LSP server | Done | — | 无 | `editors/lsp/server.mjs` 提供 diagnostics / completion / definition / hover | `TODO.md:22` | 无 |
| D04 | 跳转到契约定义 | Done | — | 无 | `findDefinition` + `DefinitionProvider`；`@is X`→`@contract X`、`&f`→`@f { }` | `TODO.md:24` | 无 |
| D05 | 悬停显示契约展开结果 | Done | — | 无 | `contractHoverMarkdown` 两段；无实例时明说「未找到」 | `TODO.md:29` | 无 |
| D06 | 特别高亮（临时探照灯） | Done | — | 无 | 两条命令 + 三个配置项 + 状态栏计数 | `TODO.md:32` | 无 |
| D07 | 重打 VSIX 并装到本机 | Done | — | 无 | 0.4.2 实测四项功能正常；后续两次重打指纹已记 | `TODO.md:35` | 无 |
| D08 | 契约互斥组 `one_of` / `exactly_one` | Done | — | 无 | `rust/tests/contract_mixin.rs` 11 条 | `TODO.md:77` | 无 |
| D09 | 数组长度约束 `items_min` / `items_max` | Done | — | 无 | `items_min` 内置（`ItemsMin`）；`items_max` 留作教科书外置示例 | `TODO.md:78` | 无 |
| D10 | 契约跨文件（include 复用） | Done（四端未对齐部分归 T01） | — | 无 | `rust/tests/contract_cross_file.rs` 3 条 + 教科书 §5.4.2 | `TODO.md:79` | 无 |
| D11 | 叠加结果与书写顺序无关 | Done | — | 无 | v0.6.3 落地，`contract_mixin.rs` 11 条 | `TODO.md:107` | 无 |
| D12 | 叠加冲突码 `E-CONTRACT-016` | Done | — | 无 | 码表 + 同一批用例 | `TODO.md:108` | 无 |
| D13 | Lua `@version v1` 未处理 | Done | — | 无 | `soupx lua/main.lua showcase.sml` 解析出 `address`；`v2` 报错 | `TODO.md:140` | 无 |
| D14 | 系统性核对特性支持矩阵 | Done | — | 无 | `docs/capability-matrix.md`（每格附实跑命令与输出） | `TODO.md:156` | 无 |
| D15 | 键位置上的裸 `:` | Done（五端文案同步并入 T01） | — | 无 | JS / C 同报 `E-PARSE-006`；Rust 文案点名 `::` 并给加引号提示 | `TODO.md:171` | 无 |
| D16 | `smltools -i 裸文件名` 报 `E-INCLUDE-010` | Done | — | 无 | 三处 `parse_file*` 空 parent 回落 `"."`；另三端核对无此病 | `TODO.md:205` | 无 |
| D17 | `sml-parse::ext` 扩展点 | Done | — | 无 | `Directive` / `Outcome` / `Diagnostic` / `DirectiveTable` / `parse_with` | `TODO.md:274` | 无 |
| D18 | `sml-contract::ext` 扩展点 | Done | — | 无 | `TypeCheck` / `ContractExt` / `TypeSpec::Ext` / `FieldSpec.ext` | `TODO.md:276` | 无 |
| D19 | `swsml` 门面导出扩展点 | Done | — | 无 | `sml::ext` / `sml::contract_ext` | `TODO.md:278` | 无 |
| D20 | `sml-parse` / `sml-contract` 升 `0.1.0-alpha.2` | Done | — | 无 | 两 crate 版本与破坏性变更说明 | `TODO.md:279` | 无 |
| D21 | `Modifier` 扩展点 | Done | — | 无 | 解析期 `apply` + 校验期 `check`；数组字段同样跑回调 | `TODO.md:284` | 无 |
| D22 | JS 侧同步外置扩展 | Done | — | 无 | `parse(text, { directives, types, warnings })`；三处副本同步 | `TODO.md:289` | 无 |
| D23 | 清 `sml-parse` unused / dead_code | Done | — | 无 | 四种配置 `cargo check -p sml-parse` 均 0 警告 | `TODO.md:323` | 无 |
| D24 | 教科书「外置扩展」+ CHANGELOG | Done | — | 无 | `site/content/zh/book/ch13-extension.md` 13.1–13.9；CHANGELOG 第 31–66 行 | `TODO.md:324` | 无 |
| D25 | README 顶层数组示例 | Done（原本就是「登记了未回填」） | — | 无 | `README.md:85-86` 时间戳已加引号 | `TODO.md:939` | 无 |
| D26 | 能力矩阵三处「过时」 | Done（复核三条都不成立） | — | 无 | 真实落点 `ch14` 与 README 已改；矩阵 §1.6 已有嵌套数组行 | `TODO.md:941` | 无 |
| D27 | 台账 #13（JS `\uXXXX`）转已修复 | Done（C++ 半未修，已在同条注明） | — | 无 | 台账该行标注 JS 已修 `!16`、C++ 只认 `\u{...}` | `TODO.md:948` | 无 |
| D28 | 台账已修条目同步 | Done | — | 无 | #9 / #13 / #16 三行改已修；基线 41 例（后一次复核 34–7） | `TODO.md:950` | 无 |
| D29 | Rust `to_sml` 不转义 NUL | Done（C 半未动，已登记） | — | 无 | `\n`/`\t`/`\r`/`\0` + `\u00XX`；`rust/tests/dump_escapes.rs` 3 条 | `TODO.md:955` | 无 |
| D30 | JS 组合契约子契约默认值不回填 | Done | — | 无 | `applyDefaults` 递归；`16_contract_compose` 输出 `inner: { x: 1, tag: t }` | `TODO.md:959` | 无 |
| D31 | JS 未开 typed-block 时无 `__type` / `__name` | Done | — | 无 | `30_typed_block_meta` 输出带两键 | `TODO.md:962` | 无 |
| D32 | 自动补全跟上新修饰符 | Done | — | 无 | 内置组 / 外置组拆分 + 严格度档位；l10n 12 条，0 missing | `TODO.md:990` | 无 |
| D33 | vendor 副本同步 + `EXPECT_VENDOR` 修正 | Done | — | 无 | `check_js_copies.py --fix` 拉齐（115667 B）；prepublish 8 项全过 | `TODO.md:998` | 无 |
| D34 | 性能基准已建 | Done | — | 无 | `rust/benches/perf.rs` + `docs/perf-baseline.md`（三组负载 + 结论） | `TODO.md:1017` | 无 |
| D35 | 第一处 A/B（无 `@` 快路径） | Done | — | 无 | small −53% / large −48% / deep −35% | `TODO.md:1022` | 无 |
| D36 | 第二处 A/B（`compute_string_spans` 惰性） | Done | — | 无 | 含 `@` 大文档 −36%（10.31 → 6.59 ms） | `TODO.md:1027` | 无 |
| D37 | 第三处（深嵌套）量清 | Done（结论：不该在解析器里改） | — | 无 | 每层恒定 377 ns；`flat@120` 433 ns/键，瓶颈是每键分配 | `TODO.md:1031` | 无 |
| D38 | 快路径推广到 7 个入口 | Done | — | 无 | 共用一个 `no_directives` 判据与三个共用构造函数 | `TODO.md:1036` | 无 |
| D39 | 分配归因 | Done | — | 无 | 零依赖 `GlobalAlloc` 计数器：每键 6.9 次分配 | `TODO.md:1039` | 无 |
| D40 | `smlkit-easy` L0 | Done | — | 无 | `Doc` + `read`/`read_file`/`raw`/`has`/`str`/`int`/`float`/`bool`/`list`/`to_sml`；7 条集成 + 6 doctest | `TODO.md:1060` | 无 |
| D41 | `swsml` 根导出 `SmlError` | Done | — | 无 | `pub use sml_codes::SmlError` | `TODO.md:1063` | 无 |
| D42 | `smlkit-easy` L1 | Done | — | 无 | `Contracts { On, Off }` + `read_with` / `read_file_with`；4 条用例 | `TODO.md:1065` | 无 |
| D43 | `smlkit-easy` L2 + `out::` 六后端 | Done | — | 无 | `tests/report.rs` 6 条 + doctest 9 条；两档编译零警告 | `TODO.md:1071` | 无 |
| D44 | 发布前查名 | Done | — | 无 | `smlkit-easy` / `smlkit` 三生态 404 | `TODO.md:1084` | 无 |
| D45 | Rust 顶层数组解析 | Done | — | 无 | `parse_impl` 支持 `[` / `{` / 键值三种顶层形态 | `TODO.md:1141` | 无 |
| D46 | Rust 词中 `@` 保留 | Done | — | 无 | 仅词首为片段标记 | `TODO.md:1142` | 无 |
| D47 | Rust 顶层形态 4 条回归 | Done | — | 无 | 顶层数组 / 标量数组 / 顶层对象 / 空数组 | `TODO.md:1143` | 无 |
| D48 | Rust 邮箱 3 条回归 | Done | — | 无 | 邮箱裸词 / 邮箱往返 / 片段定义 | `TODO.md:1144` | 无 |
| D49 | C 两处修复 + `@version` | Done | — | 无 | 顶层数组 / 词中 `@` / `@version` 支持 | `TODO.md:1145` | 无 |
| D50 | JS 顶层数组 + `@` token 化 + `@version` | Done | — | 无 | 此前完全不识别 `@`（片段定义失效） | `TODO.md:1146` | 无 |
| D51 | Lua 词中 `@` + 顶层数组 + dump 对齐 | Done | — | 无 | `dump` 顶层数组输出 `[ ]` 而非 `1: { }` | `TODO.md:1147` | 无 |
| D52 | README 片段示例修正 | Done（原前提错，已按实测重写） | — | 无 | Rust 展开块内独立 `&base`；C/C++ 不展开（C++ 见 T43） | `TODO.md:1148` | 无 |
| D53 | README 顶层形态与词中 `@` 规则 | Done | — | 无 | 中英两版同步 | `TODO.md:1152` | 无 |
| D54 | `showcase.sml` | Done | — | 无 | 已用 JS/C 验证通过 | `TODO.md:1153` | 无 |
| D55 | Rust 契约实现 | Done | — | 无 | `@contract` / `@is` + 类型/枚举/区间/数组元素校验 + 默认值 | `TODO.md:1154` | 无 |
| D56 | Rust 契约测试 12 项 + showcase 1 项 | Done | — | 无 | `tests/contract.rs`、`tests/contract_showcase.rs` | `TODO.md:1156` | 无 |
| D57 | README 契约章节 | Done | — | 无 | 中英两版 | `TODO.md:1157` | 无 |
| D58 | `showcase_contract.sml` | Done | — | 无 | 已用 Rust 验证通过 | `TODO.md:1158` | 无 |
| D59 | JS 契约 `[T]` 简写 + 嵌套数组 | Done | — | 无 | `js/_probe_contract_types.mjs` 19 例；四份副本同步 | `TODO.md:1159` | 无 |
| D60 | VSCode 跳转 + 悬浮（applyDefaults 后） | Done | — | 无 | 取不到实例时明说「未找到」，不编 | `TODO.md:1164` | 无 |
| D61 | `dev` 分支已建 | Done | — | 无 | `git branch dev main` | `TODO.md:1265` | 无 |
| D62 | `dev` 推送 Gitee | Done（GitHub 同步欠着，归 T30） | — | 无 | 随四条 PR 合入落在远端 | `TODO.md:1266` | 无 |
| D63 | 四条工作分支合入 `dev` 并删除 | Done | — | 无 | 本地 worktree 与分支已清，主工作区在 `dev` | `TODO.md:1268` | 无 |
| D64 | `feat/bindings-v2` 已删 | Done | — | 无 | `git cherry` 确认 5 笔等价存在于 `dev` | `TODO.md:1278` | 无 |
| D65 | 两条考古分支判定作废 | Done（远端仍留作重写备份） | — | 无 | `git cherry -v` 证据表已记 | `TODO.md:1283` | 无 |
| D66 | 工作树脱敏 | Done | — | 无 | 49 个文件 + 两个漏网产物，`ae0b400` + `e63b86b` | `TODO.md:1333` | 无 |
| D67 | 历史重写 | Done | — | 无 | blob / 提交信息 / 路径三处同换，覆盖全部 refs | `TODO.md:1335` | 无 |
| D68 | 隐私闸门两道 | Done | — | 无 | 私有报送件 + 隐私串；`python tools/check_private_assets.py` | `TODO.md:1336` | 无 |
| D69 | 守卫挂进 CI `guards` job | Done | — | 无 | CI 只跑通用规则（无 `tools/.private_patterns`） | `TODO.md:1341` | 无 |
| D70 | 两台远端强推 | Done（Gitee 已复核；GitHub 不可达） | — | 无 | Gitee `dev=c411211` / `main=1988fb7` 与本地一致 | `TODO.md:1344` | 无 |

---

## 表 2 · IDEAS（23 条）

| 编号 | 名称 | 状态 | 类型 | 依赖 | 下一步 | 来源(file:行) |
|---|---|---|---|---|---|---|
| I-1 | 自定义结构体的序列化 trait | Done（2026-10-27；含一个 TBD） | 语言 / 序列化 | 无 | 决定是否补 serde `Serializer` 层（现为「每类型代码生成」）；不补就把现状写进文档 | `IDEAS.md:14` |
| I-2 | Rust 原生契约，可选无视文件内契约 | Done（opt-out 2026-11-02） | 语言 / 契约 | 无 | 残余：`parse_file` 不接收 `ParseOptions`，opt-out 目前只走文本入口 | `IDEAS.md:60` |
| I-3 | `get_object` 之类访问器 | Done（已合入 `dev`，分支已删） | API | 无 | 无（`as_object`/`as_array`/`as_i64`/`as_bool` + `[idx]` 路径已齐） | `IDEAS.md:79` |
| I-4 | 注释双语化（英文在前） | Done（2026-11-02 全仓归零） | 文档 / 注释 | 无 | 未做仅剩码表 151 处生成物 doc（须改 `errors/gen_*.py` 模板并重生成） | `IDEAS.md:97` |
| I-5 | `weakbind`：插件认契约、库忽略 | Partial（2026-11-04，仅 Rust） | 语言 / 插件通道 | 编辑器提示 + 多端同步 | 兑现用途必须同步 vsix（否则只能验证语义）；与 I-15 的边界判据先落地 | `IDEAS.md:180` |
| I-6 | `featag`：带参块头标签 | 文档写 `Blocked`，**实测 Rust 已落地**（`__featags` / `from-featag` / `!strict-paths` 均在代码里） | 语言 / 块头 | T50（v5 定稿） | 回填 IDEAS 状态；四端按 S9 明确「不实现、报不支持」；补规范页 | `IDEAS.md:292`、`docs/featag-impl-slices.md:19` |
| I-7 | 契约约束的精确控制 | Done（`x_only` 2026-11-02 落地） | 语言 / 契约 | 无 | 无；剩余只有「五端同步」（归 T01） | `IDEAS.md:340` |
| I-8 | TOML 风格 `=` 分隔 | Todo（`[name]` 作块名已搁置） | 语言 / 词法 | 五端词法 + `@feature` opt-in | 先裁决「特性未开启时是否报错」，再动词法；`[name]` 维持搁置 | `IDEAS.md:452` |
| I-9 | 近似契约名提示 | Done（运行时）；**编辑器侧未做** | 语言 / 诊断 | 无 | 补 lsp / VSCode 标黄；外置契约类型与指令名进候选池 | `IDEAS.md:465` |
| I-10 | 团队搭建与宣传 | Partial（能力矩阵已产出） | 文档 / 宣传 | 能力矩阵（已完成） | 补 `CONTRIBUTING.md` / issue·PR 模板 / 3 分钟入口页；宣传语不得越过能力矩阵 §5 | `IDEAS.md:500` |
| I-11 | 中文支持完善且可剥离 | Partial（Rust 侧十二批清零） | 语言 / i18n | 无 | 非 Rust 端硬编码文案（JS / C / Lua）另开一轮；`zh` feature 已可剥离 | `IDEAS.md:520` |
| I-12 | 改名工作 | Rejected（2026-11-03 用户裁决） | 治理 / 命名 | 无 | 不再提；好名字由新 crate 承载（I-19 `swml`） | `IDEAS.md:554` |
| I-13 | 实现即文档 | Partial（只落了一个示范） | 流程 / 文档 | 无 | 每落一个能力补「语义 + 例子 + 陷阱」，例子必须可跑（doctest 或测试） | `IDEAS.md:583` |
| I-14 | 仓库排雷行动 | Partial（可修项基本修完） | 质量 / 排雷 | 无 | 剩余为「已定性不做」或「结构上做不到」；跨端项归 T01 | `IDEAS.md:601` |
| I-15 | `weakdata` 扩展数据通道 | Partial（2026-11-04，仅 Rust） | 语言 / 数据通道 | 多端同步 + 扩展 | 与 I-5 先划边界；落地后按 I-13 补「语义 + 例子 + 陷阱」 | `IDEAS.md:629` |
| I-17 | `weakdoc`：`.smlex` profile 引用 | Partial（库侧已实现，工具侧未做） | 语言 / profile | 工具侧发现规则 | 补三命名空间发现 + `--profile-dir`；把「不注入」写成可测判据 | `IDEAS.md:652` |
| I-16 | 版本后缀与发布门禁 | 方案已定；门禁未做 | 发布治理 | CI | 做 `--check-version`（当前 `smltools` **无此参数**）拦「没到门槛就 bump」 | `IDEAS.md:677` |
| I-18 | `.smlb` 二进制 + `smldb` | **文档写 Todo「零实现」= 过期**；实测 T0–T6 已落（`rust/smlb/` 存在） | 格式 / 持久化 | 设计前提：`Value` 还是 HTML 式节点模型 | T7 五端解码器（JS/Lua/C/C++ 均无 `SWSB` 命中）+ `smldb`；先定 FTS / `:has()` / `id` 唯一 / 规模信封 | `IDEAS.md:702`、`docs/unfinished.md:69` |
| I-19 | 品牌门面 `swml`（原 `smlkit`） | 名字已定（2026-11-06）；**crate 仍未建** | 命名 / 发布 | T18 / T29 | 新建 `swml` 0.1.0（独立仓、依赖 crates.io、`publish` 按纪律）；`smlkit-easy` 是否改 `swml-easy` 待裁 | `IDEAS.md:727` |
| I-20 | 长文档产出链 | Partial（`--toc` + 锚点已落） | 工具 / 长文档 | 无 | 仍缺 `--to epub`、`--split-by section`、增量 emit、emit 源定位 | `IDEAS.md:766` |
| I-21 | 语义部件目录 | Partial（跨后端漂移三修已落） | 工具 / 语义部件 | 无 | 六类部件：figure / admonition / footnote / deflist / sub·sup / toc 生成 | `IDEAS.md:792` |
| I-22 | 下游方言六条设计问题 | Todo（**有对外发布禁令**） | 语言设计 / 下游 | 用户明确指示 | 只在内部讨论；六问逐条给「采纳 / 不采纳 / 待定」的说法 | `IDEAS.md:816` |
| I-23 | 更激进的性能优化 | `Todo` 标签已过期：基准 + 两处 A/B 已落（small −53%），剩表示层 | 性能 | T16 / I-18 | 动表示层（`Tok` 借用 / interning / inline map）前先做 T17 的层级判定 | `IDEAS.md:830` |

---

## 3. 复核对账（本表数字怎么来的）

```bash
# 仓库根
Select-String TODO.md '^\s*- \[ \]'                        # 42
Select-String TODO.md '^\s*- \[x\]'                        # 70
Select-String IDEAS.md '^## I-\d+'                         # 23
Select-String errors/codes.sml '\{ id: [EWI]-'             # 166
Select-String errors/codes.sml '^\s*impls:\s*\[\s*rust\s*\]'  # 47（语言层 39 + DERIVE 8）
python tests/conformance/run_conformance.py                # 41 例，AGREE/DIVERGE 现算

# 工具能力是否存在（0 命中 = 不存在）
Get-ChildItem rust/smltools -Recurse -Include *.rs | Select-String 'check-version'  # 0 → T16 门禁未做
Get-ChildItem rust/smltools -Recurse -Include *.rs | Select-String 'split-by'       # 0 → I-20 未做
Get-ChildItem rust/smltools -Recurse -Include *.rs | Select-String '"docs"'         # 3 处均为 --hugo-section / --zola-section 默认值，无 docs 子命令 → T42 未做
```
