# 未完成面总表（`docs/open-work.md`，自动扫描 · 只列不改）

> 2026-11-04 生成。**用途**：回答「还剩什么」，一条条带来源，避免散在四处。
> ⚠️ 本文件是**清单**，不构成排期；`TODO.md` / `IDEAS.md` 仍是用户维护的权威。

**扫描源**：`TODO.md`（勾选框 + 散文标记）、`IDEAS.md`（状态行 + 内部未勾）、
`docs/*.md`（规范里的「未做/零实现/待定」）、`errors/codes.sml`（`impls` 缺端）、
`known_divergences.md`（DIVERGE）、`rust/**/*.rs`（`TODO`/`FIXME`/`todo!`）、`HANDOFF §0.2`。

## 0. 摘要（先看这个）

| 维度 | 数量 | 说明 |
|---|---|---|
| TODO 显式未勾 | 23 | 多是跨端与治理，见 §1 |
| IDEA 条目 | 23 | 状态里 `Done`/`Partially`/`Todo` 混着，见 §2 |
| 错误码只有 Rust 实现 | **42** | **新能力还没跨端**——featag / v5 都在这一档，见 §4.1 |
| 错误码其它缺端 | 82 | 含「只有某一端」与「缺一两端」，见 §4.2 |
| 一致性 DIVERGE | 有登记 | 见 §5 |
| 源码 `TODO/FIXME` | 0 | 干净 ⇒ 欠账都写在文档里，不在代码里 |
| 文档互相矛盾 | 6 | 见 §8 —— **照着过期文档做会白干** |
| **weak\* 三件已实现** | 3 | 但**零测试覆盖**，见 §9 |

**三个最该先解决的「卡点」**（不是任务，是让别人没法开工的东西）：

1. **`5.0` 未启用**：`@version 50 "camelia" dev` 现在**报错**（S0b 有意为之）⇒ featag 的新语法只能靠
   `@feature enable type.featag` 在 v4 文档上试跑，版本位这条路还没通。
2. **四端欠账 42 条码**（§4.1）：新能力全在 Rust，跨端同步是**最大的一块整活**，且必须一次性做。
3. **文档状态与代码脱节**（§8）：`IDEAS` / `featag-impl-slices` / `capability-matrix` 里都有过期说法，
   不先对齐，排期会按错的前提做。

## 1. TODO.md 显式未勾（23 项）

| 行 | 内容 |
|---|---|
| 149 | - [ ] **建立跨实现一致性测试套件（conformance suite）** |
| 166 | - [ ] **顶层标量的行为未统一** |
| 201 | - [ ] **错误边界行为不一致**，需逐一核对齐：同名键冲突（Rust 提升为数组）、 |
| 203 | - [ ] **Lua/JS 缺少单元测试**（Rust 有 32 个测试；Lua 与 JS 目前只有临时探针） |
| 204 | - [ ] **C 实现的 `sml_dump` 输出风格**与 Rust `to_sml` 是否逐字节对齐，待比对 |
| 318 | - [ ] **Lua 补契约数组**：照 Rust 同族改法（非内置名回落到契约引用）。⚠️ 用户已定「先不动 Lua」。 |
| 953 | - [ ] **Lua 嵌套数组不支持**（台账 #1–#3，`E-PARSE-006` / `E-PARSE-003`）：与 W17 的 C 修复同族 |
| 955 | - [ ] **临时探针清单**（`known_divergences.md` 末节，约 40 个 `_*` 文件）：等套件全 AGREE + 台账收敛后 |
| 957 | - [ ] **`@feature base` 只认版本号** ⇒「在文档里声明书名」没有承载位；CLI 现按顶层 `title:` 推断 |
| 989 | - [ ] **重打 `.vsix`（未做：需网络 + 需你定版本号）**：`package.json` 已是 **0.5.0**，而入库的 |
| 993 | - [ ] **weak\* 三兄弟落地必然改 vsix**：I-5 `weakport` / I-15 `weakdata` / I-17 `weakdoc`（`.smlex`） |
| 1028 | - [ ] **下一个杠杆（需要设计决策，别在性能任务里顺手做）**：动**表示**而非常数 —— |
| 1033 | - [ ] ⚠️ **先想清「哪一层能编译掉」**：文档里的 `@feature enable/disable` 是**运行期、文档级**开关， |
| 1066 | - [ ] **发布准备**（用户定的节奏）：**不单独发**，等 I-18（`.smlb`/`smldb`）+ I-19（`idyml`） |
| 1070 | - [ ] **发布前还要占三处**：GitHub org / 域名 / docs 子域 —— 否则「名字不撞」只解决一半。 |
| 1071 | - [ ] **⚠️ 命名陷阱要一直记着**：包名 `swsml` / 库名 `sml` ⇒ `Cargo.toml` 写 `swsml`， |
| 1077 | - [ ] **`k: "abc\`（转义符后遇 EOF）三端三个码**（全部实测）： |
| 1255 | - [ ] 在 **两台远端**上都设保护规则：`main` / `dev` 禁强推、需 CI 绿才能合 |
| 1258 | - [ ] 把 **PR 目标默认值**指向 `dev`（Gitee / GitHub 各设一次；仓库默认分支仍是 `main`） |
| 1259 | - [ ] 收掉存量长期分支：`align/bare-colon-e006`、`align/frag-splice-c`、 |
| 1263 | - [ ] `origin/feat/i9-contract-typo-hint` 远端删除（内容已全在 `dev`，本地已删）—— **用户手动**： |
| 1265 | - [ ] `origin/master` 远端删除（0 笔领先、落后 `main` 223 笔，实测完全包含）—— **用户手动**： |
| 1308 | - [ ] 双远端同步口径：`--delete` 推给 Gitee **不会**同步删 GitHub 上的分支， |

## 2. IDEAS.md 条目状态（23 条）

| 行 | 条目 | 状态摘要 | 内部未勾 |
|---|---|---|---|
| 14 | ## I-1 自定义结构体的序列化 trait / trait-based serializer | - **Status / 状态**：`Done`（2026-10-27）—— 两半都落地： | 0 |
| 60 | ## I-2 Rust 原生 contract，可选无视文件内契约 / native contracts with opt-out | - **Status / 状态**：`Done`（opt-out 开关，2026-11-02）；周边能力 `Done`。 | 0 |
| 79 | ## I-3 `get_object` 之类的方法？ / object accessors? | - **Status / 状态**：`Done`（2026-10-27；**已合入 `dev`**，`feat/value-accessors` 分支已删）—— | 0 |
| 97 | ## I-4 注释双语化，英文在前 / bilingual comments, English first | - **Status / 状态**：`Done`（2026-11-02 归零）—— 本仓库注释**大体已是**「English ｜ 中文」， | 0 |
| 180 | ## I-5 weakport：插件认契约，库忽略 / plugin-only contracts | - **Status / 状态**：`Todo` —— 没有这个语法；最接近的两块能力都在。 | 0 |
| 204 | ## I-6 featag：带参的块头标签 / param'd header tags | > Status / 状态：`Blocked` —— 等确认；未确认不开工。**用户补描述中（2026-10-31）**。 | 0 |
| 252 | ## I-7 契约约束的精确控制 / 让数据是我们想要的样子 · precise shape control | - **Status / 状态**：`Done` —— **计数三件套与跨字段依赖已齐（①②③）**；**④ 三态 + 块级覆盖已做**； | 0 |
| 364 | ## I-8 simple TOML-style support / 简易 TOML 风格 | - **Status / 状态**：`Todo`（`=` 分隔）；`[name]` 作块名 **搁置**（用户已定）。 | 0 |
| 377 | ## I-9 近似契约名扫描与标注 / near-miss contract name hints | - **Status / 状态**：`Done`（2026-10-27）—— 运行时已落地；**编辑器侧仍未做**。 | 0 |
| 412 | ## I-10 团队搭建与宣传 / team & outreach | - **Status / 状态**：`Partial`（2026-10-27）—— **前置（能力矩阵）已产出，入口文档仍未做**。 | 0 |
| 432 | ## I-11 中文支持完善，且可剥离 / zh support, detachable | - **Status / 状态**：`Partial`（中文能用）；查表层 `Done`（2026-10-31 复核）；**剥离（表层）= Done**（202 | 0 |
| 466 | ## I-12 改名工作（会多出一个新 crate）/ rename spawns a new crate | - **Status / 状态**：**`Rejected`（2026-11-03 用户裁决：不改名）** —— 原话「swsml 我不想改名， | 0 |
| 493 | ## I-13 实现即文档，闪光点要看得见 / docs while implementing, make the highlights visible | - **Status / 状态**：`Partial`（2026-10-31 开工）—— 落地第一个「语义 + 例子 + 陷阱」示范； | 0 |
| 513 | ## I-14 仓库排雷行动 / repo-wide landmine sweep | - **Status / 状态**：`Partial`（2026-11-03 三棒）—— 四类雷（3/4/5/6）的**可修项已基本修完**。 | 0 |
| 541 | ## I-15 `weakdata`：不进解析的扩展数据通道 / data channel extensions read, core ignores | - **Status / 状态**：`Todo` —— **概念阶段**，用户点名记住的关键词，尚未设计。 | 0 |
| 560 | ## I-17 `weakdoc`：`.smlex` profile 的引用指令 / project profiles, referenced, never injected | - **Status / 状态**：`Todo` —— **概念阶段**，用户点名记住的关键词，尚未设计。 | 0 |
| 580 | ## I-16 版本后缀与发布门禁：`a` / `s` / release gates | - **Status / 状态**：`方案已定（2026-10-30 用户确认）` —— 此前迭代太快、版本号零门槛， | 0 |
| 605 | ## I-18 持久化：`.smlb` 二进制格式 + `smldb`（规范已写、**零实现**） | - **Status / 状态**：`Todo`（规范已定、零实现；**细节未定，用户 2026-11-03 明说「smlb 等的一些细节我还没定好」⇒ 只登记 | 0 |
| 628 | ## I-19 `smlkit`：品牌门面（瘦别名）/ 国际稳定发布线 | - **Status / 状态**：`命名已定（2026-11-03 用户裁决 `smlkit`）`；**crate 未建**（ | 0 |
| 659 | ## I-20 长文档产出链：EPUB / 独立 HTML / 分章 / TOC / 增量 emit（**未做**） | - **Status / 状态**：`Partial`（2026-11-03）—— **P1 的目录 / 锚点已落地**，P0 的 `--to html` 早已 | 0 |
| 685 | ## I-21 语义部件目录：figure / admonition / footnote / deflist / sub·sup / toc + **跨后端一致** | - **Status / 状态**：`Partial`（2026-11-03）—— **跨后端漂移三修已落地**，六类部件仍未做。 | 0 |
| 709 | ## I-22 下游方言反馈的六条语言设计问题（**待明确 · 未登记过**） | - **Status / 状态**：`Todo`（问题清单，非方案；来源是下游真实用法）。 | 0 |
| 723 | ## I-23 更激进的性能优化：把「没开的 feature」从运行期判断变成不走的路径 | - **Status / 状态**：`Todo`（2026-11-03 用户点题：「记下更激进的性能优化这一项，比如特定 feature 没开时提速」）。 | 0 |

**内部未勾明细**（有的话）：


## 3. `docs/*.md` 规范里的「未做 / 待定」标记

### `docs/api-progressive-draft.md`（1 处）
- L166: \| 版本节奏 \| 与主 crate 绑定，封装层一次破坏性改动就推 `swsml` 大版本 \| 可独立发版；但要处理「依赖尚未发布的 dev 版本」的发布顺序问题（当前 workspace 版本是 `0.6.9-dev.0`） \|

### `docs/capability-matrix.md`（6 处）
- L5: > **性质：纯调查。零实现代码改动。** 唯一新增文件即本文档；临时探针均以 `_` 开头（已被 `**/_*` 忽略，不提交）。
- L44: 图例：`✅` 支持且行为符合预期 · `⚠️` 支持但有可观察差异 · `❌` 未实现或静默错误处理 · `—` 不适用/未触发
- L162: → C++ **未实现** include：把 `include "sub.sml"` 当成普通键值 `include: sub.sml` 静默吃下（无错误、未展开）。
- L241: 3. **C++ 不支持 include 指令**：把 `include "sub.sml"` 静默当键值 `include: sub.sml`，不报错也不展开。Rust/C/JS/Lua 均正确展开。→ C++ 缺失该功能（或静默降级），属「未实现 + 静默处理」双重问题。
- L266: **Q2 不一致标注档位**：用三档——「⚠️ 差异（行为/输出可观察不同）」「❌ 未实现」「⚠️ 静默处理（错当别物/丢数据/不报错）」。
- L267: 关于第四档「已知不接受此差异」：**不需要单独设档**。理由：五端错误**码**已对齐（W10 成果），仅**文案**不同——而按 I-9 决策 C，「运行时段只加中文、英文静态目录不碰」，文案差异属已知可接受。真正需要修的是「行为」层面的差异（静默/未实现/丢精度），已归入 ❌

### `docs/doc-reconcile-2026-10-28.md`（4 处）
- L20: - B1 / B2：`ch14` 已改为五端 ✅、C 已与 Rust 同组（只剩 C++ 未做行级 `&base`）。
- L43: **⏳ 仍未做**
- L77: \| B2 \| ⚠️ **过时结论**：「`fragment` 行级 `&base` 展开：C 不展开」 \| `tests/conformance/known_divergences.md` F-03（`11_fragment_splice` 里 C 与 Rust 同组，只剩 C+
- L129: *下一步（第 2 步，不在本棒执行）：待清单与澄清确认后，逐处更新文档；重打 `.vsix`、跑 site 构建、改 `editors/` 均属后续，本棒一律未做。*

### `docs/featag-impl-slices.md`（2 处）
- L47: ⚠️ **声明校验仍未做**（S4/S5）；正文手写 `__*` 也仍放行（v5 独立一条）。
- L74: ⚠️ 行为变更：featag 不能再挂在**无契约**的裸块上（S2/S3 旧用例已改）。`!any` 仍未做（S8）。

### `docs/featag-spec-draft.md`（1 处）
- L6: > S6 落点 → S7 `from-featag` → S8 `!any`；**S9 其它四端策略未做，且用户已定「之后统一同步」**）。

### `docs/feature-namespace-plan.md`（3 处）
- L118: **需要逐个清点的地方（待展开，未做）**
- L124: \| `default null` \| 可能合法 \| 是否禁掉？（待定） \|
- L125: \| `any` \| 可含 null \| 若契约开了 `!any`，是否仍允许 null？（待定） \|

### `docs/null-semantics.md`（5 处）
- L14: \| **N2** \| `optional` = 键**缺失**；写 `null` 是**值**，按类型校验 ⇒ 与 `int` 不符就报错 \| **现状已经如此，只需明文化**（零实现成本） \|
- L35: \| **C / C++ / Lua** \| **只有 dump（无 emit 后端）** \| ⬜ **未跑**（本轮未做运行时探针；需各补一条 null 往返用例） \|
- L36: \| **Rust** `latex` / `svg` / `slint` / `xml` / `custom` \| 各 emit 后端 \| ⬜ **未测** \|
- L43: \| **N5-a** \| **dump 五端一致**：`null` 必须原样往返成 `null` \| Rust/JS 已知 ✅；C/C++/Lua 待补用例 \|
- L79: ## 6. 待补（本轮没做，别当已完成）

### `docs/smlb-smldb-spec.md`（10 处）
- L16: - 📌 **支持文件名（sml / smld / smlb 三种，未来规范生效, 尚未实现）**：
- L667: > 本节只是技术分析 + 取舍，**尚未拍板**，新功能仍暂缓、暂不实现。
- L736: > 愿景方向来自产品讨论，**尚未拍板、暂不实现**，记下供决策。
- L758: 三者分工已确认 `.smld`(文本)/`.smlb`(二进制)，仅 `.sml` 兜底位待定稿。
- L765: > ⚠️ 本节是 API 形态的浮想，**未拍板、未实现**，需二次蒸馏成「API 签名 + 命名取舍 + 覆盖矩阵」的定稿。
- L814: ### 15.1 现状约定（已写进文档，是**既定事实**，非待定）
- L833: ### 15.3 「少量空间换保序」的方案分析（用户询问，未实现）
- L861: > 📌 现状：未实现、未拍板。默认仍是「对象键序不保证」。方案需用户裁决后再谈落地。
- L863: ### 15.4 「树的局部保序」可行性（用户认可方向，未实现）
- L877: **落地方向（非拍板、未实现）**：

### `docs/v5-syntax.md`（11 处）
- L3: > ⚠️ **v5 尚未定稿**。本文是 2026-11-04 的工作底稿，用于把散在各处的「v5 候选改动」集中到一页，
- L7: > - 语言层：`docs/featag-spec-draft.md`（v1.0，语义闭合、零实现）
- L18: **尚未定稿的两点**：
- L21: - 本文表 A 全部为**草案**，未实现（`Version::CURRENT` 仍是 `V4`）。
- L43: \| A13 \| **`optional` 与 null 的分工明文化** \| 现状已如此、未写明 \| `optional` = 键**缺失**；写 `null` 是值、按类型校验 \| ✅ 已定（N2，零实现成本） \|
- L59: **尚未落地、属未来拆分的**：
- L63: \| B6 \| **版本表示重构**（枚举 → 「标识 + 功能集查表」），**不再是「加一个 `Version::V5` 枚举项」** \| ⬜ 未做，设计见 **`docs/version-naming.md`** \|
- L64: \| B7 \| `type.featag` 特性位 \| ⬜ 未做 \|
- L65: \| B8 \| 跨端版本号统一（C 库 `0.2.0` / VSIX `0.5.0` / Rust `0.6.x` 各记各的） \| ⬜ 未定，CHANGELOG 明写「留待发布动作」 \|
- L66: \| B9 \| **Rust emit 对 null 统一为「不产出」** \| ⬜ 现状不一致：md **丢掉**顶层 null、html 写 `(空)`（实测）⇒ 见 `docs/null-semantics.md` N5-b \|
- L67: \| B10 \| **dump 五端 null 往返一致**（C/C++/Lua 用例待补） \| ⬜ Rust/JS 已知 OK，其余三端未跑 \|

### `docs/version-naming.md`（1 处）
- L165: \| 9 \| **`smltools` 迁移工具**（新功能） \| 把 `Removed` 版本的文档升级到新写法（至少改 `@version` 行；是否连语法一起迁移待定） \|

## 4. 错误码 `impls` 未覆盖五端（124 条）

> 这是「实现缺端」最硬的证据源：码表自己声明了谁实现了它。分成两档看 ——
> **「只有 Rust」= 新能力还没跨端**；**「缺 1–2 端」= 老能力里的历史缺口**。

### 4.1 只有 Rust 实现（42 条）—— 新能力/新语义的跨端欠账

| 码 | 行 |
|---|---|
| `E-LEX-006` | 100 |
| `E-LEX-007` | 104 |
| `E-PARSE-004` | 122 |
| `E-PARSE-007` | 134 |
| `E-PARSE-013` | 158 |
| `E-PARSE-014` | 162 |
| `E-PARSE-015` | 166 |
| `E-PARSE-016` | 170 |
| `E-PARSE-017` | 173 |
| `E-PARSE-018` | 177 |
| `E-PARSE-025` | 205 |
| `E-PARSE-026` | 210 |
| `E-PARSE-027` | 214 |
| `E-PARSE-028` | 218 |
| `E-CONTRACT-019` | 222 |
| `E-CONTRACT-025` | 226 |
| `E-CONTRACT-024` | 230 |
| `E-CONTRACT-022` | 234 |
| `E-CONTRACT-023` | 238 |
| `E-CONTRACT-021` | 242 |
| `E-CONTRACT-020` | 246 |
| `E-LIMIT-004` | 272 |
| `E-FEATURE-006` | 321 |
| `E-FEATURE-007` | 325 |
| `E-FEATURE-008` | 328 |
| `E-FEATURE-010` | 335 |
| `E-CONTRACT-011` | 386 |
| `E-CONTRACT-016` | 406 |
| `E-INCLUDE-007` | 444 |
| `E-INCLUDE-008` | 448 |
| …… | 另有 12 条 |

### 4.2 其它缺端组合（82 条）—— 含「只有某一端」与「缺一两端」，明细见表

| 码 | 已实现 | 缺 | 行 |
|---|---|---|---|
| `E-PARSE-006` | rust js cpp lua | c | 130 |
| `E-PARSE-009` | js | rust c lua cpp | 142 |
| `E-PARSE-010` | js | rust c lua cpp | 146 |
| `E-PARSE-011` | rust js | c lua cpp | 150 |
| `E-PARSE-012` | rust c lua | js cpp | 154 |
| `E-PARSE-019` | rust js lua | c cpp | 181 |
| `E-PARSE-020` | rust c js | lua cpp | 185 |
| `E-PARSE-021` | rust cpp js lua | c | 189 |
| `E-PARSE-023` | rust cpp lua | c js | 197 |
| `E-PARSE-024` | cpp | rust c js lua | 201 |
| `E-LIMIT-002` | rust js | c lua cpp | 264 |
| `E-LIMIT-003` | rust c cpp lua | js | 268 |
| `E-LIMIT-005` | rust smltools | c js lua cpp | 276 |
| `E-LIMIT-006` | rust smltools | c js lua cpp | 280 |
| `E-LIMIT-007` | rust js | c lua cpp | 283 |
| `E-LIMIT-008` | rust js | c lua cpp | 287 |
| `E-LIMIT-009` | js | rust c lua cpp | 291 |
| `E-LIMIT-010` | c cpp | rust js lua | 295 |
| `E-FEATURE-001` | rust js lua | c cpp | 301 |
| `E-FEATURE-002` | rust js | c lua cpp | 305 |
| `E-FEATURE-003` | rust js | c lua cpp | 309 |
| `E-FEATURE-005` | rust c js | lua cpp | 317 |
| `E-FEATURE-009` | rust js | c lua cpp | 331 |
| `E-CONTRACT-007` | rust js | c lua cpp | 370 |
| `E-CONTRACT-009` | rust js | c lua cpp | 378 |
| `E-CONTRACT-010` | rust c cpp lua | js | 382 |
| `E-CONTRACT-012` | rust lua | c js cpp | 390 |
| `E-CONTRACT-013` | rust js | c lua cpp | 394 |
| `E-CONTRACT-014` | js | rust c lua cpp | 398 |
| `E-CONTRACT-015` | js | rust c lua cpp | 402 |
| `E-CONTRACT-017` | js c cpp lua | rust | 410 |
| `E-INCLUDE-003` | rust c cpp lua | js | 428 |
| `E-INCLUDE-004` | rust c cpp lua | js | 432 |
| `E-INCLUDE-005` | rust js | c lua cpp | 436 |
| `E-INCLUDE-010` | rust c cpp lua | js | 455 |
| `E-INCLUDE-011` | rust cpp | c js lua | 459 |
| `E-INCLUDE-012` | rust lua c cpp smltools | js | 464 |
| `E-EXT-001` | rust js | c lua cpp | 470 |
| `E-EXT-003` | rust js | c lua cpp | 478 |
| `E-EXT-004` | rust js | c lua cpp | 481 |
| …… | | | 另有 42 条 |

## 5. 一致性台账里的 DIVERGE（9 处）

- L8: **AGREE 34 / DIVERGE 7**（2026-11-03 第二棒：`08`/`09`/`19` 一半各修好一处，32–9 → 34–7）。
- L22: - ⚠️ 剩下 7 条 DIVERGE 类：`10`/`11`（C++ 行级 `&frag`）、`19`（`1.0` 的浮点身份，**只剩 JS/Lua**）、
- L39: - 退出码：`0` = 无 DIVERGE / PARSE_FAIL；`1` = 有。**本台账还没清空 ⇒ 现在是 1，属预期**。
- L66: \| 端 \| AGREE \| DIVERGE \| PARSE_FAIL \| SKIP \|
- L74: 合计：**AGREE 17 / DIVERGE 16**（W24 修 C 单词修饰符前为 AGREE 15 / DIVERGE 18：
- L75: `c` 的 AGREE 25→28、PARSE_FAIL 4→1；`13/14` 整体转 AGREE、`16` 的 C 格转 A、`33` 仍 DIVERGE 属预期）。
- L80: 它和 `DIVERGE` 都算「不一致」，都不许静默。
- L137: 修复后 `13/14` 整体 AGREE、`16` 的 C 格转 A（余 JS 半仍 DIVERGE，见 #9 的 JS 半）。
- L249: 6. 跑出来是 DIVERGE ⇒ **先判真假**：是「某端实现不同」还是「用例写错了 / 载体承载不了」？

## 6. 源码里的 `TODO` / `FIXME` / `todo!`（0 处）


## 7. 不在 `TODO.md` / `IDEAS.md` 索引里的未完成面（人工补）

| 项 | 来源 | 说明 |
|---|---|---|
| **`.smlb` / `smldb`** | `docs/smlb-smldb-spec.md`、`docs/value-layout.md`、IDEA I-18 | 规范已写、**零实现**；`value-layout.md` 的 9 条验收**全未打勾**；细节用户未定 |
| **v5 / edition 模型** | `docs/version-naming.md`（定稿）、`v5-syntax.md` | Rust 侧 S0a/S0b 已落；**四端未同步**；`5.0` 仍 `Dev` 且未启用 |
| **featag 四端** | `docs/featag-spec-draft.md`、`featag-impl-slices.md` | 见 §4 的 `impls: [rust]` 证据 |
| **标识符规则 S1b** | `docs/featag-impl-slices.md` §2 | 白名单/同形字/NFC/单一语种；特性位已加，四端未做 |
| **null 语义 N3** | `docs/null-semantics.md` | `default null` v5 起禁用 —— 未实现 |
| **文档架构 P2–P4** | `docs/doc-architecture.md` | 搬家（阻塞于 IDEAS/TODO 引用）、C 类 209 示例、生成化 |
| **官网其余页面** | `site/layouts/index.html` | 只有首页；下载页版本落后、教科书未重组、`llms.txt` 手抄 |
| **教科书编号冲突** | `site/content/{zh,en}/book/` | ch11 两份、ch12 两份（构建后实测） |

## 8. 文档之间**互相矛盾**（以代码/实测为准，需回写过期的一侧）

> 这类条目最容易把人带沟里：照着文档做，会发现「代码早就不这样了」。

| # | 过期说法 | 位置 | 实际（判据） |
|---|---|---|---|
| 1 | featag 的 S4/S5「声明校验仍未做」、`!any` 未做 | `docs/featag-impl-slices.md` L47/L74 | 已有 `rust/tests/featag_decl.rs`、`featag_validate.rs`、`featag_any.rs`（均通过） |
| 2 | `type.featag` 特性位「⬜ 未做」 | `docs/v5-syntax.md` B7 | `sml-feature` 的 `FEATURES` 已有 `("type.featag", Feature::Featag)` |
| 3 | featag `Status: Blocked`、未开工 | `IDEAS.md` I-6（L204） | Rust 侧 S0–S9 已收官（`e7bc534`）；缺的是**四端** |
| 4 | `default null` 是否禁用「待定」、`any` 里 null「待定」 | `docs/feature-namespace-plan.md` L124/L125 | `docs/null-semantics.md` 已定 N3（禁）/ N4（允许） |
| 5 | C++「不支持 include / 把 include 静默当键值」 | `docs/capability-matrix.md` L162/L241 | I-14 已补齐裸 `include`（`380ae38`），套件 `08/09` 转 AGREE |
| 6 | `Version::CURRENT` 仍为 V4 | `docs/v5-syntax.md` L21 | 属实，但「加 `Version::V5` 枚举项」已作废，改为「标识 + 查表」（`docs/version-naming.md`） |

## 9. `weak*` 家族实况（用户 2026-11-04：「weakdoc done」）

**三件在 Rust 侧都已实现**，不是概念阶段 —— `IDEAS.md` 的 I-5 / I-15 / I-17 状态已过期：

| 指令 | 实现在哪 | 做了什么 |
|---|---|---|
| **`@weakdoc <profile>`** | `parser.rs:1483`、`smltools/main.rs:363/1252/1270` | 只登记 profile 名（库**不读文件系统**）；拒载荷块；文档级判重（`E-PARSE-030`）；工具侧按「显式 → 项目 `.smlex/profiles` → 用户全局 → 系统」查找；执行 `checks`（`require_top_level` / `forbid_contracts` / `forbid_keys`）；`applies_to` 只提示不自动挂 |
| **`@weakdata`** | `parser.rs`（`WeakData` / `weak_data`）、`ext.rs` | 同名后者覆盖（`weakdata-spec-draft.md` §3） |
| **`@weakbind`**（原 `weakport` 改名） | `parser.rs`（41 处）、`ext.rs`（20 处） | 只作用于**紧邻的下一个块**（dangling / duplicate / target-not-block 共用 `E-PARSE-019`）；软校验只读、不阻断，走 `Warning`，八档用消息前缀区分 |

**规范都在**：`docs/weakbind-spec-draft.md`（19.7 KB）、`weakdata-spec-draft.md`（4.5 KB）、
`weakdoc-spec-draft.md`（5.3 KB）；码表里相关条目 7 条。

**三个共同缺口**

1. ✅ ~~零测试~~ → **已补（2026-11-04）**：`rust/tests/weak.rs` **19 条**，覆盖
   `@weakdoc` 重复挂（`E-PARSE-030`）/ 载荷块被拒 / `@weakdata` 载荷不进树与「同名后者覆盖」/
   `@weakbind` 只作用于下一个块、字段体、三类非法（`E-PARSE-029`）/ 软校验四档
   （`block.missing_required` / `decl.unknown_field` / `decl.unknown_contract` / 空内置表跳过）/
   **嵌套块里的路径记录** / **「有无 `@weak*` 主树逐字节相同」** / 三个指令未开 `type.weak` 报 `E-FEATURE-001`。
   📌 **更正**：工具侧**并非没有测试** —— `smltools/tests/` 里已有 `resolve_profiles.rs`（显式目录优先、
   项目级向上查找、`--no-profiles`、未命中退出码 1）、`profile_checks.rs`（三项 checks + 找不到只是 warning）、
   `profile_suggest.rs`（`applies_to` 提示）、`lint_weakbind.rs`（弱声明不算契约定义）。
   ⚠️ 真正仍缺：**user-global / system 两层的查找**（现有用例只覆盖 显式 / 项目 / 关闭），
   以及 `.smlex` **读不出来**（IO / 解析 / name 不符）三条码的用例；
2. **四端未同步**：相关码的 `impls` 只有 `rust`（见 §4.1）；
3. **文档状态过期**：`IDEAS.md` I-5 / I-15 / I-17 仍写 `Todo` / 「概念阶段」，§2 表就是照抄的。
