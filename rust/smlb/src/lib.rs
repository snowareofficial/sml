// SPDX-License-Identifier: MulanPSL-2.0
//! `.smlb` — an immutable, addressable, verifiable binary envelope; SML is one of its
//! built-in payloads, not its whole identity. Shipped so far:
//! `.smlb` —— 不可变、可寻址、可验证的二进制信封；SML 只是内置载荷之一，不是全部。已交付：
//!
//! 1. **T0** — the canonical form, **implemented in `sml-value`** (see
//!    [`sml_value::Value::canonical_text`] / [`sml_value::Value::canonical_bytes`]); only
//!    re-exported here. ｜ canonical 形态（实现在 `sml-value`，这里只做门面再导出）。
//! 2. **T0** — the cross-end test vectors ([`vectors`]): `smlb vectors --gen/--check`.
//!    ｜ 跨端测试向量。
//! 3. **T1** — the container ([`container`]): file header + segment table + `dump` / `load` /
//!    `info` / `ls`. ｜ 容器：文件头 + 段表 + `dump`/`load`/`info`/`ls`。
//! 4. **T2** — digests, the manifest, capabilities:
//!    - [`sha256`] / [`sm3`] (zero-dep, standard vectors) + [`hash`] dispatch by §20 id;
//!    - [`manifest`] — `primary` / `hash_id` / `segments[]` / `caps` / defaults;
//!    - `smlb check` (structural + per-segment digests + manifest ↔ table cross-check).
//!    ｜ 摘要（SHA-256 / SM3）+ 按 §20 编号分派 + manifest + 能力位 + `check`。
//! 5. **T3 (partial)** — `get` (`smlb cat`): path lookup, correct-but-slow until `DIR` lands.
//!    ｜ `cat`：按路径取值（`DIR` 之前先做正确的慢版本）。
//!
//! Build order: `docs/smlb-smldb-spec.md` §17 / §23.4 —
//! **T0 canonical + vectors → T1 encoder → T2 container → T3 read / T4 layer chain / T5 crypto
//! → T6 tools → T7 five ends**. T0 deliberately touched no container (“are the bytes stable?”
//! before “what shell do they go in?”); T1 adds the shell; T2 makes it self-describing.
//! 施工顺序同上：T0 先把「字节稳不稳」钉死，T1 才装壳，T2 让它自描述。
//!
//! # 三条不变量（§23.1，任何改动都不得破坏）/ Three invariants
//!
//! 1. **未知段可跳过** —— 长度自描述，不允许「读前先跑解释器」；
//! 2. **签名签 `canonical_text`** —— 与 `fmt` 解耦，历史签名不因字节布局升级而失效；
//! 3. **追加不改旧字节** —— `compact` 是唯一例外，且必须重签。
//!
//! T0–T2 只碰第 2 条的前提（canonical 稳定）；第 1 条已有实体（实测未知段可跳过）；
//! 第 3 条从 T4 的 `append` 才开始有东西可守。
//!
//! # Why canonical lives in `sml-value`, not here / 为什么 canonical 不在本 crate
//!
//! The spec writes the API as `Value::canonical_text()` / `Value::canonical_bytes()` (§18.1).
//! Rust's orphan rule forbids adding inherent methods to `Value` from another crate, so they
//! must live where `Value` is defined.
//! 规格把 API 写作 `Value::canonical_*`（§18.1）；孤儿规则不允许在别的 crate 里给 `Value`
//! 加固有方法，故这两个方法必须住在定义 `Value` 的 crate。
//!
//! Cost and boundary: `sml-value` therefore knows the `VALUE` node wire format (§19.2). That
//! is in line with D2 (**encode `Value`**) and it stays free of third-party deps. **Container
//! concerns (segment table / manifest / layer chain / signatures / crypto) never enter
//! `sml-value`** — those are this crate's job from T1 on. D3 (“`.smlb` does not depend on
//! `sml-parse`”) is untouched.
//! 代价与边界：`sml-value` 因此知道节点线格式（与 D2 同向，仍零第三方依赖）；容器层一概
//! 不进 `sml-value`，D3 不受影响。

pub mod container;
pub mod hash;
pub mod layer;
pub mod manifest;
pub mod sha256;
#[cfg(feature = "sign")]
pub mod sign;
pub mod sm3;
pub mod vectors;

pub use container::{
    align_of, base_value, check, dump, dump_with, embed_body, get, hash_algo, info, load, pack, pack_with, payload,
    payloads, CheckReport, ContainerError, DumpOptions, FileInfo, Segment, SegmentHeader,
    ALIGN_BLOCK, ALIGN_MIN, EDITION, FLAG_ALIGN_BLOCK, FMT, HEADER_LEN, MAGIC, SEG_EMBED,
    SEG_HEADER_LEN, SEG_MANIFEST, SEG_STRTAB, SEG_VALUE,
};
pub use hash::{HashAlgo, HashError};
pub use layer::{
    layers, ChainReport, Delegation, Layer, LayerError, SigEntry, Trailer, LAYER_HEADER_LEN,
    TRAILER_LEN, TRAILER_MAGIC,
};
#[cfg(feature = "sign")]
pub use sign::{append_layer, verify, KeyPair, SignError, VerifyReport, ED25519};
pub use manifest::{
    check_caps, check_caps_against, CapsVerdict, Manifest, ManifestError, ManifestPayload,
    ManifestSegment, DEFAULT_CRYPTO_BLOCK, DEFAULT_DIR_DEPTH, DEFAULT_PAD_QUANTUM,
    DEFAULT_PATCH_STRATEGY,
};
pub use sml_value::canonical::CanonicalError;
pub use sml_value::{CanonicalBlocks, Map, Value, MAX_VALUE_DEPTH};
