# 全面优化计划（2026-11-06）

> 对象：**`swsml`**（解析 / 值 / 发射）+ **`amberice`**（原 `smlb` 容器）。
> 目标：时间、内存、缓存行为、指令集四条线一起排；**不改语义、不加运行时依赖**。
> 与 `docs/amberice-plan.md` 的分工：那份管**改名 + 第一批（A1）**，这份管**全面优化**。

---

## 0. 判定标准（先立尺，再动手）

| 维度 | 指标 | 工具 / 口径 | 硬门槛 |
|---|---|---|---|
| 时间 | ns/op、MB/s | `cargo bench --bench perf|value|smlb`（**不带 `--quick`**，5 轮取最小） | 目标负载 **≥15%** |
| 分配 | allocs/op | `benches/common` 的全局计数分配器 | 单项 **≥15%** 或少 N 次/键 |
| 峰值堆 | peak B/op | `common::measure`（高水位，不是总量） | **≥15%** 或降到 ≤1.2× 输出 |
| 常驻内存 | RSS 增量 | `MappedFile` 判据（100 MB → 256 B 那套） | 不许退化 |
| 缓存 | cache-misses / LLC-load-misses / IPC | Linux `perf stat`（Windows 无可靠 per-process 计数） | 看趋势，不作硬门槛 |
| 体积 | 二进制 / 产物 | `llvm-size` / `strip` 前后 | **≥5%** 才留 |
| 启动 | CLI `--version` / `--help` 时间 | 手写计时（不引 hyperfine） | 可选 |

**三条铁律**：① 不改键序 / 错误码 / canonical 字节 / 段布局；② 每项独立提交、可单独回滚；③ 数字进 `docs/perf-baseline.md`，无数字的「优化」不算数。

---

## 1. 现状账（已测，`--quick`，同机 2026-11-06）

| 层 | 负载 | 操作 | ns/op | 峰值堆 | allocs |
|---|---|---|---|---|---|
| 解析 | `small` 205 B | parse | 8,528 | — | 101（10.1/元素） |
| 解析 | `large` 909 KiB | parse | 6,780,440 | — | 27,868 |
| 解析 | `large@directive` | parse | 9,873,580 | — | — |
| 解析 | `deep` 120 层 | parse | 66,702 | — | 881 |
| 词法 | 10 键 | tokenize | **未计时** | — | 28（2.8/键） |
| 发射 | `large` | to_sml | 3,283,550 | — | — |
| 发射 | `flat@4000` | to_sml | 1,102,555 | 128.3 KiB | 12,016 |
| 发射 | `flat@4000` | canonical_bytes | 1,447,155 | **792.0 KiB** | 51 |
| 容器 | `flat@4000` | dump | 2,376,413 | 792.0 KiB | 202 |
| 容器 | `flat@4000` | load（整树） | 3,344,453 | 454.7 KiB | 8,173 |
| 容器 | `flat@4000` | Store::get（A1 后） | **13,966** | 136 B（稳态） | 5 |
| 容器 | 103.9 KiB | MappedFile::open | — | 124 B | 1 |

归因（已成立的三条）：

1. **每键 ≈6.9 次分配**（词法 2.8 + 解析 4.1）；`large` 一次 parse 27,868 次。
2. **编码缓冲从 0 翻倍**：`canonical_bytes`/`dump` 峰值 792 KiB，是白付的高水位。
3. **容器层瓶颈是逐节点解码**：zstd 把 103.9 KiB 压到 18.0 KiB，`load` 时间与分配几乎不变。

---

## 1.1 阶段 0 已落地（2026-11-06）

新增 `benches/profile.rs`（**不改 `perf.rs`**，历史基线保持可比）与 `tools/perf_counters.sh`。

```bash
cargo bench --bench profile --features amberice -- --quick
```

| 负载 | 操作 | ns/op | 峰值堆 | allocs |
|---|---|---|---|---|
| `small` 205 B | parse | 17,970 | 6.4 KiB | 101 |
| `flat@4000` 68.1 KiB | parse | 11,512,370 | **1.4 MiB** | 32,562 |
| `deep@120` 28.8 KiB | parse | 549,359 | 150.6 KiB | 873 |
| `small` | tokenize | 5,087 | 3.2 KiB | 33 |
| `flat@4000` | tokenize | 3,158,952 | 861.0 KiB | 11,926 |
| `deep@120` | tokenize | 277,486 | 24.9 KiB | 136 |
| `flat@1000` | get×1000 均摊（中键） | 8,266 | 120 B | 4,000 |
| `flat@4000` | get×1000 均摊（中键） | 28,771 | 136 B | 5,000 |
| `flat@16000` | get×1000 均摊（中键） | 125,366 | 136 B | 5,000 |

**三条判读（直接改了排期依据）**

1. **tokenize 占 parse 约 27–28%**（`small` 5,087/17,970；`flat@4000` 3.16 ms/11.5 ms）⇒ P6/SIMD 词法的整体收益上限确实在 −10%~−20%，不再是估算。
2. **parse 峰值堆** `flat@4000` 1.4 MiB / 32,562 allocs ⇒ P1（键所有权）、P2（小容量）、V1（interning）的收益可用这一行验，不必再猜。
3. **`get` 的 O(键数) 已去掉，剩 O(兄弟数)**：三种规模下 **allocs 恒 4–5/get、峰值恒 120–136 B**，但中键耗时 8.3 µs → 28.8 µs → 125.4 µs 近似线性。前者证明 A1 的表缓存生效；后者是 `subtree_at` 的兄弟比较（已知取舍），归 **A5 `DIR` 段**，不要当成 A1 没做完。

> ⚠️ `--features amberice` 会连带打开 `preserve_order`，本表 parse 是**保序档**；默认档数字看 `benches/perf.rs`，两者不可直接相减。

## 1.2 阶段 1 进度（2026-11-06）

| 项 | 状态 | 实测（确定性指标：allocs / 峰值堆） |
|---|---|---|
| **E1** `to_sml` 预分配 + `quote_if_needed` 改 `Cow<str>` | ✅ `63f52a8` | `flat@4000 to_sml` allocs **12,016 → 2**；`canonical_text` **8,015 → 15**；`small to_sml` 66 → 38；峰值 128.3 → 125.3 KiB |
| **E2** canonical/STRTAB 精确预分配 | ✅ 部分 `5ef6f53` | `flat@4000 canonical_bytes` allocs **51 → 38（−25%）**；`small` 15 → 12；`deep@120` 25 → 20；峰值仍 792 KiB |
| **P1** 字段写入接管键所有权 | ✅ `75febff` | parse allocs `small` 101 → 90、`flat@4000` **32,562 → 28,562（正好每键 −1）**、`deep@120` 873 → 753；同机时间 −12%~−15%；峰值不变 |
| **E2 续**：两趟编码（阶段 2 提前做） | ✅ `8b9cb28` | `canonical_bytes` 峰值 **792.0 → 728.0 KiB（−8%）**、allocs 38 → 28；`deep@120` 12.4 → 11.4 KiB / 20 → 16 |
| **P2** `Object::with_capacity(4)` | ❌ 不做 | 分析即可判死：`flat@4000` 只有一个对象，`Vec` 仍要长到 4096，预置 4 只省 2 次分配；`deep@120` 有 120 个单键对象，预置 4 会让每块多占 3×64 B，却仍是 1 次分配 ⇒ 纯内存回归 |
| **② 紧凑 interning 表** | ✅ `4b2cadb` | `StrTab` 的 `HashMap<&str,u32>` → 自写开放寻址（槽只存 `u32` 下标，命中回查真串防碰撞）。`canonical_bytes` 峰值 **728.0 → 239.0 KiB（−67%）**、时间 1.64 ms → 0.76 ms（−54%）、allocs 28 → 26；`deep@120` 11.4 → 3.9 KiB。相对最初基线：792.0 → 239.0 KiB（**−70%**）、51 → 26 allocs |
| **① P3 入口快路径** | ✅ `48a8e55` | `strip_version` / `strip_features` 加保守判据（连 `version` / `feature` 一词都没有 ⇒ 跳过逐行重建）。同机连续两次：`large@directive` **−19.8%**、`deep` −17.4%、`large` −12.9%、`small` 无变化（本就走 `no_directives` 快路径） |
| **③ P5 借用 token（A 方案）** | ✅ 已落地 | 用户拍 **A（破 API）**：`Tok<'a>` + `Word/Str(Cow<'a, str>)`；词法器改为「连续切片零分配、转义/注释穿插才 Owned」。同机连续两次测：`flat@4000 tokenize` **11,926 → 26 allocs（−99.8%）**、时间 −60%；`parse` **28,562 → 8,662 allocs（−70%）**、时间 −37%；`perf large parse` −39%、`large@directive` −33%。详见 §1.3 |

## 1.3 ③ A 方案：生命周期扩散与退路

**做了什么**

- `sml-lex`：`Tok<'a>`，`Word`/`Str` 用 `Cow<'a, str>`；词法器只记词起点，连续切片直接 `Cow::Borrowed`，只有引号串含转义、或注释插进词中间（token 不再是原文连续切片）才 `Cow::Owned`。**这两个例外是必须的**，否则转义解码和跨注释的词会静默改数据。
- 新增 `TokOwned = Tok<'static>` 与 `Tok::into_owned()`，并在 `sml-parse` / `swsml` 两处再导出 —— 这是 `Tok<'a>` 借用版的退路：`tokenize(t)?.into_iter().map(Tok::into_owned)` 一行回到可独立存活的 token 流。
- `sml-parse`：`Parser<'a>` 的 `toks: Vec<Tok<'a>>`；但**显式栈帧不带生命周期** —— `BlockFrame.closing` 改成 `TokOwned`（它本来就只是 `RBrace`/`RBrack` 标记）。若让 `Frame<'a>` 带上 `'a`，`&mut Vec<Frame<'a>>` 的不变性会把整个 `&mut self` 借用撑到 `'a`（`run_stack` 里 16 处 E0503/E0499）。这是本次最大的一处结构性调整。
- `sml-include`：主文本 token 借用原文；子文件 `read_to_string` / 清洗后的 `cleaned` 是临时 `String`，其 token 经 `into_owned()` 脱离后再并入输出流。

**为什么比「纯 `&'a str` 的 A」好用**：`Cow` 自己处理转义与 include，用户不必手工造 owned；`TokOwned` + `into_owned()` 给了明确的脱离路径。仍是 `0.7.0` 破 API（`Vec<Tok<'a>>` 不能 `'static`、不能脱离源文本返回），但上一轮列的「难用」点里，转义和 include 两类已由库消化。

**验证**：`cargo check --workspace --all-targets` 绿；`cargo test --workspace --exclude smlkit-easy` 109 个 test binary 全绿；`smlkit-easy` / `sml-parse --all-features` / `sml-lex --all-features` 全绿；`amberice vectors --check` 21 例逐字节一致。

> ⚠️ 时间数字均为 `--quick` 同机连续两次对照（A 前 / A 后），正式口径待重跑后写回 `docs/perf-baseline.md`。`deep@120 tokenize` 时间 +10.5%（178.8 → 197.7 µs）但 allocs −88%，判为噪声/小幅回归，后续跟进。

**峰值为什么能降到 239 KiB**：两趟编码干掉 `nodes` Vec 翻倍，紧凑表再把 `HashMap` 每桶 24 B 压成 4 B 的 `u32` 槽；id 分配顺序（首次出现序）未变 ⇒ `vectors --check` 21 例逐字节一致。

> ⚠️ 本轮早段**时间**列受整机噪声污染（同期未改动的 `perf` 基准 `small parse` 从 8,528 ns 变成 19,517 ns，约 2.3×）；P1 的前后对照是在噪声消退后的**同机连续两次**测的（8,898 → 7,530 ns）。其余时间数字仍建议重跑正式口径再写回 `docs/perf-baseline.md`。

---

## 2. 优化目录（按层，编号即排期单位）

### 2.1 词法 / 解析（`sml-lex` / `sml-parse`）

| 编号 | 手法 | 预期（待测） | 风险 | 依赖 |
|---|---|---|---|---|
| **P1** | **键所有权**：`insert_or_merge` 增 owned 变体，键 `String` 直接进 `Box<str>` | 每键 −1 alloc，时间 −3%~−8% | 低-中（容量差可能 realloc） | 无 |
| **P2** | **`Object` 小容量起步**（`with_capacity(4)`） | 每块 −1~2 alloc，−2%~−5% | 低-中（小文档内存↑） | 无 |
| **P3** | **入口单趟扫描**：BOM / 版本剥离 / 首个 `@` 合并 | `small` −5%~−10% | 中（`scan` 是唯一漏斗） | 无 |
| **P4** | **词法字符分类表**（256 B 查表替 `is_whitespace`/`match`） | tokenize −5%~−10% | 低 | 无 |
| **P5** | **`Tok::Word(String)` → 借用 `&'a str`** | allocs −28%，时间 −10%~−25%，**局部性最好** | **高**：公开枚举，68 处使用点 + 生命周期扩散 | 需 API 决策 |
| **P6** | **SIMD 词法**（字节分类 / 跳空白 / 找结构符） | 若 tokenize 占 30%，整体 −10%~−20% | 高：unsafe + 平台回退 | P4 + 先补 tokenize 计时 |

### 2.2 值表示（`sml-value`）

| 编号 | 手法 | 预期 | 风险 | 依赖 |
|---|---|---|---|---|
| **V1** | **键 interning**（解析期全局串表，重复键共享） | 重复键多的文档 −5%~−15% | 中高：跨 crate、与 `.ambr` 串表同源 | 设计决策 |
| **V2** | **inline 小 map**（0/1 键对象不单独分配） | 每块 −1 alloc，−3%~−6% | 中：`Value` 48 B 红线 | V1 之后 |
| **V3** | **尺寸红线断言**：`Entry ≤ 64`、`Tok ≤ 32`、`Object ≤ 40` | 防回归（不是提速） | 低 | 无 |
| **V4** | `get` / `eq_ordered` 已是 0 分配（4000 键 64/74 ns、28 µs） | 不动 | — | — |

### 2.3 发射（`to_sml` / `canonical` / 六个后端）

| 编号 | 手法 | 预期 | 风险 | 依赖 |
|---|---|---|---|---|
| **E1** | `to_sml` 输出 `with_capacity` + 手写整数写出 | 时间 −5%~−15% | 低 | 无 |
| **E2** | `canonical_bytes` / `dump` **预分配**（按值规模估） | 峰值 792 KiB → ≤1.2× 输出；时间 −15%~−30% | 低 | 无 |
| **E3** | markdown / html / latex 输出预分配 | 长文档 −5%~−15% | 低 | 长文档基准（新负载） |
| **E4** | 浮点格式化共用一份 `format_float`（已有，防两套） | 防漂移 | 低 | 无 |

### 2.4 容器（`amberice`）

| 编号 | 手法 | 预期 | 风险 | 依赖 |
|---|---|---|---|---|
| **A1** | STRTAB 惰性缓存 ✅ **已落地** | get −93.2%，稳态 128 KiB → 136 B | — | — |
| **A2** | `load` 逐节点分配（8,173 → 目标 ≤5,000） | 时间 −15%~−25% | 中高：值模型/arena | V1 之后 |
| **A3** | `dump` 编码缓冲复用（同 E2） | 与 E2 同批 | 低 | E2 |
| **A4** | `Writer` 缓冲与 syscall 次数（40 MB/s） | 待测 | 低 | 无 |
| **A5** | mmap 首触页 / `DIR` 段（get 不必扫全表） | 冷启动页数 O(键数)→O(log n) | 中：**格式项** | 与 T7 一起定 |
| **A6** | 加密路径（分块 / AAD 次数） | 待测 | 中 | 无 |

### 2.5 CLI / 构建

| 编号 | 手法 | 预期 | 风险 |
|---|---|---|---|
| **C1** | `smltools` 启动时间（`--version` 粗测） | 待测；≥15% 才动 | 低 |
| **C2** | 大文件 IO 缓冲 / 流式（`Writer` 已有，补读侧） | 待测 | 低 |
| **B1** | release 档 `lto` / `codegen-units`（**只动发布档**） | 待测；bench 档不改，避免基线不可比 | 低 |
| **B2** | 体积：strip、去 panic 格式化（库不可 `panic=abort`） | ≥5% 才留 | 低-中 |

### 2.6 ISA（SIMD / 国密）

| 编号 | 手法 | 预期 | 风险 |
|---|---|---|---|
| **S1** | 词法 SIMD（= P6） | 见 P6 | 高 |
| **S2** | **SM3 ISA 后端**（ARMv8.2 `FEAT_SM3` / x86 SM3 扩展 / RISC-V `Zvksh` / LoongArch LSX） | 本机无该 ISA，**只能验正确性**；加速比标「待硬件验证」 | 中 |
| **S3** | SHA-256（已 `SHA-NI`）/ zstd（上游 SIMD） | 不动 | — |

---

## 3. 优先级矩阵

| 阶段 | 内容 | 理由 | 出口判据 |
|---|---|---|---|
| **阶段 0 · 尺子** ✅ 已落地 | `benches/profile.rs` + `tools/perf_counters.sh`，三条数字见 §1.1 | 没有这三行，P4/P6 的收益无法归因 | **已过**：基准能复现，tokenize 占比已知 |
| **阶段 1 · 低风险** | E2 + A3（同一路径）、E1、P1、P2、P3 | 全是常数级、单文件、可独立回滚 | 每项 ≥15%，否则回滚并记录 |
| **阶段 2 · 结构** | P5（借用 token）、V1/V2（表示层）、A2（节点分配） | 收益最大，但要 API / 值模型决策 | 先出设计页，再动代码；每项独立 |
| **阶段 3 · ISA** | S1（词法 SIMD）、S2（SM3 后端） | 前面不清，向量化收益会被分配器和缺页吞掉 | 目标负载 ≥15% + bit-for-bit + 回退 |
| **阶段 4 · 格式** | A5（`DIR` 段）、typed 载荷（`docs/amberice-plan.md` §7） | 动格式、动跨端 | 与 T7 / v5 一起定 |

整体目标（**留出噪声余量后**，不是承诺）：

| 指标 | 现在 | 目标 |
|---|---|---|
| `small` parse（正式口径） | 7,073 ns | ≤5,500 ns（−22%） |
| `large` parse | 5,229,215 ns | ≤4,200,000 ns（−20%） |
| `to_sml` `large` | 2,074,170 ns | ≤1,800,000 ns（−13%） |
| `canonical_bytes` 峰值 | 792 KiB | ≤128 KiB（−84%） |
| `load` allocs | 8,173 | ≤5,000（−39%） |
| `Store::get` | 13,966 ns | 保持（已达成） |

---

## 4. 与其它工作的顺序

1. **改名（B 档）先做**：59 个文件机械替换，越晚做、`smlb` 写得越多。
2. 阶段 0 → 阶段 1（不依赖改名，但改名后文件名/包名稳定，基准更省心）。
3. 阶段 2 需你拍：`Tok` 借用是否允许破 API（0.7.0）。
4. **i18n / 文档治理单独一棒**（与 T28 / T40–T42 同批）：`swml` / `swsml` / `sml-*`（`smltools` 不算）的对外措辞**动前问用户**，不塞进性能提交。
5. typed 载荷在阶段 1 之后评估（先有数字，再决定要不要第二套编解码器）。

---

## 5. 风险与「不做」

- **不做**：值语义/键序/错误码/字节布局的任何变更；引 criterion / itoa / smol_str 等依赖；把 `@feature` 编译掉（运行期开关）；为跑分改 golden。
- **主要风险**：放宽 `Value` 48 B 红线（V2）、`Tok` 生命周期扩散（P5）、SIMD 的 unsafe 面（P6）、`load` arena 改变公开 API（A2）。
- **回滚策略**：每项一个 commit；不达标就 revert，并把「为什么没收益」写进 `docs/perf-baseline.md`。
