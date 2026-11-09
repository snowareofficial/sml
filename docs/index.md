# docs 索引

> **本文件由 `cargo run -p smldocs -- --index` 生成，勿手改。**
> 生成于 2026-11-09，数据源 `docs/meta.sml`（50 份文档）。体检结论见 [`report.md`](report.md)。

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

## swsml（17 份）

| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |
|---|---|---|---|---|---|
| [`swsml/archive/idyml-rejected.md`](swsml/archive/idyml-rejected.md) | 2026-10-15 | 2026-11-03 | **archive** | — | idyml 改名方案（2026-11-03 否决） |
| [`swsml/archive/iter-parse.md`](swsml/archive/iter-parse.md) | 2026-10-30 | 2026-11-02 | **archive** | — | 迭代式解析改造（已完成并合入 dev） |
| [`swsml/plan/featag-slices.md`](swsml/plan/featag-slices.md) | 2026-11-04 | 2026-11-06 | **current** | — | featag 实现切片 S0–S9（S0–S8 已落，S9 其它端待同步） |
| [`swsml/plan/features-plan.md`](swsml/plan/features-plan.md) | 2026-11-04 | 2026-11-06 | **stale** | `docs/swsml/spec/features.md` | v5 版本位落地步骤与被推翻的旧清单 |
| [`swsml/plan/perf-plan.md`](swsml/plan/perf-plan.md) | 2026-11-06 | 2026-11-08 | **current** | — | 全面优化计划：时间 / 内存 / 缓存 / 指令集四线 |
| [`swsml/ref/capability-matrix.md`](swsml/ref/capability-matrix.md) | 2026-10-27 | 2026-11-04 | **draft** | — | 五端能力矩阵（2026-10-27 快照，v5 版本口径待补） |
| [`swsml/ref/perf-baseline.md`](swsml/ref/perf-baseline.md) | 2026-11-03 | 2026-11-08 | **current** | — | 性能基线 v0（只覆盖 parse；value / amberice 两份 bench 未并入） |
| [`swsml/spec/featag.md`](swsml/spec/featag.md) | 2026-11-04 | 2026-11-06 | **current** | — | featag 语法 / 落点 / from-featag（Rust 已实现，其它端未同步） |
| [`swsml/spec/features.md`](swsml/spec/features.md) | 2026-11-04 | 2026-11-06 | **current** | — | 特性命名空间：4 个域、两段上限、新增特性带域 |
| [`swsml/spec/null.md`](swsml/spec/null.md) | 2026-11-04 | 2026-11-06 | **current** | — | null / none 五条语义与五端现状 |
| [`swsml/spec/smlex.md`](swsml/spec/smlex.md) | 2026-11-04 | 2026-11-06 | **current** | — | .smlex 内容格式 v1.0（格式定稿，applies_to / hints / emit 未实现） |
| [`swsml/spec/v5.md`](swsml/spec/v5.md) | 2026-11-04 | 2026-11-06 | **draft** | — | v5 变更总表（工作底稿，v5 未定稿） |
| [`swsml/spec/value-layout.md`](swsml/spec/value-layout.md) | 2026-11-02 | 2026-11-06 | **draft** | — | 值层内存布局提案（未拍板，需二次蒸馏） |
| [`swsml/spec/version.md`](swsml/spec/version.md) | 2026-11-04 | 2026-11-06 | **current** | — | 版本 = 功能集（edition）；版本位与三态生命周期 |
| [`swsml/spec/weakbind.md`](swsml/spec/weakbind.md) | 2026-11-04 | 2026-11-06 | **draft** | — | weakbind 语法 / 作用域 / 旁路表（Rust 单端已实现） |
| [`swsml/spec/weakdata.md`](swsml/spec/weakdata.md) | 2026-11-04 | 2026-11-06 | **draft** | — | weakdata 语法与判别（Rust 已实现，其它端与编辑器未做） |
| [`swsml/spec/weakdoc.md`](swsml/spec/weakdoc.md) | 2026-11-04 | 2026-11-06 | **draft** | — | weakdoc 语法与 .smlex 发现（库侧已实现，工具侧未做） |

## amberice（7 份）

| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |
|---|---|---|---|---|---|
| [`amberice/archive/design-origin.md`](amberice/archive/design-origin.md) | 2026-11-06 | 2026-11-06 | **archive** | — | .ambr 设计起点与用户原话汇编（A/B/C 三级，逐条带出处） |
| [`amberice/decisions.md`](amberice/decisions.md) | 2026-11-02 | 2026-11-09 | **current** | — | .ambr 决策记录：D1–D41 已裁决、P1–P11 待定、§12 草案、§15 键序、§16.3 脱敏与串口场景 |
| [`amberice/plan/plan.md`](amberice/plan/plan.md) | 2026-11-06 | 2026-11-09 | **current** | — | amberice 改名范围 + 第一批优化（A1）与里程碑 |
| [`amberice/ref/ports.md`](amberice/ref/ports.md) | 2026-11-06 | 2026-11-09 | **current** | — | 多端 .ambr 解码器进度台账（C# / Go / Ruby 三端未做） |
| [`amberice/spec/format.md`](amberice/spec/format.md) | 2026-11-02 | 2026-11-09 | **current** | — | .ambr 格式规范：信封 / canonical / VALUE 段 / 算法表 / 默认值 / 侧信道 |
| [`amberice/spec/smldb.md`](amberice/spec/smldb.md) | 2026-11-02 | 2026-11-06 | **draft** | — | smldb 数据库层：表结构 / 后端选型 / 选择器 / 四层 API（全部未开工） |
| [`amberice/spec/tools.md`](amberice/spec/tools.md) | 2026-11-02 | 2026-11-09 | **current** | — | .ambr 配套工具面：子命令清单以 amberice --help 为准，另含五端解码器 |

## swml（3 份）

| 文档 | 写于 | 最后核对 | 状态 | 替代者 | 职责 |
|---|---|---|---|---|---|
| [`swml/plan/api-draft.md`](swml/plan/api-draft.md) | 2026-11-02 | 2026-11-06 | **draft** | — | 门面未定项：根命名空间债务 / 命名二义 / feature 传播 / 发布口径 |
| [`swml/spec/api.md`](swml/spec/api.md) | 2026-11-02 | 2026-11-09 | **current** | — | swml 门面公开面：根 = swsml 全集，easy = 四个概念 |
| [`swml/status.md`](swml/status.md) | 2026-11-06 | 2026-11-06 | **current** | — | swml 发布状态（未发布 / publish=false）与 workspace 口径冲突 |

## repo（23 份）

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
| [`repo/archive/handoff/HANDOFF-W24.md`](repo/archive/handoff/HANDOFF-W24.md) | 2026-10-12 | 2026-10-12 | **archive** | — | 会话交接（W24） |
| [`repo/archive/handoff/HANDOFF.md`](repo/archive/handoff/HANDOFF.md) | 2026-09-18 | 2026-11-05 | **archive** | — | 主交接文档（2026-09-18 至 2026-11-05 的长流水） |
| [`repo/archive/open-work.md`](repo/archive/open-work.md) | 2026-11-04 | 2026-11-04 | **archive** | `docs/repo/ref/unfinished.md` | 三次扫描叠加的旧待办清单（已被取代） |
| [`repo/archive/task/TASK-agent-w19.md`](repo/archive/task/TASK-agent-w19.md) | 2026-09-19 | 2026-10-10 | **archive** | — | VSCode 扩展验收任务书（W19） |
| [`repo/archive/task/TASK-agent-w24.md`](repo/archive/task/TASK-agent-w24.md) | 2026-10-12 | 2026-10-12 | **archive** | — | 任务书（W24） |
| [`repo/archive/task/TASK-agent-w8-w9.md`](repo/archive/task/TASK-agent-w8-w9.md) | 2026-09-18 | 2026-10-28 | **archive** | — | 任务书（W8–W9） |
| [`repo/archive/task/TASK-agent.md`](repo/archive/task/TASK-agent.md) | 2026-09-18 | 2026-10-28 | **archive** | — | 任务书（主） |
| [`repo/plan/doc-architecture.md`](repo/plan/doc-architecture.md) | 2026-11-04 | 2026-11-06 | **stale** | `docs/index.md` | 文档四层架构 + P1–P4（提议；已由本次治理落地） |
| [`repo/ref/pending-decisions.md`](repo/ref/pending-decisions.md) | 2026-11-06 | 2026-11-06 | **current** | — | 裁决台账：五条已全部裁决 + 仍待裁项的去处 |
| [`repo/ref/scan.md`](repo/ref/scan.md) | 2026-11-07 | 2026-11-09 | **current** | — | 自动扫描的计数结果（tools/scan_status.py --write 生成，勿手改） |
| [`repo/ref/todo-ideas-tables.md`](repo/ref/todo-ideas-tables.md) | 2026-11-06 | 2026-11-09 | **current** | — | TODO / IDEAS 条目表：卡在谁、什么状态 |
| [`repo/ref/unfinished.md`](repo/ref/unfinished.md) | 2026-11-06 | 2026-11-09 | **current** | — | 唯一「还剩什么」清单（分类 / 判据 / 复核命令） |

