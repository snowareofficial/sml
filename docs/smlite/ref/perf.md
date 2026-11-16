# smlite 性能基线

> 本文记录 **smlite 独立 crate** 与 `serde_json` 的对拍数字，口径对齐 [`perf-baseline.md`](../../swsml/ref/perf-baseline.md) 的 `vs_json` 方法，但**不并入那份正式口径**（那是 swsml 主线的数字）。
> 基准源码：`rust/benches/smlite_vs_json.rs`（`harness = false`，自带计数分配器）。
> 数字取数日 2026-11-17；正文不写历史，对照列用于解释差异来源。

## 1. 怎么跑

```bash
cd rust
cargo bench --bench smlite_vs_json
```

零依赖手写计时器（不引 criterion），理由同 `perf-baseline.md`：dev 依赖锁版本下限，且目标是「先有一份可复现的数字」。

## 2. 口径

| 项 | 值 |
|---|---|
| 工具链 | rustc `1.99.1 (58cf8f9c3 2026-10-13)` / cargo `1.99.1 (5f94df478 2026-08-27)` |
| 平台 | windows x86_64 |
| crate 构建档 | bench profile（`opt-level = 2`、`lto = false`，见 `rust/Cargo.toml`） |
| 取数 | 11 批交错，每批 smlite 与 serde_json 各跑一次，逐形状取 **min**；对调度抖动最不敏感 |
| 计数 | 计数分配器记 `allocs` 与 `B/op`；两次独立进程交叉核对倍数 |
| 倍数 | `smlite 耗时 / serde_json 耗时`；**< 1 表示 smlite 更快** |
| smlite 构建 | bench 通过 dev-dependency 引入，`default-features = false`（no_std 核心 + alloc，无 std/互转/排序后端） |

## 3. 负载（三形状，与 `vs_json.rs` 同构造）

| 形状 | 内容 | 输入规模（smlite/json） |
|---|---|---|
| `flat@4000` | `k{i}: value-{i}` 逐行 / `{"k{i}":"value-{i}",...}` | 68.1 / 79.9 KiB |
| `block@2000` | `b{i} { x: {i} y: s }` / `{"b{i}":{"x":i,"y":"s"},...}` | 42.8 / 50.6 KiB |
| `array@2000` | `a{i}: [1 2 3 4 5]` / `{"a{i}":[1,2,3,4,5],...}` | 36.0 / 38.0 KiB |

三形状刻意让 smlite 输入更小（裸词省引号），但吞吐按各自输入字节计，倍数看的是「同一事实各解析器花的力」。

## 4. 正式数字（2026-11-17，两次独立进程）

| 形状 | serde_json | smlite | 倍数 | json allocs | smlite allocs | json B/op | smlite B/op |
|---|---|---|---|---|---|---|---|
| `flat@4000` | 1.96–1.98 ms | 1.48–1.50 ms | **0.75–0.77×** | 8620 | 8012 | 457876 | 580308 |
| `block@2000` | 2.72–2.73 ms | 1.95–1.98 ms | **0.71–0.73×** | 10308 | 12011 | 1477578 | 788042 |
| `array@2000` | 1.75–1.82 ms | 1.44–1.62 ms | **0.82–0.89×** | 6308 | 6011 | 975578 | 1038042 |

两次进程跨度：倍数区间即两次的 min，差异 < 0.07×，稳定性可接受。smlite 在三形状上快 **12–29%**（倍数 < 1）。

## 5. 分配与归因

- **分配次数与 serde_json 同量级**：`flat` 8012 vs 8620、`array` 6011 vs 6308；`block` 偏高（12011 vs 10308）但耗时仍更低 —— 与 `perf-baseline.md`「分配次数不是瓶颈、差别在每次分配的大小」一致。
- **对照数据**（同口径、批量构建与借用 token 落地前的实测）：

| 形状 | 对照倍数 | 对照 smlite 耗时 | 对照 smlite allocs |
|---|---|---|---|
| `flat@4000` | 12.42× | 25.80 ms | 12011 |
| `block@2000` | 2.56× | 7.24 ms | 14010 |
| `array@2000` | 3.66× | 7.11 ms | 16010 |

- **两刀改动解释了差异**（实现在 `rust/smlite/src/value.rs` 与 `parse.rs`，测试钉子见 `tests/smoke.rs`）：
  1. **批量建对象**：每键 `insert` 曾线性查重（顶层 4000 键 ⇒ O(n²) 比较）；改为先收集条目、末尾 `Object::from_entries` 一次构建 —— 无重名时借用排序扫描、`O(n log n)` 且零额外分配。
  2. **借用 token**：`parse_bare` 返回 `&'a str`，数字/关键字零分配、仅字符串值分配一次；此前先 `String::from` 再在 `classify` 内二次分配。
- 结论：**`flat@4000` 从 12.42× 降到 0.75×** 主要来自消除 O(n²) 查重 —— 顶层键越多，收益越大，与「`flat` 曾最慢、`block/array` 键少故轻」的分布吻合。

## 6. 边界与不做

- 数字是**本机一次口径**（两进程交叉），不是统计置信区间；跨机比较须同工具链、同 profile。
- 与 `perf-baseline.md` 的 swsml 数字**不可直接混用**：值模型不同（smlite 的 `Value` 更轻、Float 无 raw）、解析器全新、构建档独立。
- 不覆盖 eclog 全量语法（归 eclog-rs）、不覆盖 `to_string` 渲染吞吐（当前 bench 只测 parse）、不覆盖 `swsml-compat` / `object-sort` 档位。

## 7. 复核命令表

| 命令 | 用途 |
|---|---|
| `cargo bench --bench smlite_vs_json` | 重跑对拍，核对三形状倍数与分配 |
| `cargo test -p smlite` | 默认档测试（含重名键语义钉子） |
| `cargo test -p smlite --no-default-features` | no_std 核心档 |
| `cargo run -p smldocs -- --check` | 文档门禁 |
