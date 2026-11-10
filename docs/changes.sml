# docs/changes.sml —— 文档变更记录
#
# 记录型：`doc / at / commit / summary`。正文里不写历史，历史归这里和 git。
#   doc      受影响的文档（新路径）
#   at       变更日期
#   commit   这一批**基于**的提交（本文件无法记录自身所在提交的哈希；
#            要追这一批就 `git log -- docs/meta.sml`）
#   summary  一句话：搬自哪里 / 由什么拆分 / 为什么删
#
# 只记「文档资产的变动」。代码变动看 CHANGELOG.md 与 git log。
# 事后补记的条目，commit 填变更所在提交；早期条目填的是该批基于的提交，两种口径并存。

changes {
  change: [
    { doc: "docs/index.md" at: "2026-11-06" commit: "e16de31" summary: "新建：由 docs/meta.sml 生成的索引，取代手写的 docs/README.md（原索引有 5 处状态错误）" }
    { doc: "docs/meta.sml" at: "2026-11-06" commit: "e16de31" summary: "新建：全部文档的元数据单一清单" }
    { doc: "docs/changes.sml" at: "2026-11-06" commit: "e16de31" summary: "新建：本文件，文档变更记录" }
    { doc: "docs/README.md" at: "2026-11-06" commit: "e16de31" summary: "删除：手写索引被 docs/index.md 取代，读法并入生成模板" }

    { doc: "docs/amberice/spec/format.md" at: "2026-11-06" commit: "e16de31" summary: "拆分自 docs/amberice-smldb-spec.md：§0-4、§8-9、§11、§18-21、§23-25（§2/§3 按 D2 改写为编 Value）" }
    { doc: "docs/amberice/spec/smldb.md" at: "2026-11-06" commit: "e16de31" summary: "拆分自 docs/amberice-smldb-spec.md：§5-7、§13-14、§17，标 draft（未开工）" }
    { doc: "docs/amberice/spec/tools.md" at: "2026-11-06" commit: "e16de31" summary: "拆分自 docs/amberice-smldb-spec.md：§22 配套工具" }
    { doc: "docs/amberice/decisions.md" at: "2026-11-06" commit: "e16de31" summary: "拆分自 docs/amberice-smldb-spec.md：§10 结案表、§12 草案、§15 键序、§16 决策记录（D1–D41 + P1–P10）" }
    { doc: "docs/amberice/plan/plan.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/amberice-plan.md（amberice/plan/）" }
    { doc: "docs/amberice/ref/ports.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/ports-progress.md；修正构建命令为 cargo build -p amberice-ffi，删掉已完成的施工步骤" }
    { doc: "docs/amberice/archive/design-origin.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/amberice-design-origin.md" }

    { doc: "docs/swsml/spec/version.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/version-naming.md" }
    { doc: "docs/swsml/spec/null.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/null-semantics.md；规则来源改指 plan/features-plan.md" }
    { doc: "docs/swsml/spec/featag.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/featag-spec-draft.md" }
    { doc: "docs/swsml/spec/v5.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/v5-syntax.md" }
    { doc: "docs/swsml/spec/value-layout.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/swsml/spec/value-layout.md" }
    { doc: "docs/swsml/spec/weakbind.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/weakbind-spec-draft.md" }
    { doc: "docs/swsml/spec/weakdata.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/weakdata-spec-draft.md" }
    { doc: "docs/swsml/spec/weakdoc.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/weakdoc-spec-draft.md" }
    { doc: "docs/swsml/spec/smlex.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/smlex-format-draft.md" }
    { doc: "docs/swsml/spec/features.md" at: "2026-11-06" commit: "e16de31" summary: "拆自 docs/feature-namespace-plan.md 的规范部分（§1-3、§6 已定域清单）" }
    { doc: "docs/swsml/plan/features-plan.md" at: "2026-11-06" commit: "e16de31" summary: "拆自 docs/feature-namespace-plan.md 的施工清单（§4-5），标 stale" }
    { doc: "docs/swsml/ref/capability-matrix.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/capability-matrix.md" }
    { doc: "docs/swsml/ref/perf-baseline.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/perf-baseline.md" }
    { doc: "docs/swsml/plan/featag-slices.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/featag-impl-slices.md" }
    { doc: "docs/swsml/plan/perf-plan.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/perf-plan.md" }
    { doc: "docs/swsml/archive/iter-parse.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/iter-parse-plan.md" }
    { doc: "docs/swsml/archive/idyml-rejected.md" at: "2026-11-06" commit: "e16de31" summary: "移动自仓库根 IDYML.md；替代物更正为 swml + swml::easy" }

    { doc: "docs/swml/spec/api.md" at: "2026-11-06" commit: "e16de31" summary: "拆自 docs/api-progressive-draft.md 的已拍板部分，按 rust/swml 实际 API 重写" }
    { doc: "docs/swml/plan/api-draft.md" at: "2026-11-06" commit: "e16de31" summary: "拆自 docs/api-progressive-draft.md 的未定与风险部分" }
    { doc: "docs/swml/status.md" at: "2026-11-06" commit: "e16de31" summary: "新建：从 TODO.md §11.1 与 rust/swml/README.md 抽出发布状态；workspace 口径冲突标待裁决" }

    { doc: "docs/repo/ref/unfinished.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/unfinished.md" }
    { doc: "docs/repo/ref/todo-ideas-tables.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/todo-ideas-tables.md" }
    { doc: "docs/repo/ref/pending-decisions.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/pending-decisions.md；改写为「五条已全部裁决」的台账（原文假装还挂在用户那里）" }
    { doc: "docs/repo/plan/doc-architecture.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/doc-architecture.md，标 stale（其 P1-P4 由本次治理落地）" }
    { doc: "docs/repo/archive/open-work.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/open-work.md，superseded_by 指向 unfinished.md" }
    { doc: "docs/repo/archive/doc-reconcile-2026-10-28.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/doc-reconcile-2026-10-28.md" }
    { doc: "docs/repo/archive/doc-defects.md" at: "2026-11-06" commit: "e16de31" summary: "移动自 docs/doc-defects.md" }
    { doc: "docs/repo/archive/handoff/" at: "2026-11-06" commit: "e16de31" summary: "移动自仓库根：11 份 HANDOFF*.md（含 HANDOFF.md 174 KB）" }
    { doc: "docs/repo/archive/task/" at: "2026-11-06" commit: "e16de31" summary: "移动自仓库根：4 份 TASK-agent*.md（另有 3 份从未入库的 TASK-hy3-* 按 .gitignore 规则挪进 _scratch/）" }

    { doc: "examples/showcase.sml" at: "2026-11-06" commit: "e16de31" summary: "移动自仓库根 showcase.sml（示例归 examples/；README 两处链接同步）" }
    { doc: "examples/showcase_contract.sml" at: "2026-11-06" commit: "e16de31" summary: "移动自仓库根 showcase_contract.sml（同一批；rust/tests/contract_showcase.rs 的夹具路径同步）" }
    { doc: "AGENTS.md" at: "2026-11-06" commit: "e16de31" summary: "新建：agent 阅读规则（先读索引 / 只读 current / draft 与 stale 须点名 / 正文不写历史 / 新 doc 先登记）" }
    { doc: "docs/amberice/spec/overview.md" at: "2026-11-10" commit: "c7ff213" summary: "新建：从 amberice / amberice-ffi / 各端解码器与门禁源码重新导出产品线定义，不引用既有文档正文" }

    { doc: "docs/amberice/ref/claims-audit.md" at: "2026-11-10" commit: "8fcf26e" summary: "新建：2026-11-10 会话 45 条论断逐条核实，标 draft 并加正文顶部「临时件」横幅；待处理：并进 decisions.md §16.1 或整体移入 amberice/archive/" }
    { doc: "docs/amberice/decisions.md" at: "2026-11-10" commit: "8fcf26e" summary: "新增 §16.5 写路径 / 归档边界讨论与节级「临时节」横幅；文件仍 current，D1–D54、P12、§16.3 等既有裁决未降级" }
    { doc: "_scratch/writepath/" at: "2026-11-10" commit: "8fcf26e" summary: "删除：微基准一次性数据（未入库）；复现方法留在 docs/amberice/ref/claims-audit.md §7" }

    { doc: "docs/meta.sml" at: "2026-11-10" commit: "3b2d83a" summary: "状态回填：scope 与 updated 对齐源码（perf-baseline 四份 bench / featag S9 策略 / smlex 工具侧 / weak* 实现面）；current 文档的过期数字、横幅与 T7 口径同步改写" }

    { doc: "docs/amberice/ref/claims-audit.md" at: "2026-11-10" commit: "136803b" summary: "改写：补 §0.1 会话后落地对照（原子写 / 尾垃圾检测 / 回归用例）、§7 可复现脚本，§8 / §9 复核命令对齐 136803b；仍保持 draft" }
    { doc: "docs/amberice/decisions.md" at: "2026-11-10" commit: "136803b" summary: "改写：§16.5 的代码事实与探针进度对齐 136803b；仍为待裁决节，D1–D54 / P12 / §16.3 未动" }
    { doc: "docs/repo/ref/unfinished.md" at: "2026-11-10" commit: "136803b" summary: "补 §12：临时产物去留与三条探针登记进唯一欠账清单" }

    { doc: "docs/amberice/ref/claims-audit.md" at: "2026-11-10" commit: "ce88149" summary: "新增 45 条论断核实台账（draft）与 §16.5 待裁讨论，全仓状态回填，索引同步到 52 份" }
    { doc: "docs/index.md" at: "2026-11-10" commit: "d55a556" summary: "重生成索引：meta.sml 的 scope / updated 回填同步进生成物" }
    { doc: "docs/repo/ref/unfinished.md" at: "2026-11-10" commit: "9d95c37" summary: "回填核对基准、补记 amberice 原子写变更、收口 claims-audit 措辞" }
    { doc: "docs/amberice/ref/claims-audit.md" at: "2026-11-10" commit: "1088b3a" summary: "核实 45 条论断：修正 §0.1 当前事实、证据引用与 §7 / §8 表述" }
    { doc: "docs/index.md" at: "2026-11-10" commit: "009dc9f" summary: "生成物头行改记 meta.sml 内容摘要（去掉生成日期），report.md 同步；scan.md 正文未变不重写；CI 加生成物幂等校验" }
  ]
}
