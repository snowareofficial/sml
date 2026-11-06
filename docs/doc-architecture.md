# 文档架构（v0.1 · 提议 · 未定稿）

> 2026-11-04。用户：「文档和官网还要大改」「文档那些可能要对已有的进行调整和重构」「当前文档有不少缺陷」。
> 本文件只做**架构与迁移计划**，不含具体改写；`IDEAS.md` / `TODO.md` 未动。

---

## 1. 现状（实证）

| 资产 | 规模 | 问题 |
|---|---|---|
| 站点教科书 `site/content/{zh,en}/book/` | 各 **17 篇**（`intro` + `ch01`–`ch14` + `appendix` + `_index`） | 两语**并行手写** ⇒ 同一句话要改两处，漂移必然 |
| `docs/` | **13 份** md | **三类混放**：规范草案（featag / version / null / v5）、审计记录（doc-reconcile / capability-matrix）、计划（iter-parse / api-progressive / perf-baseline / value-layout / amberice-spec）—— **没有权威标记**，读者不知道哪份说了算 |
| 根目录 | `README.md` / `README.en.md` / `llms.txt` / `CHANGELOG.md` / `CHANGELOG.en.md` | 版本口径、错误码条数、能力矩阵在多处**手写**（历史上同时存在 141/144/143/139 四个数字） |
| 教科书代码块 | 大量 | **不可执行、无 doctest 保护** ⇒ 语法变了照样「绿」 |

**归纳成四条**

1. **没有权威源**：同一事实散在多处，谁新谁旧看不出来；
2. **双语双份**：示例与句子各写一遍，改一处必漏另一处；
3. **数字靠人记**：码数、用例数、支持矩阵、性能数字；
4. **示例不可跑**：文档里的 `.sml` 片段不参与测试。

## 2. 目标分层（四层 + 一条权威规则）

| 层 | 职责 | 位置 | 权威性 |
|---|---|---|---|
| **规范（normative）** | 语法与语义的**唯一权威**：featag / 版本命名 / 标识符 / null 语义 / 特性位 | `docs/spec/` | ⭐ 最高。任何冲突以它为准 |
| **教程（tutorial）** | 带读者从 0 到能用 | `site/content/*/book/` | 引用规范，不复述细节 |
| **参考（reference）** | 表：错误码 / 修饰符 / 版本矩阵 / C-ABI / CLI | 由**生成器**产出 | 生成物，禁止手改 |
| **迁移（migration）** | 破坏性改动 + `--migrate` 规则表 | `CHANGELOG` 两栏 + 规则表 | 与码表同源 |

**权威规则一句话**：*规范说「是什么」，教程说「怎么用」，参考是生成的，迁移说「怎么改」。*
同一句话**只允许出现在一层**，其余层用链接。

## 3. 单一事实来源（必须生成，不许手抄）

| 事实 | 现状 | 目标 |
|---|---|---|
| 错误码（编号/文案/impls） | 已是生成物（`errors/codes.sml` → 5 端） | 保持 ✅ |
| **错误码总数** | 手写（曾四个版本同时在世） | **从 `errors.json` 读 `count`**，文档里写生成值 |
| **版本支持矩阵**（Rust/JS/C/Lua/C++ 各到哪一代） | 教科书 + 各端 README 手写 | 一张表 → 生成到各处 |
| **能力矩阵**（`docs/capability-matrix.md`） | 手写 | 半生成（用例名 + 矩阵由测试元数据产出） |
| **标识符白名单 / script 归属**（新） | 无 | 与码表同一套生成链 → 五端 |
| **示例代码块** | 手写、不可跑 | zh/en **共用同一份示例片段**，双向引用 + 可执行（能挂 doctest 的挂） |
| `llms.txt` | 手写副本（曾与主库漂） | 生成摘要 |

## 4. 页面地图（现状 → 目标）

| 现状 | 目标动作 |
|---|---|
| `docs/featag-spec-draft.md` | 成熟后 → `docs/spec/featag.md`（去掉「草案」） |
| `docs/version-naming.md` | → `docs/spec/version.md`（已定稿） |
| `docs/null-semantics.md` | → `docs/spec/null.md` |
| `docs/featag-impl-slices.md` | 留在 `docs/plan/`（施工计划，不是规范） |
| `docs/feature-namespace-plan.md` | 拆：规范部分 → `docs/spec/features.md`；计划部分留 plan |
| `docs/capability-matrix.md` | → 参考层（半生成） |
| `docs/doc-reconcile-2026-10-28.md`、`docs/iter-parse-plan.md` | → `docs/archive/`（已完成/一次性记录） |
| `docs/api-progressive-draft.md`、`value-layout.md`、`amberice-smldb-spec.md` | 留 `docs/plan/`（未定稿方案） |
| 教科书 ch01–ch14 | 内容保留；**示例抽到共用片段**；新增「featag / 版本 / 标识符」三节（随代码切片交付） |

## 5. 迁移阶段

| 阶段 | 做什么 | 产出 | 是否改内容 |
|---|---|---|---|
| **P1 盘点** | 全仓扫「与代码不符 / 与规范不符 / 示例不可跑 / 数字漂移」 | `docs/doc-defects.md`（文件:行 + 判据 + 严重度） | ❌ 只列 |
| **P2 架构落地** | 建 `docs/spec|plan|archive` 目录；移动文件；补权威标记；确定生成物边界 | 目录 + `docs/README.md`（索引，说明哪份权威） | 只移动 |
| **P3 内容迁移** | 按 P1 清单逐条修；双语示例抽共用片段 | 修完的教程/参考 | ✅ |
| **P4 生成化** | 数字/矩阵/白名单/llms 全部改为生成 + `--check` 门禁 | 生成器 + CI 检查 | ✅（结构性） |

⚠️ **顺序不可颠倒**：先 P1 才有依据改，先 P2 才有地方放。P3 与代码切片**并行**推进（每片交付自己的文档影响）。

## 6. 验收标准（怎么算「文档改完了」）

1. 任一事实**只有一个源**：改它只需改一处，其余是引用或生成；
2. `python errors/gen_* --check` 与新增的 `gen_docs --check` 全绿；
3. 教科书示例**可执行**（能跑的都跑，跑不了的标 `no_run` 并写明原因）；
4. `docs/README.md` 一页能说清「哪份权威、哪份过期、哪份是计划」；
5. zh/en 的**示例完全一致**（同一片段），只有叙述文字不同。
