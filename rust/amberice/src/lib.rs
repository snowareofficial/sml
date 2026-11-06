// SPDX-License-Identifier: MulanPSL-2.0
//! `.ambr` — an immutable, addressable, verifiable binary envelope.
//! `.ambr` —— 不可变、可寻址、可验证的通用二进制信封。
//!
//! # 定位 / Positioning
//!
//! `.ambr` 是通用二进制信封，SML 只是内置载荷之一、不是全部。原因：容器层（段表 /
//! manifest / 摘要 /「未知段可跳过」）从不依赖 SML；它提供的不可变、可寻址、可签名校验
//! 是通用能力，适用于任意字节（`.dat`、日志、存档、传输载荷等）。把 SML 从「唯一身份」
//! 降为「载荷之一」，是为了让 `.ambr` 能独立于 SML 生态被采用，而非被绑定在文本格式上。
//!
//! `.ambr` is a general-purpose binary envelope; SML is one built-in payload, not the whole
//! identity. Why: the container layer (segment table / manifest / digests / skippable unknown
//! segments) never depended on SML; what it provides — immutability, addressability, verifiable
//! signing — is generic and applies to any bytes (`.dat`, logs, archives, transport payloads).
//! Demoting SML from "the identity" to "a payload" lets `.ambr` be adopted independently of
//! the SML ecosystem.
//!
//! # 已交付 / Shipped
//!
//! - **T0** canonical（实现在 [`sml_value::Value::canonical_text`]，本 crate 再导出）
//!   + 跨端测试向量（[`vectors`]: `amberice vectors --gen/--check`）。
//! - **T1** 容器（[`container`]）：文件头 + 段表 + `dump` / `load` / `info` / `ls`。
//! - **T2** 摘要与 manifest（[`sha256`] / [`sm3`] / [`hash`] / [`manifest`]）+ `amberice check`。
//! - **T3**（部分）路径取值：`amberice cat`（`DIR` 落地前为「正确但慢」的版本）。
//! - **T4/T5** 层链与签名（[`layer`] / [`sign`]，`sign` 需 feature）：ED25519 + append-only 层。
//!
//! # 三条不变量 / Three invariants (§23.1)
//!
//! 1. 未知段可跳过 —— 长度自描述，不许「读前先跑解释器」。
//! 2. 签名签 `canonical_text` —— 与 `fmt` 解耦，历史签名不因布局升级而失效。
//! 3. 追加不改旧字节 —— `compact` 是唯一例外且必须重签。
//!
//! # 为什么 canonical 住在 `sml-value` / Why canonical lives in `sml-value`
//!
//! 规格把 API 写作 `Value::canonical_*`（§18.1）；孤儿规则不允许在别的 crate 给 `Value`
//! 加固有方法，故实现住在定义 `Value` 的 `sml-value`，本 crate 只做门面再导出（符合 D2，
//! 仍零第三方依赖）。容器层（段表 / manifest / 层链 / 签名 / 加密）一概不进 `sml-value`。

pub mod container;
pub mod crypto;
pub mod hash;
pub mod layer;
pub mod manifest;
pub mod sha256;
pub mod store;
#[cfg(feature = "sign")]
pub mod sign;
/// 内存映射读取（T3 的 RSS 判据），feature `mmap`。
/// Memory-mapped reading, behind the `mmap` feature.
#[cfg(feature = "mmap")]
pub mod mmap;
pub mod sm3;
pub mod vectors;

#[cfg(feature = "crypto")]
pub use crypto::{KdfParams, KeySource, ENC_AES256_GCM, KDF_ARGON2ID, KDF_NONE};
/// 键序已证明的值（TryFrom<Value>）—— dump_canonical 的入参。
pub use sml_value::CanonicalValue;
pub use container::{
    align_of, base_value, base_value_with_keys, check, dump, dump_with, dump_encrypted,
    embed_body, get, get_encrypted, hash_algo, info, load, load_encrypted, pack,
    pack_from_readers, pack_from_readers_with, pack_iter,
    pack_iter_with, pack_with,
    payload,
    payloads, CheckReport, ContainerError, DumpOptions, FileInfo, Segment, SegmentHeader, Writer,
    ALIGN_BLOCK, ALIGN_MIN, COMP_ZSTD, EDITION, FLAG_ALIGN_BLOCK, FMT, HEADER_LEN, MAGIC,
    frame_bytes_at, inflate_frames_all, manifest_of, parse_frame_index, FrameEntry,
    FRAME_ENTRY_LEN,
    MAX_DECOMPRESSED, SEG_EMBED, SEG_FRAMES, SEG_HEADER_LEN, SEG_MANIFEST, SEG_STRTAB,
    SEG_VALUE,
};
pub use hash::{HashAlgo, HashError};
pub use layer::{
    layers, ChainReport, Delegation, Layer, LayerError, SigEntry, Trailer, LAYER_HEADER_LEN,
    TRAILER_LEN, TRAILER_MAGIC,
};
#[cfg(feature = "sign")]
pub use sign::{
    append_layer, append_layer_checked, compact, verify, verify_trusted, KeyPair, SignError,
    VerifyReport, ED25519,
};
pub use manifest::{
    check_caps, check_caps_against, CapsVerdict, Manifest, ManifestError, ManifestPayload,
    ManifestSegment, DEFAULT_CRYPTO_BLOCK, DEFAULT_DIR_DEPTH, DEFAULT_PAD_QUANTUM,
    DEFAULT_PATCH_STRATEGY,
};
pub use store::Store;
#[cfg(feature = "mmap")]
pub use mmap::MappedFile;
pub use sml_value::canonical::CanonicalError;
pub use sml_value::{subtree_at, value_from_subtree, Path, Subtree};
pub use sml_value::{CanonicalBlocks, Map, Value, MAX_VALUE_DEPTH};
