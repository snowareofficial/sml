# 未完成清单（2026-11-06 快照 · 一处看全）

> **权威仍是 `TODO.md` / `IDEAS.md`**（用户维护）；本文件只是把散在四处的欠账合成一页，
> 每条带来源行号，方便「还剩什么」一眼看完。
>
> ⚠️ **本文件取代 `docs/open-work.md`**：那份是 2026-11-04 一天之内三次扫描的叠加（§1–§9 早快照、
> §10「当前口径」、§11 第三次核查），条目与行号已漂移，留着只查历史细节。
>
> 记录口径：**只列「还没做完」与「文档说错了」**；已完成的不进正文，统一在 §9。

---

## 0. 数字（先看这个）

| 维度 | 数量 | 出处 / 复核方式 |
|---|---|---|
| `TODO.md` 显式未勾 | **42** | `Select-String TODO.md "^\s*- \[ \]"` |
| `IDEAS.md` 未完成条目（Todo / Partial / Blocked / 方案已定未建） | **17** | I-5/6/8/9/10/11/13/14/15/16/17/18/19/20/21/22/23（I-12 已 Rejected，不计） |
| 错误码「只有 Rust」 | **42** | `errors/codes.sml` 的 `impls`；另有 82 条其它缺端组合（`open-work.md` §4，2026-11-04 快照） |
| 一致性台账 DIVERGE | **7**（AGREE 34，41 例） | `tests/conformance/known_divergences.md`（2026-11-03 数据，需重跑） |
| 文档互相矛盾 / 过期 | `open-work.md` 已登记 **6 条** + 本轮新发现 **8 条** | §8 |
| CI 是否跑 fuzz / bench | **都不跑** | `.github/workflows/*.yml` 无 `fuzz` / `bench` 关键字 |

**一句话**：单端（Rust）功能基本清空；剩**两条主线** —— **四端同步** 与 **v5 定稿**；
`.smlb` 库侧 T0–T6 已落，剩 **T7 五端** 与 **`smldb`**；再外挂一批发布治理与文档债。

---

## 1. 三条卡点（不解决，别人没法开工）

1. **`5.0` 未启用**：`@version 50 "camelia" dev` 实测报 `E-FEATURE-004`（S0b 有意为之）
   ⇒ featag / weak* / v5 新语法只能靠 `@feature enable ...` 在 v4 文档上试跑，版本位这条路没通。
   来源：`open-work.md:332`、`docs/v5-syntax.md:21`。
2. **四端欠账 42 条码**：新能力全在 Rust，跨端同步必须一次性做（§2）。
3. **文档状态与代码脱节**：照着过期文档做会白干（§8），且**没有可重跑的扫描脚本**
   （`open-work.md` 自称「自动扫描」，实际是人工整理）。

---

## 2. 主线 A：跨端同步（最大一块）

| 项 | 内容 | 来源 |
|---|---|---|
| 错误码 | 42 条 `impls: [rust]`（featag 6 / weak* 3 / v5 若干 / 老能力历史缺口 82 条另计） | `errors/codes.sml`；`open-work.md` §4 |
| 一致性套件 | 41 例，AGREE 34 / DIVERGE 7：`10`/`11` 的 C++ 行级 `&frag`、`19` 的 JS/Lua 浮点身份等 | `tests/conformance/known_divergences.md`；`python tests/conformance/run_conformance.py` |
| featag 四端 | Rust S0–S9 已收官；C/JS/Lua/C++ 未认识 `type.featag` | `IDEAS.md` I-6（状态 `Blocked` 已过期） |
| weak* 四端 | `@weakbind` / `@weakdata` / `@weakdoc` 库+工具均完成，仅 Rust | `IDEAS.md` I-5/I-15/I-17（`Partial`） |
| 版本模型 | `Edition` 已替换 `Version`，四端未同步 | `docs/version-naming.md` |
| TODO 里的跨端 7 项 | 顶层标量口径 L166 · 错误边界 L201 · Lua/JS 单测 L203 · C `sml_dump` 对齐 L204 · Lua 契约数组 L318 · Lua 嵌套数组 L967 · `k: "abc\` 三端三码 L1093 | `TODO.md` |
| 临时探针 | `known_divergences.md` 末节约 40 个 `_*` 文件，等套件全 AGREE 后清 | `TODO.md:969` |

---

## 3. 主线 B：v5 定稿（`.smlb` 的前置）

| 项 | 内容 | 来源 |
|---|---|---|
| `5.0` 启用 | 现在 `Dev` + 报错；A 表条目未实现 | `docs/v5-syntax.md` A 表（11 处 ⬜） |
| B6 | 版本表示重构（标识 + 功能集查表）已做；文档仍写 ⬜ | `docs/v5-syntax.md:63`（**过期**） |
| B7 | `type.featag` 特性位已加；文档仍写 ⬜ | `docs/v5-syntax.md:64`（**过期**） |
| B8 | 跨端版本号统一（C 0.2.0 / VSIX 0.5.0 / Rust 0.6.x） | `docs/v5-syntax.md:65` |
| B9 | Rust emit 对 null 的产出不统一（md 丢、html 写 `(空)`） | `docs/v5-syntax.md:66`、`docs/null-semantics.md` N5-b |
| B10 | 五端 null 往返一致（C/C++/Lua 用例待补） | `docs/v5-syntax.md:67` |
| A13/N3 | `default null` v5 起禁用 —— 未实现 | `docs/null-semantics.md` |
| 标识符 S1b | `lex.ident` 其实已实现（`sml-lex/identifiers.rs`）；**文档记成未做** | `open-work.md:359-365`（**过期**） |

---

## 4. 主线 C：`.smlb` / `smldb`（I-18）

### 4.1 已落地（别再当欠账）

T0 canonical + 21 例向量 · T1 容器（40 B 头/段表/对齐）· T2 manifest/caps/payloads/pack·unpack ·
T3 随机访问（Store + 零拷贝 + 按需解帧 + mmap）· T4 层链签名 + `--trust` + 写者锁 + `compact` ·
T5 段级 zstd + 帧级 FRAMES + 按块 AEAD · T6 `sml::envelope` + CLI 互转。
验收命令见 `HANDOFF-2026-11-05.md §0`；测试 `cargo test -p smlb --features "preserve_order,sign,compress,crypto,mmap"`（全绿）。

### 4.2 还没做

| 项 | 内容 | 来源 |
|---|---|---|
| **T7 五端解码器** | JS / Lua / C / C++ **未开工**（只做 JS + C 也可先覆盖浏览器与嵌入式） | `HANDOFF-2026-11-05.md:43` |
| **`smldb`** | 未开工：选择器（§7 已从 `.smlb` 挪过来重新定义）、FTS 方案、表结构 | `spec §16.2` / `§23.3` |
| `DIR` 段 | 故意不写（只省每层 O(兄弟数) 键比较）；`Store::get` 现在每次建 `STRTAB` 表（O(键数)） | `spec:1440` |
| P7 `SOURCE` 段 | 是否带「原文 + 行号映射」未定 ⇒ 注释/源码位置保不保 | `spec §16.2` |
| P2 层链形态 | 实现已按 D19/D20 落地，文档仍列「待定」 | `spec §16.2`（**过期**） |
| P6 | D2 的施工图 —— 已由 T0/T2 实现，文档仍列「待定」 | `spec §16.2`（**过期**） |
| P10 | 能力裁剪与格式扩展共用判定函数 —— 已有 `check_caps`，未明确「同一函数」判据 | `spec §16.2` |
| canonical 三条 | 顶层标量可回读 · 浮点跨端口径 · 元数据字节侧前置 | `spec §23.3` |
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
| I-19 | 门面 crate 未建；**2026-11-06 改名 `smlkit` → `swml`**（`swsml` 退居 core） | 未建 |
| I-20 | 长文档链：`--split-by` / EPUB / 增量 emit / 源定位 未做 | `Partial` |
| I-21 | 六类语义部件（figure/admonition/footnote/deflist/sub·sup/toc）未做 | `Partial` |
| I-22 | 下游方言六问（源文件未入库，**有对外发布禁令**） | `Todo` |
| I-23 | 表示层性能（把「没开的 feature」变成不走的路径） | `Todo` |
| I-6 | featag：Rust 已完成，`IDEAS` 仍写 `Blocked` | **状态过期** |

---

## 6. 工具 / 站点 / 编辑器

| 项 | 内容 | 来源 |
|---|---|---|
| `smltools --migrate` | 不存在（V9 列为 v5 新功能） | `docs/version-naming.md` V9 |
| `smltools --split-by` | 不存在（I-20） | `open-work.md:373` |
| 官网 | 只有首页；下载页版本落后；教科书 ch11/ch12 各两份；`llms.txt` 手抄 | `open-work.md:268-269,343` |
| 示例 doctest | 209 个示例无 doctest | `open-work.md:343` |
| `.vsix` | 源码 `0.5.0`，入库产物只到 `0.4.3`；重打需网络 + 版本号（用户动作） | `TODO.md:1003` |
| weak* 改 vsix | 三个 weak 指令任一落地都要同步扩展 | `TODO.md:1007` |

---

## 7. 发布治理（发布前要清）

`TODO.md` L802–L810：清 DeepSeek 开发痕迹 · 国际化（README/crate 文档/CLI 帮助）·
双授权（MIT/Apache-2.0 + 木兰）· 各 crate 补 GitHub `repository` 地址。

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
| `.swl` 文件后缀 | **已裁决（2026-11-06）：定 `.swl`** | 文本后缀 `.swl`；`.sml` 作**长期读取别名**（516 个受控文件不动）；`.smld` 作废；**写出默认值改到 v5**；规范落点 `docs/amberice-smldb-spec.md §0`（`TODO.md §11.2`） |
| 官网重写 | **待做，顺序绑命名** | 只有首页 / 下载页版本落后 / 教科书 ch11·ch12 重复 / `llms.txt` 手抄；等 11.1–11.2 定稿（`TODO.md §11.3`） |
| 发布工作流 `release.yml` | **待做** | tag + 人工审批；crates.io 按拓扑序 + dry-run；VSIX/Open VSX/Release 附属产物；顺序在命名与迁 GitHub 之后（`TODO.md §12.1`） |
| 定时部署 `deploy.yml` | **待做，对象待明确** | Cloudflare 现为 **Pages**（`sml-site`）非 Worker；VPS 部署内容仓库无记录；**触发改用 Ruby 脚本 + 系统级定时器**（Actions cron 不准点且 60 天停用）；`TODO.md §12.2` |
| swsml / smlb 深水区 | **新任务，未开工** | 先打 tag 存档（基线 = 两份 bench + 测试 + fuzz 语料）→ 有数字的优化（≥15%）→ 按面做漏洞/缺陷检查（clippy / miri / osv / ASAN 长跑）；`TODO.md §13` |
| `_*` 文件归档 + 文档元数据 | **新任务，一刀切** | 受版本控制 `_*` 18 个、磁盘约 231 个；**一次切完、不留新旧并存**；元数据与扫描器**都用 SML 自己**（`docs/meta.sml` + `smltools docs`），不用 YAML/Python（`TODO.md §14`） |

---

## 8. 文档治理（用户 2026-11-06 点名：**过于混乱、落后**）

> 已记进 `TODO.md`（发布前待办）。这里给出症状与「做完的定义」，否则这条会变成一句抱怨。

**症状（都有实例）**

| 类型 | 实例 |
|---|---|
| 同一事实多处各说一套 | `open-work.md` 一天内三份快照叠在一起（§1–§9 / §10 / §11） |
| 已完成的没回填 | `HANDOFF-2026-11-05.md:44-45` 写「fuzz 只有 1 个 target / bench 只有解析」；实际 **9 个 target + 3 份 bench**（2026-11-06） |
| 同上 | `docs/perf-baseline.md` 只有 `parse`；`value` / `smlb` 两份 bench 未进基线 |
| 同上 | `IDEAS.md` I-6 仍 `Blocked`（Rust S0–S9 已收官）、I-18 仍「零实现」（T0–T6 已落） |
| 已过期的没删 | `spec §16.2` 的 P2/P6 标「待定」，实际已实现 |
| 同上 | `docs/v5-syntax.md` B6/B7 标 ⬜，代码已做；`featag-impl-slices.md` S4/S5 标未做，测试已有 |
| 残句 / 错位 | `TODO.md` L811–813 是三行无归属的碎句（上一条列表与下一条「雷 2」之间） |
| 无法重跑 | `open-work.md` 自称「自动扫描」，仓库里**没有生成脚本** |

**做完的定义**

1. `docs/unfinished.md`（本文件）是**唯一**「还剩什么」清单；`open-work.md` 归档不再更新；
2. 每份规范/交接头部有「最后核对日期 + 对应代码版本（commit）」；
3. `tools/` 下有一个能重跑的扫描脚本（TODO 勾选 / IDEAS 状态 / 码表 `impls` / 文档 ⬜），
   产出即本文件，禁止手工维护两份；
4. 「过期说法」一律**删掉或改写**，不保留「曾经如此」的正文（历史归 git）。

**2026-11-06 追加口径（用户，含对“分批”的纠正）**：**一刀切，彻底**，不许新旧并存；
整理只做**复制 / 合并 / 重写**；文档类**不许直接读原文** —— 先看生成的索引/报告；
每份文档必须有**写日期、职责、状态**，**变更记录单独成文**；目标：**可扫描、可报告、可统一修改**。
**元数据与扫描器都用 SML 自己**（`docs/meta.sml` + `smltools docs`），不引 YAML、不用 Python 扫描器 —— dogfooding。
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
| 未做 | **CI 不跑 fuzz/bench**（需 nightly + ASAN，可另开 job）；`smldb` 无 target；T7 五端无一致性 fuzz | §0 |

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

## 11. 新会话的唯一任务：把 TODO / IDEAS 做成**两张表**（2026-11-06 用户定）

> 用户原话：「新会话的任务是做 todo、ideas 两张表，**单纯做两张表**」。
> 所以：**只产表，不执行任何条目、不改代码、不勾选、不删原文件、不跑长测试**（允许读文件与 grep 核对）。

**输入（只读）**

| 来源 | 取什么 |
|---|---|
| `TODO.md` | 全部 `- [ ]`（当前 **42**）+ 已勾的 `- [x]`（作为 Done 行） |
| `IDEAS.md` | I-1…I-23 的编号 / 名称 / Status / 内部未勾 |
| `docs/unfinished.md` | 分类、判据、复核命令（本文件） |
| `errors/codes.sml` | 跨端欠账的硬证据（`impls` 字段） |
| `docs/smlb-smldb-spec.md` §16.2 / §23.3 | `.smlb` 内部未定项（P2/P7/P10…） |

**输出：两张 Markdown 表**（可以是一个文件两张表，也可以两个文件；由你定）

- **TODO 表**：`编号 | 主题 | 状态 | 优先级 | 依赖 | 完成判据 | 来源(file:行) | 卡在谁`
- **IDEAS 表**：`编号 | 名称 | 状态 | 类型 | 依赖 | 下一步 | 来源(file:行)`

**规则（按重要性）**

1. **一行一条**；同一件事在多处出现只留一行，其余并进「来源」列，不合并不同条目；
2. **状态以实测/代码为准**，不以文档自述为准（现实例：I-6 写 `Blocked` 但 Rust S0–S9 已收官；I-18 写「零实现」但 T0–T6 已落）；
3. **已完成也保留一行**，标 `Done`，单独放表尾 —— 目的是“别再当欠账”，不是删记录；
4. **每行都要能回答**：是什么 / 为什么没做 / 做完的标准 / 卡在谁（人、决策、还是外部条件）；
5. **数字可复核**：表头给出取数命令（`Select-String` / `git grep` / `python tests/conformance/...`），
   不写“约”“左右”这类无法验证的量；
6. 表里**不许出现**“见上文”“同上”这类依赖上下文的写法 —— 单行拿来就能排期。

⚠️ 做完两张表就停。`TODO.md §10.1/§10.2`（命名 / 迁 GitHub）与 §12–§14 都是**发布或大改动作**，
要单独排期，不要在这个会话里顺手做。
