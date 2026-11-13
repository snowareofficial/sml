# docs/meta.sml —— 全部文档的元数据单一清单
#
# 读者入口不是这个文件，而是由它生成的 docs/index.md（索引）与 docs/report.md（体检报告）。
# 工具：`cargo run -p smldocs -- --index` / `--report` / `--check` / `--fix`
#       （仓库治理内部 crate，publish = false；`smltools` 是给 SML 用户用的产品 CLI）
#
# 字段（一个都不能少）：
#   title      相对仓库根的路径（唯一键）
#   date       写于（首次入库日期）
#   updated    最后核对日期（不是最后修改日期）
#   owner      swsml | amberice | swml | repo
#   status     current | draft | stale | archive | keep
#   scope      一句话职责
#   supersedes      被本文取代的旧文档（没有就 null）
#   superseded_by   取代本文的文档（status = stale 时**必填**）
#
# status 口径：
#   current  权威，可直接引用
#   draft    未定稿：能看，但不是结论
#   stale    已被取代：必须写 superseded_by，正文不再维护
#   archive  一次性记录 / 已完成方案：只作历史
#   keep     例外：被 CI 或构建硬引用、不能按常规生命周期处理
#
# 新增文档必须先在这里登记，否则 `docs --check` 直接红。

docs {
  doc: [
    { title: "docs/amberice/archive/design-origin.md" date: "2026-11-06" updated: "2026-11-06" owner: "amberice" status: "archive" scope: ".ambr 设计起点与用户原话汇编（A/B/C 三级，逐条带出处）" supersedes: null superseded_by: null }
    { title: "docs/amberice/decisions.md" date: "2026-11-02" updated: "2026-11-11" owner: "amberice" status: "current" scope: ".ambr 决策记录：D1–D55 已裁决、P12 待定、§12 草案、§15 键序、§16.3 已定（D53）、§16.4 孪生格式、§16.5 写路径已裁（D55）/ 其余待裁" supersedes: null superseded_by: null }
    { title: "docs/amberice/plan/interrupt-path-layers.md" date: "2026-11-12" updated: "2026-11-12" owner: "amberice" status: "draft" scope: "随时中断 / 操作路径 / 分层与 trait 的创新讨论纪要（支点 8 条、未收口 U1–U9、用户原话、示例代码形态）；不作结论" supersedes: null superseded_by: null }
    { title: "docs/amberice/plan/multi-writer.md" date: "2026-11-12" updated: "2026-11-12" owner: "amberice" status: "draft" scope: "多写者 / 数据属性 / 写路径讨论记录（判据 / 设计空间 / 排除项 / 未实测，附录为用户原话）；不作结论" supersedes: null superseded_by: null }
    { title: "docs/amberice/plan/plan.md" date: "2026-11-06" updated: "2026-11-10" owner: "amberice" status: "current" scope: "amberice 改名范围 + 第一批优化（A1）与里程碑" supersedes: null superseded_by: null }
    { title: "docs/amberice/ref/ports.md" date: "2026-11-06" updated: "2026-11-10" owner: "amberice" status: "current" scope: "多端 .ambr 解码器进度台账（全端已通，含 C# / Go / Ruby；摘要仅 SHA-256）" supersedes: null superseded_by: null }
    { title: "docs/amberice/ref/claims-audit.md" date: "2026-11-10" updated: "2026-11-10" owner: "amberice" status: "draft" scope: "45 条论断逐条核实（成立 / 需修正 / 未实测 / 判断）；draft 不作结论，去留待用户裁决" supersedes: null superseded_by: null }
    { title: "docs/amberice/spec/format.md" date: "2026-11-02" updated: "2026-11-10" owner: "amberice" status: "current" scope: ".ambr 格式规范：信封 / canonical / VALUE 段 / 算法表 / 默认值 / 侧信道" supersedes: null superseded_by: null }
    { title: "docs/amberice/spec/overview.md" date: "2026-11-10" updated: "2026-11-10" owner: "amberice" status: "current" scope: "amberice 是什么：以源码为唯一事实来源的产品线定义（边界 / 格式 / 能力面 / 多端 / 实现状态）" supersedes: null superseded_by: null }
    { title: "docs/amberice/spec/smldb.md" date: "2026-11-02" updated: "2026-11-06" owner: "amberice" status: "draft" scope: "smldb 数据库层：表结构 / 后端选型 / 选择器 / 四层 API（全部未开工）" supersedes: null superseded_by: null }
    { title: "docs/amberice/spec/tools.md" date: "2026-11-02" updated: "2026-11-10" owner: "amberice" status: "current" scope: ".ambr 配套工具面：子命令清单以 amberice --help 为准，另含五端解码器" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/doc-defects.md" date: "2026-11-04" updated: "2026-11-04" owner: "repo" status: "archive" scope: "P1 文档盘点结果（2026-11-04 快照）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/doc-reconcile-2026-10-28.md" date: "2026-10-28" updated: "2026-11-03" owner: "repo" status: "archive" scope: "2026-10-28 文档对账的一次性记录" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-2026-10-26.md" date: "2026-10-26" updated: "2026-10-26" owner: "repo" status: "archive" scope: "会话交接（2026-10-26）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-2026-10-27.md" date: "2026-10-27" updated: "2026-10-27" owner: "repo" status: "archive" scope: "会话交接（2026-10-27）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-2026-10-28-2.md" date: "2026-10-28" updated: "2026-10-28" owner: "repo" status: "archive" scope: "会话交接（2026-10-28 第二份）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-2026-10-28.md" date: "2026-10-28" updated: "2026-10-28" owner: "repo" status: "archive" scope: "会话交接（2026-10-28）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-2026-10-30-2.md" date: "2026-10-30" updated: "2026-10-30" owner: "repo" status: "archive" scope: "会话交接（2026-10-30 第二份）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-2026-10-30-3.md" date: "2026-10-30" updated: "2026-10-30" owner: "repo" status: "archive" scope: "会话交接（2026-10-30 第三份）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-2026-10-30.md" date: "2026-10-30" updated: "2026-10-30" owner: "repo" status: "archive" scope: "会话交接（2026-10-30）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-2026-11-02.md" date: "2026-11-02" updated: "2026-11-02" owner: "repo" status: "archive" scope: "会话交接（2026-11-02，含待议清单 §4.5）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-2026-11-05.md" date: "2026-11-05" updated: "2026-11-05" owner: "repo" status: "archive" scope: "会话交接（2026-11-05，.ambr T0–T6 交接）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF-W24.md" date: "2026-10-12" updated: "2026-10-12" owner: "repo" status: "archive" scope: "会话交接（W24）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/handoff/HANDOFF.md" date: "2026-09-18" updated: "2026-11-05" owner: "repo" status: "archive" scope: "主交接文档（2026-09-18 至 2026-11-05 的长流水）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/open-work.md" date: "2026-11-04" updated: "2026-11-04" owner: "repo" status: "archive" scope: "三次扫描叠加的旧待办清单（已被取代）" supersedes: null superseded_by: "docs/repo/ref/unfinished.md" }
    { title: "docs/repo/archive/task/TASK-agent-w19.md" date: "2026-09-19" updated: "2026-10-10" owner: "repo" status: "archive" scope: "VSCode 扩展验收任务书（W19）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/task/TASK-agent-w24.md" date: "2026-10-12" updated: "2026-10-12" owner: "repo" status: "archive" scope: "任务书（W24）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/task/TASK-agent-w8-w9.md" date: "2026-09-18" updated: "2026-10-28" owner: "repo" status: "archive" scope: "任务书（W8–W9）" supersedes: null superseded_by: null }
    { title: "docs/repo/archive/task/TASK-agent.md" date: "2026-09-18" updated: "2026-10-28" owner: "repo" status: "archive" scope: "任务书（主）" supersedes: null superseded_by: null }
    { title: "docs/repo/plan/doc-architecture.md" date: "2026-11-04" updated: "2026-11-06" owner: "repo" status: "stale" scope: "文档四层架构 + P1–P4（提议；已由本次治理落地）" supersedes: null superseded_by: "docs/index.md" }
    { title: "docs/repo/ref/pending-decisions.md" date: "2026-11-06" updated: "2026-11-10" owner: "repo" status: "current" scope: "裁决台账：D1–D6 已全部裁决（含发布拓扑 / 仓库分工 / 官网）+ 仍待裁项的去处" supersedes: null superseded_by: null }
    { title: "docs/repo/ref/todo-ideas-tables.md" date: "2026-11-06" updated: "2026-11-13" owner: "repo" status: "current" scope: "TODO / IDEAS 条目表：卡在谁、什么状态" supersedes: null superseded_by: null }
    { title: "docs/repo/ref/unfinished.md" date: "2026-11-06" updated: "2026-11-13" owner: "repo" status: "current" scope: "唯一「还剩什么」清单（分类 / 判据 / 复核命令）" supersedes: null superseded_by: null }
    { title: "docs/repo/ref/scan.md" date: "2026-11-07" updated: "2026-11-10" owner: "repo" status: "current" scope: "自动扫描的计数结果（tools/scan_status.py --write 生成，勿手改）" supersedes: null superseded_by: null }
    { title: "docs/swml/plan/api-draft.md" date: "2026-11-02" updated: "2026-11-06" owner: "swml" status: "draft" scope: "门面未定项：根命名空间债务 / 命名二义 / feature 传播 / 发布口径" supersedes: null superseded_by: null }
    { title: "docs/swml/spec/api.md" date: "2026-11-02" updated: "2026-11-09" owner: "swml" status: "current" scope: "swml 门面公开面：根 = swsml 全集，easy = 四个概念" supersedes: null superseded_by: null }
    { title: "docs/swml/status.md" date: "2026-11-06" updated: "2026-11-10" owner: "swml" status: "current" scope: "swml 发布状态（dev2 成批发 / publish=false）+ 门面源码与对外仓分工" supersedes: null superseded_by: null }
    { title: "docs/swsml/archive/idyml-rejected.md" date: "2026-10-15" updated: "2026-11-03" owner: "swsml" status: "archive" scope: "idyml 改名方案（2026-11-03 否决）" supersedes: null superseded_by: null }
    { title: "docs/swsml/archive/iter-parse.md" date: "2026-10-30" updated: "2026-11-02" owner: "swsml" status: "archive" scope: "迭代式解析改造（已完成并合入 dev）" supersedes: null superseded_by: null }
    { title: "docs/swsml/plan/crossend-sync.md" date: "2026-11-12" updated: "2026-11-13" owner: "swsml" status: "current" scope: "跨端同步施工切片：码表分类、weak 三兄弟 / Edition 四端、验收命令与开工裁决" supersedes: null superseded_by: null }
    { title: "docs/swsml/plan/featag-slices.md" date: "2026-11-04" updated: "2026-11-10" owner: "swsml" status: "current" scope: "featag 实现切片 S0–S9（S0–S9 已收官；其它四端按 S9 策略不实现）" supersedes: null superseded_by: null }
    { title: "docs/swsml/plan/features-plan.md" date: "2026-11-04" updated: "2026-11-06" owner: "swsml" status: "stale" scope: "v5 版本位落地步骤与被推翻的旧清单" supersedes: null superseded_by: "docs/swsml/spec/features.md" }
    { title: "docs/swsml/plan/perf-plan.md" date: "2026-11-06" updated: "2026-11-08" owner: "swsml" status: "current" scope: "全面优化计划：时间 / 内存 / 缓存 / 指令集四线" supersedes: null superseded_by: null }
    { title: "docs/swsml/ref/capability-matrix.md" date: "2026-10-27" updated: "2026-11-10" owner: "swsml" status: "draft" scope: "五端能力矩阵（2026-10-27 快照，只作历史对照；v5 版本口径待补）" supersedes: null superseded_by: null }
    { title: "docs/swsml/ref/perf-baseline.md" date: "2026-11-03" updated: "2026-11-10" owner: "swsml" status: "current" scope: "性能基线：perf / value / profile / amberice 四份 bench 的正式口径数字" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/featag.md" date: "2026-11-04" updated: "2026-11-10" owner: "swsml" status: "current" scope: "featag 语法 / 落点 / from-featag（Rust 已实现；其它四端按 S9 策略不实现）" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/features.md" date: "2026-11-04" updated: "2026-11-06" owner: "swsml" status: "current" scope: "特性命名空间：4 个域、两段上限、新增特性带域" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/null.md" date: "2026-11-04" updated: "2026-11-06" owner: "swsml" status: "current" scope: "null / none 五条语义与五端现状" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/smlex.md" date: "2026-11-04" updated: "2026-11-10" owner: "swsml" status: "current" scope: ".smlex 内容格式 v1.0（格式定稿；checks 与 applies_to 提示已接 --lint，hints / emit 未实现）" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/v5.md" date: "2026-11-04" updated: "2026-11-10" owner: "swsml" status: "draft" scope: "v5 变更总表（工作底稿，v5 未定稿；B6/B7 已落地，表 A 待定稿实测）" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/value-layout.md" date: "2026-11-02" updated: "2026-11-06" owner: "swsml" status: "draft" scope: "值层内存布局提案（未拍板，需二次蒸馏）" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/version.md" date: "2026-11-04" updated: "2026-11-13" owner: "swsml" status: "current" scope: "版本 = 功能集（edition）；版本位与三态生命周期（五端字面量已同步，见 §7）" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/weakbind.md" date: "2026-11-04" updated: "2026-11-13" owner: "swsml" status: "current" scope: "weakbind 语法 / 作用域 / 旁路表 / 默认忽略口径（语言层五端已实现；C-ABI / 绑定层见 crossend-sync.md）" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/weakdata.md" date: "2026-11-04" updated: "2026-11-12" owner: "swsml" status: "current" scope: "weakdata 语法与判别（Rust 已实现；多端同步见 crossend-sync.md）" supersedes: null superseded_by: null }
    { title: "docs/swsml/spec/weakdoc.md" date: "2026-11-04" updated: "2026-11-13" owner: "swsml" status: "current" scope: "weakdoc 语法与 .smlex 发现 / W-CLI-001 提示（库侧、CLI 发现与提示均已实现）" supersedes: null superseded_by: null }
  ]
}
