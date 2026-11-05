# smlb

不可变、可寻址、可验证的通用二进制信封；SML 是内置载荷之一。

**English below** ｜ 中文在上方

---

# 中文

`smlb`（`.smlb` 文件格式）是通用二进制信封：它把任意字节封装进带段表、摘要、manifest
与层链签名的容器里，提供不可变、可寻址、可签名校验的能力。

**为什么不是「SML 的二进制」**：容器层（段表 / manifest / 摘要 /「未知段可跳过」）从不
依赖 SML；它提供的不可变、可寻址、可签名校验是通用能力，适用于任意字节（`.dat`、日志、
存档、传输载荷等）。SML 只是内置载荷之一 —— 这样 `.smlb` 能独立于 SML 生态被采用。

## 能力

| 模块 | 提供 |
|---|---|
| `container` | 文件头 + 段表：`pack` / `load` / `dump` / `info` / `ls` / `check` |
| `hash` / `sha256` / `sm3` | 摘要（SHA-256 / SM3，零依赖，标准向量） |
| `manifest` | 自描述元数据 + 能力位 |
| `layer` | 层链（append-only 历史：`append_layer` / `verify`） |
| `sign`（feature `sign`） | ED25519 签名 / 验签 / `keygen` |
| `vectors` | 跨端测试向量（`smlb vectors --gen/--check`） |

## 安装

**完整构建**（所有能力都挂在 feature 后；`default = []` 只留只读子集）：

```bash
cargo test -p smlb --features "preserve_order,sign,compress,crypto,mmap"
# 只想要「读写都正确」的最小集合：preserve_order 一个就够
```

> ✅ 默认构建（无 feature）就能读写：`smlb pack` / `smltools --to-smlb` 都可用。
> 键序现在是**值的一部分**（`Map::Unordered` / `Map::Ordered` 两个显式态），不是编译开关。
> - 文本输入：`parse_ordered()` 保源序；`parse()` 保持历史字节序（不能签 canonical）
> - 程序构造：`Map::ordered()` / `Map::unordered()` / `OrderedMap` 显式二选一
> - 遇到无序对象，`dump` 报 `key-order-unavailable` 并**指路上面这些入口**，开 feature 不解决
>
> 过渡期 `preserve_order` 只影响 `Map::new()` 的默认态，不影响「能不能签」；待调用点
> 全部显式化后即删除。

尚未发布（`publish = false`，随 SML v5 一起开放）。当前用 path 依赖：

```toml
[dependencies]
smlb = { path = "../smlb" }
# 需要签名时启用：
# smlb = { path = "../smlb", features = ["sign"] }
```

## 三条不变量

1. 未知段可跳过（长度自描述，不许「读前先跑解释器」）。
2. 签名签 `canonical_text`（与 `fmt` 解耦，历史签名不因布局升级而失效）。
3. 追加不改旧字节（`compact` 是唯一例外且必须重签）。

## 许可

MulanPSL-2.0

---

# English

`smlb` (the `.smlb` file format) is a general-purpose binary envelope: it wraps arbitrary bytes
into a container with a segment table, digests, a manifest, and a layer-chain signature,
providing immutability, addressability, and verifiable signing.

**Why not "the binary form of SML"**: the container layer (segment table / manifest / digests /
skippable unknown segments) never depended on SML; what it provides is generic and applies to
any bytes (`.dat`, logs, archives, transport payloads). SML is just one built-in payload — this
lets `.smlb` be adopted independently of the SML ecosystem.

## Capabilities

| Module | Provides |
|---|---|
| `container` | file header + segment table: `pack` / `load` / `dump` / `info` / `ls` / `check` |
| `hash` / `sha256` / `sm3` | digests (SHA-256 / SM3, zero-dep, standard vectors) |
| `manifest` | self-describing metadata + capabilities |
| `layer` | layer chain (append-only history: `append_layer` / `verify`) |
| `sign` (feature `sign`) | ED25519 sign / verify / `keygen` |
| `vectors` | cross-end test vectors (`smlb vectors --gen/--check`) |

## Installation

Not yet published (`publish = false`, opens with SML v5). Use a path dependency for now:

```toml
[dependencies]
smlb = { path = "../smlb" }
# Enable for signing:
# smlb = { path = "../smlb", features = ["sign"] }
```

## Three invariants

1. Unknown segments are skippable (length is self-describing; no "run an interpreter first").
2. Signatures sign `canonical_text` (decoupled from `fmt`; old signatures survive layout upgrades).
3. Append never rewrites old bytes (`compact` is the only exception and must re-sign).

## License

MulanPSL-2.0
