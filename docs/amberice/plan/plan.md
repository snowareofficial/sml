# `amberice` 改名 + swsml / amberice 极致优化：方案

> 2026-11-06 · **只是方案，未动一行代码、未改任何名字**。
> 口径：`amberice` = `smlb` 改名后的名字（用户 2026-11-06 裁决：`smlb` 要改成 `amberice`）。
> 本文所有「预期」都标了待测；不写「约」「左右」这类没法复核的量。

---

## 0. 已测基线（优化前）

`cargo bench --bench perf -- --quick`（本机，bench profile，5 轮取最小）：

| 负载 | 操作 | ns/op | 吞吐 |
|---|---|---|---|
| `small` 205 B | parse | 8,528 | 22.9 MB/s |
| `large` 909.3 KiB | parse | 6,780,440 | 131.0 MB/s |
| `large@directive` | parse | 9,873,580 | 89.9 MB/s |
| `deep` 120 层 | parse | 66,702 | 10.4 MB/s |
| `large` | to_sml | 3,283,550 | 270.4 MB/s |

分配账（同一次 bench）：`one` 15 / `ten` 77 / `small` **101（10.1/元素）** / `large` **27,868**；`tokenize-only(10 键)` 28（**2.8/键**）。

`cargo bench --bench value -- --quick` 与 `cargo bench --bench smlb --features "smlb-compress,smlb-crypto,smlb-mmap" -- --quick`（同名机同档）：

| 负载 | 操作 | ns/op | 峰值堆 | allocs |
|---|---|---|---|---|
| `flat@4000` 68.1 KiB | `to_sml` | 1,102,555 | **128.3 KiB** | **12,016** |
| `flat@4000` 68.1 KiB | `canonical_text` | 842,045 | 128.0 KiB | 8,015 |
| `flat@4000` 68.1 KiB | `canonical_bytes` | 1,447,155 | **792.0 KiB** | 51 |
| `flat@4000` 68.1 KiB | `dump` | 2,376,413 | **792.0 KiB** | 202 |
| `flat@4000` 103.9 KiB 文件 | `load(整树)` | 3,344,453 | 454.7 KiB | **8,173** |
| `flat@4000` 103.9 KiB 文件 | `Store::get(命中)` | **204,158** | **128.1 KiB** | 29 |
| `flat@4000` 103.9 KiB 文件 | `load+get_path(对照)` | 4,177,973 | 454.8 KiB | 8,178 |
| `flat@4000` 18.0 KiB zstd | `load(zstd+分帧)` | 3,553,690 | 530.2 KiB | 8,179 |
| `flat@4000` 103.9 KiB 文件 | `MappedFile::open` | — | **124 B** | 1 |
| `flat@4000` 4000 键 | `Value::get(命中/未命中)` | 64 / 74 | 0 B | 0 |

> ⚠️ quick 模式比 `docs/swsml/ref/perf-baseline.md` 的正式口径慢约 20%（该文件记 `small` 7,073 ns）。正式 A/B 必须跑**不带 `--quick`** 的版本。

---

## 1. 占用实测（2026-11-06，逐个发请求）

| 位置 | 结果 | 判读 |
|---|---|---|
| crates.io `amberice` | **404** | 空，可用（唯一的硬占用判据） |
| PyPI `amberice` | **404** | 空 |
| npm `amberice` | **404** | 空 |
| crates.io `smlb` | **404** | 也还没发 ⇒ **改名零弃用成本**（这是关键前提） |
| GitHub `github.com/amberice` | **200** | **已被占**，不能当 org |
| GitHub `amberice-lang` | **404** | 可用（建议候选） |
| GitHub `amberice-rs` | **404** | 可用（次选） |
| 域名 `amberice.com` | DNS 解析到 `52.20.84.62` | 已被注册/停放 |
| 域名 `amberice.dev` / `.io` / `.rs` / `getamberice.com` | 均未解析 | 候选；**未解析 ≠ 可注册**，要买时再查 registrar |
| 商标 | **未查** | CNIPA / USPTO 都要查，属发布前动作，不在本方案范围 |

**结论**：注册表三处全空 + `smlb` 自身未发布 ⇒ 改名窗口现在开着；但 GitHub org `.com` 都已被占，**对外身份要落在 `amberice-lang`，不是 `amberice`**。

---

## 2. 改名方案

### 2.1 命名边界（先把「谁是 amberice」钉死）

**`amberice` = `smlb` 这个 crate / CLI 的新名字，只接管这一层。不是 `sml`，不是 `swsml`，也不是门面 `swml`。**

| 层 | 现名 | 改后 | 动不动 |
|---|---|---|---|
| 语言 | SML | **SML** | 不动；`amberice` 不是语言名 |
| 包（crates.io） | `swsml` | **`swsml`** | 不动（I-12 `Rejected`） |
| 库名（`use …`） | `sml` | **`sml`** | 不动 |
| CLI 工具 | `smltools` | **`smltools`** | 不动 |
| 数据库层 | `smldb` | **`smldb`** | 不动（还没开工，名字保留） |
| 二进制信封 crate / CLI | `smlb` | **`amberice`** | **本条唯一要改的** |
| 门面（未建） | `swml`（原 `smlkit`） | **`swml`** | 与本条无关（I-19） |

一句话写进 README 第一段：**`amberice` 是 `.smlb` 信封的实现与 CLI 名；语言仍叫 SML，包仍叫 `swsml`。**

**对外文案不在本次范围**（用户 2026-11-06 补充）：

| 项 | 处置 |
|---|---|
| `swml`（门面包装）· `swsml` · `sml-*`（**`smltools` 不算**） | 这些是**要国际化、高度给人看**的面，README / crate 文档 / CLI 帮助的措辞与命名**动之前先问用户**，不随改名顺手改 |
| 这批 i18n 工作的排期 | 与**文档治理**放一起（`TODO.md` §九 / §十四、本表 T28 / T40–T42、T08），不单开一棒 |
| `amberice` / `smlkit-easy` 算不算这一批 | 待那一棒开工时**问用户**再定；本轮只在 `amberice` 上做机械改名 |

### 2.2 A / B 两档分别叫什么

> **已定（2026-11-06 用户）：走 B 档** —— 后缀 **`.ambr`**、magic **`AMBR`**。
> （若那句「B」指的是备选 `.amberice`，只需改下表两格，其余方案不变。）

两档**改的是同一批 crate 名**，差别只在「格式身份」要不要一起换：

| 层 | 现状 | A 档（只改实现名） | B 档（实现名 + 格式名） |
|---|---|---|---|
| crate / lib / bin | `smlb` | **`amberice`** | **`amberice`** |
| feature 前缀 | `smlb-*` | **`amberice-*`** | **`amberice-*`** |
| 文档里的格式称呼 | `.smlb` 二进制信封 | **`amberice` 信封（`.smlb` 格式）** | **`amberice` 信封** |
| 文件后缀 | `.smlb` | **`.smlb`（不动）** | **`.ambr`**（候选见下表） |
| magic（4 B） | `SWSB` | **`SWSB`（不动）** | **`AMBR`** |
| MIME | 无 | `application/x-smlb`（可选） | `application/vnd.amberice` |
| 规范文件名 | `smlb-smldb-spec.md` | `amberice-smldb-spec.md` | 同 A |
| canonical 向量 | `smlb-canonical.v1.json` | 改名，**内容不变** | 改名，**字节全变**（magic 在文件里） |
| `.smlb` 文件兼容 | — | 全部可读 | 旧文件不认（未发布，无真实用户） |

**A 档**：`amberice` 是**实现名**、`.smlb` 是**格式名**（同 `gzip` ↔ `.gz`）。零格式破坏，但两套词干并存。
**B 档**：一个词干贯穿三层 —— `amberice`（crate）↔ `.ambr`（后缀）↔ `AMBR`（magic）。代价是 spec / 向量 / 示例 / T7 解码器全部按新名写。

#### 后缀与 magic 候选（按「少重名」实测）

| 候选 | 冲突核查 | 结论 |
|---|---|---|
| `.smlb` | 与语言名绑定，但本格式**不是 SML 专属**（能装任意 payload / EMBED） | A 档保留 |
| `.amb` | **Ambisonic 音频**（Wikipedia「Ambisonic data exchange formats」明写 `.amb`） | ✗ 撞 |
| `.ambe` | **AMBE 声码器帧**（DVSI 编解码器，DMR/D-STAR 语料就是 `.ambe`） | ✗ 撞 |
| `.amber` | 「Amber」是通用词（颜色/品牌/多个开源项目），搜索必被稀释 | ✗ 太泛 |
| **`.ambr`** | 未查到文件扩展占用；`AMBR` 是 Nasdaq 一支股票代码（不同领域，可接受） | ✓ **推荐** |
| `.amberice` | 唯一性最好，但长（9 字符），命令行手敲成本高 | ✓ 保守选项 |

magic 同理：`AMBR`（推荐，与 `.ambr` 同词干）/ `AMBI`（次选）/ `AMBE`（**禁止**，撞声码器）/ `SWSB`（A 档保留）。
MIME 用 `application/vnd.amberice`（B）或 `application/x-smlb`（A，仅占位）。

**为什么 B 档在语义上更对**：这个容器一直是「通用信封」——spec 原文写「SML 是内置载荷之一」，还有 EMBED 段装任意文件。叫 `.smlb` 等于把「通用信封」误标成「SML 专用」，`.ambr` 才与能力对齐。

**少重名的四条规矩**

1. 一个词干贯穿到底（B 档：`amberice` / `.ambr` / `AMBR`），不要 `amberice` 配 `.smlb` 再配 `SWSB` 三套词。
2. **不保留 `smlb` 作为别名**（除文档里一句「原名 smlb」），否则永远两套名字并存。
3. 不复用 `amb` / `ambe` / `amber` —— 三个都已有主（音频、声码器、通用词）。
4. 注册表与组织名分开查：crate `amberice` 三处全空；GitHub `amberice` 已占 ⇒ org 用 `amberice-lang`。

### 2.3 逐项映射表（A 档）

| 现状 | 新 | 备注 |
|---|---|---|
| `[package] name = "smlb"` | `amberice` | `publish = false` 保持 |
| `[lib] name = "smlb"` | `amberice` | `use amberice::…` |
| `[[bin]] name = "smlb"` | `amberice` | `USAGE` / 错误前缀 `smlb:` 一起改 |
| 目录 `rust/smlb/` | `rust/amberice/` | `git mv`，保留历史 |
| workspace `members` / 根 `swsml` 的 optional dep | `amberice` | `rust/Cargo.toml` L39 / L85 |
| features `smlb` / `smlb-compress` / `smlb-crypto` / `smlb-mmap` | `amberice` / `amberice-compress` / `amberice-crypto` / `amberice-mmap` | 连**转发特性**一起改（L134–142） |
| `swsml` 的 `[[bench]] name = "smlb"` | `amberice` | benches/smlb.rs → benches/amberice.rs |
| fuzz targets `smlb_load` / `smlb_get` / `smlb_pack` / `smlb_layer` / `smlb_crypto` | `amberice_*` | `rust/fuzz/Cargo.toml` + corpus 目录 |
| `tests/vectors/smlb-canonical.v1.json` | `amberice-canonical.v1.json` | **内容不动**（向量是字节契约），只改路径与 CLI 默认值 |
| `docs/amberice/spec/format.md` | `docs/amberice/spec/format.md` | `smldb` 名字不变 |
| `sml::envelope` 的 `sml_to_smlb` / `smlb_to_sml` / `value_to_smlb` | 见 2.4 | 函数名里出现格式名，属 API 破坏 |

影响面（可复核）：`git grep -l smlb` = **59 个受控文件**；`git grep -c smlb` 词命中 **1,002 行**；`smlb::` **415 处**；其中 `rust/smlb/` 内部 26 个文件。

### 2.4 API 设计

**库（amberice）**

- 根导出**一个不改**：`dump` / `load` / `Store` / `CanonicalValue` / `pack` / `Manifest` / `layer` / `crypto` / `mmap`。改的只有 crate 路径，**语义零变更**。
- feature 名按 2.2 改；`default = []` 不变。
- 版本：`workspace.version` 仍 `0.6.9-dev.0`，**不因改名 bump**（bump 是发版动作）；正式发布直接 `0.7.0`。
- 错误类型与码不变（`ContainerError` 等），只改 `Display` 里可能出现的 `smlb` 字样（如有）。

**`swsml::envelope`（桥）**

三个函数名现在把格式名写死了，改名后必然误导。三选一：

| 选项 | 结果 | 评价 |
|---|---|---|
| i 保持 `sml_to_smlb` / `smlb_to_sml` | 零破坏 | 名字与 crate 脱节，下次再改还得破一次 |
| **ii `sml_to_binary` / `binary_to_sml` / `value_to_binary`** | 用**中立词**，以后改格式名不牵连 | **推荐**；`0.7.0` 一并破 |
| iii `to_envelope` / `from_envelope` | 与模块同名 | 与 `smlb` 叫法一样是「新造词」，不算更清楚 |

**CLI**

- 命令名 `amberice`；子命令集合与退出码（0/1/2）**一个不改**——脚本依赖它们。
- 输入：同时接受 `.smlb` 与 `.amb`（读端宽松，不校验扩展名，本来也不校验）。
- 输出：默认仍写 `.smlb`（A 档），等 B 档裁决再改默认。
- `--version` 打印 `amberice <ver>`；`--help` 里 `.smlb` 的说明保留，加一句「`.smlb` 是格式名」。

### 2.5 执行顺序（一次切完，不留两套）

1. `git tag -a pre-amberice-2026-11-06`（可回滚点，不打发版 tag）。
2. `git mv rust/smlb rust/amberice`；`git mv benches/smlb.rs benches/amberice.rs`；向量文件改名。
3. 机械替换（**按词边界**，别把 `.smlb` 后缀一起替换）：
   - `smlb::` → `amberice::`
   - `use smlb` / `package = "smlb"` / `-p smlb` / `smlb/`（路径）
   - `smlb-compress|crypto|mmap` → `amberice-*`
   - **不替换**：`.smlb`（后缀）、`SWSB`（magic）、`smlb-smldb-spec` 在正文里的历史提法（改成新名 + 一句「原 smlb」）。
4. CLI 文案 / 错误前缀 / 向量默认路径 / fuzz target 名 / CI workflow。
5. `cargo check --workspace --all-targets -j 1` → `cargo test --workspace -j 1` → `cargo run -p amberice --features preserve_order -- vectors --check`。
6. 文档全仓扫（59 文件），一次改完；`README` 第一段加「amberice 是工具名，`.smlb` 是格式名」。
7. 把 `TODO.md §10.1` / `§11.4` 的「待裁决」改成已裁（用户口径：`smlb` → `amberice`），`IDEAS.md` I-18 同步。

**验收**：`git grep -nw smlb` 只剩「格式代号/历史说明」白名单；`cargo test --workspace -j 1` 全绿；`vectors --check` 逐字节通过。

---

## 3. 极致优化方案

### 3.1 纪律（先于技术）

- 先存档（§13.1）：`pre-deep-2026-11-06` tag + 两份 bench 原始输出 + `smlb` 测试结果 + fuzz 语料快照。
- 每项独立可回滚；**同一负载 bench 提升 ≥15% 才保留**，否则回滚并记录「为什么没收益」。
- **禁止为跑分改语义**：键序、错误码、canonical 字节、段布局、公开 API 语义都不许动。
- 正式测量：不带 `--quick`、同机、同负载、5 轮取最小；同时记录 `allocs/op`。

### 3.2 候选表（按「收益确定性 / 风险」排）

| 编号 | 层 | 改动 | 为什么现在能做 | 预期（待测） | 风险 |
|---|---|---|---|---|---|
| **A1** | amberice | `Store::get` 不再每次重建字符串表 | 实测 `get_with_stats` 在分帧路径上 `parse_strtab_table` 一次、`value_from_subtree` 里**又** `parse_strtab` 一次，两趟 O(键数) + 两个 `Vec` | get 路径 −50%~−85%（键越多越明显） | 低：`Store` 加一个缓存字段 + 新增 `value_from_subtree_with_table`；2 个调用点 |
| **A2** | amberice | `canonical_bytes` 预分配 + 写穿 | 现 51 allocs/次（文档记录） | 分配 −30%~−50%，时间 −15%~−30% | 低 |
| **A3** | amberice | `dump` 输出预分配 + 整数/浮点手写写出 | 大文档 31 MB/s | 吞吐 +15%~+25% | 低 |
| **B1** | swsml | `to_sml` 输出缓冲 `String::new()` → `with_capacity(估算)` | `dump.rs:258` 从零长起，大文档要反复扩容 | −5%~−15% | 低 |
| **B2** | swsml | 键不再「复制成 `Box<str>`」：`insert_or_merge` 增加 owned 变体，直接吃 `String` | `insert_field_val(key: String)` 已经拿到所有权，却在 `insert_or_merge(&key)` 里又分配一次 | 每键 −1 次分配（10.1 → ~9.1），时间 −3%~−8% | 低-中：要处理 `Box<str>` 与 `String` 容量差（`into_boxed_str` 可能 realloc，需实测） |
| **B3** | swsml | `Tok::Word(String)` → 借用 `&'a str` | `tokenize-only` 2.8 allocs/键 全在这里 | 分配 −28%，时间 −10%~−25% | **高**：`Tok` 是公开枚举，sml-parse 68 处、sml-include 3 处；生命周期会扩散 |
| **B4** | swsml | 入口「单趟扫描」：BOM / 版本剥离 / 首个 `@` 判定合并成一次 | `small` 的固定开销占大头（10 键 8.5 µs） | `small` −5%~−10% | 中：`scan` 是唯一漏斗，别绕开安全校验 |
| **B5** | swsml | `Object` 小容量起步（`Vec::with_capacity(4)`） | 每块 `entries` 从 0 长起 | 分配 −1~2/块；时间 −2%~−5% | 低-中：小文档内存占用上升 |

**明确不做**（本轮）：

- 值表示大改（键 interning / inline map / arena）——撞 `Value` 公开形态与 `.smlb` canonical，属 T16 的「需要设计决策」。
- 把 SML 特性位编译掉——`@feature` 是**运行期、文档级**开关（T17 已写明）。
- 引第三方库（criterion / itoa / smol_str 等）——零依赖是卖点，且 dev 依赖也受纪律约束。

### 3.3 里程碑

| 阶段 | 内容 | 通过判据 |
|---|---|---|
| M0 | 存档 + 全量基线（`perf` / `value` / `amberice` 三份） | tag 能重建同机同数字 |
| M1 | **A1**（收益最确定）→ 复测 | get 路径 ≥15%；`cargo test -p amberice --features …` 全绿 |
| M2 | **B1 + B2** → 复测 | `to_sml` / parse 各 ≥15%（分别算账） |
| M3 | **A2 + A3** → 复测 | 两项各 ≥15%，或记录「不达标 + 原因」 |
| M4 | **B3** 立项评审（API 破坏） | 你点头才动；否则永远停在 B1/B2 |
| M5 | 数字与结论写回 `docs/swsml/ref/perf-baseline.md` | 别人能按文档复现 |

> M1–M3 的顺序不是可选的：A1 是算法级（O(键数)→O(1)），B1/B2 是常数级；先把算法级吃掉，再看常数级还剩多少。

### 3.4 A1 的具体设计（最值得先做的一项）

现状（`rust/smlb/src/store.rs`）：

```text
get_with_stats(path)
  ├─ 非分帧: subtree_at → value_from_subtree → parse_strtab(全表)      # 1 趟
  └─ 分帧:   parse_strtab_table(全表) → 导航 → value_from_subtree → parse_strtab(全表)  # 2 趟
```

改法：

1. `Store` 增加 `OnceLock<Box<[&'a str]>>`（或构造期直接解析）；`strtab_body()` 已有，生命周期 `'a` 现成。
2. 新增内部 `value_from_subtree_with_table(table, subtree)`；`value_from_subtree` 保留为「自己解析一次」的便捷入口（公开 API 不变）。
3. `get_with_stats` 两条路径都复用缓存表。
4. 判据：`benches/smlb.rs` 的 `get` 用例 ≥15%；额外加一条「同键取 1000 次」的用例证明 O(1)（现在应是 O(1000×键数)）。

**✅ A1 已落地（2026-11-06，同一个会话里测完）**

实际改法比原设计多一处：`subtree_at` 自己也在每次调用里解析表（所以只缓存 `value_from_subtree` 那侧只能拿一半收益）。

| 改动 | 位置 |
|---|---|
| `Store` 增加 `OnceLock<StrTab<'a>>`，**首次 get 才解析**（不是 `open` 时） | `rust/smlb/src/store.rs` |
| 新增 `subtree_at_with_table` / `value_from_subtree_with_table`，旧函数保留为便捷入口 | `rust/sml-value/src/canonical.rs` |
| `subtree_bytes` / `subtree_vec` / `get_with_stats` 全部复用缓存表 | `rust/smlb/src/store.rs` |

| 指标 | 改前 | 改后 | 变化 |
|---|---|---|---|
| `Store::get(命中)` 4000 键 | 204,158 ns | **13,966 ns** | **−93.2%（14.6×）** |
| 同上，峰值堆（稳态） | 128.1 KiB | **136 B** | 表只建一次，之后零增量 |
| 同上，allocs | 29 | **5** | −24 |
| `MappedFile::open` | 124 B / 1 alloc | 124 B / 1 alloc | 未破坏 T3 的 O(1) 开文件 |
| `MappedFile::store`（= open） | 3.3 KiB / 63 | 3.3 KiB / 63 | 同上 |
| `MappedFile::store+get`（冷启动） | 128.4 KiB / 92 | 128.4 KiB / 80 | 首次 get 仍付一次建表 |

「惰性」是必须的：一版曾把表放在 `open` 里建，`get` 同样快，但 `MappedFile::store` 的峰值从 3.3 KiB 涨到 128.2 KiB —— 那等于拿 T3 的 RSS 判据换速度，已回退为惰性。

验证：`cargo test -p smlb --features "preserve_order,sign,compress,crypto,mmap"` 全绿。

---

### 3.5 内存：现在花在哪（全部实测）

| 现象 | 数字 | 判读 |
|---|---|---|
| `canonical_bytes` on 68.1 KiB 值 | 峰值 **792.0 KiB**，51 allocs | 输入文本只有 68.1 KiB；bench 自注「峰值 ≈ 编码结果本身」⇒ 输出缓冲从 0 翻倍涨上来，至少多付一次 ~2× 搬移 |
| `dump` 同负载 | 峰值 **792.0 KiB**，202 allocs | 同一条编码路径；`to_sml` 反而只有 128.3 KiB（文本路径按行写出） |
| `Store::get` on 4000 键文件（**改前**） | 峰值 **128.1 KiB**，29 allocs | 峰值里主要是 `Vec<&str>` 字符串表：4000 × 16 B = 64 KiB，翻倍到 128 KiB；**每次 get 都重建**。A1 后：136 B / 5 allocs（稳态），详见 §3.4 |
| `load(整树)` 103.9 KiB 文件 | 峰值 454.7 KiB，**8,173 allocs** | 峰值 ≈ 文件的 4.4 倍；分配次数 = 节点数 |
| `load(zstd+分帧)` 18.0 KiB 文件 | 3.55 ms，**8,179 allocs** | 体积压到 1/6，时间和分配**几乎不变** ⇒ 瓶颈是逐节点解码，不是解压 |
| `MappedFile::open` | **124 B**，1 alloc | 零拷贝成立；但 `Store::get` 会把整张 STRTAB 扫一遍 ⇒ 第一次 get 的缺页数 ≈ 全表页数 |

**内存与速度同向的三条**（也是收益最大的三条）：

1. **A1 表缓存**：get 的 128 KiB 峰值是**纯冗余**。缓存后单次 get 峰值增量 ≈ 0（表常驻一次 64 KiB，且可选 offsets 8 B/键降到 32 KiB）。
2. **A2 预分配**：`canonical_bytes`/`dump` 的 792 KiB 峰值 → 目标 ≤ 最终字节数 × 1.2，且少掉最后一次翻倍搬移。
3. **B3 借用 token**：4000 个 `String` 块 = 4000 个分配器头（每块 8~16 B 元数据）+ 散落堆页；借用后 token 字节留在源串里，**内存与局部性同时改善**。

补三条**编译期红线**（现在只有 `Value ≤ 48 B`）：

```rust
const _: () = assert!(size_of::<Value>() <= 48);   // 已有
const _: () = assert!(size_of::<Entry>() <= 64);   // 建议新增：Entry = Box<str> + Value，正好 1 cache line
const _: () = assert!(size_of::<Tok>() <= 32);     // 建议新增：Word/Str(String) 24 + tag
const _: () = assert!(size_of::<Object>() <= 40);  // 建议新增：Vec 24 + OnceLock 16
```

> 断言的用处：这轮会动 `Object` / `Tok` / `Entry`，任何一个涨 8 B，cache line 就从「1 行 1 条」变成「1 条跨 2 行」，解析会静默变慢——断言把它变成编译错误。

### 3.6 缓存命中率：结构判断 + 怎么量

**结构事实（可复核的源码位置）**

| 结构 | 布局 | 对缓存的影响 |
|---|---|---|
| `Value` | ≤48 B（编译期断言，`sml-value/src/value.rs:70`） | 一个 Entry 64 B = **正好一条 cache line**，顺序遍历友好 |
| `Entry` | `Box<str>`(16) + `Value`(48) = 64 B | 键的**字节在另一块堆内存** ⇒ 每次按键取值多一次指针跳转 + 一条 cache line |
| `Object` | `Vec<Entry>` + `OnceLock<Box<Index>>` | 顺序写（parse）友好；索引首次 `get` 才建（9..=4096 排序下标 4 B/键） |
| `Tok` | `Vec<Tok>` 连续；`Word(String)` 24 B + 堆块 | token 数组本身连续，但每个词的字节散落 ⇒ 解析期每词一次随机访问 |
| `Store` | mmap 零拷贝 + 每次 get 扫整张 STRTAB | 首次 get 触达全表页；4000 键 ≈ 64 KiB 表 ⇒ 至少 16 页 |

**能说的三句话（不需要计数器也能成立）**

1. parse 是**顺序写 + 顺序读源串**，硬件预取友好；真正的随机访问来自「键 → 堆上字节」和「token → 堆上字节」。
2. `Value::get` 命中/未命中 64 ns / 74 ns（4000 键，零分配）⇒ 索引层已经没有明显缓存病；再优化它收益很小。
3. `Store::get` 204 µs 里，字符串表扫描是 O(键数) 且**与路径无关**——这是唯一「缓存不友好且完全可消除」的一处。

**怎么量（分两条路，别在本机编数字）**

| 方式 | 指标 | 适用 |
|---|---|---|
| Linux `perf stat`（CI 的 rust job 或任意 Linux 盒子） | `cycles` / `instructions` / `cache-references` / `cache-misses` / `L1-dcache-load-misses` / `LLC-load-misses` / `branch-misses` / `page-faults` | **唯一的真计数器**；Windows 本机没有可靠的 per-process 替代品 |
| 规模曲线（跨平台代理） | 25k / 100k / 400k 键的 **ns/键** 与 **峰值堆/键**；曲线出现拐点即 L2/L3 饱和 | 不需要任何工具，`value-layout.md` 已有这个测法 |

`perf stat` 的用法（每条都要跑 3 次取中位数，别只看一次）：

```bash
perf stat -e cycles,instructions,cache-references,cache-misses,L1-dcache-load-misses,LLC-load-misses,branch-misses \
  ./target/release/deps/perf-<hash> --quick
```

> 明确不做：用 `Get-Counter '\Processor(*)\Cache Faults/sec'` 冒充「命中率」——那是**全系统**口径，噪声比信号大。

### 3.7 SIMD：哪里能真赚，哪里是假的

**现状：`rust/` 里 `std::arch` / `is_x86_feature_detected` / `target_feature` 命中数为 0** —— 没有任何手写 SIMD。

| 候选 | 是否值得 | 理由 | 预期上限 |
|---|---|---|---|
| **词法字节分类**（跳空白 / 找 `{ } [ ] , : @ " #` / 扫裸词 / 扫引号串） | **值得，但有前提** | 结构性字符全是 ASCII ⇒ 一条 `_mm_cmpeq_epi8` + `movemask` 一次判 16/32 字节；UTF-8 续字节 ≥0x80 天然不等于任何结构符，可整段跳过 | 若 tokenize 占 parse 的 30%，向量化 3× ⇒ **整体 −20%**；但**现在基准没测 tokenize 的时间**（只测了它的分配），所以先补一行再谈 |
| 转义扫描（`dump` / `canonical` 找需转义字节） | 有限 | 那两条路径是**分配受限**（792 KiB 峰值 / 12016 allocs），先做预分配收益更大；SIMD 只能省扫描 | −3%~−6% |
| SHA-256（`.smlb` 段摘要 / 签名） | **不用做** | `sha2 0.10` 依赖 `cpufeatures`，已在运行时检测 SHA-NI；自己手写只会更慢 | 0（已加速） |
| **SM3（`sm3.rs`）** | **要做，但分两步；不能在本机验收益** | **ISA 支持是存在的**：ARMv8.2 `FEAT_SM3`（Advanced SIMD SM3 指令）、RISC-V 向量密码 `Zvksh`、x86 的 SM3 扩展（Intel / 兆芯一类，AVX 形态；具体 CPUID 位以实现手册为准）、龙芯可用 LSX 向量化（是否有专用 SM3 指令以架构手册为准）。**但本机是 AMD Ryzen AI 9 H365，没有 SM3 扩展** ⇒ 只能验正确性，验不了加速比 | 标量先做循环展开 + 消息扩展预计算（本机可测，−10%~−20%）；ISA 后端等有硬件的机器再上 |
| zstd 压缩 | 不用做 | 上游已含 SIMD 路径 | 0 |
| 键比较 / `eq_ordered` | 不用做 | `memcmp` 本身已是 SIMD；实测 4000 键 28 µs、0 分配 | 0 |

**SIMD 准入门槛**（四条全过才留）：

1. 目标负载 ≥15% 且**不能只在小文档上好看**（`small` / `large` 都要报）；
2. 输出 bit-for-bit 相同（`tokenize` 已有 `tokenize_matches_tokenize_spans` 对拍测试，再加现行 fuzz 语料）；
3. 必须有标量回退，且**非 x86 平台默认走回退**（不能让 aarch64 编译不过）；
4. `unsafe` 集中在单模块，`cargo miri test` 覆盖；`#[target_feature]` 的函数不得内联进安全代码。

**SM3 单独一条路线**（与词法 SIMD 分开排）：

1. **先标量**：`sm3.rs` 做循环展开 + 消息扩展预计算，用现有标准向量验正确性，本机可测收益。
2. **再分派**：`HashAlgo::Sm3` 入口按目标平台运行时探测，三套后端 + 标量回退：
   - aarch64：`FEAT_SM3`（Advanced SIMD SM3）；
   - x86：SM3 扩展（Intel / 兆芯，AVX 形态）；`is_x86_feature_detected!` 若不支持该特性名，就手写 CPUID；
   - loongarch / riscv：先用标量或 LSX/Zvksh 向量化，按各自手册。
3. **不在这台机器上宣称加速比**：本机无 SM3 扩展，只有标准向量能证明「算得对」。加速比必须在**有该 ISA 的机器**上出数字，否则一律记「待硬件验证」，不进性能基线。
4. 判据与其它项一致：正确性（标准向量 + `check`/`verify` 全绿）优先于速度；无硬件验证不得进正式版性能结论。

**顺序上的结论**：SIMD 是**最后**一项，不是第一项。当前 10.1 allocs/元素、`Store::get` 每次扫全表、`canonical_bytes` 峰值 792 KiB——这三件没清掉之前，向量化省下的那点扫描时间会被分配器和缺页吞掉。

### 3.8 先补三条基准（不改产品代码，只加测量）

1. `parse` 的**峰值堆**（现在只有 allocs/总字节，没有高水位）——`benches/common` 已有 `measure()`，接上即可。
2. `tokenize` 的**时间**（现在只有分配行）——决定 3.7 里 SIMD 的上限。
3. `Store::get` 的**同键 1000 次**（现在只有单次）——证明 A1 的 O(1) 而不是「碰巧快」。

---

## 4. 优先级（把内存/缓存/SIMD 一起排）

| 序 | 项 | 主要收益维度 | 为什么排这里 |
|---|---|---|---|
| 1 | ~~**A1** STRTAB 表缓存~~ ✅ **已落地** | 实测速度 **−93.2%**、稳态峰值 128 KiB → 136 B、去 O(键数) | 见 §3.4；`open` 仍 O(1)（惰性建表） |
| 2 | **A2** canonical/dump 预分配 | 峰值 ≤ 1.2× 输出、速度 −15%~−30% | 792 KiB 高水位是白送的浪费 |
| 3 | **B1+B2** 输出预分配 + 键所有权 | 速度 −5%~−15%、每键 −1 alloc | 低风险、可与 A2 同批 |
| 4 | **A3** dump 写出优化 | 吞吐 +15%~+25% | 与 A2 同一条路径，一起测 |
| 5 | **B3** 借用 token | 分配 −28%、**缓存局部性**最大改善 | API 破坏，需你点头 |
| 6 | **B4/B5** 入口单趟 / 小容量 | small −5%~−10% | 常数级 |
| 7 | **SIMD 词法** | 预期 −10%~−20% | 前面不清，收益会被吞；且要先补 tokenize 计时 |

---

## 5. 要你拍的 4 个决定

1. ~~格式名走 A 档还是 B 档~~ → **已定 B 档（2026-11-06）**：`amberice` + `.ambr` + `AMBR`。
   备选 `.amberice` 仍可一句话改；`.amb` / `.ambe` / `.amber` 已排除（撞 Ambisonic / AMBE 声码器 / 通用词）。
2. **读端是否收旧后缀**：A 档无需；B 档是否让 CLI 仍接受 `.smlb` 一段时间（建议**只接受、不写出**，并在读到时打一条 note）。
3. **`Tok` 借用化（B3）**：允许 `sml-lex` / `sml-parse` 在 `0.7.0` 做破坏性 API 变更吗？→ 不允许就停在 B1/B2。
4. **对外身份**：GitHub org 用 `amberice-lang`（404，可用）？域名 `amberice.dev`（未解析）还是 `.rs`？→ 都要在发布前占。

## 6. 复核命令

```bash
# 占用
curl -s -o /dev/null -w "%{http_code}" https://crates.io/api/v1/crates/amberice   # 404
curl -s -o /dev/null -w "%{http_code}" https://pypi.org/pypi/amberice/json         # 404
curl -s -o /dev/null -w "%{http_code}" https://registry.npmjs.org/amberice        # 404
curl -s -o /dev/null -w "%{http_code}" https://github.com/amberice                # 200（已占）
curl -s -o /dev/null -w "%{http_code}" https://github.com/amberice-lang           # 404

# 改名影响面
git grep -l smlb | wc -l       # 59 个文件
git grep -n smlb | wc -l       # 1002 行提到
git grep -o "smlb::" | wc -l   # 415 处路径引用

# 性能
cd rust && cargo bench --bench perf        # 正式口径，别加 --quick
cargo bench --bench value
cargo bench --bench smlb --features "smlb-compress,smlb-crypto,smlb-mmap"
```

---

## 7. ✅ 已收录进 v1：`amberice` 的「bincode 式」typed 载荷（2026-11-06 用户提出并裁决）

> **2026-11-06 用户裁决「v1 带吧」** ⇒ 本节内容已成正式规范：
> `docs/amberice/spec/format.md §25`（`0x70 TYPED` / `0x71 SCHEMA`，含「`VALUE` 必写」等四条收录前提）。
> 本节保留作设计讨论与取舍记录；**以 §25 为准**，两处冲突时按 §25 执行。
> 实现状态：Rust 侧已落（`rust/amberice/src/typed.rs`，`Writer::put_var` / `Store::var`）；五端「跳过」侧 N03 未做。

### 7.1 先把「方便」拆开：你要的可能是两件不同的事

| 说法 | 现状 | 是否要新增 |
|---|---|---|
| 「变量名 → 值」的表（按 id 取一个变量） | `VALUE` + `STRTAB` 已经是这个（SML 本身就是「带名字的树」） | 不要；只是现在取变量要写路径 |
| 「变量 + 它的 id」，字段不带名字、按数字 id 编 | **没有**。现在存 Rust 值必须先转 `Value`，每个键都进 `STRTAB` | **要**，这就是 bincode 的位置 |

真正的新增是**强类型载荷**：`var_id` + 字段 id 序列 + schema。名字只出现在 schema 里一次，载荷里全是数字。

### 7.2 形态：两个新段；**schema 默认不写进文件**

段 id 现状（`rust/amberice/src/container.rs`）：`0x01 VALUE` / `0x02 STRTAB` / `0x03 DIR` / `0x04 MANIFEST` / `0x10 FRAMES` / `0x20 EMBED` / `0x30 SIG` / `0x40 PROFILE` / `0x50 SOURCE` / `0x60 PROVENANCE` / `0x70 TYPED` / `0x71 SCHEMA`（后两段 = spec §25）。
**未知段可跳过**是既有规则（表项自带 `len`，`load` 只认自己需要的 `seg_id`）⇒ 加段不需要动版本位。

- `0x70 TYPED`：段头是 `count` + `count × (var_id, off, len)` 的**目录**（按 `var_id` 升序 ⇒ 二分 O(log n)），后面跟载荷字节。**目录在段内**，所以 `var(id)` 不依赖 schema。
- `0x71 SCHEMA`：**可选**。只放「给人/给别的语言看」的东西：`type_id → 字段/变体 id → 名字与类型`。不写它也能读，只是读端必须编译期知道类型。

| 模式 | 文件里有什么 | 谁需要类型信息 | 适用 |
|---|---|---|---|
| **外置 schema（默认）** | 只有 `TYPED` | 调用方编译期知道 `T` | 热路径、内部存储、体积最小 |
| **内嵌 schema（可选）** | `TYPED` + `SCHEMA` | 读端动态读 | 工具 `cat`、跨语言、长期归档 |

**id 由使用者给**：`VarId(u32)`，`0` 保留（表示「未指定」）；写入时重复 `var_id` 直接报错；**不自动分配**、不猜测。

### 7.3 线格式（零依赖、可手写）

| 项 | 写法 | 理由 |
|---|---|---|
| 无符号整数 | LEB128 varint（复用 canonical 的 varint） | 小数字占 1 字节 |
| 有符号整数 | zigzag → varint | 负数不膨胀 |
| `f32` / `f64` | 固定 8/4 字节小端 | 二进制里不需要「十进制展开」；那套只服务 canonical 文本 |
| 字符串 / 字节 | varint 长度 + 原始字节 | 不做 UTF-8 二次校验（`&str` 已保证） |
| 数组 | varint 元素数 + 元素 | 同样带预算 |
| `Option` | 1 字节 tag（0/1） | 可空字段仍要区分「无」与「零值」 |
| 枚举 | varint 变体 id + 载荷 | 「变量 + 其 id」的核心 |
| 结构体 | 按字段 id 升序**全量写** | **类型不变** ⇒ 不需要 presence bitset / 缺省字段跳过，解码路径最短 |

**硬约束**：深度上限、总字节上限、元素数上限（与 `MAX_DECOMPRESSED` 同族），防「声明 4G 元素」的炸弹；解码全程 `safe`，不写 `unsafe`。

### 7.4 「变量类型不变」是一条契约，不是一句提醒

它的直接后果（决定了上面为什么能去掉 presence bitset）：

1. **写入时记 `type_id`**：类型路径 + 字段 id 表 + 变体 id 表的稳定指纹（不是 `type_name` 字符串本身，改名不该改指纹——若改名也要变，就明说，别让它悄悄漂）。
2. **读出时比对**：`type_id` 不等 ⇒ `TypedError::TypeMismatch`，**不猜、不升级、不强转**；错误里带上「文件里是什么 / 代码里要什么」。
3. **v1 不做 schema 演进**：加字段、改字段类型、改变体顺序，都算**新类型**（要么新 `type_id`，要么新 `var_id`）。想演进就显式换 id——这样旧数据不会因为「decode 尽量兼容」而读出错值。
4. 因此 `Option<T>` 的「有没有」是**值语义**，不是「字段缺不缺」；两件事不混。

### 7.5 API 草案

```rust
pub trait Encode { fn encode(&self, e: &mut Encoder<'_>); }
pub trait Decode: Sized { fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError>; }

// 默认：类型在编译期已知，schema 不落盘
pub fn put_var<T: Encode>(w: &mut Writer, id: VarId, v: &T) -> Result<(), TypedError>;
pub fn get_var<T: Decode>(s: &Store, id: VarId) -> Result<T, TypedError>;

// 可选：把 schema 一起写进去（工具 / 跨语言才需要）
pub fn put_var_documented<T: Encode>(w: &mut Writer, id: VarId, v: &T) -> Result<(), TypedError>;
pub fn schema_of<T: Encode>() -> Schema;

impl<'a> Store<'a> {
    pub fn var(&self, id: VarId) -> Result<&'a [u8], ContainerError>;  // 零拷贝，O(log n)
}
```

可选 `serde` 桥（feature `typed-serde`），但核心不依赖 serde；也不引入 `bincode` 本体——只要「数字 id + varint」这套思路。

**再强一档：用类型化句柄把「取错类型」变成编译错误（未落地）**

当前实现：`VarId` + `Store::var<T: Decode>(&self, id) -> T` / `Writer::put_var`，
类型由 `Decode` 约束在取值点校验。下面这版把类型绑进句柄，属于后续增强，不是现有签名：

```rust
pub struct VarKey<T> { id: VarId, _t: PhantomData<fn() -> T> }
impl<T> VarKey<T> { pub const fn new(id: u32) -> Self { … } }

// 取的时候只能拿同一个 T：VarKey<Config> 不能传给 get::<Other>
pub fn get<T: Decode>(s: &Store, k: VarKey<T>) -> Result<T, TypedError>;
```

`VarKey` 是零尺寸的编译期凭证，`id` 还是你给；区别是「id 与类型绑定」这件事由 API 保证，不靠文档提醒。

**哪些限制能落在 API 上，哪些只能落在运行期**

| 限制 | 手段 | 报错时机 |
|---|---|---|
| 只有无指针类型能走 raw | sealed `Plain` + `put_pod<T: Plain>` | 编译期 |
| 不能取错类型 | `VarKey<T>` 句柄 | 编译期 |
| 借用解码（`&str` 原地读） | 单独 `DecodeBorrowed<'de>`；不实现就只能 owned 解码 | 编译期 |
| 无 padding | derive 生成 `const` 断言 | 编译期 |
| 大端不支持 Pod | 函数只在 `#[cfg(target_endian = "little")]` 下存在 | 编译期（函数不存在） |
| 文件里的类型与代码不符 | `type_id + layout_hash` 校验 | **运行期（必须有）** |
| `Arc` 共享身份 | 不实现 `Encode for Arc`，或做引用表 | 设计选择 |
| 不可信输入 | 长度 / 深度 / 元素数预算 | 运行期 |

### 7.6 与现有机制的关系（不写清就会分叉）

| 机制 | 关系 |
|---|---|
| canonical `Value` | **仍是唯一**「跨端相等 / 签名语义」的载体；`TYPED` 不参与 canonical 相等 |
| 签名 | 段摘要覆盖 `TYPED` / `SCHEMA` 字节；`verify` 逻辑不变 |
| 压缩 / 加密 / mmap | 完全复用段级机制；`var(id)` 只解命中段 |
| 随机访问 | `VALUE` 靠 `subtree_sz`；`TYPED` 靠 `SCHEMA` 的 offset 表 ⇒ 更直接 |
| 版本 | `schema_id + schema_hash` 进 manifest；不匹配**明确报错**，不猜 |
| 五端 | v1 **只做 Rust**，标 `experimental`；跨端同步单列，不塞进 T7 清单 |

### 7.7 收益 / 代价（全部待测，先出数字再决定）

- 收益：数字密集结构体积预计 −30%~−60%，解码快 2~5×；**改字段名不破坏数据**（id 稳定，名字只在 schema 里）。
- 代价：多一套编解码器（估 600~900 行 + 测试）；没有 schema 就读不了（`cat` 需要 `--schema`）；跨语言是**第二份欠账**。
- 不适合：字符串/散文为主的数据——键名本来就不多，收益会被 schema 开销吃掉。

### 7.8 建议落地顺序

1. `rust/amberice/src/typed.rs`（模块常开，无独立 feature），**只做外置 schema**；先手写 3 个类型（struct / enum / Vec）跑通。
2. 加 `0x70 TYPED` 段 + `Store::var`；确认 `check` / `sign` / `compress` 自动覆盖。
3. 对照 bench：同一份 4000 字段数据，`Value` vs `typed` 的字节数与时间，出表后再决定要不要内嵌 schema。
4. 内嵌 schema 与 CLI `--schema` 放第二阶段。

### 7.9 「直接存内存 dump」：默认不能；只有显式声明 `Pod` 才行

**默认编码是逐字段，不是 `memcpy`。** 直接把变量那块内存写出去，四个硬原因：

1. **padding 是未初始化的**：`#[repr(C)]` 也只保证顺序，不保证没洞；把 padding 写进文件等于泄漏栈/堆里的旧字节，读回来还是 UB。
2. **指针无意义**：`String` / `Vec` / `Box` / `&str` 存的是本进程的堆地址，换个进程就是野指针。
3. **布局是编译器私有**：`repr(Rust)` 的字段顺序、enum tag、niche 优化都不稳定；升级 rustc 就可能变。
4. **跨平台不一致**：端序、`usize` 宽度、对齐、`bool`/`char` 的非法位型。

bincode 也不是内存 dump —— 它同样逐字段写。真正「按归档布局」的是 `rkyv`，那仍然不是原样内存。

**能做的三种形态**（按安全性排）：

| 形态 | 是什么 | 适用 | 代价 |
|---|---|---|---|
| **默认 typed** | 字段逐个编码 | 常规结构体 / 枚举 | 无 |
| **`Pod` 原始段** | 只允许「无指针、无 padding、全字节已初始化」的类型，字节原样写 | `f64` 数组、张量、定长记录 | 需 `unsafe impl Pod`；记录 `size/align/endian/layout_hash` |
| **opaque blob** | 任意字节 + `type_id`，只存不解 | 调试快照、缓存 | 不可跨版本/跨平台解码，只能原样取回 |

**`Pod` 的判据**（不靠感觉）：

- 类型必须是 `Copy` + 无指针 + 无 `Drop`；
- 用 `offset_of!` 链检查「字段偏移之和 == `size_of`」⇒ 没有 padding；有洞就显式加 `_pad: [u8; N]`；
- 只允许白名单原始类型（整数 / 浮点 / `[T; N]` / `#[repr(C)]` 组合），`bool` / `char` / enum **不进 Pod**（有非法位型）；
- 写入记 `endian`，读出必须同端序；跨端序明确拒绝，不自动转。

**解码校验**：`type_id + size + align + endian + layout_hash` 五项任一不符 ⇒ 报错，**绝不**把一段字节直接 `transmute` 成结构体。

**「内存 dump」若指调试快照**：走 opaque blob，能存能取，但不要在文档里暗示「可读」或「可跨版本」；真要可读，就退回默认 typed。

### 7.10 编译期判「有没有指针」：stable 上能做到，且不用 UB

**能做，但不是靠 nightly 的 auto trait，而是靠「sealed trait + derive 白名单 + const 布局断言」。**

> 先说清楚：`Plain` **只服务 `Pod` 快路径**。普通结构体只需要 `#[derive(Encode, Decode)]`，字段里可以是 `String` / `Vec` / `bool` / enum —— 一行代码，零限制；`Plain` 是「我要大数组零拷贝」时才加的第二个 derive。

```rust
// 库内：trait 是 sealed 的，用户不能自己 unsafe impl
mod sealed { pub trait Sealed {} }
pub trait Plain: sealed::Sealed + Copy + 'static {}

#[derive(Plain)]
#[repr(C)]
struct Point { x: f32, y: f32 }
```

derive 展开三件事（全部在编译期）：

1. **字段 bound**：`where f32: Plain, f32: Plain` —— 任何一个字段不是 `Plain`（`String` / `Vec` / `&str` / `Box` / 裸指针 / `bool` / `char` / enum）就**当场编译失败**，错误信息是「`String` 没有实现 `Plain`」，不是运行期才发现。这就是「自动判断是不是指针」。
2. **const 布局断言**：用 `offset_of!` 算相邻字段间距，判「无 padding」：
   ```rust
   const _: () = {
       assert!(size_of::<Point>() == 8);
       assert!(offset_of!(Point, x) == 0);
       assert!(offset_of!(Point, y) == 4);   // 4 == 0 + size_of::<f32>()
   };
   ```
   末字段 `offset + size == size_of::<T>()` ⇒ 尾巴也没有洞。
3. **记录 `layout_hash`**：`size / align / 每个字段的 offset / 端序`，进文件头或 `SCHEMA`。

白名单只有：整数、浮点、`[T; N]`、`#[repr(C)]` 组合、`()`。**不含** `bool` / `char` / enum（有非法位型）、不含任何带生命周期的东西。

**但「没指针」≠「能 `transmute` 读回」**，还有四件事：

| 风险 | 处置 |
|---|---|
| padding | 上面 const 断言直接禁掉；要洞就显式 `_pad: [u8; N]`，且必须真的初始化 |
| 端序 | 写入记 `endian`，读出不同端序**拒绝**，不自动转 |
| 对齐 | 从 `&[u8]` 直接读需要对齐；不对齐就 `read_unaligned` 或逐字段拷贝 |
| 非法位型 | 白名单已经排除 `bool` / `char` / enum |

所以两条路，别混：

- **默认**：逐字段读写（写端也不能整块 `memcpy` —— 读未初始化 padding 本身就是 UB）；
- **`Pod` 快路径**：只有「`Plain` + 无 padding + 端序固定 + 对齐满足」才允许按原始字节读，且读写都走 `MaybeUninit` + 校验，不做 `transmute` 到任意类型。

**「保留类型 + 数据」的收益**（这是那个「意想不到的效果」）：

- 读之前先比 `type_id + layout_hash + size`：类型不符**直接报错**，而不是按错布局硬解（那才是不 UB 的关键）。
- `layout_hash` 还能拿来生成 C 头文件 / 跨语言解码器 / 文档（与 T42 的 `smltools docs` 同一条思路）—— 一份类型定义，三处自动产出。
- 调试时能直接说「这是 `Point`，8 字节，两个 f32」，而不是一段十六进制。

### 7.11 指针限制到底限制了什么（体感）

**默认路径没有指针限制**。`Encode/Decode` 干的事就是「跟着指针走、存指向的数据」——这本来就是序列化：

```rust
#[derive(Encode, Decode)]
struct Doc {
    title: String,        // 存 "…" 这些字节，不存堆地址
    tags: Vec<String>,    // 存元素，不存 (ptr, len, cap)
    body: Box<str>,       // 同上
    next: Option<Box<Doc>>, // 递归树可以；这是所有权链，不是自引用
}
```

| 类型 | 默认 typed | 存的是什么 | Pod 快路径 |
|---|---|---|---|
| `String` / `Vec<T>` / `Box<T>` | ✅ | 指向的数据（解码后重新分配） | ❌ |
| `&str` / `&'a T` | ✅ | 指向的数据（解码成 owned） | ❌ |
| `Option<Box<T>>` / 递归树 | ✅ | 整棵树 | ❌ |
| `Arc<T>` / `Rc<T>` | ✅（需 impl） | 内容；**共享身份会丢**（解成两份独立数据） | ❌ |
| `*const T` / `*mut T` | ❌ | 地址无意义 | ❌ |
| 真自引用（`&self` 指自己） | ❌ | 需要改成索引/偏移 | ❌ |
| `Box<dyn Trait>` | ❌（需手工映射成 enum） | 类型擦除后无法还原 | ❌ |

**体感上只会遇到三件事**：

1. **Pod 快路径不能用 `String`**：想给大数组加速时才碰到；解法是「头用默认 typed、数组用 Pod」，或改定长 buffer。默认路径完全不受影响。
2. **共享身份丢失**：同一份 `Arc<T>` 在两处被编码两次、解码成两份。数据正确、指针身份不在——要保留共享得另做「引用 id 表」（v1 不做）。
3. **自引用/裸指针要手工建模**：图结构用 `Vec<usize>` 索引，不能存地址。这是所有序列化格式的共同边界，不是本设计特有的。

所以「指针限制」限制的是**地址身份**，不是**指向的数据**。日常 Rust 类型（String/Vec/Option/Box/enum/嵌套结构）在默认路径上零感知。

### 7.12 已定 / 待定

**已定（2026-11-06 用户）**

| 项 | 结论 | 落到设计里的位置 |
|---|---|---|
| schema 位置 | **可选；默认不落盘**（编译期已知类型） | §7.2 默认只写 `TYPED`；要跨语言/工具才加 `SCHEMA` |
| 变量 id | **使用者给**，不自动分配 | `VarId(u32)`，0 保留；重复即报错；目录按 id 升序 |
| 类型 | **不变**：读出必须同类型，不兼容、不强转 | §7.4 的 `type_id` 校验 + `TypeMismatch`；v1 无 schema 演进 |

**待定**

1. `type_id` 指纹要不要包含**类型名**（含 ⇒ 改名即失效；不含 ⇒ 两个同构类型会撞，需要命名空间）。建议：包含「模块路径 + 类型名」，并把「改名 = 新 id」写进文档。
2. 是否提供 `serde` 桥（`typed-serde` feature），还是只做自研 trait（建议：自研为主）。
3. 错误怎么报：容器层现在用 `ContainerError` 枚举、**不占码表**；typed 层建议同样先做枚举，等要对外发布再按码表纪律加域加码。
4. 是否加 `#[derive(Plain)]`（放 `swsml-derive`，零新依赖）：它是 §7.10 的入口。建议加，但**只生成布局断言与 marker**，不生成 `unsafe` 的 transmute。
