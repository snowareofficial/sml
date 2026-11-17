# docs 索引

> **本文件由 `cargo run -p smldocs -- --index` 生成，勿手改。**
> 数据源 `docs/meta.sml`（64 份文档，摘要 `f00de98b442430d1`）。摘要变了就必须重跑生成；
> 本文件不记生成日期 / HEAD，避免跨天或提交后假漂移。体检结论见 [`report.md`](report.md)。

## 一眼看结论

- 断链 **0** 处；未登记 **0** 份；`updated` 超期 **0** 份；`stale` 无替代者 **0** 份。
- 状态口径：`current` 可引用；`draft` 只能看不能用；`stale` 已作废；`archive` 只作历史。

## 怎么读

1. 只想用 SML → 读站点教程（`site/content/*/book/`），不要进 `docs/`；
2. 想知道语法怎么定 → 本表 `swsml/spec/` 下 `status: current` 的那几份；
3. 想动手实现 → `*/plan/`；想查「还剩什么」→ `repo/ref/unfinished.md`；
4. 只想问「当年为什么这么定」→ `*/archive/`（只作历史，勿照做）。

> 维护规则：新增文档必须先登记 `docs/meta.sml`；正文不写历史（历史进 `docs/changes.sml`）；
> 数字不手写，引用生成物或实测命令。`--check` 进 CI，任一命中即红。

## swsml（20 份）

| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |
|---|---|---|---|---|---|
| [`swsml/archive/idyml-rejected.md`](swsml/archive/idyml-rejected.md) | 2026-10-15 | 2026-11-03 | **archive** | — | idyml 改名方案（2026-11-03 否决） |
| [`swsml/archive/iter-parse.md`](swsml/archive/iter-parse.md) | 2026-10-30 | 2026-11-02 | **archive** | — | 迭代式解析改造（已完成并合入 dev） |
| [`swsml/plan/crossend-sync.md`](swsml/plan/crossend-sync.md) | 2026-11-12 | 2026-11-17 | **current** | — | 跨端同步施工切片：码表分类、weak 三兄弟 / Edition 四端、验收命令与开工裁决；§7 后续十片交接清单（P8–P17，P8–P11 已完成） |
| [`swsml/plan/featag-slices.md`](swsml/plan/featag-slices.md) | 2026-11-04 | 2026-11-15 | **current** | — | featag 实现切片 S0–S9（Rust 全部收官；四端已补 S2–S5b，S6–S8 仍只有 Rust；示例按「每项都带 `!`」的新写法） |
| [`swsml/plan/features-plan.md`](swsml/plan/features-plan.md) | 2026-11-04 | 2026-11-06 | **stale** | `docs/swsml/spec/features.md` | v5 版本位落地步骤与被推翻的旧清单 |
| [`swsml/plan/perf-plan.md`](swsml/plan/perf-plan.md) | 2026-11-06 | 2026-11-08 | **current** | — | 全面优化计划：时间 / 内存 / 缓存 / 指令集四线 |
| [`swsml/ref/capability-matrix.md`](swsml/ref/capability-matrix.md) | 2026-10-27 | 2026-11-17 | **draft** | — | 五端能力矩阵（2026-10-27 快照 + 2026-11-17 增量核对；只作历史对照，v5 版本口径待补） |
| [`swsml/ref/grammar-divergences.md`](swsml/ref/grammar-divergences.md) | 2026-11-14 | 2026-11-16 | **current** | — | grammar.sml 按语法特性切片后的五端差异台账：第一处拒绝、码是否同、能力缺口归因；含 contract-multi 五端同源口径、featag 的 `!` 唯一标记与区位置收敛、@type 四端落地（NFA + Unicode 表）、JS/Lua dump 载体 NC；配套 tools/check/probe_grammar.py |
| [`swsml/ref/perf-baseline.md`](swsml/ref/perf-baseline.md) | 2026-11-03 | 2026-11-15 | **current** | — | 性能基线：perf / value / profile / amberice 四份 bench 的正式口径数字；含 2026-11-14 微观归因（hotspots / alloc_split）、2026-11-15 与 serde_json 的对拍（vs_json）与四笔已否决的优化账 |
| [`swsml/ref/silent-divergences.md`](swsml/ref/silent-divergences.md) | 2026-11-14 | 2026-11-15 | **current** | — | 兼容套件检不出的差异：错误码矩阵 / 特性门控 / 值身份三类盲区与逐条取证；多契约已五端实现；配套 tools/check/probe_matrix.py |
| [`swsml/spec/featag.md`](swsml/spec/featag.md) | 2026-11-04 | 2026-11-15 | **current** | — | featag 语法 / 落点 / from-featag：每一项都必须带 `!`（块的 `!` 是唯一标记），featag 区紧跟契约首词；S2–S5b 四端已实现，S6–S8 仍只有 Rust |
| [`swsml/spec/features.md`](swsml/spec/features.md) | 2026-11-04 | 2026-11-16 | **current** | — | 特性命名空间：20 个特性（含 type.featag / lex.ident / type.weak）、4 个域、两段上限、新增特性带域；v5 边界 |
| [`swsml/spec/null.md`](swsml/spec/null.md) | 2026-11-04 | 2026-11-06 | **current** | — | null / none 五条语义与五端现状 |
| [`swsml/spec/smlex.md`](swsml/spec/smlex.md) | 2026-11-04 | 2026-11-10 | **current** | — | .smlex 内容格式 v1.0（格式定稿；checks 与 applies_to 提示已接 --lint，hints / emit 未实现） |
| [`swsml/spec/v5.md`](swsml/spec/v5.md) | 2026-11-04 | 2026-11-16 | **draft** | — | v5 变更总表（工作底稿，v5 未定稿；B6/B7 已落地，表 A 待定稿实测；A2 示例按「每项都带 `!`」）；§0 边界：0.6.4 之后的内容一律归 v5，随 0.7 定稿 |
| [`swsml/spec/value-layout.md`](swsml/spec/value-layout.md) | 2026-11-02 | 2026-11-06 | **draft** | — | 值层内存布局提案（未拍板，需二次蒸馏） |
| [`swsml/spec/version.md`](swsml/spec/version.md) | 2026-11-04 | 2026-11-16 | **current** | — | 版本 = 功能集（edition）；版本位与三态生命周期（五端字面量已同步，见 §7）；§2.1 版本序列与 v5 边界（0.6.9 是 0.7 前最后一个 0.6，0.6.4 之后归 v5） |
| [`swsml/spec/weakbind.md`](swsml/spec/weakbind.md) | 2026-11-04 | 2026-11-13 | **current** | — | weakbind 语法 / 作用域 / 旁路表 / 默认忽略口径（语言层五端已实现；C-ABI / 绑定层见 crossend-sync.md） |
| [`swsml/spec/weakdata.md`](swsml/spec/weakdata.md) | 2026-11-04 | 2026-11-12 | **current** | — | weakdata 语法与判别（Rust 已实现；多端同步见 crossend-sync.md） |
| [`swsml/spec/weakdoc.md`](swsml/spec/weakdoc.md) | 2026-11-04 | 2026-11-13 | **current** | — | weakdoc 语法与 .smlex 发现 / W-CLI-001 提示（库侧、CLI 发现与提示均已实现） |

## amberice（13 份）

| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |
|---|---|---|---|---|---|
| [`amberice/archive/design-origin.md`](amberice/archive/design-origin.md) | 2026-11-06 | 2026-11-06 | **archive** | — | .ambr 设计起点与用户原话汇编（A/B/C 三级，逐条带出处） |
| [`amberice/decisions.md`](amberice/decisions.md) | 2026-11-02 | 2026-11-11 | **current** | — | .ambr 决策记录：D1–D55 已裁决、P12 待定、§12 草案、§15 键序、§16.3 已定（D53）、§16.4 孪生格式、§16.5 写路径已裁（D55）/ 其余待裁 |
| [`amberice/plan/interrupt-path-layers.md`](amberice/plan/interrupt-path-layers.md) | 2026-11-12 | 2026-11-12 | **draft** | — | 随时中断 / 操作路径 / 分层与 trait 的创新讨论纪要（支点 8 条、未收口 U1–U9、用户原话、示例代码形态）；不作结论 |
| [`amberice/plan/multi-writer.md`](amberice/plan/multi-writer.md) | 2026-11-12 | 2026-11-12 | **draft** | — | 多写者 / 数据属性 / 写路径讨论记录（判据 / 设计空间 / 排除项 / 未实测，附录为用户原话）；不作结论 |
| [`amberice/plan/nostd-subset-eval.md`](amberice/plan/nostd-subset-eval.md) | 2026-11-17 | 2026-11-17 | **current** | — | P17 no_std amberice 评估：逐模块 std 占用取证、可裁 / 必留 std / core+alloc 接口最小面与 P16 前置；只出评估不动代码 |
| [`amberice/plan/plan.md`](amberice/plan/plan.md) | 2026-11-06 | 2026-11-10 | **current** | — | amberice 改名范围 + 第一批优化（A1）与里程碑 |
| [`amberice/ref/claims-audit.md`](amberice/ref/claims-audit.md) | 2026-11-10 | 2026-11-10 | **draft** | — | 45 条论断逐条核实（成立 / 需修正 / 未实测 / 判断）；draft 不作结论，去留待用户裁决 |
| [`amberice/ref/ports.md`](amberice/ref/ports.md) | 2026-11-06 | 2026-11-13 | **current** | — | 多端 .ambr 解码器进度台账（全端 22/22 已复测并进 CI；AMBR 错误码域；摘要仅 SHA-256） |
| [`amberice/ref/swsml-alignment.md`](amberice/ref/swsml-alignment.md) | 2026-11-13 | 2026-11-13 | **current** | — | amberice 与 swsml 的对齐面（edition / 特性位 / 码表 / C-ABI 刻意差异 / 支持平台 / AMBR 码）；守护 tools/check/check_amberice_align.py |
| [`amberice/spec/format.md`](amberice/spec/format.md) | 2026-11-02 | 2026-11-13 | **current** | — | .ambr 格式规范：信封 / canonical / VALUE 段 / 算法表 / 默认值 / 侧信道 / AMBR 错误码域 |
| [`amberice/spec/overview.md`](amberice/spec/overview.md) | 2026-11-10 | 2026-11-10 | **current** | — | amberice 是什么：以源码为唯一事实来源的产品线定义（边界 / 格式 / 能力面 / 多端 / 实现状态） |
| [`amberice/spec/smldb.md`](amberice/spec/smldb.md) | 2026-11-02 | 2026-11-17 | **draft** | — | smldb 数据库层：表结构 / 后端选型 / 选择器 / 四层 API（T8 第一刀已落：Turso 后端 + BLOB + 精确路径/标签索引；FTS/选择器未定） |
| [`amberice/spec/tools.md`](amberice/spec/tools.md) | 2026-11-02 | 2026-11-10 | **current** | — | .ambr 配套工具面：子命令清单以 amberice --help 为准，另含五端解码器 |

## swml（3 份）

| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |
|---|---|---|---|---|---|
| [`swml/plan/api-draft.md`](swml/plan/api-draft.md) | 2026-11-02 | 2026-11-06 | **draft** | — | 门面未定项：根命名空间债务 / 命名二义 / feature 传播 / 发布口径 |
| [`swml/spec/api.md`](swml/spec/api.md) | 2026-11-02 | 2026-11-09 | **current** | — | swml 门面公开面：根 = swsml 全集，easy = 四个概念 |
| [`swml/status.md`](swml/status.md) | 2026-11-06 | 2026-11-10 | **current** | — | swml 发布状态（dev2 成批发 / publish=false）+ 门面源码与对外仓分工 |

## repo（25 份）

| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |
|---|---|---|---|---|---|
| [`repo/archive/doc-defects.md`](repo/archive/doc-defects.md) | 2026-11-04 | 2026-11-04 | **archive** | — | P1 文档盘点结果（2026-11-04 快照） |
| [`repo/archive/doc-reconcile-2026-10-28.md`](repo/archive/doc-reconcile-2026-10-28.md) | 2026-10-28 | 2026-11-03 | **archive** | — | 2026-10-28 文档对账的一次性记录 |
| [`repo/archive/handoff/HANDOFF-2026-10-26.md`](repo/archive/handoff/HANDOFF-2026-10-26.md) | 2026-10-26 | 2026-10-26 | **archive** | — | 会话交接（2026-10-26） |
| [`repo/archive/handoff/HANDOFF-2026-10-27.md`](repo/archive/handoff/HANDOFF-2026-10-27.md) | 2026-10-27 | 2026-10-27 | **archive** | — | 会话交接（2026-10-27） |
| [`repo/archive/handoff/HANDOFF-2026-10-28-2.md`](repo/archive/handoff/HANDOFF-2026-10-28-2.md) | 2026-10-28 | 2026-10-28 | **archive** | — | 会话交接（2026-10-28 第二份） |
| [`repo/archive/handoff/HANDOFF-2026-10-28.md`](repo/archive/handoff/HANDOFF-2026-10-28.md) | 2026-10-28 | 2026-10-28 | **archive** | — | 会话交接（2026-10-28） |
| [`repo/archive/handoff/HANDOFF-2026-10-30-2.md`](repo/archive/handoff/HANDOFF-2026-10-30-2.md) | 2026-10-30 | 2026-10-30 | **archive** | — | 会话交接（2026-10-30 第二份） |
| [`repo/archive/handoff/HANDOFF-2026-10-30-3.md`](repo/archive/handoff/HANDOFF-2026-10-30-3.md) | 2026-10-30 | 2026-10-30 | **archive** | — | 会话交接（2026-10-30 第三份） |
| [`repo/archive/handoff/HANDOFF-2026-10-30.md`](repo/archive/handoff/HANDOFF-2026-10-30.md) | 2026-10-30 | 2026-10-30 | **archive** | — | 会话交接（2026-10-30） |
| [`repo/archive/handoff/HANDOFF-2026-11-02.md`](repo/archive/handoff/HANDOFF-2026-11-02.md) | 2026-11-02 | 2026-11-02 | **archive** | — | 会话交接（2026-11-02，含待议清单 §4.5） |
| [`repo/archive/handoff/HANDOFF-2026-11-05.md`](repo/archive/handoff/HANDOFF-2026-11-05.md) | 2026-11-05 | 2026-11-05 | **archive** | — | 会话交接（2026-11-05，.ambr T0–T6 交接） |
| [`repo/archive/handoff/HANDOFF-2026-11-15.md`](repo/archive/handoff/HANDOFF-2026-11-15.md) | 2026-11-15 | 2026-11-16 | **archive** | — | 会话交接（2026-11-15）：五端语法收敛 —— 多契约合并视图 / featag 的 `!` 唯一标记与区位置 / `@type` 模式匹配器四端 / 数组位置裸块 / dump 值位置；probe_grammar 4/20 → 1/20；含 I-30 的卡点与下一步 |
| [`repo/archive/handoff/HANDOFF-W24.md`](repo/archive/handoff/HANDOFF-W24.md) | 2026-10-12 | 2026-10-12 | **archive** | — | 会话交接（W24） |
| [`repo/archive/handoff/HANDOFF.md`](repo/archive/handoff/HANDOFF.md) | 2026-09-18 | 2026-11-15 | **archive** | — | 主交接文档（2026-09-18 起的长期记录；顶部索引已指向 HANDOFF-2026-11-15.md） |
| [`repo/archive/open-work.md`](repo/archive/open-work.md) | 2026-11-04 | 2026-11-04 | **archive** | `docs/repo/ref/unfinished.md` | 三次扫描叠加的旧待办清单（已被取代） |
| [`repo/archive/task/TASK-agent-w19.md`](repo/archive/task/TASK-agent-w19.md) | 2026-09-19 | 2026-10-10 | **archive** | — | VSCode 扩展验收任务书（W19） |
| [`repo/archive/task/TASK-agent-w24.md`](repo/archive/task/TASK-agent-w24.md) | 2026-10-12 | 2026-10-12 | **archive** | — | 任务书（W24） |
| [`repo/archive/task/TASK-agent-w8-w9.md`](repo/archive/task/TASK-agent-w8-w9.md) | 2026-09-18 | 2026-10-28 | **archive** | — | 任务书（W8–W9） |
| [`repo/archive/task/TASK-agent.md`](repo/archive/task/TASK-agent.md) | 2026-09-18 | 2026-10-28 | **archive** | — | 任务书（主） |
| [`repo/plan/doc-architecture.md`](repo/plan/doc-architecture.md) | 2026-11-04 | 2026-11-06 | **stale** | `docs/index.md` | 文档四层架构 + P1–P4（提议；已由本次治理落地） |
| [`repo/ref/pending-decisions.md`](repo/ref/pending-decisions.md) | 2026-11-06 | 2026-11-16 | **current** | — | 裁决台账：D1–D7 已全部裁决（含发布拓扑 / 仓库分工 / 官网 / 0.6.9 与 v5 边界）+ 仍待裁项的去处 |
| [`repo/ref/scan.md`](repo/ref/scan.md) | 2026-11-07 | 2026-11-10 | **current** | — | 自动扫描的计数结果（tools/scan_status.py --write 生成，勿手改） |
| [`repo/ref/silent-failures.md`](repo/ref/silent-failures.md) | 2026-11-17 | 2026-11-17 | **current** | — | 静默失败台账：exit 0 但值树/文本已错的 S1–S8（多词裸值 / 全角冒号 / 未知扩展名 / 空产物 / 根块名不认识 / 模板丢参数 / math 透传 / 同名块聚合成数组后模板取空），逐条最小复现 + 修法优先级 |
| [`repo/ref/todo-ideas-tables.md`](repo/ref/todo-ideas-tables.md) | 2026-11-06 | 2026-11-17 | **current** | — | TODO / IDEAS 条目表：卡在谁、什么状态；2026-11-15 增补 I-25…I-30，2026-11-16 增补 I-31 / I-32（wasm·native access server 衍生语言项目 / 项目经验固化为模板），2026-11-17 确认 I-28 的 smlite / eclog-rs 定位与依赖链；2026-11-17 增补 I-34（SML 自动识别：`--from auto` / 文本判定 / MIME 三选，现状只按扩展名）与 I-35（静默失败治理，台账见 `docs/repo/ref/silent-failures.md`） |
| [`repo/ref/unfinished.md`](repo/ref/unfinished.md) | 2026-11-06 | 2026-11-17 | **current** | — | 唯一「还剩什么」清单（分类 / 判据 / 复核命令） |

## smlite（2 份）

| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |
|---|---|---|---|---|---|
| [`smlite/plan/api.md`](smlite/plan/api.md) | 2026-11-17 | 2026-11-17 | **current** | — | smlite crate 骨架与公开 API 设计：自建精简值模型（Float 无 raw / 插入序 Object）、解析器边界、feature 旗帜（swsml-compat / no_std 接力点）与 swsml 值层互转契约；本步只出设计不动代码 |
| [`smlite/ref/perf.md`](smlite/ref/perf.md) | 2026-11-17 | 2026-11-17 | **current** | — | smlite 性能基线：与 serde_json 对拍的口径/三形状数字/分配归因；批量建对象+借用 token 后从落后 2.5-12x 翻转为快 12-29%，不并入 swsml 正式口径 |

## eclog（1 份）

| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |
|---|---|---|---|---|---|
| [`eclog/plan/api.md`](eclog/plan/api.md) | 2026-11-17 | 2026-11-17 | **current** | — | eclog-rs 设计与 3a/3b 落地：Eclog-CPP 全量语法对齐、独立不依赖 smlite、自建带 raw 的 Number 与带记法/定界符的 Str、解析器状态机移植与树/事件双出口、双 scheme 渲染器与 RFC 7396 merge、C++ oracle 逐字节对拍（23 个解析事件 + 46 份渲染期望 + 2 份构造边界）；3c amberice 未开工 |

