# `docs/` 索引：**哪份说了算**

> 2026-11-04 建立。这个目录此前 **13 份文档平铺**、规范/计划/审计混放，读者无法判断权威性。
> 本页给每份文档标**类型 + 状态**，是 P2（结构重构）的第一步。
>
> **2026-11-06 更新**：登记新增三份（`unfinished.md` / `smlb-design-origin.md` / `open-work.md`），
> 并修正两处过期状态（`smlb-smldb-spec.md` 已不是「零实现」；`perf-baseline.md` 只覆盖 `parse`）。
> 当天用户点名「文档过于混乱、落后」，治理项已记进 `TODO.md` §九。
>
> ⚠️ **暂不移动文件**：`IDEAS.md` / `TODO.md`（由你自己维护）里有 6 处引用 `docs/*.md`，
> 现在搬家会产生断链。等目录调整与你那边的编辑对齐后再动路径。

## 类型与权威性（只认这四类）

| 类型 | 含义 | 权威性 |
|---|---|---|
| **规范 spec** | 语法/语义的**唯一权威** | ⭐ 冲突时以它为准 |
| **计划 plan** | 施工方案、切片、盘点 —— 可改可废 | 不构成承诺 |
| **参考 ref** | 表与基线数字（应尽量生成） | 以生成物为准 |
| **归档 archive** | 一次性记录、已完成的方案 | 只作历史，勿照做 |

## 现有文档

| 文件 | 类型 | 状态 | 一句话 |
|---|---|---|---|
| `version-naming.md` | **spec** | **定稿 v1.0** | 版本 = 功能集（edition）；`50 "camelia" dev`；三态生命周期 |
| `feature-namespace-plan.md` | **spec + plan** | 规范部分定稿 | 17 个特性归 4 域；新增特性带域；v5 落地九步 |
| `null-semantics.md` | **spec** | 五条已定 | `optional`=键缺失；`default null` v5 禁；输出侧 null 不产出 |
| `featag-spec-draft.md` | **spec** | v1.0 语义闭合，零实现 | `!featag` 语法、`__featags`、落点规则、`from-featag` |
| `v5-syntax.md` | **spec 索引** | 底稿（v5 未定稿） | 表 A 语言层 / 表 B 库与工具层 / 表 C 不进 v5 |
| `featag-impl-slices.md` | plan | 计划 | 9 片施工计划（S0→S9），每片带验收与文档影响 |
| `doc-architecture.md` | plan | 提议 | 文档四层架构 + 单一事实来源 + P1–P4 |
| `doc-defects.md` | plan | 2026-11-04 快照 | P1 盘点结果（用户可见文档无硬错误，风险在 209 个无 doctest 的 sml 块） |
| `api-progressive-draft.md` | plan | 已拍板（Q1–Q10 全定） | `smlkit-easy` 的分层与裁决记录 |
| `iter-parse-plan.md` | **archive** | 已完成并合入 | 迭代式解析改造（`9f7d528`） |
| `doc-reconcile-2026-10-28.md` | **archive** | 一次性记录 | 文档对账（多数结论已落进代码/文档） |
| `capability-matrix.md` | ref | 需更新（版本口径待 v5） | 五端能力矩阵 |
| `perf-baseline.md` | ref | 2026-11-03，**只覆盖 `parse`** | 性能基准与分配账（`value` / `smlb` 两份 bench 未并入） |
| `value-layout.md` | plan | 草案 v0.1，9 条验收全未打勾 | 内存值布局（与 `.smlb` 绑定） |
| `smlb-smldb-spec.md` | plan | **T0–T6 已落**；T7 五端、`smldb` 未开工 | `.smlb` 格式 / `smldb`；§16.2 与 §23.3 仍有未定项 |
| `unfinished.md` | **ref（唯一清单）** | 2026-11-06 快照 | 「还剩什么」只看这份；`open-work.md` 已归档 |
| `smlb-design-origin.md` | archive | 2026-11-06 汇编 | `.smlb` 设计起点与原话（A/B/C 三级，逐条带出处） |
| `open-work.md` | archive | 2026-11-04，**已被取代** | 三次扫描叠加；只查历史细节，勿据此排期 |

## 读的顺序（给你自己，也给新人）

1. 只想用 SML → 读**站点教程**（`site/content/*/book/`），不要进 `docs/`；
2. 想知道**当前语法到底怎么定** → `version-naming.md` → `v5-syntax.md` → 对应 spec；
3. 想动某一项实现 → 对应 `*-slices.md` / `*-plan.md`；
4. 只想知道历史为什么这样定 → `HANDOFF-*.md`（仓库根目录）与 `archive`。

## 当前进度（P1–P4，见 `doc-architecture.md` §5）

| 阶段 | 状态 | 说明 / 阻塞 |
|---|---|---|
| **P1 盘点** | ✅ 完成（2026-11-04） | `doc-defects.md`：用户可见文档**无硬错误**（4 处误报 + 9 处待 v5），风险在 **209 个无 doctest 的 sml 块** |
| **P2 索引** | ✅ 完成 | 就是本文 |
| **P2 搬家** | ⛔ **阻塞** | `IDEAS.md` / `TODO.md`（用户维护）里有 6 处 `docs/*.md` 引用，搬家会断链；等编辑对齐 |
| **P3 内容** | ⛔ 部分阻塞 | 9 处版本口径**必须等 edition 落地**（现在改会写错）；C 类 209 块可动 |
| **P4 生成化** | ⬜ 未开始 | 需 ① 码数/矩阵生成器 ② 标识符白名单/script 表生成器（新）③ `llms.txt` 生成（现为手抄副本） |

## 维护规则

1. **每份新文档必须在本文登记**（文件 / 类型 / 状态 / 一句话），否则视为不存在；
2. **只有 spec 能称权威**；plan 里的决定要转进 spec 才算数；
3. 一次性记录完成后移入 `archive`（**记住：现在还没搬，见文首**）；
4. 数字（码数 / 用例数 / 性能）**不许手写**，改为引用生成物。
