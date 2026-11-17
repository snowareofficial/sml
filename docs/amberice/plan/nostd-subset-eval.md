# P17 no_std amberice 评估报告

> 对应 [`crossend-sync.md §7`](../../swsml/plan/crossend-sync.md) 的 P17 条目「no_std amberice（容器 / 存储）」。
> **评估与落地均已完成（2026-11-17）**：§0–§5 是裁剪取证（结论仍按源码复核），§6 是实际落地的
> feature 形态，§7 是实测验收结果。P16（`sml-value` 迁 `core + alloc`）与 P17 同批落地。
> 定位依据 [`todo-ideas-tables.md`](../../repo/ref/todo-ideas-tables.md) 的 I-28。
> 事实来源以源码为准；feature 分层与边界叙述对齐 [`overview.md`](../spec/overview.md) 的 current 口径。

## 0. 结论摘要

| 问题 | 结论 |
|---|---|
| 当前状态（评估时） | `amberice` 与 `sml-value` 内 `#![no_std]` / `no_std` 为 **0 命中**（同工作区的 `smlite` / `eclog-rs` 另有 `#![no_std]`，那是各自「零硬依赖」的定位承诺，与本 crate 的裁剪不是一回事）。`amberice` 当时 `default = []`，只依赖零依赖的 `sml-value`。 |
| 哪些可裁 | CLI（`main.rs`）、向量工具（`vectors.rs`）、`mmap.rs` + `memmap2`、`ingest.rs` 的 fs 链、`Writer` 临时文件实现、`pack_from_readers*`、`StreamReader`，以及 `sign` / `compress` / `crypto` 三组可选依赖。 |
| 哪些必须留 std | `Writer<W: std::io::Write>`、`pack_from_readers<R: std::io::Read>`、`StreamReader<R: Read + Seek>`、`MappedFile`、`Ingestor` 原子写、各错误类型的 `std::error::Error` impl 都绑 std；它们与 no_std 互斥，靠 feature 分层保留（§6.2）。 |
| 接口怎么留 | 容器**读侧已是 `&[u8]` 面**、写侧**已是 `&[u8] -> Vec<u8>` 面**（见 §6）；最小子集 = 这组字节面 + 三个纯计算摘要 + `dir` / `Store` 的切片 API + 值层，运行在 `core + alloc` 上。 |
| 前置依赖 | `sml-value` 也要 no_std（P16），**已同批完成**（§6.2）；它在 32 位目标默认 `compile_error!`（`rust/sml-value/src/lib.rs`），嵌入式往往 32 位，故真目标验收带 `allow-32bit`（§7）。 |
| 结论 | `amberice` 增 `std`（默认开）/ `no_std`（opt-in）两档；`--no-default-features --features no_std` 即子集，`thumbv7em-none-eabi` 实机目标编过（§7）。 |

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

> 行号是**评估时（改动前）**的坐标。§6/§7 的落地改动在每个模块头部插入了 prelude 导入、
> 给 std-only 项加了 `cfg` 门，故现在的行号已整体下移若干行；按符号名（`Writer`、
> `pack_from_readers`、`ContainerError::Io`、`Lazy`、`KeyMap`）定位比按行号可靠。

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

### 6.2 实际落地的 feature 形态

`amberice`：`default = ["std"]`、`std = ["sml-value/std"]`、`no_std = []`。
判定「哪一档」只看 `no_std` 这个开关，`data` 侧（`#![no_std]`）的条件是
`all(feature = "no_std", not(feature = "std"))` —— 这样「两个开关同时打开」时只报一条
`compile_error!`，而不是叠 9 条 `cannot find crate std` 的噪音。

- **为什么是 `std` 默认开、而不是 `default = []` + `no_std`**：`sml-value` 也要分档，而 Cargo
  feature 只能加不能减 —— 一旦 `sml-value` 有了 `std` 档，`amberice` 必须用
  `default-features = false` 依赖它，再由自己的 `std` 去转发。反过来做，`--features no_std`
  会把 `sml-value` 的 std 档一起拖进来，得到一个「声称 no_std 其实链了 std」的假产物。
- **默认构建零变化**：`cargo build -p amberice` 的产物、依赖树、公开 API 与改造前一致。
- **`std` 档专属**（`not(feature = "no_std")` 门）：`Writer`、`pack_from_readers*`、
  `stream` 整模块（`StreamReader`）、`ingest` 整模块、`vectors` 整模块、CLI
  （`[[bin]] required-features = ["std"]`）、`ContainerError::Io` 变体与
  `From<std::io::Error>`、各错误类型的 `std::error::Error` impl。
- **两档共用**（`core + alloc`）：字节面、三个摘要、`dir` / `Store`、`layer` / `manifest` /
  `redact` / `typed`、`sml-value` 值层。
- **`sign` / `crypto` 在 no_std 档可用**（2026-11-17 第二刀）：验签、签名、层链、AEAD、
  HKDF、Argon2id 全是纯计算，唯一 std-only 的东西是**熵源**。所以把 `getrandom` 挪进
  `std` 档，两处取熵改成「调用方给」：
  `KeyPair::generate()`（随机种子）只在 std 档；no_std 用 `KeyPair::from_seed`。
  `dump_encrypted`（随机每文件盐）只在 std 档；no_std 用 `dump_encrypted_with_salt`。
  上游库显式关默认特性（`ed25519-dalek` / `aes-gcm` / `hkdf` / `sha2` / `argon2`），
  实测在 `thumbv7em-none-eabi` 上编过。
- **压缩：`compress` 仍限 std，新增 `compress-pure`（只解压、纯 Rust、no_std 可用）。**
  `compress` 是 zstd 的 C 绑定（要 libc），写压缩段只能靠它；`compress-pure` 用 `ruzstd`
  只做**解压**，于是嵌入式可以读压缩文件、不能写。两者同开时以 `compress` 为准（C 后端更快）。
  `ruzstd` 的选型台账就写在 `amberice/Cargo.toml` 里：RUSTSEC-2026-0344（解压炸弹，HIGH，
  2026-10-13）的 patched 版本正是 `>= 0.9.1`，故钉 0.9.1 起；即便如此，输出上限仍用我们
  自己的 `MAX_DECOMPRESSED`（逐块累加、超限立即停），不依赖解码器给的额度。
  差分测试：`tests/compress_pure.rs` 读的是 **zstd C 绑定写出的**
  `examples/showcase-zstd.ambr`，两套实现对拍。
- **`mmap` 与 no_std 组合仍是硬错误**，但理由与压缩不同：不是「不支持」，而是 `memmap2`
  本身编不了 no_std，而 Cargo 的 feature 无法「开特性但不拉依赖」。**no_std 本来也不需要它** ——
  内存映射是 OS 能力，裸机上把 Flash / DMA 缓冲当 `&[u8]` 传给 `Store::open`，走的就是同一条
  零拷贝路径（`mmap` 省的只是「调用方自己读文件」这一步）。
- **两处能力降级，已在代码注释与文档标明**：
  - `Lazy<T>`（`sml-value` 与 `amberice` 各一份同名包装）：三档后端 ——
    std 带 `std::sync::OnceLock`、no_std 用 `core::cell::OnceCell`（`!Sync`）、
    no_std + `embassy` feature 用 `embassy_sync::once_lock::OnceLock`（`Sync` 回来了）。
    `core` 里没有 `OnceLock`；自己写自旋版要 `UnsafeCell` + 原子状态位，是本 crate
    唯一可能引入 UB 的地方 —— **宁可多一个可选依赖也不自己写锁**。
    三者接口不一致（`get`/`try_get`、`set`/`init`、embassy 无 `get_mut`），故包一层统一成
    `new` / `get` / `get_or_init` / `take` / `set`；`Object` 的 `index_insert` 相应改成
    `take` + 改 + `set`，三种后端逐字相同。
    **代价**：`embassy` 档要求目标提供 `critical-section` 实现（embassy 的芯片 crate 会提供）。
    `amberice` 里有一条编译期钉子：该档下 `Store` 与 `Value` 必须重新满足 `Sync`，
    否则 feature 白加 —— 少了它，这个 feature 就是「开关拨了但没效果」。
  - `KeyMap<K, V>`：std 档 `HashMap`，no_std 档 `BTreeMap`（`alloc` 没有 `HashMap`）。
    只用于私有缓存（`Store` 的键→id、`TYPED` 的 schema 表），不进公开签名；代价是
    no_std 档查找从 O(1) 变 O(log n)。
- **`sml-value` 侧（P16）**：`default = ["std"]`、`std = []`，`#![cfg_attr(not(feature = "std"), no_std)]`；
  `serde` 与 no_std 组合是硬错误（`serde::de::value::Error` 只在 serde 的 std 档存在）。
  新增 `sml_value::is_integral`：`f.fract()` 是 std-only（要 libm），no_std 下改用指数位判断
  （纯整数运算），签名校验与文本格式化共用这一份。
- `smlite` 侧的接力点仍只有 `ByteEnvelope` trait（`smlite` 核心已自可 `#![no_std]`），
  本 crate 不替它定 API。

### 6.3 对 smlite 的交付面

- smlite 需要的容器能力 = §6.1 的字节面（写 `&[u8] -> Vec<u8>`、读 `&[u8] -> T`），不需要 `Writer` / `mmap` / `ingest`。
- 语法互操作（无引号串、`#` 注释、顶层省略 `{}`、JSON-compatible 渲染、RFC 7396 Merge Patch）由 smlite 自己实现；amberice 只提供值得信赖的字节信封与摘要，不认识 eclog 文本语法。
- 数据交接点是 `sml-value::Value` / `CanonicalValue`：smlite 与 swsml 的值层互转在第 1 步定，本步不改值模型。

## 7. 验收结果（2026-11-17 实测）

| 判据 | 命令 | 结果 |
|---|---|---|
| 子集能编（宿主） | `cargo build -p amberice --no-default-features --features no_std` | 通过，无警告 |
| 子集能编（真裸机目标） | `cargo build -p amberice --no-default-features --features no_std,allow-32bit --target thumbv7em-none-eabi` | 通过（`rustup target add thumbv7em-none-eabi`） |
| 依赖树 | `cargo tree -p amberice --no-default-features --features no_std` | 只有 `amberice` → `sml-value`，无可选依赖 |
| 两开关同开 | `cargo build -p amberice --features no_std` | 只报一条 `compile_error!`（提示加 `--no-default-features`） |
| 子集 + 签名 + 加密（宿主） | `cargo build -p amberice --no-default-features --features no_std,sign,crypto` | 通过 |
| 子集 + 签名 + 加密（裸机） | 同上 + `allow-32bit --target thumbv7em-none-eabi` | 通过 |
| 与不可用依赖组合 | `--features no_std,compress`（同理 `mmap`） | 报 `compile_error!`，说明为何不支持 |
| 默认档零回归 | `cargo test -p amberice --no-fail-fast` | 全绿 |
| **no_std 档运行时（宿主）** | `cargo test -p amberice --no-default-features --features no_std` | **88 passed / 0 failed**（19 个套件；`roundtrip` / `store` / `dir` / `layers` / `hostile_chain` / `unsigned` / `user_segment` / `frames` / `source` 都真跑） |
| **no_std 档运行时（32 位、零 std）** | `python tools/check/check_nostd_wasm.py` | **通过**：475 KiB 的 wasm、`import` 表为空、`run() = 0` |
| **裸机链接 + ARM 核心执行** | `python tools/check/check_nostd_qemu.py` | **通过**：466 KiB 的 ELF，`QEMU mps2-an500`（Cortex-M7）上打印 `run() = 0`，退出码 0 |
| `embassy` 后端（`Sync` 回归） | `cargo build -p amberice --no-default-features --features no_std,embassy,allow-32bit,sign,crypto --target thumbv7em-none-eabi` | **通过**；库里的 `const _` 钉子断言 `Store` 与 `Value` 都满足 `Sync` |
| 只解压的压缩（`compress-pure`） | `cargo build -p amberice --no-default-features --features no_std,compress-pure,allow-32bit --target thumbv7em-none-eabi` | **通过** |
| Cortex-M0（无原子指令） | `cargo build -p amberice --no-default-features --features no_std,allow-32bit,sign,crypto,compress-pure --target thumbv6m-none-eabi` | **通过**（M0 没有 CAS 指令；库本身不依赖原子，只有探针的 bump 分配器用） |
| 纯 Rust 解码器差分测试 | `cargo test -p amberice --features compress-pure --test compress_pure` | **5 passed**（读 C 绑定写的真实文件；上限与损坏都有断言） |
| no_std 档跑纯解压测试 | `cargo test -p amberice --no-default-features --features no_std,compress-pure` | **93 passed / 0 failed** |
| 全可选档 | `cargo test -p amberice --features sign,compress,crypto,mmap` | 全绿 |
| 下游 | `cargo test -p sml-value` / `-p swsml` / `-p smlite -p eclog-rs -p amberice-ffi -p smldb` | 全绿 |

为什么 no_std 档能跑测试：测试二进制本身是 std 的，`#![no_std]` 只作用于被链接的库，
所以在宿主上 `cargo test --no-default-features --features no_std` 走的是**子集的真实代码路径**。
依赖 std-only API 的用例（`Writer` / `pack_from_readers` / `StreamReader` / `Ingestor` /
`KeyPair::generate` / `dump_encrypted` / 向量文件的读写半边）用
`#![cfg(not(feature = "no_std"))]` 单独门控。`vectors` 模块为此拆成两半：
`CASES` / `render` / `verify_self` 两档都能用，`generate` / `check` / `*_container` 留 std。

**32 位 + 零 std 的运行时证据从哪来**：本机没有 `qemu-system-arm`，改用 wasm32 ——
它同样满足要证的两件事（指针 32 位；产物不链 std）。`rust/nostd-probe` 是独立 workspace
（与 `rust/qsm` 同一做法），自己不链 std：自带 bump 分配器与 `panic_handler`，出口只有一个
`run() -> i32`。`tools/check/check_nostd_wasm.py` 构建后交给 node 实例化，**先查 import 表
必须为空**（一旦链进 std / wasi 就会要求外部导入，这里当场判红），再要求 `run() = 0`。
探针在 wasm 上真跑过的：canonical → dump → load → 相等、路径取值、`check` 逐段校验、
`Store::get`（惰性缓存走 no_std 的 `OnceCell` 分支）、`vectors::verify_self`（20 例）、
签名（`from_seed` → `append_layer` → `verify`）、AEAD（`dump_encrypted_with_salt`
→ `load_encrypted`）、SHA-256 与 SM3 已知向量。这条已进 CI 的 guards job。

**裸机那一层怎么补的**：`rust/nostd-probe-qemu` 复用同一份 `workflow()`，用
`cortex-m-rt`（向量表 / `.bss` 清零 / `memory.x`）链成 `thumbv7em-none-eabi` 的 ELF，
在 `QEMU mps2-an500`（Cortex-M7，4 MiB RAM）上以 semihosting 退出。这一层证的是
**链接脚本、向量表、静态竞技场与栈共存、真 ARM 指令集执行** —— wasm 证不到的部分。
探针的竞技场从 6 MiB 收到 1 MiB（实测全流程 139 KiB），保证能在 4 MiB RAM 里跑完。

仍然**未**验证的两项：① `compress`（zstd C 绑定要 libc）与 `mmap`（memmap2 无 no_std 档）
仍是硬错误，要带进去只能换实现或另找纯 Rust 方案；② 没有在**真硅片**上跑过 —— QEMU 覆盖了
链接与执行，但不覆盖具体芯片的时钟树 / 外设初始化 / Flash 等待周期；那属于移植文档的范围，
不是本 crate 的语义问题。

`cargo test --workspace` 在本机不可用：`swsml` 的 `lib` 与 `cdylib` 同名产出 `sml.dll`，
并行构建时 `link.exe` 报 `LNK1104`。这是既有构建布局问题，与本次改动无关，故按 crate 分跑。

## 8. 堆预算实测（回答「要不要做 no-alloc 档」）

`rust/nostd-probe` 的 bump 分配器会记录 bump 指针快照，`tools/check/check_nostd_wasm.py`
跑完顺带打印各阶段的**累计分配字节**。一份很小的文档（嵌套对象 + 两个字符串数组）实测：

| 阶段 | 本阶段 | 累计 |
|---|---:|---:|
| 构造值 + canonical | 864 B | 864 B |
| `dump`（写 `.ambr`） | 7 940 B | 8 804 B |
| `load`（解回值树） | 8 660 B | 17 464 B |
| `get`（路径取值） | 9 640 B | 27 104 B |
| `check`（逐段校验） | 13 928 B | 41 032 B |
| `Store::open` + `get` | 4 453 B | 45 485 B |
| `vectors::verify_self`（20 例） | 16 811 B | 62 296 B |
| 签名 + 验签 + 回读 | 29 840 B | 92 136 B |
| 加密 + 解密 | 49 776 B | 141 912 B |
| SHA-256 / SM3 | 0 | 141 912 B |

总量 **139 KiB / 2 610 次分配**（含签名与 AEAD；不含密码学时为 61 KiB）。

**结论：不做 no-alloc 档。** 原因不是「工作量」，是**模型层面做不到**：值层 `Value` 的定义
就是 `Vec` + `String`（数组与字符串都要自己持有），去掉 `alloc` 等于换一套值模型 ——
那是 P16 已经裁过一遍的边界，不是本 crate 能单独改的。真正无堆可行的只剩「段表遍历 +
摘要 + 按路径返回字节切片」，而实测 `Store::open` + `get` 只吃 **4.4 KiB**，
省下来的收益远小于「多一档、多一套 API、多一份测试矩阵」的代价。

给嵌入式用户的实用口径：**堆预算按「文档大小 × 常数 + 固定 ~60 KiB」估**（不加密），
一次 `load` 会把整棵树物化；只想取几个字段用 `Store::open` + `get`，那条路是零拷贝的。

## 9. 上板清单（真硅片那一步）

本机**没有接任何调试探针或串口**（`Get-PnpDevice` 无 ST-Link / J-Link / DAP，`SerialPort` 列表为空），
所以「在真硅片上跑一遍」这一步没有条件做。能预先钉住的都钉住了：

| 层面 | 结论 |
|---|---|
| 目标覆盖 | `wasm32-unknown-unknown`（32 位、零 std，运行时）、`thumbv7em-none-eabi`（Cortex-M7，QEMU 运行时）、`thumbv6m-none-eabi`（Cortex-M0，无 `compare_exchange`，仅编译） |
| 依赖写法 | `amberice = { default-features = false, features = ["no_std", "allow-32bit"] }`，按需加 `sign` / `crypto` / `compress-pure` / `embassy` |
| 堆 | 一份小文档全流程实测 139 KiB；建议 `embedded-alloc` 给 256 KiB 起。用 `Store::open` + `get` 只取字段则 ~5 KiB 起 |
| 32 位 | 必须显式开 `allow-32bit`（`sml-value` 的截断风险门）；本仓库在 32 位下的语义已由 wasm32 运行时验证 |
| 中断 / 多任务共享 | 默认 no_std 档 `Store` / `Value` 是 `!Sync`，塞不进 `static`；开 `embassy` 后恢复，且目标要提供 `critical-section` 实现（芯片 crate 自带） |
| 链接 | 只需要 `cortex-m-rt` 的 `link.x` + 自己的 `memory.x`；`rust/nostd-probe-qemu` 是最小样板（4 个文件） |
| 不在本 crate 范围 | 时钟树、Flash 等待周期、外设初始化 —— 属芯片 HAL |

**还缺的唯一一步**：把 `rust/nostd-probe-qemu` 的 `memory.x` 换成目标芯片的地址、把 semihosting
换成串口/`defmt`，然后上板跑 `workflow()` 看返回值。给我芯片型号就能把这份样板配成具体值。

## 10. 复核命令表

在 `rust/` 目录执行（文档门禁同此目录）：

| 命令 | 用途 |
|---|---|
| `rg -n 'std::' rust/amberice/src/*.rs` | 在仓库根执行，复核模块级 std 命中（在 `rust/` 下则去掉 `rust/` 前缀）。 |
| `rg -n 'no_std' rust/amberice rust/sml-value --glob '*.rs' --glob '*.toml'` | 复核两档开关与门控点。 |
| `cargo tree -p amberice` | 默认依赖图（应只有 `sml-value`）。 |
| `cargo tree -p amberice --features sign,compress,crypto,mmap` | 核对可选依赖落点。 |
| `python tools/check/check_nostd_wasm.py` | no_std 子集在 wasm32 的运行时门禁（import 表为空 + `run()` 返回 0）。 |
| `python tools/check/check_nostd_qemu.py` | 同一份探针在 QEMU Cortex-M 上的门禁（裸机链接 + `run()` 返回 0；没有 qemu/wsl 时跳过）。 |
| `cargo build ... --features no_std,embassy` | 校验 embassy 后端确实把 `Store` / `Value` 的 `Sync` 要了回来。 |
| `cargo run -p smldocs -- --index` | 刷新 `docs/index.md`（生成物）。 |
| `cargo run -p smldocs -- --check` | 门禁：未登记 / 断链 / `updated` 超期 / `stale` 无替代者。 |
