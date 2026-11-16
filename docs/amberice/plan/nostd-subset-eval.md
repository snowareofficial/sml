# P17 no_std amberice 评估报告

> 对应 [`crossend-sync.md §7`](../../swsml/plan/crossend-sync.md) 的 P17 条目「no_std amberice（容器 / 存储）：先出评估报告（哪些部分可裁、哪些必须留 std），再决定动不动手」。
> 结论为「出评估、不动手」：本报告只给裁剪边界与接口草案，`rust/amberice/` 源码与依赖方向均不改。
> 定位依据 [`todo-ideas-tables.md`](../../repo/ref/todo-ideas-tables.md) 的 I-28（`amberice` no_std 子集是 `smlite` 的前置依赖）。
> 事实来源以源码为准；feature 分层与边界叙述对齐 [`overview.md`](../spec/overview.md) 的 current 口径。

## 0. 结论摘要

| 问题 | 结论 |
|---|---|
| 当前状态 | 全仓源码与 `Cargo.toml` 中 `#![no_std]` / `no_std` feature 为 **0 命中**；`amberice` 默认 `default = []`，只依赖零依赖的 `sml-value`（`rust/amberice/Cargo.toml:100`、`rust/sml-value/Cargo.toml:40`）。 |
| 哪些可裁 | CLI（`main.rs`）、向量工具（`vectors.rs`）、`mmap.rs` + `memmap2`、`ingest.rs` 的 fs 链、`Writer` 临时文件实现、`pack_from_readers*`、`StreamReader`，以及 `sign` / `compress` / `crypto` 三组可选依赖。 |
| 哪些必须留 std | 若要保持现有 API 原样，则 `Writer<W: std::io::Write>`、`pack_from_readers<R: std::io::Read>`、`StreamReader<R: Read + Seek>`、`MappedFile`、`Ingestor` 原子写、各错误类型的 `std::error::Error` impl 都绑 std；它们与 no_std 互斥，只能靠 feature 分层保留。 |
| 接口怎么留 | 容器**读侧已是 `&[u8]` 面**、写侧**已是 `&[u8] -> Vec<u8>` 面**（见 §6）；最小子集 = 这组字节面 + 三个纯计算摘要 + `dir` / `Store` 的切片 API + 值层，运行在 `core + alloc` 上。 |
| 前置依赖 | `sml-value` 也要 no_std（P16）；它在 32 位目标默认 `compile_error!`（`rust/sml-value/src/lib.rs:40-44`），嵌入式往往 32 位，故 `allow-32bit` 的截断风险同在关键路径上。 |
| 建议 | 先由 P16 把值层迁 `core + alloc`，再按 §4/§6 裁容器；本报告不落地 feature、不改代码。 |

## 1. 评估范围与取证方法

- 范围：`rust/amberice/src/` 全部 20 个模块；`rust/sml-value/src/` 全部 10 个模块；两者的 `Cargo.toml` feature 与依赖声明。
- 方法（可复核，命令在 `rust/` 或仓库根执行）：

```text
rg -n 'std::' rust/amberice/src/*.rs rust/sml-value/src/*.rs
rg -n '#!\[no_std\]|no_std' rust --glob '*.rs' --glob '*.toml'
cargo tree -p amberice                              # 默认依赖图
cargo tree -p amberice --features sign,compress,crypto,mmap
```

- 口径：`std::fmt::Display` / `core::fmt::Display`、`std::error::Error`、`std::io::ErrorKind` 视为 **std 专有**；`Vec` / `Box` / `String` / `Cow` / `BTreeMap` 视为 `alloc` 可达；`std::collections::HashMap` 无 `alloc` 等价物，单列为冲突点。
- 测试代码里的 `String/Vec/format!` 不计入生产面（均在 `#[cfg(test)]` 门内）。

## 2. 现状基线

- `amberice`：`default = []`，零第三方依赖；`preserve_order` 是空转发；`sign` / `compress` / `crypto` / `mmap` / `source` / `allow-32bit` 全是可选 feature（`rust/amberice/Cargo.toml:72-100`）。
- `sml-value`：零第三方依赖（serde 可选）；`sml` / `serde` / `ser_text` 由 feature 门隔开，canonical / object / text_rules / value / map / path 无条件编译（`rust/sml-value/src/lib.rs:46-77`）。
- 依赖方向：当前是 `swsml -> amberice -> sml-value`，方向不因本评估改变；I-28 确认的 `amberice -> smlite -> swsml` 是后续投产的目标拓扑，本步不接线。
- 平台风险：`sml-value` 在 32 位目标默认 `compile_error!`，需 `--features allow-32bit` 才接受 `u64 as usize` / `usize as u32` 的截断（`rust/sml-value/src/lib.rs:37-44`）。

## 3. 逐模块取证台账

`std::` 命中量为生产 + 测试合计的量级参考；判定以下表所引行号为准。

### 3.1 纯计算（core-only 生产路径）

| 模块 | `std::` | 证据与判定 |
|---|---|---|
| `sha256.rs` | 0 | `digest(&[u8]) -> [u8; 32]` `:43`、`verify` `:138`、增量器 `:55/:72/:83/:116`；仅测试用 alloc（`:202` 起）。 |
| `sm3.rs` | 0 | `digest` `:38`、`compress` `:96`；仅测试用 alloc（`:157` 起）。 |
| `blake3.rs` | 0 | `digest(&[u8]) -> [u8; 32]` `:303`、核心 `:10-33/:66/:154`；仅测试用 alloc（`:309` 起）。 |
| `hash.rs` | 6 | `digest` `:279`、`digest_by_id` `:292` 函数体 core-only；但 `impl Display` `:169-170/:262-263`、`impl Error` `:188/:275`，`HashPath.subs: Vec<u16>` `:143`、`name() -> String` `:246-248` 需 alloc。 |

### 3.2 核心字节面（少量 std，多为错误 trait 或可迁 API）

| 模块 | `std::` | 证据与判定 |
|---|---|---|
| `container.rs` | 41 | 字节面 `:942/:947/:1158/:1164/:1180/:1189/:1683/:1793/:1881/:1916/:2288/:2674/:3044/:3063/:3080/:3131`；错误/格式化 `:506/:566-567/:734/:736-737`；IO/fs 耦合 `:1211/:1224/:1333/:1358/:1365/:1367/:1671/:1676`；`Cow` `:2356` 可换 `alloc::borrow::Cow`。 |
| `lib.rs` | 0 | 仅模块与 feature 门：`sign` `:55/:94-98`、`mmap` `:62/:104`、`source` `:84-85`；字节面再导出 `:77-82`。 |
| `dir.rs` | 3 | `build(&[u8],u32,u32) -> Vec<u8>` `:66`、`parse(&[u8])` `:90`、`find` `:53` 无 fs/io；std 只在错误 trait `:238-239/:253`。 |
| `store.rs` | 15 | `Store::open(&[u8])` `:263`、`get` `:551`、`text` `:563`、`value` `:571`、`payload` `:596` 全是切片入口；`HashMap` `:35`、`OnceLock` `:66/:71/:73/:289/:293-294/:342/:354/:368`、`Cow` `:596/:602` 需处理。 |
| `manifest.rs` | 3 | 解析/序列化是字节操作、无 fs/io；错误 trait `:654-655/:670`；feature 能力检查 `:501-505/:893-903`。 |
| `layer.rs` | 3 | 无 fs/io；错误 trait `:533-534/:554`。 |
| `redact.rs` | 3 | 纯规则/字节操作；错误 trait `:80-81/:91`。 |
| `error.rs` | 2 | `use std::fmt` `:19`、`impl std::error::Error` `:99`；Display 可迁 core，Error trait 需门控。 |

### 3.3 std 重耦合（可裁）

| 模块 | `std::` | 证据与判定 |
|---|---|---|
| `stream.rs` | 2 | `use std::io::{Read, Seek, SeekFrom}` `:13`、`io_err` `:22`、`impl<R: Read + Seek>` `:49`、`open(mut src: R)` `:52`、`payload -> Vec<u8>` `:213`、`load` `:252`。 |
| `ingest.rs` | 11 | `std::path` `:13`、`std::time::Instant` `:14`、`open(path)` `:126`、`std::fs::read` `:128/:221`、`process::id` `:255`、`write/OpenOptions/rename` `:256-266`；纯逻辑岛 `merge_into` `:243`、`layer_count` `:209`。 |
| `mmap.rs` | 3 | `std::fs::File` `:29`、`std::path::Path` `:30`、`memmap2::Mmap` `:32`、`open` `:47-52`；由 `lib.rs:62` 的 `#[cfg(feature = "mmap")]` 隔开。 |
| `typed.rs` | 6 | `std::collections::{BTreeMap, HashMap}` `:37`、`vars` `:415`、`parse_schema -> HashMap` `:693/:705`、`types: &HashMap` `:774/:789`；`std::str::from_utf8` `:312/:839` 可换 core。 |
| `sign.rs` | 3 | 整模块由 `lib.rs:55` 门控；`ed25519_dalek` `:26`、`getrandom::fill` `:66`；`verify(&[u8], algo)` `:288` 本身是字节面。 |
| `crypto.rs` | 0 | 无 `std::`，但外部依赖全部 cfg 隔离：`aes_gcm` `:33-36`、`hkdf + sha2` `:132-147`、`argon2` `:230/:238-244`、`getrandom` `:260`。 |
| `main.rs` | 49 | CLI：`PathBuf` `:14`、`ExitCode` `:15`、`args()` `:81`、fs 读写遍布 `:198/:345-348/:356/:482/:495/:526/:535-539/...`、`temp_dir + process::id` `:1486-1488/:1599-1601`。 |
| `vectors.rs` | 5 | 向量工具：`Path/PathBuf` `:34`、`fs::read_to_string` `:542/:576`、`create_dir_all` `:602`、`write` `:606`。 |

### 3.4 `sml-value` 依赖层（P16 范围，不在本步改）

| 模块 | 证据与判定 |
|---|---|
| `canonical.rs` | `std::fmt::Error` `:236-237`、`impl Error` `:302`、`from_utf8` `:739/:794/:1311`（可换 core）；alloc `Vec<&str>` `:790`、`Vec/String` `:1457/:1465/:1487-1491`；32 位截断 `:790-791/:1052-1058/:1373/:1377/:1390/:1395/:1455/:1465/:1483`。 |
| `object.rs` | `BTreeMap` `:23` → `alloc::collections`；`OnceLock` `:24/:160/:167/:173/:197/:432/:603` → `core::sync` 或门控；`mem/ptr/Iter/IntoIter` `:297/:461/:481-552` 均 core/alloc；u32↔usize 压缩 `:83/:101-102/:115/:132/:212-217/:240/:262-263`。 |
| `text_rules.rs` | 唯一 std 是 `use std::borrow::Cow` `:16` / `quote_if_needed` `:70`；等价 `alloc::borrow::Cow`，无 IO。 |
| `value.rs` | `use std::fmt` `:31`、`mem::size_of/replace/take/forget` `:85/:268/:273/:280/:288/:294/:298/:303/:308/:313/:319/:328` 全可迁 core；无 `Error` impl。 |
| `path.rs` | `std::fmt` `:66`、`FromStr` `:67`（可迁 core）、`impl Error` `:168`（需门控）。 |
| `map.rs` | `BTreeMap` `:18`、`btree_map::Entry` `:157`、`vec::IntoIter` `:210/:376`、`mem::take` `:276`，均 alloc/core。 |
| `serde_bridge.rs` / `ser_text.rs` | `::std::collections::BTreeMap` `:11`、`::std::fmt` `:12`、`mem::take` `:919`；由 `lib.rs:73-77` feature 门隔开，no_std 首刀不带。 |
| `dump.rs` | `std::` 0 命中；由 `lib.rs:71-72` 的 `sml` feature 隔开，是 core+alloc 文本发射器。 |

## 4. 可裁清单（no_std 首刀）

| 可裁对象 | 依据 | 裁后替代 |
|---|---|---|
| `main.rs` CLI、`vectors.rs` 向量工具 | §3.3 | 留在 std 构建；no_std 构建不编 `[[bin]]` 相关模块。 |
| `mmap.rs` + `memmap2` | `mmap.rs:29-32/:47-52` | 调用方自己把文件读成 `&[u8]` 传入 `Store::open`（`Cargo.toml:91` 注释已写明这是设计退路）。 |
| `ingest.rs` 的 fs 链 | `ingest.rs:126-266` | 保留 `merge_into` `:243` 等纯逻辑；落盘交给宿主。 |
| `Writer` 临时文件实现、`pack_from_readers*` | `container.rs:1333/:1358/:1543-1651/:1211/:1224` | 用 `Vec<u8>` 组合 `dump_with` / `pack_with` 在内存里完成。 |
| `StreamReader` | `stream.rs:13/:49/:52` | 用 `Store::open(&[u8])` 的切片面替代。 |
| `sign` / `compress` / `crypto` 可选依赖 | `Cargo.toml:83/:86/:89`、`default=[]` | no_std 构建不开这三个 feature；zstd 是 C 绑定、`memmap2` 无 no_std，冲突先天存在。 |
| `std::error::Error` impl | 各模块错误 trait 行 | 放到 `std` feature 后；no_std 下保留 `core::fmt::Display`。 |

## 5. 必留 std 与冲突点

若要求现有 API 原样可用，以下必须留在 std 侧；它们的存在说明「全 crate 一刀切 no_std」不可行，必须分层。

| 冲突点 | 证据 | 说明 |
|---|---|---|
| `Writer<W: std::io::Write>` | `container.rs:1333/:1358` | `new` 用 `AtomicU64` + `temp_dir` + `process::id` + `OpenOptions`（`:1361-1375`），`finish` 用 `Read + Seek` 回读临时文件（`:1543-1651`）。 |
| `pack_from_readers<R: std::io::Read>` | `container.rs:1211/:1224` | 函数体 `read_to_end` `:1229-1231`，绑定 std IO trait。 |
| `StreamReader<R: Read + Seek>` | `stream.rs:13/:49/:52` | trait 边界本身是 std 的。 |
| `MappedFile` | `mmap.rs:29-32/:47-52` | `memmap2` 无 no_std。 |
| `Ingestor` 原子写 | `ingest.rs:126-266` | fs/path/time/process 全链。 |
| `HashMap` | `typed.rs:37/:693/:705/:774/:789`、`store.rs:35` | `alloc` 无 `HashMap`；须换成 `BTreeMap`（改变行为/性能）或对 `TYPED`/`SCHEMA` 做 feature 门控。这是**待裁决项**，本报告不替用户选。 |
| `std::error::Error` | `error.rs:99` 等 | 生态互操作用；no_std 下需 `#[cfg(feature = "std")]` 之类的开关。 |

## 6. 接口最小面（core + alloc）

容器现有 API 已天然分层：**读侧收 `&[u8]`、写侧出 `Vec<u8>`**，是 no_std 子集的直接候选，无需发明新语法。

### 6.1 建议保留的字节面

| 组 | 入口 | 证据 |
|---|---|---|
| 写 | `dump_with` / `pack_with` / `pack_iter_with` / `write_file(...) -> Vec<u8>` | `container.rs:947/:1164/:1189/:1683`（`dump` `:942`、`pack` `:1158` 同族） |
| 读 | `info` / `body` / `load` / `base_value` / `check` / `embed_body` / `payloads` / `payload` / `get` | `container.rs:1793/:1881/:1916/:2288/:2674/:3044/:3063/:3080/:3131` |
| 摘要 | `hash::digest` / `digest_by_id` + `sha256::digest` / `sm3::digest` / `blake3::digest` | `hash.rs:279/:292`、`sha256.rs:43`、`sm3.rs:38`、`blake3.rs:303` |
| 索引 / 随机访问 | `dir::{build, parse, find}`、`Store::{open, get, value, payload}` | `dir.rs:66/:90/:53`、`store.rs:263/:551/:571/:596` |
| 值层（P16） | `Value` / `CanonicalValue` / `canonical_*` / `text_rules` / `Object` | `rust/sml-value/src/lib.rs:46-70`；迁移动作见 §3.4 |

### 6.2 feature 旗帜草案（未落地，需后续确认）

以下命名是草案，仅用于说明「接口怎么留」，本步不写进 `Cargo.toml`：

- 保留 `default = []` 的加法式开关（推荐，避免打断既有构建）：增加 `no_std` feature，`lib.rs` 顶部用 `cfg_attr(feature = "no_std", no_std)` + `extern crate alloc`；`std::error::Error` impl、`Writer`、`pack_from_readers`、`StreamReader`、`MappedFile`、`Ingestor` 统一加 `not(feature = "no_std")` 门。
- 现有 `sign` / `compress` / `crypto` / `mmap` 保持可选；no_std 构建默认不开，避免把 zstd C 绑定、`memmap2`、getrandom 熵源拖进来。
- `smlite` 侧的接力点与 `swsml-compat` 旗属第 1 步设计范围，本报告只确认「smlite 依赖 no_std amberice」这条方向（I-28），不替它定 API。

### 6.3 对 smlite 的交付面

- smlite 需要的容器能力 = §6.1 的字节面（写 `&[u8] -> Vec<u8>`、读 `&[u8] -> T`），不需要 `Writer` / `mmap` / `ingest`。
- 语法互操作（无引号串、`#` 注释、顶层省略 `{}`、JSON-compatible 渲染、RFC 7396 Merge Patch）由 smlite 自己实现；amberice 只提供值得信赖的字节信封与摘要，不认识 eclog 文本语法。
- 数据交接点是 `sml-value::Value` / `CanonicalValue`：smlite 与 swsml 的值层互转在第 1 步定，本步不改值模型。

## 7. 验收口径（未来 no_std 构建的判据，本步不实现）

- `cargo build -p amberice --no-default-features --features no_std` 通过，`rustc` 不报 `std` 未解析。
- 编出的 rlib 不链入 `std::fs` / `std::io` / `std::env` / `std::process` / `mmap2` / zstd 符号。
- 最小用例：把一份 `.ambr` 字节读入 `Vec<u8>`，调用 `info` + `load` + `check` + `get` 得到值与校验报告，全程不碰文件系统。
- 32 位目标（嵌入式常见）需显式接受 `allow-32bit` 的截断风险；该风险的逐点清单在 §3.4，属 P16 的处理范围。
- `sign` / `compress` / `crypto` 在 no_std 下的可用性**未评估结论**：本报告只确认它们与 no_std 首刀互斥；若后续要带签名/加密进 no_std，需单独评估 `getrandom` 熵源、zstd C 绑定、`argon2` 的 alloc 需求。

## 8. 复核命令表

在 `rust/` 目录执行（文档门禁同此目录）：

| 命令 | 用途 |
|---|---|
| `rg -n 'std::' rust/amberice/src/*.rs` | 在仓库根执行，复核模块级 std 命中（在 `rust/` 下则去掉 `rust/` 前缀）。 |
| `rg -n 'no_std' . --glob '*.rs' --glob '*.toml'` | 确认当前 0 命中（含 `#![no_std]` 与 feature 名）。 |
| `cargo tree -p amberice` | 默认依赖图（应只有 `sml-value`）。 |
| `cargo tree -p amberice --features sign,compress,crypto,mmap` | 核对可选依赖落点。 |
| `cargo run -p smldocs -- --index` | 刷新 `docs/index.md`（生成物）。 |
| `cargo run -p smldocs -- --check` | 门禁：未登记 / 断链 / `updated` 超期 / `stale` 无替代者。 |
