# `.ambr` 论断核实台账（2026-11-10）

> **产品线** amberice ｜ **状态** draft（待裁决）｜ **最后核对** 2026-11-10
> 代码事实以 §0.1 的复核基线与 §8 的后续扩充为准；复核命令见 §9。
>
> **draft（未定稿）**：本文件是 2026-11-10 会话 45 条论断的核实台账，**不得当结论引用**。
> 去留（逐条采纳并进 `decisions.md`，或整体移入 `docs/amberice/archive/`）由用户裁决，见 §10。
> §2 标「现状见 §0.1」的条目是会话当时的代码事实，当前实现以 §0.1 与源码为准。
>
> 用途：把 2026-11-10 关于 `.ambr` 的每一条论断逐条落到证据上。
> `decisions.md §16.5` 的待裁项以本文为证据底稿；本文只做核实，不做裁决。
>
> 四档口径：
> **成立** = 有代码 / 文档 / 实测支撑；
> **需修正** = 原表述有误，已在下文给出正确说法；
> **未实测** = 代码可推断但没跑，不得当结论引用；
> **判断** = 取舍与优先级，本来就不是事实命题。

## 0. 结论速览

45 条：**成立 33**、**需修正 6**、**未实测 1**、**属判断 5**。

需修正的六条里，最要紧的是 **C# / Go / Ruby 三端 FFI「未做」是错的** —— 它们已于
2026-11-10 实测通过（22/22 逐字符 + 往返，`ref/ports.md`）；`docs/meta.sml` 的 `scope`
与生成的索引按「全端已通」维护，多端状态以 `ref/ports.md` 正文为唯一事实来源。

统计是会话当时的核实快照，不是当前实现状态；其中标「未实测」的 #20 已于 2026-11-10 实测，
`check` 的可探测性缺口也已修复，见 §8。

### 0.1 会话后落地（基线 `136803b`，尾部检测扩充见 §8）

审计 #11 / #12 / #20 已由 `fix(amberice): 原子写与尾部垃圾检测` 落地。下表在 `136803b`
复核；`check` 的尾部检测此后又扩充了一档（见 §8），复核当前实现以 §9 命令为准。
§2 表格保留会话当时的核实快照，标「现状见 §0.1」的行不再反映现状。

| 审计项 | 当前事实 | 源码 |
|---|---|---|
| #10 / #11 | `append` 仍默认覆盖原文件，但落盘走 `write_atomic`：先写同目录 tmp 文件（`with_extension("tmp-<pid>")`，替换扩展名，如 `foo.ambr` → `foo.tmp-1234`）再 `rename`，不截断正式文件；该路径不做 `fsync` | `rust/amberice/src/main.rs`（`write_atomic`、append 分支） |
| #12 | 崩在 tmp 写入阶段不会毁掉原文件；正式文件名下始终是旧文件或完整新文件 | 同上；`rust/amberice/src/ingest.rs`（tmp + fsync + rename） |
| #18 / #20 | `check` 回扫最多 1 MiB 找完整 Trailer：找到且其后有多余字节 ⇒ 报尾垃圾；**找不到但段区之后还有字节** ⇒ 报「层记录被截断或 Trailer 损坏」。`info` 仍只校验段边界，`open_raw` 还校验 caps / 加密自洽 / 重复段 / 段摘要，但都不检查文件尾 | `rust/amberice/src/container.rs`（`check` 第 6 步、`open_raw`）、`rust/amberice/src/layer.rs`（`find_trailer`） |
| #21 | 已有 `check_reports_trailing_garbage` 用例（尾部多 20 B ⇒ `check` 报「20 B」且结论不通过），另有截尾与无层防误报用例 | `rust/amberice/tests/unsigned.rs` |
| #9 / #16 | 未变：`append_layer` 仍整文件复制进新 `Vec<u8>`，追加成本仍是 O(文件大小) | `rust/amberice/src/sign.rs` |

## 1. 边界与已裁决项

| # | 论断 | 结论 | 证据 |
|---|---|---|---|
| 1 | `.ambr` 是不可变档；热数据走 `smldb` 的 BLOB + 缓存（D4） | 成立 | `docs/amberice/decisions.md` D4 |
| 2 | 不承担可变 DB（MVCC）与追加式日志（D14） | 成立 | `decisions.md` D14 |
| 3 | 高频采集按「写前脱敏 + 批量追加 + `history: keep\|drop`」收口（D53） | 成立 | `decisions.md` D53 |
| 4 | 规模信封：单文档 ≤ 8 MB、文档数 ≤ 10⁵、写入 ≤ 1/s、每批 fsync | 成立（裁决） | `decisions.md §10` 第 5 行；CLI 派发全表无速率控制，**代码无任何限流** |
| 5 | 衍生格式只能定义语义、不能定义字节编码（D24） | 成立 | `decisions.md` D24 |
| 6 | `amberice` 只依赖 `sml-value`，其余依赖全为 optional | 成立 | `rust/amberice/Cargo.toml`：`sml-value` 之外均带 `optional = true` |
| 7 | 默认档零第三方依赖 | 成立 | 同上；`Cargo.toml` 的 `default = []`（空默认集，不拉可选依赖），四项 feature 才拉依赖 |
| 8 | 多写者并发交 `smldb`（D21） | 成立 | `decisions.md` D21 |

## 2. 写路径（本次讨论的核心）

| # | 论断 | 结论 | 证据 |
|---|---|---|---|
| 9 | `append_layer` 把旧文件整体复制进新 `Vec<u8>` | 成立 | `rust/amberice/src/sign.rs`：`out.extend_from_slice(bytes)` |
| 10 | CLI `append` 默认原地写（不给 `-o` 就覆盖原文件） | 成立（现状见 §0.1） | `rust/amberice/src/main.rs`：`(None, _) => file.as_str()` |
| 11 | 该写法是 `std::fs::write`，先截断再写，不是 tmp+rename | 成立（现状见 §0.1） | 同上；`fs::write` 走 `File::create` |
| 12 | 因此崩在中途会毁掉基文档、段表与全部历史 | 成立（由 #11 推出；现状见 §0.1） | 无需实测：截断发生在写入之前 |
| 13 | 「8 MB 文件追加一条补丁是百毫秒级重写」 | **需修正** | 实测点只有 4 MB = 1.84 ms、16 MB = 121.79 ms（`spec/smldb.md §5.5`），**8 MB 无可用实测点**。曾在开发机跑过一次临时基准，因该机波动大已作废（见 §7），不得引用。正确说法：8 MB 落在 fsync 悬崖区间内，具体值需在标准环境复测 |
| 14 | 层只增不减；`value()` 按序合并全部层 | 成立 | `rust/amberice/src/store.rs`：`value()` → `apply_patches` |
| 15 | `compact` 是维护必需，而非可选功能 | 判断 | 由 #14 推出：层数线性增长，读当前值线性变贵 |
| 16 | `append` 的成本是 O(文件大小) 而不是 O(补丁) | 成立 | 由 #9 + #10 推出；与写入频率无关 |
| 17 | `Trailer::parse` 只看 EOF 前 64 B | 成立 | `rust/amberice/src/layer.rs`：`bytes[bytes.len() - TRAILER_LEN..]` |
| 18 | 容器解析不校验文件尾，只校验段边界 | 成立（现状见 §0.1） | `rust/amberice/src/container.rs`：`info()` 只做 `table_end > bytes.len()` 与 `end > bytes.len()`；`open_raw()` 直接复用 |
| 19 | 无 Trailer 时 `layers()` 返回空链（视为「没层」） | 成立 | `layer.rs`：`let Some(t) = Trailer::parse(bytes) else { return Ok(Vec::new()) }` |
| 20 | 半截追加能被「优雅降级」（读端退回基文档） | **未实测**（会话快照；现状见 §0.1 / §8） | 由 #18 + #19 可推断为静默忽略；但未跑。另有两个未排除分支：半截尾若让 magic 对齐到 EOF-64，`layers()` 会因 `layer_count` 不符报 `CountMismatch`；会话当时 `check` 不检查截尾，**丢失的追加不会被任何命令报出来**（该可探测性缺口已在 §8 修复） |
| 21 | 现有测试没覆盖「尾部半截写」 | 成立（现状见 §0.1） | `rust/amberice/tests/`：`roundtrip::truncation_is_rejected` 只截整文件（切段），`hostile_chain` 只打伪造 `off` / `count`，无 torn-append 用例 |
| 22 | `CountMismatch` 是硬错分支 | 成立 | `layer.rs`：错误枚举内含 `CountMismatch { declared, found }` |

## 3. 读端与载荷

| # | 论断 | 结论 | 证据 |
|---|---|---|---|
| 23 | `Store` 只有只读 API，不是可写内存模型 | 成立 | `store.rs` 的 `pub fn` 清单无任何 `&mut self` 写入方法 |
| 24 | `DIR` 带 `head_layer`，追加后整段失效但只影响速度 | 成立 | `spec/format.md §19.3`「与当前 head 不一致 ⇒ 整段忽略」 |
| 25 | `FrameCursor` 只解覆盖目标区间的帧；坏最后一帧不影响前面的取值 | 成立 | `spec/format.md §19.12`（含实测用例描述） |
| 26 | 每条 `payloads[]` 开销「约 40 B」 | **需修正** | 摘要按 **64 字符十六进制串**写进 manifest（`rust/amberice/src/manifest.rs`：`Value::Str(hex(&p.digest))`），再加 `name` / `len` / 对象键 ⇒ **约 100 B 量级**（估算，源码无实测数字） |
| 27 | 一个文件至多一个 `EMBED` 段，段内可多载荷 | 成立 | `manifest.rs` 结构注释 |
| 28 | 格式不带自定界，文档边界由容器告知 | 成立 | `decisions.md §12.3`「不内带自定界」 |

## 4. 归档、签名与压缩

| # | 论断 | 结论 | 证据 |
|---|---|---|---|
| 29 | 摘要挡意外损坏；蓄意伪造归签名 | 成立 | `spec/overview.md §3.7` |
| 30 | 多签名全部必须通过，不做短路 | 成立 | `decisions.md` D36 / D40 |
| 31 | `compact` 后旧 `--trust` 锚点必须重建 | 成立 | `sign.rs` `compact` 文档注释（锚点必须重建）；D54 只管 `--evidence` 旁证，不是本条依据 |
| 32 | 「签名不提供更多安全，只提供可归因」 | **需修正** | 签名确实挡**蓄意伪造**（摘要挡不了）。默认不签的理由是「没有第二方验签」，不是签名无用 |
| 33 | SHA-256 / SM3 / BLAKE3 为自实现，不引外部散列库 | 成立 | `rust/amberice/src/{sha256,sm3,blake3}.rs` 无任何 `use` / `extern crate` |
| 34 | 「`crypto.rs` 也是零依赖自实现」 | **需修正** | `crypto.rs` 依赖 `aes-gcm`；`crypto` feature = `aes-gcm` + `hkdf` + `sha2` + `argon2` + `getrandom`。自实现的只有三种摘要 |
| 35 | 重复文本打包实测 200,810 B → 844 B（zstd） | 成立 | `spec/format.md §19.11` |
| 36 | 把多份会话塞进一个大归档，下一步就会长成 `smldb` | 判断 | 索引需求 → D21 的地盘 |

## 5. 多端与工程面

| # | 论断 | 结论 | 证据 |
|---|---|---|---|
| 37 | `amberice` 与 `amberice-ffi` 均 `publish = false` | 成立 | 两个 `Cargo.toml` |
| 38 | `errors/codes.sml` 无 `AMBR` 码，错误以 `AMBR: …` 文本透传 | 成立 | `errors/codes.sml` 无匹配；`spec/overview.md §1.2` |
| 39 | C# / Go / Ruby 三端 FFI 未做 | **需修正** | `ref/ports.md`：三端 **2026-11-10 已实测通过**（22/22 逐字符 + 往返）；`docs/meta.sml` 的 `scope` 与索引按「全端已通」维护 |
| 40 | 各端原生解码器只承诺未压缩、未加密的 `VALUE` + `STRTAB` | 成立 | `spec/overview.md §5` |
| 41 | FFI 面刻意只到「文本」为止 | 成立 | `ref/ports.md` 注释 |
| 42 | 「串口上位机常见端恰在 FFI 欠账里」 | **需修正** | 欠账已清（#39）。串口上位机的语言面不再是阻塞项，剩下的阻塞项只有 `publish = false` 与错误码映射 |

## 6. 场景判断（本就不是事实命题）

| # | 论断 | 结论 | 说明 |
|---|---|---|---|
| 43 | 高频暂存的缺口在写路径，不在字节布局 | 判断 | 由 #9–#16 支撑；换格式不改变写路径 |
| 44 | 暂存层若要做，形态是提交协议而非格式 | 判断 | 设计建议，见 `decisions.md §16.5.5` |
| 45 | 形态 A/B/C 决定论：是否反复改同一份文件，比写入多快更能决定需求 | 判断 | 见 `decisions.md §16.5.1` |

## 7. 本机微基准（非 `amberice` 实测）

口径：PowerShell + .NET `FileStream`（`Create` / `Append` + `Flush(true)`），8 MB / 64 MB 各 3 次。
一次性数据写进 `_scratch/writepath/`（gitignored），跑完即删，该目录当前不存在。复现执行下面
这段，测试数据落在该目录，结果只打印到控制台：

```powershell
$dir = "_scratch/writepath"
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$block = [byte[]]::new(1MB)
$patch = [byte[]]::new(200)

foreach ($size in 8MB, 64MB) {
  $path = Join-Path $dir "$size.bin"
  foreach ($i in 1..3) {
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $fs = [System.IO.File]::Create($path)
    for ($n = 0; $n -lt $size / 1MB; $n++) { $fs.Write($block, 0, $block.Length) }
    $fs.Flush($true); $fs.Dispose()
    $rewrite = $sw.Elapsed.TotalMilliseconds

    $sw.Restart()
    $fs = [System.IO.File]::Open($path, [System.IO.FileMode]::Append)
    $fs.Write($patch, 0, $patch.Length)
    $fs.Flush($true); $fs.Dispose()
    $append = $sw.Elapsed.TotalMilliseconds

    "{0} / {1}: rewrite {2:N2} ms, append {3:N2} ms" -f $size, $i, $rewrite, $append
  }
  Remove-Item $path
}
Remove-Item $dir -Recurse -Force
```

| 尺寸 | 整文件重写 + fsync | 追加 200 B + fsync |
|---|---|---|
| 8 MB | 1964 / 1466 / 1566 ms | 7.80 / 1.09 / 1.02 ms |
| 64 MB | 11992 / 10404 / 10404 ms | 1.20 / 1.32 / 0.89 ms |

读法：**本机不是标准测试环境**（波动大，同一尺寸三次差到 30 %），这批数字**不可作为基线、
不可跨机器比较、不得进裁决**；它只保留一件事：本机上整文件重写与尾部追加的量级差约为
3 个数量级（1466 ms 对 1.0 ms；按最慢的 7.80 ms 也有约 2.3 个数量级）。
要比基线，按 `swsml/ref/perf-baseline.md` 的口径另立环境。

## 8. 探针 2 实测与修复（2026-11-10）

会话时标「未实测」的 #20 已跑完：1 层文件（1056 B）分别截掉 1 / 32 / 64 / 128 B，
四档结果一致 —— `info` = OK、`load` = OK、`layers()` = 空链、`Store::get` 返回基文档旧值，
`CountMismatch` 四档均未命中。当时的 `check` 返回 `ok=true`「结论 通过」且无告警：
第 6 步只在 `find_trailer` 找到完整 Trailer 且其后有多余字节时报警，截尾破坏 Trailer 后
该分支不触发 ⇒ **半截追加静默丢一次写入**。

**已修复**：`check` 第 6 步增加一档 —— 找不到完整 Trailer、但段区（按 `info.align`
对齐后的段末）之后还有字节时，报「层记录被截断或 Trailer 损坏」并判未通过。四档复跑
均为 `ok=false`，例如「段区之后还有 319 B，但找不到完整 Trailer」；无层文件不误报
（`check_plain_file_has_no_tail_warning`）。

**未改**：`info` / `load` / `layers()` 的读端行为，截尾仍退回基文档。规范仍不得写
「半截尾可优雅退化」，尾部恢复本身仍待设计。数字与口径见 `decisions.md §16.5.8`。

## 9. 复核方式

| 查什么 | 怎么看 |
|---|---|
| 追加是否仍整文件 | `rust/amberice/src/sign.rs` 的 `append_layer` |
| 落盘是否原子 | `main.rs` 的 `write_atomic`（tmp + rename）、`ingest.rs` 的提交与 `recover` |
| 尾垃圾是否被检 | `container.rs` 的 `check()` 第 6 步、`layer.rs` 的 `find_trailer`；回归见 `tests/unsigned.rs` |
| 只读 API 面 | `store.rs` 的 `pub fn` 清单 |
| 多端状态 | `ref/ports.md` 状态表（正文为唯一事实来源） |
| 依赖与 feature | `rust/amberice/Cargo.toml` 的 `[dependencies]` / `[features]` |
| 错误码 | `errors/codes.sml` 搜 `AMBR` |

## 10. 去留（待用户裁决）

本文保持 `draft`，**不得当结论引用**；`decisions.md §16.5` 按同一「待裁决 · 不作结论」口径标注。
由用户选一条做完，别让本文长期停在 `draft`：

1. **采纳**：把 §2–§6 里站得住的条目并进 `decisions.md §16.1`（成为 D55/D56…），
   本文改 `status: archive`（或删除并在 `docs/changes.sml` 记一条 `change`）。
2. **归档**：`git mv docs/amberice/ref/claims-audit.md docs/amberice/archive/claims-audit.md`，
   同步 `meta.sml` 的 `title` 与 `status: archive`，在 `docs/changes.sml` 记一条搬迁，
   再跑 `--index` / `--check`。
3. **保持 draft**（当前状态）：只作证据底稿，裁决前不改动既有 D1–D54。
