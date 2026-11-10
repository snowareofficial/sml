# 未完成清单（2026-11-09 核对 · 一处看全）

> **最后核对：2026-11-10**（HEAD `3b2d83a` + 工作区未提交改动）。

> **权威仍是 `TODO.md` / `IDEAS.md`**（用户维护）；本文件只是把散在四处的欠账合成一页，
> 每条带来源行号，方便「还剩什么」一眼看完。
>
> **本文件取代 `docs/repo/archive/open-work.md`**：那份是 2026-11-04 一天之内三次扫描的叠加（§1–§9 早快照、
> §10「当前口径」、§11 第三次核查），条目与行号已漂移，留着只查历史细节。
>
> 记录口径：**只列「还没做完」与「文档说错了」**；已完成的不进正文，统一在 §9。

---

## 0. 数字（先看这个）

> 数字由 `python tools/scan_status.py --write` 生成，见 [`scan.md`](scan.md)。
> 本节只留结论；**不再手工维护表格**（`TODO.md` §9 第三条）。

- 单端（Rust）功能基本清空；剩**两条主线** —— **四端同步** 与 **v5 定稿**。
- `.smlb` 库侧 T0–T6 已落，剩 **`smldb`**；**T7 多端解码器全端已通**（22/22 逐字符，含 C# / Go / Ruby FFI；状态见 `docs/amberice/ref/ports.md`）。
- 外挂一批发布治理与文档债（§7、§8）。

---

## 1. 三条卡点（不解决，别人没法开工）

1. **`5.0` 未启用**：`@version 50 "camelia" dev` 实测报 `E-FEATURE-004`（S0b 有意为之）
   ⇒ featag / weak* / v5 新语法只能靠 `@feature enable ...` 在 v4 文档上试跑，版本位这条路没通。
   来源：`open-work.md:332`、`docs/swsml/spec/v5.md:21`。
2. **四端欠账 46 条码**：其中语言层 **38** 条属跨端欠账（DERIVE 8 条是 Rust 专属绑定层），新能力全在 Rust，跨端同步必须一次性做（§2）。
3. **生成物与实时扫描未同步**：`docs/repo/ref/scan.md` 由 `tools/scan_status.py --write` 生成，文档改动后必须重跑，否则计数过期（§8）。

---

## 2. 主线 A：跨端同步（最大一块）

| 项 | 内容 | 来源 |
|---|---|---|
| 错误码 | 46 条 `impls: [rust]`，其中语言层 **38** 条属跨端欠账（DERIVE 8 条是 Rust 专属绑定层，不计） | `errors/codes.sml`；`docs/repo/ref/scan.md` |
| 一致性套件 | 41 例 **AGREE 41 / DIVERGE 0**；`19` 的浮点身份是 JS/Lua 的载体限制，以 `*` 放行，**待登记为永久取舍** | `tests/conformance/known_divergences.md` 头部；`python tests/conformance/run_conformance.py` |
| featag 四端 | Rust S0–S9 已收官；C / C++ / JS / Lua 按 **S9 策略不实现**，遇 v5 报「本端不支持」，属设计取舍非欠账 | `docs/swsml/plan/featag-slices.md` S9 |
| weak* 四端 | `@weakbind` / `@weakdata` / `@weakdoc` 库+工具均完成，仅 Rust | `IDEAS.md` I-5/I-15/I-17（`Partial`） |
| 版本模型 | `Edition` 已替换 `Version`，四端未同步 | `docs/swsml/spec/version.md` |
| TODO 里的跨端 7 项 | 顶层标量口径 L166 · 错误边界 L201 · Lua/JS 单测 L203 · C `sml_dump` 对齐 L204 · Lua 契约数组 L318 · Lua 嵌套数组 L967 · `k: "abc\` 三端三码 L1093 | `TODO.md` |

---

## 3. 主线 B：v5 定稿（`.smlb` 的前置）

| 项 | 内容 | 来源 |
|---|---|---|
| `5.0` 启用 | 现在 `Dev` + 报 `E-FEATURE-004`；表 A 实现已先行，定稿时按 11 条逐条实测 | `docs/swsml/spec/v5.md` A 表、`docs/repo/ref/todo-ideas-tables.md` T50 |
| B8 | 跨端版本号统一（C 0.2.0 / VSIX 0.5.0 / Rust 0.6.x） | `docs/swsml/spec/v5.md` 表 B |
| B9 | Rust emit 对 null 的产出不统一（md 丢、html 写 `(空)`） | `docs/swsml/spec/v5.md` 表 B、`docs/swsml/spec/null.md` N5-b |
| B10 | 五端 null 往返一致（C/C++/Lua 用例待补） | `docs/swsml/spec/v5.md` 表 B |
| A13/N3 | `default null` v5 起禁用 —— 未实现 | `docs/swsml/spec/null.md` |

---

## 4. 主线 C：`.smlb` / `smldb`（I-18）

> **定位**：`.ambr` 是**基座**，场景归附属 crate，不往核心里塞。段表不存逐段偏移（按 `len` 走）、
> `Store` 零拷贝、分层追加——这三条有一部分是**专门给 `smldb` 准备的**，不是为普通配置文件优化。

### 4.1 已落地（别再当欠账）

T0 canonical + 21 例向量 · T1 容器（40 B 头/段表/对齐）· T2 manifest/caps/payloads/pack·unpack ·
T3 随机访问（Store + 零拷贝 + 按需解帧 + mmap）· T4 层链签名 + `--trust` + 写者锁 + `compact` ·
T5 段级 zstd + 帧级 FRAMES + 按块 AEAD · T6 `sml::envelope` + CLI 互转。
验收命令见 `HANDOFF-2026-11-05.md §0`；测试 `cargo test -p smlb --features "preserve_order,sign,compress,crypto,mmap"`（全绿）。

### 4.2 还没做

| 项 | 内容 | 来源 |
|---|---|---|
| **`smldb`** | 未开工：选择器（§7 已从 `.smlb` 挪过来重新定义）、FTS 方案、表结构 | `spec §16.2` / `§23.3` |
| `DIR` 段 | 故意不写（只省每层 O(兄弟数) 键比较）；`Store::get` 现在每次建 `STRTAB` 表（O(键数)） | `spec:1440` |
| P7 `SOURCE` 段 | 是否带「原文 + 行号映射」未定 ⇒ 注释/源码位置保不保 | `spec §16.2` |
| P2 层链形态 | 实现已按 D19/D20 落地，文档仍列「待定」 | `spec §16.2`（**过期**） |
| P6 | D2 的施工图 —— 已由 T0/T2 实现，文档仍列「待定」 | `spec §16.2`（**过期**） |
| P10 | 能力裁剪与格式扩展共用判定函数 —— 已有 `check_caps`，未明确「同一函数」判据 | `spec §16.2` |
| canonical 三条 | 顶层标量可回读（**已做**：`ParseOptions::allow_top_scalar`）· 元数据字节侧前置（**已做**：`ambr_pack_text` + Lua 纯实现）· **浮点跨端口径**（`19`：JS/LuaJIT 载体限制，已登记放行） | `spec §23.3`（前两条**已过期**） |
| `compact` 旧签名证据 | 合并多层后如何保留「旧签名证据」未定 | `spec §23.3` |

---

## 5. 单端功能（`IDEAS.md`）

| 编号 | 内容 | 状态 |
|---|---|---|
| I-8 | TOML 风格 `=` 分隔（`[name]` 块名已搁置） | `Todo` |
| I-9 | 近名契约提示：运行时已做，**编辑器侧未做** | `Done`（含缺口） |
| I-10 | 入口文档（能力矩阵已有，面向新手的入口文档没有） | `Partial` |
| I-11 | 非 Rust 端硬编码中文文案（Rust 侧十二批已清零） | `Partial` |
| I-13 | 「实现即文档」示范只落了一个 | `Partial` |
| I-14 | 排雷：可修项基本修完，剩余跨端 | `Partial` |
| I-16 | 版本后缀已定；**`smltools --check-version` 不存在** | 方案已定、门禁未做 |
| I-19 | 门面 crate `rust/swml` 已建（`pub use swsml::*` + `easy` 模块）；`publish = false`，**未发布** | 部分完成 |
| I-20 | 长文档链：`--split-by` / EPUB / 增量 emit / 源定位 未做 | `Partial` |
| I-21 | 六类语义部件（figure/admonition/footnote/deflist/sub·sup/toc）未做 | `Partial` |
| I-22 | 下游方言六问（源文件未入库，**有对外发布禁令**） | `Todo` |
| I-23 | 表示层性能（把「没开的 feature」变成不走的路径） | `Todo` |
| I-6 | featag：Rust 已完成，`IDEAS` 仍写 `Blocked` | **状态过期** |

---

## 6. 工具 / 站点 / 编辑器

| 项 | 内容 | 来源 |
|---|---|---|
| `smltools --migrate` | 不存在（V9 列为 v5 新功能） | `docs/swsml/spec/version.md` V9 |
| `smltools --split-by` | 不存在（I-20） | `open-work.md:373` |
| 官网 | 只有首页；下载页版本落后；教科书 ch11/ch12 各两份；`llms.txt` 手抄 | `open-work.md:268-269,343` |
| 示例 doctest | 209 个示例无 doctest | `open-work.md:343` |
| `.vsix` | 源码 `0.5.0`，入库产物只到 `0.4.3`；重打需网络 + 版本号（用户动作） | `TODO.md:1003` |
| weak* 改 vsix | 三个 weak 指令任一落地都要同步扩展 | `TODO.md:1007` |

---

## 7. 发布治理（发布前要清）

`TODO.md` L802–L810：清 DeepSeek 开发痕迹 · 国际化（README/crate 文档/CLI 帮助）·
各 crate 补 GitHub `repository` 地址。双授权按 `docs/repo/ref/pending-decisions.md` D2 保持
`MulanPSL-2.0` 单授权，不列入发布前待办。

`TODO.md` L1080–L1087：发布节奏（等 I-18/19 的前提已不成立，需重定）· 占 GitHub org / 域名 /
docs 子域 · 命名陷阱（包名 `swsml` / 库名 `sml`）。

**只能你做**（`TODO.md` L1271–L1324）：两台远端分支保护 · PR 默认目标指 `dev` ·
删存量长期分支 · 删 `origin/feat/i9-contract-typo-hint` / `origin/master` · 双远端同步口径。

### 7.1 2026-11-06 新增（`TODO.md` §十）

| 项 | 状态 | 说明 |
|---|---|---|
| 命名候选 `amberice` | **已裁决（2026-11-06）：`smlb` 改名后的名字** | B 档已落地（`0f1c7f4`：`.ambr` / `AMBR`）；`swsml` **已发 18 版**（最新 `0.6.9-dev.0`）保持 core；`smlkit` → `swml` 另算（下一行） |
| 主仓迁 GitHub | **用户准备中** | 前置：Gitee 推荐先拿完；清单见 `TODO.md §10.2`（双远端 / CI / Pages / `repository` / 保护规则） |
| 门面改名 `smlkit` → `swml` | **部分完成：crate 已建（2026-11-06）** | `rust/swml` = `pub use swsml::*` + `easy` 模块（原 `smlkit-easy` 并入，不再单独发）；三注册表 404 已核；`swsml` 保持 core；**发布未动**（`TODO.md §11.1`） |
| 别名 `swmltool` / `swmltools` | **待做，有硬前提** | `smltools` 是 bin-only ⇒ 必须先抽 `lib` 才能做真别名；**纯占位违反 crates.io 政策**（`TODO.md §11.1`） |
| `.swl` 文件后缀 | **已裁决（2026-11-06）：定 `.swl`** | 文本后缀 `.swl`；`.sml` 作**长期读取别名**（516 个受控文件不动）；`.smld` 作废；**写出默认值改到 v5**；规范落点 `docs/amberice/spec/format.md §0`（`TODO.md §11.2`） |
| 官网重写 | **待做，顺序绑命名** | 只有首页 / 下载页版本落后 / 教科书 ch11·ch12 重复 / `llms.txt` 手抄；等 11.1–11.2 定稿（`TODO.md §11.3`） |
| 发布工作流 `release.yml` | **待做** | tag + 人工审批；crates.io 按拓扑序 + dry-run；VSIX/Open VSX/Release 附属产物；顺序在命名与迁 GitHub 之后（`TODO.md §12.1`） |
| 定时部署 `deploy.yml` | **待做** | 按 `docs/repo/ref/pending-decisions.md` D4：`cron → CF Worker → repository_dispatch → GitHub Actions`（多平台构建与发布）；无 VPS，rsync / 健康检查 / 回滚方案作废；`TODO.md §12.2` |
| swsml / smlb 深水区 | **13.1 已存档；13.2 的 ①②③ 实测早已修完** | 存档：tag `pre-deep-2026-11-07` + `baseline-2026-11-07/`（两份 bench 原始输出 + fuzz 语料 2549 文件）。优化实测：`Store::get` **10.4 µs**（`TODO` 记的 150 µs 已过期，**14×**），`canonical_bytes` 已两趟预分配，`to_sml` 已 `with_capacity` + 直接 `push_str`。**唯一真实欠账**：`load` 8173 allocs（`flat@4000`，节点级分配，需 arena 类改动，非「15% 小步」）。`clippy` **0 警告**（182 → 0：`missing_safety_doc` 45、两轮 `--fix`、文档类 42、结构类 31、机械类 19 全部清完；3 处 `large_enum_variant` 与 1 处 `should_implement_trait` 是带理由的 `#[allow]`）。`-D warnings` 门禁已写进 `ci.yml`（本地实跑 `cargo clippy --workspace --all-targets -- -D warnings` exit=0） | `TODO.md §13` |
| `_*` 文件归档 + 文档元数据 | **新任务，一刀切** | 受版本控制 `_*` 18 个、磁盘约 231 个；**一次切完、不留新旧并存**；文档元数据用 SML 自己（`docs/meta.sml` + `smldocs`），不引 YAML；`scan.md` 由 `tools/scan_status.py --write` 生成（`TODO.md §14`） |
| `swml` / `amberice` 独立 GitHub 仓 | **待裁决（2026-11-07）** | `swsml` monorepo 仍是根仓库；`amberice` 先做**只读镜像**（`git subtree split -P rust/amberice`，接收 issue 不接 PR）；`swml` 建占位仓等命名定稿；真独立需 `sml-value` 先发 crates.io |

---

## 8. 文档治理（用户 2026-11-06 点名：**过于混乱、落后**）

> 已记进 `TODO.md`（发布前待办）。这里给出症状与「做完的定义」，否则这条会变成一句抱怨。

**仍存在的症状**

| 类型 | 实例 |
|---|---|
| 用户维护文件的状态没回填 | `IDEAS.md` I-6 仍 `Blocked`（Rust S0–S9 已收官）、I-18 仍「零实现」（T0–T7 已落） |
| 已过期的没删 | `spec §16.2` 的 P2/P6 标「待定」，实际已实现 |
| 残句 / 错位 | `TODO.md` L811–813 是三行无归属的碎句（上一条列表与下一条「雷 2」之间） |

**做完的定义**

1. `docs/repo/ref/unfinished.md`（本文件）是**唯一**「还剩什么」清单；`open-work.md` 归档不再更新；
2. 每份规范/交接头部有「最后核对日期 + 对应代码版本（commit）」；
3. `tools/scan_status.py` 是唯一计数来源（TODO 勾选 / IDEAS 状态 / 码表 `impls` / 文档 ⬜），
   产出 `docs/repo/ref/scan.md`；文档改动后重跑 `--write`，禁止手工维护两份；
4. 「过期说法」一律**删掉或改写**，不保留「曾经如此」的正文（历史归 git）。

**2026-11-06 追加口径（用户，含对“分批”的纠正）**：**一刀切，彻底**，不许新旧并存；
整理只做**复制 / 合并 / 重写**；文档类**不许直接读原文** —— 先看生成的索引/报告；
每份文档必须有**写日期、职责、状态**，**变更记录单独成文**；目标：**可扫描、可报告、可统一修改**。
**文档元数据用 SML 自己**（`docs/meta.sml` + `smldocs`），不引 YAML —— dogfooding；`scan.md` 的计数由 `tools/scan_status.py --write` 生成。
执行方案见 `TODO.md §14`（本节的 4 条判据被它细化，不冲突、不另起体系）。

---

## 9. 2026-11-05 / 06 已完成（别再当欠账）

| 项 | 内容 | 判据 |
|---|---|---|
| fuzz 覆盖 | 9 个 target：`parse`（多入口）/ `parse_file` / `value_canonical` / `smlb_load` / `smlb_get` / `smlb_pack` / `smlb_layer` / `smlb_crypto` / `regex_compile` | `cargo +nightly fuzz build` 全绿；短跑无 panic |
| fuzz 基础设施 | `common.rs`：分配预算 + 错误分类断言；`smlb` 全能力 feature 打开 | `rust/fuzz/Cargo.toml` |
| fuzz 找到并修的 bug | 层链伪造 Trailer：`last_layer_off + 16` 溢出、按 `layer_count` 预分配 4G 项、`prev_off` 成环死循环 | `rust/smlb/src/layer.rs`；回归 `rust/smlb/tests/hostile_chain.rs`（3 例） |
| bench 覆盖 | `benches/value.rs`（to_sml/canonical/get/eq_ordered）、`benches/smlb.rs`（dump/load/pack/get/Writer/解密/mmap）、`benches/common/mod.rs`（峰值堆计数） | `cargo bench --bench value`；`cargo bench --bench smlb --features "smlb-compress,smlb-crypto,smlb-mmap"` |
| 新增 feature | `smlb-crypto`、`smlb-mmap`（与既有 `smlb-compress` 同形） | `rust/Cargo.toml` |
| T7 多端解码器 | JS / Lua / C / C++ / 纯 Java 解码器 + C# / Go / Python / Ruby FFI 绑定；容器向量 22/22 **逐字符**一致 | `docs/amberice/ref/ports.md`、`lua/test_ambr_write.lua`、`tools/check/check_ambr_write.py` |
| TODO / IDEAS 两张表 | 交付物 [`todo-ideas-tables.md`](todo-ideas-tables.md)：TODO 表（未完成 / Done / 会话新增）与 IDEAS 表 I-1…I-23，逐条带状态、依赖、完成判据、来源行号、卡在谁 | `docs/index.md` 中该文档 `status: current`；取数命令见该文 §0 / §3 |
| 未做 | **CI 不跑 fuzz/bench**（需 nightly + ASAN，可另开 job）；`smldb` 无 target；五端无一致性 fuzz | §0 |

---

## 10. 复核命令（别照抄过期数字）

```bash
# 仓库根
python tests/conformance/run_conformance.py                       # 41 例，AGREE/DIVERGE 现算

cd rust
cargo test -p smlb --features "preserve_order,sign,compress,crypto,mmap"
cargo bench --bench value
cargo bench --bench smlb --features "smlb-compress,smlb-crypto,smlb-mmap"

cd fuzz
cargo +nightly fuzz build
cargo +nightly fuzz run smlb_load corpus/smlb_load -- -runs=20000
```

---

## 11. 两张表：已完成

TODO / IDEAS 两张表在 [`todo-ideas-tables.md`](todo-ideas-tables.md)（`status: current`），
逐条带状态、依赖、完成判据、来源行号与卡在谁；取数命令与复核对账在该文 §0 / §3。
本文件只列欠账，已完成项统一记在 §9。

`TODO.md` §10 / §12–§14 属发布或大改动作，按各自排期单独处理，不随文档整理顺手做。

---

## 12. amberice 临时产物与未跑探针（2026-11-10）

> 证据底稿 `docs/amberice/ref/claims-audit.md`（`status: draft`）与讨论记录
> `docs/amberice/decisions.md §16.5`（待裁决 · 不作结论）都不得当结论引用。
> 代码基线 `136803b`：审计 #11 / #12 / #20 已落地（原子写 + 尾垃圾检测），现状见底稿 §0.1。

| 项 | 内容 | 做完的判据 | 来源 |
|---|---|---|---|
| 底稿去留 | 逐条采纳并进 `decisions.md §16.1`，或整体移入 `docs/amberice/archive/` | `meta.sml` 的 `title` / `status` / `scope` 同步，`changes.sml` 记一条，`--index` / `--check` 重跑 | `claims-audit.md §10` |
| §16.5 去留 | 随底稿一并处理，节级「待裁决」横幅按结果去掉或随节归档 | 索引中 `decisions.md` 的 `scope` 不再写「待裁决」；D1–D54 / P12 / §16.3 状态不变 | `decisions.md §16.5` |
| 探针 1 · 成本 | 8 MB / 64 MB 上 `append` 与「seek + 写尾」的耗时对比 | 有 `amberice` 实测数字；只有磁盘下限微基准不算 | `decisions.md §16.5.8` |
| 探针 2 · 鲁棒性 | 有层文件截尾 1 / 32 / 64 / 128 B，看 `info` / `cat` / `check` | 四条都有结果；跑通前规范不得写「半截尾可优雅退化」 | `decisions.md §16.5.8`、`claims-audit.md §8` |
| 探针 3 · 层数 | 100 层时 `value()` 与 `Store::get` 耗时，用来定 `compact` 阈值 | 有实测数字并补进底稿 | `decisions.md §16.5.8` |
