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

// 两档：`std`（默认，全量）与 `no_std`（子集，跑在 `core + alloc` 上）。
// 评价与裁剪边界见 `docs/amberice/plan/nostd-subset-eval.md`。
//
// 条件里带 `not(feature = "std")` 是为了「两个开关同时打开」时只报下面那条 compile_error，
// 不再叠 9 条 `cannot find crate std` 的噪音（否则调用方要先从噪音里找出真正的原因）。
// ｜ `not(std)` keeps the contradictory-combo diagnosis to a single error.
#![cfg_attr(all(feature = "no_std", not(feature = "std")), no_std)]

extern crate alloc;

// `std` 与 `no_std` 同时打开 = 调用方忘了 `--no-default-features`。Cargo 的 feature 只能加
// 不能减，所以这里必须显式拦住：否则会静默编出一个「说是 no_std、其实链了 std」的产物。
// ｜ Cargo features are additive; reject the contradictory combination loudly.
#[cfg(all(feature = "std", feature = "no_std"))]
compile_error!(
    "`std` 与 `no_std` 互斥：no_std 构建请用 `--no-default-features --features no_std`。\n\
     `std` and `no_std` are mutually exclusive; pass `--no-default-features --features no_std`."
);

// 这四组可选依赖与 no_std 先天冲突：`sign` 要 getrandom 熵源、`compress` 是 zstd 的 C 绑定、
// `crypto` 的 aes-gcm / argon2 / hkdf 只评估过 std 档、`mmap` 的 memmap2 没有 no_std。
// 后续要带进 no_std，须单独评估（P17 评估 §7 最后一行的待办）。
// ｜ These optional deps have no no_std story yet; reject rather than fail obscurely.
#[cfg(all(feature = "no_std", any(feature = "compress", feature = "mmap")))]
compile_error!(
    "`no_std` 子集不含 `compress`（zstd 是 C 绑定，要 libc）与 `mmap`（memmap2 编不了）。\
     只解压用 `compress-pure`（纯 Rust 的 ruzstd）；`mmap` 本来就不需要 ——\
     no_std 下直接把 Flash / DMA 缓冲当 `&[u8]` 传给 `Store::open`，零拷贝路径相同。\
     `sign` / `crypto` 可用 —— 熵源由调用方给（`KeyPair::from_seed` / `dump_encrypted_with_salt`）。"
);

/// crate 内 prelude：两种档位共用，让模块正文不必区分 `std::` / `alloc::`。
///
/// 为什么需要：`no_std` 下 `Vec` / `String` / `Box` / `Cow` 与 `format!` 都不在 prelude 里；
/// `std` 构建拿到的又是同一批类型（`std` 只是 `alloc` 的超集），所以一处列全，
/// 两种档位的模块代码逐字相同。｜ Shared prelude so module bodies are tier-agnostic.
#[allow(unused_imports)]
pub(crate) mod prelude {
    pub use alloc::borrow::{Cow, ToOwned};
    pub use alloc::boxed::Box;
    pub use alloc::format;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
}

/// 惰性一次性容器（只用 `new` / `get` / `get_or_init`）：std 档 `OnceLock`，
/// no_std 档退到 `core::cell::OnceCell`（`core` 里没有 `OnceLock`，实测 `E0432`）。
///
/// **能力差异**：`OnceCell` 不是 `Sync`，故 no_std 档的 [`store::Store`] 默认失去 `Sync`。
///
/// 要把它要回来就开 `embassy` feature：后端换成 `embassy_sync::once_lock::OnceLock`
/// （`Sync` 以 `T: Sync` 为条件），代价是目标必须提供 `critical-section` 实现 ——
/// embassy 的芯片 crate 会提供，裸机自己搭则要挂 `critical-section` 的实现 crate。
/// 宁可多一个可选依赖，也不自己写 `UnsafeCell` + 原子状态位的自旋锁：那是本 crate
/// 唯一可能引入 UB 的地方，不值得为省一个 feature 去冒。
/// ｜ Default no_std uses `OnceCell` (`!Sync`); the `embassy` feature restores `Sync`.
#[cfg(not(feature = "no_std"))]
pub(crate) struct Lazy<T>(std::sync::OnceLock<T>);
#[cfg(all(feature = "no_std", not(feature = "embassy")))]
pub(crate) struct Lazy<T>(core::cell::OnceCell<T>);
#[cfg(all(feature = "no_std", feature = "embassy"))]
pub(crate) struct Lazy<T>(embassy_sync::once_lock::OnceLock<T>);

impl<T> Lazy<T> {
    #[inline]
    pub(crate) fn new() -> Self {
        #[cfg(not(feature = "no_std"))]
        {
            Lazy(std::sync::OnceLock::new())
        }
        #[cfg(all(feature = "no_std", not(feature = "embassy")))]
        {
            Lazy(core::cell::OnceCell::new())
        }
        #[cfg(all(feature = "no_std", feature = "embassy"))]
        {
            Lazy(embassy_sync::once_lock::OnceLock::new())
        }
    }

    #[inline]
    pub(crate) fn get(&self) -> Option<&T> {
        #[cfg(not(feature = "no_std"))]
        {
            self.0.get()
        }
        #[cfg(all(feature = "no_std", not(feature = "embassy")))]
        {
            self.0.get()
        }
        #[cfg(all(feature = "no_std", feature = "embassy"))]
        {
            self.0.try_get()
        }
    }

    #[inline]
    pub(crate) fn get_or_init<F: FnOnce() -> T>(&self, f: F) -> &T {
        self.0.get_or_init(f)
    }

    /// 放一个值进去；已有值时把传入值退回（三种后端语义一致）。
    #[inline]
    pub(crate) fn set(&self, value: T) -> Result<(), T> {
        #[cfg(not(feature = "no_std"))]
        {
            self.0.set(value)
        }
        #[cfg(all(feature = "no_std", not(feature = "embassy")))]
        {
            self.0.set(value)
        }
        #[cfg(all(feature = "no_std", feature = "embassy"))]
        {
            self.0.init(value)
        }
    }
}

/// `Store` 有 `#[derive(Debug)]`，故缓存字段要能 Debug（三种后端都有 Debug，但都是
/// 「包装类型 + 内部值」的形状，这里统一成 `Lazy(值)` / `Lazy(<uninit>)`）。
impl<T: core::fmt::Debug> core::fmt::Debug for Lazy<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.get() {
            Some(v) => f.debug_tuple("Lazy").field(v).finish(),
            None => f.write_str("Lazy(<uninit>)"),
        }
    }
}

/// 编译期钉子：开了 `embassy` 后 `Store` 与 `Value` 必须重新是 `Sync` ——
/// 否则这个 feature 就白加了（它唯一的目的就是让这两者能进 `static` / 共享状态）。
#[cfg(all(feature = "no_std", feature = "embassy"))]
const _: fn() = || {
    fn assert_sync<T: Sync>() {}
    assert_sync::<store::Store<'static>>();
    assert_sync::<sml_value::Value>();
};

/// 内部键索引表：std 档 `HashMap`，no_std 档 `BTreeMap`（`alloc` 没有 `HashMap`）。
///
/// 只用于私有缓存（`Store` 的键→id 表、`TYPED` 的 schema 表），不进任何公开签名，
/// 故换实现不改变 API；代价是 no_std 档的查找从 O(1) 变成 O(log n)。
/// ｜ Internal key index: `HashMap` under std, `BTreeMap` under no_std.
#[cfg(not(feature = "no_std"))]
pub(crate) type KeyMap<K, V> = std::collections::HashMap<K, V>;
#[cfg(feature = "no_std")]
pub(crate) type KeyMap<K, V> = alloc::collections::BTreeMap<K, V>;

/// 预分配容量的键索引表（`BTreeMap` 没有 `with_capacity`，忽略该提示即可）。
/// ｜ Pre-sized key map; `BTreeMap` has no `with_capacity`.
#[cfg(not(feature = "no_std"))]
pub(crate) fn new_key_map<K, V>(cap: usize) -> KeyMap<K, V> {
    std::collections::HashMap::with_capacity(cap)
}
#[cfg(feature = "no_std")]
pub(crate) fn new_key_map<K, V>(_cap: usize) -> KeyMap<K, V> {
    alloc::collections::BTreeMap::new()
}

pub mod blake3;
pub mod container;
pub mod crypto;
/// `DIR`（0x03）路径索引：派生数据，缺失/过期只影响速度（N05 / §19.3）。
/// `DIR` path index: derived data; missing or stale only costs speed (N05 / §19.3).
pub mod dir;
/// `.ambr` 统一错误类型（稳定码 `E-AMBR-*` + 机器可判类别）。
/// ｜ Unified error type with stable codes and a machine-readable kind.
pub mod error;
pub mod hash;
#[cfg(not(feature = "no_std"))]
pub mod ingest;
pub mod layer;
pub mod manifest;
pub mod sha256;
pub mod store;
#[cfg(feature = "sign")]
pub mod sign;
/// 流式读取：按需读段 / 载荷，不整份载入。绑 `std::io::{Read, Seek}`，no_std 档不编。
/// ｜ Streaming reader: needs `std::io`; not compiled under `no_std`.
#[cfg(not(feature = "no_std"))]
pub mod stream;
/// 内存映射读取（T3 的 RSS 判据），feature `mmap`。
///
/// **no_std 档不编**：内存映射是 OS 能力，裸机上没有可映射的对象。
/// 该档的等价物是 `Store::open(&[u8])` —— 宿主把 Flash / DMA 缓冲当切片传进来即可，
/// 零拷贝的读取路径与 `MappedFile` 完全一样。
/// ｜ Not compiled under `no_std`: pass a slice to `Store::open` instead.
#[cfg(all(feature = "mmap", not(feature = "no_std")))]
pub mod mmap;
pub mod sm3;
/// `TYPED`（0x70）强类型变量：编译期已知类型走数字 id，不写名字（N04 / §25）。
/// `TYPED` (0x70) typed variables: numeric ids instead of names when the type is known.
pub mod redact;
pub mod typed;
/// 跨端测试向量：`CASES` / `render` / `verify_self` 是纯计算（两档都可用，嵌入式可拿
/// `verify_self()` 自检）；`generate` / `check` 等读写文件的部分属 std 档。
/// ｜ Cross-end vectors: the table and self-check are pure; the file I/O half is std-tier.
pub mod vectors;

pub use crypto::{FileKeys, KdfParams, KeySource};
pub use error::AmbrError;
/// 键序已证明的值（TryFrom<Value>）—— dump_canonical 的入参。
pub use sml_value::CanonicalValue;
/// 用户级动作与类型；段布局常量、帧工具、段头结构见 [`container`]。
/// ｜ User-level actions and types; segment layout symbols live in [`container`].
pub use container::{
    base_value, base_value_with_keys, check, dump, dump_with, dump_encrypted, embed_body, get,
    get_encrypted, info, load, load_encrypted, pack, pack_iter, pack_iter_with, pack_with, payload,
    payloads, CheckReport, ContainerError, DumpOptions, FileInfo,
};
/// 调用方自备每文件盐的加密写入口：两种档位都可用（no_std 的唯一加密写法）。
/// ｜ Encrypt with a caller-supplied per-file salt; available in both tiers.
#[cfg(feature = "crypto")]
pub use container::dump_encrypted_with_salt;
/// `Writer` 与 `pack_from_readers*` 绑 `std::io` / 临时文件，属 std 档。
/// ｜ `Writer` / `pack_from_readers*` are std-tier (std::io + temp files).
#[cfg(not(feature = "no_std"))]
pub use container::{pack_from_readers, pack_from_readers_with, Writer};
/// `SOURCE`（0x50）原文写入（D46）：只在 `source` feature 下存在，且必须显式调用。
#[cfg(feature = "source")]
pub use container::dump_with_source;
pub use hash::HashAlgo;
/// 落盘追加（原子写）：fs / path / Instant / process::id 全链，属 std 档。
/// ｜ Ingest is std-tier (fs / path / time / pid).
#[cfg(not(feature = "no_std"))]
pub use ingest::{
    Commit, History, IngestError, IngestOptions, Ingestor, PushOutcome, Recovery,
};
pub use layer::{append_unsigned, layers};
pub use redact::{
    RedactAction, RedactError, RedactRule, RedactRules, RedactViolation,
};
#[cfg(feature = "sign")]
pub use sign::{
    append_layer, append_layer_checked, compact, verify, verify_trusted, verify_trusted_with,
    verify_with, KeyPair, SignError, VerifyReport, ED25519,
};
pub use manifest::{
    check_caps, check_caps_against, CapStatus, CapabilityRegistry, CapsVerdict, Manifest,
    ManifestError, ManifestPayload, ManifestSegment,
};
pub use store::{Store, SubtreeBytes};
#[cfg(all(feature = "mmap", not(feature = "no_std")))]
pub use mmap::MappedFile;
pub use sml_value::canonical::CanonicalError;
pub use sml_value::{Map, Path, Value, MAX_VALUE_DEPTH};
