# amberice 是什么

> 本文以本仓库源码为唯一事实来源，不引用 `docs/` 下任何既存叙述。每条结论都给出可复核的代码位置；格式常量以 `rust/amberice/src/container.rs` 与 `rust/sml-value/src/canonical.rs` 的实现为准。

**一句话定义**：amberice 是 `.ambr` 这条独立产品线，实现一种**不可变、可寻址、可验证的通用二进制信封**。SML 值树是它的内置载荷之一，任意字节（文件、日志、存档、传输载荷）也可以是载荷。参考实现是 Rust，配套命令行、C ABI、多端解码器与绑定、跨端字节向量。

它回答的是「装什么、怎么定位、怎么证明没被改」，不回答「文本怎么解析」或「数据怎么查询」。

## 1. 边界

### 1.1 属于 amberice

| 能力 | 落地位置 |
|---|---|
| 自描述的容器布局：文件头 + 段表 + 段体，未知段按长度跳过 | `rust/amberice/src/container.rs` |
| 值载荷：`Value` 的 canonical 字节（`VALUE` + `STRTAB`），键序即源序 | `rust/sml-value/src/canonical.rs` |
| 通用字节载荷：`EMBED` 段 + manifest 的 `payloads[]`，可打包 / 列举 / 取出 / 解包 | `container.rs`、`rust/amberice/src/main.rs` |
| 完整性：逐段摘要（SHA-256 / SM3）、`MANIFEST` 自描述、`caps` 能力声明 | `sha256.rs`、`sm3.rs`、`hash.rs`、`manifest.rs` |
| 可信：追加式层链 + Ed25519 签名 / 验签 / 换钥授权 / 历史回放 | `layer.rs`、`sign.rs` |
| 随机访问：不解整树按路径取子树、可选路径索引、内存映射 | `store.rs`、`dir.rs`、`mmap.rs` |
| 可选的段级 zstd 压缩、按块 AES-256-GCM 加密、强类型旁路通道 | `crypto.rs`、`typed.rs`、`DumpOptions` |
| 一份格式，两端以上实现：Rust 写、多端读，字节由向量钉住 | `tests/vectors/`、`rust/amberice/src/vectors.rs` |

### 1.2 容易混同、但代码里不成立的说法

| 说法 | 代码事实 |
|---|---|
| `.ambr` 是「SML 的二进制序列化」 | `rust/amberice/Cargo.toml` 只依赖 `sml-value`（值模型），不依赖 `sml-parse` / `swsml`。容器层、manifest、层链、加密都不认识文本语法；SML 解析只出现在 `amberice-ffi` 的可选 `sml` feature 与门面 `sml::envelope` 里 |
| amberice 就是 smldb（数据库层） | 全仓没有 smldb 的 crate、源码目录、CLI 子命令或测试向量；`main.rs` 的 `USAGE` 无数据库类命令。smldb 不属于已交付能力 |
| 容器错误有五端错误码 | `errors/codes.sml` 没有 AMBR 码；`ContainerError::kind()` 给稳定机器可读分类，C ABI 错误以 `AMBR:` 前缀写进调用方缓冲 |
| 顶层 `amberice/` 目录是实现 | 该目录只有 Python FFI 绑定；实现在 `rust/amberice/`。`rust/amberice-ffi/` 是另一个独立 crate（C ABI） |
| 已经是发布的 crate | `rust/amberice/Cargo.toml` 与 `rust/amberice-ffi/Cargo.toml` 均设 `publish = false` |
| CLI 能加密 / 内存映射 | `main.rs` 明确：`dump_encrypted` 与 `MappedFile` 只有库接口，没有 CLI 子命令 |

## 2. 产品线组成

| 层 | 路径 | 职责 |
|---|---|---|
| 编码地基 | `rust/sml-value/src/canonical.rs`、`text_rules.rs` | `canonical_text`（签名对象）、`canonical_bytes`（自包含 L0）、`canonical_blocks`（拆成两段体）、`CanonicalValue` |
| 参考实现 | `rust/amberice/src/` | 容器、摘要、manifest、随机访问、层链、签名、加密、强类型、向量、CLI（`main.rs`） |
| C ABI 头 | `rust/amberice/include/amberice.h` | `ambr_*` 符号与错误约定的公开声明 |
| C ABI 实现 | `rust/amberice-ffi/src/lib.rs` | 独立 cdylib，让 `amberice` 保持纯 rlib；写能力由 `sml` feature 门控 |
| 库级信封 | `rust/src/envelope.rs`、`rust/src/lib.rs` | `sml::envelope` 与根再导出；仅在门面 `amberice` feature 下存在 |
| 产品 CLI | `rust/smltools/src/main.rs` | `--from-amberice` / `--to-amberice` / `--amberice-compress`，走文本转换管线 |
| 跨端契约 | `tests/vectors/amberice-canonical.v1.json`、`tests/vectors/amberice-container.v1.json` | 前者钉节点编码，后者钉整个文件；由 `amberice vectors --gen` 生成、`--check` 逐行比对 |
| 原生解码器 / 写入器 | `c/ambr.*`、`cpp/ambr.*`、`js/ambr*.mjs`、`lua/lib/ambr_*.lua`、`bindings/java/src/main/java/sml/Ambr*.java` | 各端只解 `VALUE` + `STRTAB`，其余段跳过；写入经 C ABI 或纯语言实现 |
| FFI 绑定 | `bindings/go/amberice/`、`bindings/csharp/Amberice.cs`、`bindings/ruby/lib/amberice.rb`、`amberice/bindings/python/ambr.py` | 字节进、canonical 文本出；值树交给该端已有的 SML 解析器 |
| 质量线 | `rust/fuzz/fuzz_targets/amberice_*.rs`、`rust/benches/amberice.rs`、`.github/workflows/ci.yml`、`tools/check/check_ambr_write.py` | 恶意输入、基准、CI 档位、写入门禁 |
| 仓库治理 | `rust/smldocs/` | 文档索引 / 门禁，是 `publish = false` 的内部 crate，与产品 CLI `smltools` 分开 |

## 3. 文件格式：`.ambr` v1

> 本节是从源码抄出的**摘要**；字节级权威仍是 `docs/amberice/spec/format.md §19`。两处不一致时以代码与跨端向量为准，并回写规范。

### 3.1 文件头（40 B，全部小端）

`write_file` 写出、`info` 读回的字段：

| 偏移 | 长度 | 字段 | 值 / 语义 |
|---|---|---|---|
| 0 | 4 | magic | `AMBR` |
| 4 | 2 | edition | 低字节 major、高字节 minor；常量 `EDITION = 0x0004` |
| 6 | 1 | fmt | 字节布局补丁位，当前 `0`；非 0 一律拒 |
| 7 | 1 | hdr_len | 头部总长，写出为 `40`；读者允许更大值以跳过将来新增字段 |
| 8 | 8 | seg_table_off | `u64`，写出时等于 `hdr_len`（段表紧跟文件头） |
| 16 | 4 | seg_count | 段表项数 |
| 20 | 2 | flags | 位 0 = 段体 4 KiB 对齐；为 0 时按 8 B 对齐 |
| 22 | 2 | reserved | 0，读时忽略 |
| 24 | 8 | tail_off | 尾部表预留，写出为 0；层链 Trailer 靠 EOF 定位 |
| 32 | 8 | reserved | 0，读时忽略 |

### 3.2 段表与段体

- 段表自 `seg_table_off` 起，`seg_count × 41 B`；表项为 `seg_id:u8 | comp:u8 | enc:u8 | flags:u16 | len:u32 | hash:32 B`。
- **段体不存偏移**：首个段体起点是 `align_up(段表尾, granule)`，其后每段 `align_up(前段尾, granule)`；`granule` 由头 `flags` 位 0 决定（8 B 或 4096 B）。位置靠走表得出，因此位置不是语义。
- 段头的 `hash` 覆盖**落盘字节**（压缩 / 加密之后），算法由 manifest 的 `hash_id` 指定；全 0 表示「这份文件没带摘要」。
- 段头 `flags` 位 0 表示「加密文件里本段声明明文」，声明与 `enc` 实际值不一致直接拒。
- 写者按 `seg_id` 升序写段表与段体，使明文 `dump` 是值的纯函数：同值同字节。
- 单段 `len` 是 `u32`，单段上限 4 GiB。

### 3.3 段 id

| id | 名称 | 作用 |
|---|---|---|
| `0x01` | `VALUE` | canonical 节点树（默认主内容） |
| `0x02` | `STRTAB` | 键与字符串值共用的去重字符串表 |
| `0x03` | `DIR` | 路径 → 偏移索引，派生数据，默认不写 |
| `0x04` | `MANIFEST` | 文件自描述，必写，永不压缩 / 加密 |
| `0x10` | `FRAMES` | 节点树切成独立 zstd 帧 + 帧索引，按需解帧 |
| `0x20` | `EMBED` | 外来字节载荷，原样存取；一个文件至多一个 |
| `0x60` | `PROVENANCE` | 已登记，读者按未知段跳过 |
| `0x70` | `TYPED` | 编译期已知类型的强变量，附加旁路通道 |

不在表里的 id 同样合法：读者按 `len` 跳过，不解析、不猜语义。`0x30 SIG` / `0x40 PROFILE` 已登记但实现不写；
`0x50 SOURCE` 在 `source` feature + 显式 `put_source` / `dump_with_source` 下才写（D46），默认不写。

### 3.4 三条不变量

1. **未知段可跳过**：长度自描述，读文件不需要先跑解释器。
2. **签名签 `canonical_text`**：与字节布局、`fmt` 解耦，布局升级不使历史签名失效。
3. **追加不改旧字节**：层链用反向指针只写 EOF；`compact` 是唯一例外且必须重签。

### 3.5 `VALUE` / `STRTAB`：canonical 节点树

- 节点 = `tag:u8 | flags:u8 | subtree_len:u32le | payload`（6 B 头）；`subtree_len` 只数载荷，故跳过整棵子树是 O(1)。
- tag 0–7 依次为 null / false / true / int / float / str / arr / obj；`Int` 用 zigzag + LEB128，`Float` 是 8 B 小端 `f64`，`Str` 与对象键都指向 `STRTAB` id。
- `STRTAB` = `(len:varint ‖ utf8)*`，去重、id 按首次出现顺序；对象按**源序**写键值对。
- 两种形态：`canonical_bytes` 是自包含 L0（表前多一个 `count:varint`），`MANIFEST` 与层记录用它；`canonical_blocks` 把同一编码拆成 `VALUE` / `STRTAB` 两个段体，容器直接落段。
- 键序是**数据层事实**：`Map::Unordered` 一律返回 `KeyOrderUnavailable`，`parse_ordered()` / `Map::ordered()` 产出的值可签；`dump` 的入参是 `CanonicalValue`，把键序证明放在类型上。非有限浮点与深度超限也是错误，不静默降级。
- 文本侧与字节侧分工：`canonical_text` 是签名对象、跨端对拍基准与人类可读差异；`canonical_bytes` 是容器载荷。元数据键在文本侧固定前置，在字节侧保持源序。

### 3.6 `MANIFEST`：自描述

字段：`primary`（主内容段 id）、`hash_id`、`segments[]`（段的可签视图：id / comp / enc / len / 摘要，**不含偏移、不含自己**）、`payloads[]`、`crypto`、`caps`、`patch`、`defaults`、`created`、`tool`。

- 它用自包含 L0 编码，才能在还不知道 `STRTAB` 在哪时读出；引导顺序因此是「先解码、再按它声明的 `hash_id` 验自己的摘要」。
- `caps.required` 有读不懂的能力就拒绝打开；`caps.optional` 读不懂就跳过。
- `segments[]` 与段表故意重复：段表负责定位，前者负责可签；`amberice check` 交叉核对两者。
- 解码时对未知键忽略、缺省键取默认常量（`defaults` 字段可覆盖其中三项）；`primary` / `hash_id` / `segments` 三者缺一即错。`created` / `tool` 只展示，不参与验证。

### 3.7 摘要算法

| id | 算法 | 状态 |
|---|---|---|
| `0x30` | SHA-256 | 默认；零依赖自实现 |
| `0x31` | SM3 | 零依赖自实现 |
| `0x32` | BLAKE3 | 零依赖标量实现（无密钥 32 B 摘要），官方 KAT 覆盖单块与跨 chunk |

未知 id 明确拒绝，绝不猜一个算法去验。摘要挡的是意外损坏；蓄意伪造归签名。

### 3.8 附加通道

- `DIR`：`(父对象偏移, 键 id, 子偏移)` 的排序索引，把逐层线性比键变成二分。它带建索引时的层数，层数变了整段忽略；缺失 / 过期 / 损坏一律回退线性导航，不会给出错误答案。
- `TYPED`：按 `var_id` 存数字 id + 固定布局的强类型值，读时比对 `TYPE_ID`。`VALUE` 仍必写，canonical 相等与签名都不看这一段。
- `SCHEMA`：可选，`type_id → 字段/变体` 的 canonical `Value`；`Writer::put_schema` 写、`Store::schema` 读。不写也能读 `TYPED`（D45）。
- `FRAMES`：帧数 + 每项 16 B 的帧索引（解压后偏移 / 压缩后偏移 / 压缩长 / 解压长），各帧独立压缩；`FrameCursor` 跳过不解压、只解真正读到的区间。
- `EMBED`：段体是各载荷按序拼接，`payloads[]` 按序给 `name / len / 摘要`；偏移不存，按 `len` 走位。`unpack` 拒绝含 `..`、绝对路径、盘符的载荷名。

### 3.9 层链与签名

- 文件尾 64 B Trailer：magic `SMLTRLR1` / 层数 / 最后一层偏移 / 链头哈希；`Trailer::parse` 只看 EOF 前 64 B，没有魔数即视为无层。
- 层记录 = 16 B 头（`len` / `prev_off` / reserved）+ L0 体；`prev_off` 组成反向链，追加只碰 EOF。偏移不进签名。
- 层内容：`index` / `prev_hash` / `author` / `base_hash` / 可选 `patch` / 可选 `delegation` / `sig[]`。`base_hash` 把基文档绑进签名范围；`sig` 数组里的签名全部必须通过。
- 签名对象是**去掉 `sig` 字段后的层 `canonical_text`**，算法 Ed25519（`0x40`）；验签用 `verify_strict`。
- `load` 返回基值按层补丁合并后的当前值；`history` / `Store::history_at` 回放第 N 层为止的值。
- CLI 的 `verify --trust` 把链头钉在外部锚点，防整条链被剥离；`append --expect-head` 做追加冲突检测；`compact` 合并多层并重签，旧锚点需重建。
- 未启用 `sign` feature 的构建仍能读链结构与钉锚点，但 `verify` 明确提示「签名一个字都没验」。

### 3.10 压缩与加密

- 压缩 id `0x01` = zstd，段级生效且只有真的变小才写压缩形态；`FRAMES` 内部已是压缩帧，不再套一层。单段解压上限 `MAX_DECOMPRESSED = 256 MiB`，防解压炸弹。
- 加密 id `0x10` = AES-256-GCM，段体按 4 KiB 块做 AEAD：`plain_len` / `block_count` 头 + 每块密文与 16 B tag；nonce 由层 id、块号、每文件 salt 组成，AAD 绑定 `seg_id` / `comp` / 块号 / 块数 / 明文长度，改段头会当场认证失败。
- 密钥入口 `KeySource` 两条路：原始 32 B 密钥（HKDF 派生）或口令（Argon2id `0x51`，参数写进 manifest）；文件只声明算法与参数，不写密钥从哪来。
- 写顺序「先压后加」，读顺序「先解密再解压」，摘要始终对落盘字节验。加密文件强制 4 KiB 对齐，`MANIFEST` 恒明文，且因每文件随机 salt 而**不承诺字节确定**。
- 零拷贝随机访问不支持加密段与压缩段（`EncryptedNotZeroCopy` / `ZeroCopyNeedsPlain`）；分帧走
  `SubtreeBytes::Owned`（只解目标区间），不再是错误。

### 3.11 错误模型

- Rust 侧：`ContainerError`（`kind()` 给 `bad-magic` / `digest-mismatch` / `missing-capability` / `auth-failed` / `decompress-too-large` 等稳定分类）、`ManifestError`、`CanonicalError`、`SignError`。这些错误不占 `errors/codes.sml` 的五端码位。
- C ABI 侧：`ambr_text` / `ambr_pack_text` 失败返回 NULL，把 `AMBR: …` 写进调用方缓冲（NUL 结尾、按 `errsz` 截断、不越界）；`catch_unwind` 挡住 panic 越过 FFI 边界。

## 4. 能力面

### 4.1 Rust API（`amberice` 根再导出）

| 组 | 入口 |
|---|---|
| 写 | `dump`、`dump_with`、`dump_encrypted`、`pack`、`pack_with`、`pack_iter_with`、`pack_from_readers_with`、`Writer`（流式追加段与 `put_var`） |
| 读 | `load`、`load_encrypted`、`base_value`、`get`、`get_encrypted`、`info`、`check`、`embed_body`、`payloads`、`payload` |
| 随机访问 | `Store`（`open` / `get` / `get_with_stats` / `text` / `text_of` / `subtree_bytes` / `value` / `history_at` / `var` / `payload` / `segments` / `manifest` / `layers`）、`MappedFile` |
| 层链 | `layers`、`append_layer`、`append_layer_checked`、`verify`、`verify_trusted`、`compact`、`KeyPair` |
| 声明与自描述 | `Manifest`、`check_caps`、`check_caps_against`、`HashAlgo`、`DumpOptions` |
| 库级信封 | `sml::envelope` 的 `wrap` / `unwrap` / `list` / `sml_to_amberice` / `amberice_to_sml` / `value_to_amberice` / `value_to_amberice_with`（门面 `amberice` feature） |

`Store::open` 不构造 `Value`、不复制大块数据：打开只读头、段表、manifest，按路径下行时靠 `subtree_len` 跳过兄弟节点，只解命中的子树；补丁按顶层键合并。

### 4.2 命令行

子命令清单的唯一事实来源是 `rust/amberice/src/main.rs` 的 `USAGE` 常量：

- 读与检查：`info`、`ls`、`check`、`cat`（`--schema` 按内嵌 schema 渲染 `TYPED`）、`text`、`get`；
  以上读命令都支持 `--mmap`（D47），构建没开 `mmap` 时退回普通读取并提示
- 归档：`pack`（可选 `--zstd [N]`）、`unpack`
- 层链：`keygen`、`sign`、`append`、`compact`、`delegate`、`verify`、`tree`、`history`
- 版本：`migrate --to fmt=N`（骨架，只支持 0；有层必须 `--seed` 重签）
- 向量：`vectors --gen` / `vectors --check`
- 元信息：`--help`、`--version`

退出码：`0` 成功、`1` 失败（不匹配 / IO / canonical 错误）、`2` 用法错误。`cat` 只取当前值，`get` 是零拷贝取子树，`history --at N` 才做历史回放。`unpack` 挡目录穿越。

### 4.3 feature 分层

| 档位 | 内容 |
|---|---|
| `amberice` crate 默认 | `default = []`，只依赖 `sml-value`，零第三方依赖 |
| `preserve_order` | 兼容别名，转发到 `sml-value` 的空 feature；canonical 能否产出取决于值本身，与档位无关 |
| `sign` | Ed25519 签名 / 验签 / keygen |
| `compress` | zstd 段级压缩与 `FRAMES` |
| `crypto` | AES-256-GCM、HKDF、Argon2id |
| `mmap` | `MappedFile` 内存映射 |
| 门面 `swsml` | `amberice`、`amberice-compress`、`amberice-crypto`、`amberice-mmap`；关掉 `amberice` 时依赖树里完全没有它 |
| `amberice-ffi` | `sml` 打开写路径 `ambr_pack_text`（解析放在 FFI 层，格式 crate 保持不依赖解析器） |

### 4.4 C ABI

`rust/amberice/include/amberice.h` 声明、`rust/amberice-ffi/src/lib.rs` 实现：

| 组 | 符号 |
|---|---|
| 读 | `ambr_text`、`ambr_free_str` |
| 写（`sml` feature） | `ambr_pack_text`、`ambr_free_bytes` |
| 能力 | `ambr_caps`，位 0 = 读、位 1 = 写 |
| 宿主缓冲 | `ambr_alloc`、`ambr_dealloc`（给没有 malloc 的 wasm 宿主） |

返回 canonical 文本而不是值树：各 FFI 消费端本来就有 SML 解析器，这样 ABI 只需几个符号，结果还能与跨端向量逐字节比对。产物名是 `amberice_ffi`（`amberice` 已被纯 rlib 占用）。

## 5. 多端实现与跨端契约

| 端 | 实现 | 能力 |
|---|---|---|
| C | `c/ambr.c`、`c/ambr_write.c` | 只读解码 `sml_ambr_load`；写入经 C ABI |
| C++ | `cpp/ambr.cpp`、`cpp/ambr_write.cpp` | 同上 |
| JS | `js/ambr.mjs`、`js/ambr_write.mjs` | 只读解码；写入走 wasm 编译的 `amberice-ffi`，不重写编码器 |
| Lua | `lua/lib/ambr_pack.lua`、`ambr_write.lua` | 纯 Lua 5.3+ 写入器，与 Rust `dump` 对同一值产出相同字节；无整数运算能力时明确报告 |
| Java | `bindings/java/.../Ambr.java`、`AmbrWrite.java` | 纯 Java 只读解码（不要求本地库），另有写入器 |
| Go / C# / Ruby / Python | `bindings/go/amberice/`、`bindings/csharp/Amberice.cs`、`bindings/ruby/lib/amberice.rb`、`amberice/bindings/python/ambr.py` | 经 C ABI 做「字节进、canonical 文本出」，再用本端 SML 解析器物化值树 |

共同边界：各端原生解码器只承诺**未压缩、未加密**的 `VALUE` + `STRTAB`，其余段按表项 `len` 跳过；遇到 `comp != 0` / `enc != 0` 明确拒绝，不猜语义。绑定层不翻译错误文案，`AMBR:` 原样透传。

**摘要校验的已知边界**：各端目前只算 SHA-256（`hash_id = 0x30`）。`0x31 SM3` / `0x32 BLAKE3` 的文件在五端会报「摘要不符」——这是能力缺失，不是文件损坏。五端补 `hash_id` 分派排在 D44 后续。

跨端契约由两份生成物钉住：`amberice-canonical.v1.json` 钉节点编码，`amberice-container.v1.json` 钉整个文件（头、段表、对齐、manifest、压缩与加密用例）。用例表在 `rust/amberice/src/vectors.rs`，`amberice vectors --check` 重新生成并逐行比对；各端解码器照 `ambr_hex` 解出值与 `canonical_text` 对拍，不需要读 Rust 实现。CI 把带 `preserve_order,sign,compress` 的 `cargo test -p amberice`、四端对拍脚本、`amberice-ffi --features sml` 构建与写入门禁单列，避免被默认档合并掩盖。

## 6. 当前实现状态（代码可查）

- 两个 Rust crate 都是 `publish = false`；`EDITION = 0x0004` 硬编码在 `container.rs`，版本升级需手动跟。
- 三个摘要算法均已实现；BLAKE3 只做无密钥 32 B 摘要（无 keyed / XOF）。
- `FRAMES` 的按需解帧已接入 `Store::get` 与 `subtree_bytes`（后者对分帧结果物化为 `Owned`）。
- wasm + zstd 组合只验证过编译，压缩向量的运行时行为未验（`rust/amberice/Cargo.toml` 台账）。
- 加密与内存映射只有库接口，没有 CLI 子命令。
- smldb 没有实现代码，不属于本产品线已交付能力。
- 顶层 `amberice/` 目录只放 Python 绑定，与 `rust/amberice/` 不是一回事。

## 7. 复核命令

在 `rust/` 目录执行；本次实测记录：

| 命令 | 结果 |
|---|---|
| `cargo run -p amberice -- --version` | 输出 `amberice 0.6.9-dev.0` |
| `cargo test -p amberice --features preserve_order,sign,compress` | 全部通过（含 1 项 ignored） |
| `cargo run -p amberice --features preserve_order -- vectors --check` | 返回 1：容器向量第 5 行 `note` 文案与生成器输出差一个装饰符号；向量文件是 `vectors --gen` 的生成物，比对判据仍是这条命令 |
| `cargo run -p smldocs -- --check` | 无断链 / 无未登记 / 无超期 |
| `cargo build -p amberice-ffi --features sml` | 通过；产出带写能力的 cdylib，供 C / C++ / Python 写入门禁使用 |

## 8. 关键文件速查

| 想确认什么 | 看哪里 |
|---|---|
| 布局常量、段 id、写 / 读主流程 | `rust/amberice/src/container.rs` |
| canonical 文本与字节规则 | `rust/sml-value/src/canonical.rs` |
| 模块总表与公开 API | `rust/amberice/src/lib.rs` |
| 子命令与退出码 | `rust/amberice/src/main.rs` 的 `USAGE` |
| feature 台账与依赖方向 | `rust/Cargo.toml`、`rust/amberice/Cargo.toml`、`rust/amberice-ffi/Cargo.toml` |
| C ABI 符号 | `rust/amberice/include/amberice.h`、`rust/amberice-ffi/src/lib.rs` |
| 层链 / 签名 / 加密 / 索引 | `layer.rs`、`sign.rs`、`crypto.rs`、`dir.rs`、`typed.rs`（均在 `rust/amberice/src/`） |
| 跨端字节契约 | `rust/amberice/src/vectors.rs`、`tests/vectors/` |
