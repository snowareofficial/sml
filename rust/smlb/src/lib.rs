// SPDX-License-Identifier: MulanPSL-2.0
//! `.smlb` — the **binary form** of SML. At T0 this crate ships exactly two things:
//! `.smlb` —— SML 的**二进制形式**。本 crate 在 T0 只交付两件事：
//!
//! 1. The canonical form — **implemented in `sml-value`** (see
//!    [`sml_value::Value::canonical_text`] / [`sml_value::Value::canonical_bytes`]); only
//!    re-exported here. ｜ canonical 形态（实现在 `sml-value`，这里只做门面再导出）。
//! 2. The cross-end test vectors ([`vectors`]): `smlb vectors --gen/--check`.
//!    ｜ 跨端测试向量。
//!
//! Build order: `docs/smlb-smldb-spec.md` §17 / §23.4 —
//! **T0 canonical + vectors → T1 encoder → T2 container → T3 read / T4 layer chain / T5 crypto
//! → T6 tools → T7 five ends**. T1 lands the encoder skeleton (41 B segment header, `STRTAB`
//! placement) here; T0 deliberately touches no container, because “are the bytes stable?” has
//! to be answered before “what shell do they go in?”.
//! 施工顺序同上。T1 才落「编码器骨架 + 段头（41 B）+ `STRTAB` 落点」；T0 刻意不碰容器。
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
//! `sml-value`** — those are this crate's job from T2 on. D3 (“`.smlb` does not depend on
//! `sml-parse`”) is untouched.
//! 代价与边界：`sml-value` 因此知道节点线格式（与 D2 同向，仍零第三方依赖）；容器层一概
//! 不进 `sml-value`，D3 不受影响。
//!
//! # Three invariants (§23.1 — no change may break them) / 三条不变量
//!
//! 1. **Unknown segments are skippable** — length is self-describing; no “run an interpreter
//!    before reading”. ｜ **未知段可跳过** —— 长度自描述，不允许「读前先跑解释器」。
//! 2. **Signatures cover `canonical_text`** — decoupled from `fmt`, so a byte-layout upgrade
//!    never invalidates historical signatures. ｜ **签名签 `canonical_text`**。
//! 3. **Appends never rewrite old bytes** — `compact` is the only exception and must re-sign.
//!    ｜ **追加不改旧字节**。
//!
//! T0 only touches the precondition of #2 (stable canonical); #1 and #3 gain substance at T2/T4.

pub mod vectors;

pub use sml_value::canonical::CanonicalError;
pub use sml_value::{Map, Value};
