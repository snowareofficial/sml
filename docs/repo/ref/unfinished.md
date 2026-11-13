# 未完成清单（2026-11-13 核对 · 一处看全）

> **最后核对：2026-11-13**。核对基准不锚定 HEAD；要复现当时的代码版本用
> `git log -1 --format='%h %cs %s' -- rust/`。本行只维护核对日期，提交推进不改这里。

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

- 单端（Rust）功能基本清空；剩**两条主线** —— **四端同步**（P3–P7 已收官，P5 的**字面量**
  也已五端同步；只剩 `Removed` 三态 / 兼容检查 / 迁移工具与逐码实现缺口）与 **v5 定稿**。
- `.smlb` 库侧 T0–T6 已落，剩 **`smldb`**；**T7 多端解码器全端已通**（22/22 逐字符，含 C# / Go / Ruby FFI；状态见 `docs/amberice/ref/ports.md`）。
- 外挂一批发布治理与文档债（§7、§8）。

---

## 1. 三条卡点（不解决，别人没法开工）

1. **`5.0` 未启用**：`@version 50 "camelia" dev` 实测报 `E-FEATURE-004`（S0b 有意为之）
   ⇒ featag / weak* / v5 新语法只能靠 `@feature enable ...` 在 v4 文档上试跑，版本位这条路没通。
   来源：`open-work.md:332`、`docs/swsml/spec/v5.md:21`。
2. **仅 Rust 的码 44 条**：其中语言层 **36** 条（DERIVE 8 条是 Rust 专属绑定层，不计）。
   **「通道」欠账已清**（五端都有「成功 + 诊断列表」出口，见 §9），剩下是**逐码实现缺口**
   —— 例如 `W-PARSE-002` 四端有通道但片段合并路径未接（§2）。
3. **生成物与实时扫描未同步**：`docs/repo/ref/scan.md` 由 `tools/scan_status.py --write` 生成，文档改动后必须重跑，否则计数过期（§8）。

---

## 2. 主线 A：跨端同步（最大一块）

| 项 | 内容 | 来源 |
|---|---|---|
| 错误码 | 44 条 `impls: [rust]`，其中语言层 **36** 条（DERIVE 8 条是 Rust 专属绑定层，不计）。**逐码缺口**，非「无通道」：五端都有诊断出口，缺的是具体告警没接（如 `W-PARSE-002`） | `errors/codes.sml`；`docs/repo/ref/scan.md` |
| 一致性套件 | **60 例 AGREE 60 / DIVERGE 0**；`19` 的浮点身份是 JS/Lua 的载体限制，以 `*` 放行，**待登记为永久取舍** | `tests/conformance/known_divergences.md` 头部（2026-11-13 口径）；`python tests/conformance/run_conformance.py --require-all` |
| featag 四端 | Rust S0–S9 已收官；C / C++ / JS / Lua 按 **S9 策略不实现**，遇 v5 报「本端不支持」，属设计取舍非欠账 | `docs/swsml/plan/featag-slices.md` S9 |
| 版本模型 | **字面量五端已同步（2026-11-13，P5）**：`vN` / `N` / `N.0` / `N0` / 代号全接受，`5.0` 族统一 `E-FEATURE-004`，且版本决定特性基集。仍欠：`Removed` 三态与专用码、兼容检查模块、`smltools --migrate` | `docs/swsml/spec/version.md` §7 |
| FFI / 绑定层能力对等 | **`§0.6` 绑定层列整列收口**：诊断 / 多错误 / 契约表与契约映射（含 `sml_builtin_clone`）/ 三条 weak 旁路表 / 扩展选项 / 文件入口，C-ABI + 五门绑定全部对齐。剩本地四端（C / C++ / JS / Lua）的文件入口仍不交诊断 —— 各端 API 面议题，与 C-ABI 无关 | `docs/swsml/plan/crossend-sync.md` §0.6 / P7；`rust/src/c_abi.rs`；`bindings/README.md` |
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
| 官网 | 新官网由用户单独做；本仓现有 `site/` 内容挂到 `/doc` 子路径（D6）；现有首页 / 下载页版本落后、教科书 ch11/ch12 各两份、`llms.txt` 手抄 | `docs/repo/ref/pending-decisions.md` D6、`open-work.md:268-269,343` |
| 示例 doctest | 209 个示例无 doctest | `open-work.md:343` |
| `.vsix` | 源码 `0.5.0`，入库产物只到 `0.4.3`；重打需网络 + 版本号（用户动作） | `TODO.md:1003` |
| weak* 改 vsix | **源码侧已同步（2026-11-12）**：VSCode / LSP / Zed + `smltools` 高亮基线副本 + 回归钉子（逐项核对 Rust 的 `FEATURES` / `BUILTIN_DIRECTIVES` / `EDITIONS`）；只剩 `.vsix` 产物重打（同上一行，用户动作） | `TODO.md:1003,1007`；`editors/vscode/README.md` |

---

## 7. 发布治理（发布前要清）

`TODO.md` L802–L810：清 DeepSeek 开发痕迹 · 国际化（README/crate 文档/CLI 帮助）·
各 crate 补 GitHub `repository` 地址（**方向已定，精确 URL 待新仓名**，见 D6）。
双授权按 `docs/repo/ref/pending-decisions.md` D2 保持 `MulanPSL-2.0` 单授权，不列入发布前待办。

`TODO.md` L1080–L1087：发布节奏已由 2026-11-10 裁定（**dev1 准备发布、dev2 成批发布**，见 D1 / D6）·
占 GitHub org / 域名 / docs 子域 · 命名陷阱（包名 `swsml` / 库名 `sml`）。

**只能你做**（`TODO.md` L1271–L1324）：两台远端分支保护 · PR 默认目标指 `dev` ·
删存量长期分支 · 删 `origin/feat/i9-contract-typo-hint` / `origin/master` · 双远端同步口径。
要收 PR 还须先解决 `sync-from-gitee.yml` 的 guard 与 PR 合并独有提交的冲突（见 D6）。

### 7.1 2026-11-06 新增（`TODO.md` §十）

| 项 | 状态 | 说明 |
|---|---|---|
| 命名候选 `amberice` | **已裁决（2026-11-06）：`smlb` 改名后的名字** | B 档已落地（`0f1c7f4`：`.ambr` / `AMBR`）；`swsml` **已发 18 版**（最新 `0.6.9-dev.0`）保持 core；`smlkit` → `swml` 另算（下一行） |
| 对外主仓 | **方向已定（D6）：GitHub `swsml` 仓，workflow 单向推送，收 star / issue / PR** | 本仓保持权威开发源；**同步方向不反转**；时点待定，清单见 `TODO.md §10.2`（双远端 / CI / Pages / `repository` / 保护规则）；收 PR 前须调整 `sync-from-gitee.yml` 的 guard |
| 门面改名 `smlkit` → `swml` | **部分完成：crate 已建（2026-11-06）；门面先不动（2026-11-11）** | `rust/swml` = `pub use swsml::*` + `easy` 模块（原 `smlkit-easy` 并入，不再单独发）；三注册表 404 已核；`swsml` 保持 core；**不参与 dev1 / dev2 发布、不拆仓**（D6、`TODO.md §11.1`） |
| 别名 `swmltool` / `swmltools` | **待做，有硬前提** | `smltools` 是 bin-only ⇒ 必须先抽 `lib` 才能做真别名；**纯占位违反 crates.io 政策**（`TODO.md §11.1`） |
| `.swl` 文件后缀 | **已裁决（2026-11-06）：定 `.swl`** | 文本后缀 `.swl`；`.sml` 作**长期读取别名**（516 个受控文件不动）；`.smld` 作废；**写出默认值改到 v5**；规范落点 `docs/amberice/spec/format.md §0`（`TODO.md §11.2`） |
| 官网 | **用户单独做，本仓不实施** | 新官网由用户自建；本仓现有 `site/` 内容挂到新官网 `/doc` 子路径（D6）；旧链接 301 与构建方式待定 |
| 发布工作流 `release.yml` | **待做** | tag + 人工审批；crates.io 按拓扑序 + dry-run；VSIX/Open VSX/Release 附属产物；顺序在命名与迁 GitHub 之后（`TODO.md §12.1`） |
| 定时部署 `deploy.yml` | **待做** | 按 `docs/repo/ref/pending-decisions.md` D4：`cron → CF Worker → repository_dispatch → GitHub Actions`（多平台构建与发布）；无 VPS，rsync / 健康检查 / 回滚方案作废；`TODO.md §12.2` |
| swsml / smlb 深水区 | **13.1 已存档；13.2 的 ①②③ 实测早已修完** | 存档：tag `pre-deep-2026-11-07` + `baseline-2026-11-07/`（两份 bench 原始输出 + fuzz 语料 2549 文件）。优化实测：`Store::get` **10.4 µs**（`TODO` 记的 150 µs 已过期，**14×**），`canonical_bytes` 已两趟预分配，`to_sml` 已 `with_capacity` + 直接 `push_str`。**唯一真实欠账**：`load` 8173 allocs（`flat@4000`，节点级分配，需 arena 类改动，非「15% 小步」）。`clippy` **0 警告**（182 → 0：`missing_safety_doc` 45、两轮 `--fix`、文档类 42、结构类 31、机械类 19 全部清完；3 处 `large_enum_variant` 与 1 处 `should_implement_trait` 是带理由的 `#[allow]`）。`-D warnings` 门禁已写进 `ci.yml`（本地实跑 `cargo clippy --workspace --all-targets -- -D warnings` exit=0） | `TODO.md §13` |
| `_*` 文件归档 + 文档元数据 | **新任务，一刀切** | 受版本控制 `_*` 18 个、磁盘约 231 个；**一次切完、不留新旧并存**；文档元数据用 SML 自己（`docs/meta.sml` + `smldocs`），不引 YAML；`scan.md` 由 `tools/scan_status.py --write` 生成（`TODO.md §14`） |
| 对外 GitHub 仓 | **已裁定（D6，2026-11-10）** | 只做 **GitHub `swsml` 对外主仓**（workflow 推送、收 star / issue / PR）；`amberice` 不再单独建只读镜像仓，dev2 与 `bamcore` / `bambin` 一起发；crate 源码留本仓，真独立需 `sml-value` 先发 crates.io |

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
| 跨端同步 P3–P7（2026-11-13） | 三端预处理 / contract / fragment / env / bareword / `@when` / `@for` 收进 conformance；四端「成功 + 诊断列表」出口；`weak*` 五端 + `W-CONTRACT-001` 七档 + 旁路表；C-ABI 诊断 / 扩展选项 / 内置契约表 / 三条 weak 旁路表 / 聚合结果 `sml_parse_all`；五个绑定层对齐诊断、选项、弱表 | `54` 例 AGREE 54 / DIVERGE 0；`cpp/build_verify.py` 8 档全绿；`cargo test --test c_abi` 33 例；`bindings/*` 各自 ALL OK |
| 码表台账回填（2026-11-13） | `impls` 与实现对齐（`W-PARSE-001` → 五端 done；`W-CONTRACT-001` → 五端 partial）；重生成补齐 `W-CLI-001` 产物滞后（`COUNT` 166 → 167） | `python errors/gen_json.py --check`（`gen_codes` / `gen_js_errors` 同形）三份全通过 |
| 未做 | **CI 不跑 fuzz/bench**（需 nightly + ASAN，可另开 job）；`smldb` 无 target；五端无一致性 fuzz | §0 |

---

## 10. 复核命令（别照抄过期数字）

```bash
# 仓库根
python tests/conformance/run_conformance.py --require-all         # 54 例，AGREE/DIVERGE 现算
python tests/conformance/run_conformance.py --pending --require-all  # 待收敛目录应为空

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

## 12. amberice 临时产物与探针实测（2026-11-10）

> 证据底稿 `docs/amberice/ref/claims-audit.md`（`status: draft`）与讨论记录
> `docs/amberice/decisions.md §16.5`（待裁决 · 不作结论）都不得当结论引用。
> 审计 #11 / #12 / #20 与 `check` 的尾部检测均已落地；基线在 `136803b` 复核，
> 后续扩充见底稿 §0.1 / §8，实测数字见 `decisions.md §16.5.8`。
> 三条探针已于 2026-11-10 实测完，数字与口径见 `decisions.md §16.5.8`；
> 探针 2 暴露的 `check` 可探测性缺口已修复（读端行为不变）。

| 项 | 内容 | 做完的判据 | 来源 |
|---|---|---|---|
| 底稿去留 | 逐条采纳并进 `decisions.md §16.1`，或整体移入 `docs/amberice/archive/` | `meta.sml` 的 `title` / `status` / `scope` 同步，`changes.sml` 记一条，`--index` / `--check` 重跑 | `claims-audit.md §10` |
| §16.5 去留 | 随底稿一并处理，节级「待裁决」横幅按结果去掉或随节归档 | 索引中 `decisions.md` 的 `scope` 不再写「待裁决」；D1–D54 / P12 / §16.3 状态不变 | `decisions.md §16.5` |
| 探针 1 · 成本 | ✅ 已实测：8 MB `append_unsigned` 38.720 ms / 64 MB 296.232 ms；尾部追加 200 B 恒定 1.2–1.8 ms | 数字与口径见 `decisions.md §16.5.8` | `cargo run --release -p amberice --example probe_layers` |
| 探针 2 · 鲁棒性 | ✅ 已实测并修复：读端仍退回基文档，但 `check` 四档均判「未通过」并指出 Trailer 损坏 | 回归用例 `rust/amberice/tests/unsigned.rs::check_reports_truncated_trailer`，另有无层文件防误报用例 | `decisions.md §16.5.8`、`claims-audit.md §8` |
| 探针 3 · 层数 | ✅ 已实测：100 层 / 39,603 B，`load` 0.3209 ms、`Store::value` 0.2837 ms、`Store::get` 0.2300 ms | `compact` 阈值改看体积与层数，不只看读耗时 | `decisions.md §16.5.8` |
| D55 · 尾部恢复 + 真追加 | 读端能发现半截尾并给出明确处置；`append_to_file` 只写 EOF（层记录 + 新 Trailer）并 fsync；恢复扫描界覆盖单层上限 | 探针 1 / 2 复跑；`tests/` 补 torn-append 恢复用例；`check` 与读端结论一致 | `decisions.md` D55 |
