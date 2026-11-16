// SPDX-License-Identifier: MulanPSL-2.0
//! no_std relay point for a byte envelope (feature `amberice`).
//! no_std 字节信封接力点（feature `amberice`）。
//!
//! This trait is a **contract only**: no `amberice` dependency is wired in yet. Once
//! P16 (no_std sml-value) and P17 (no_std amberice subset) land, an adapter can
//! implement it without changing smlite's core.
//! 本 trait 只是**契约**：尚未接入 `amberice` 实依赖。待 P16（no_std sml-value）与
//! P17（no_std amberice 子集）落地后，适配器可在不改 smlite 核心的前提下实现它。

use alloc::vec::Vec;

use crate::error::SmliteError;

/// A byte envelope that wraps / unwraps a payload. ｜ 包装 / 解包载荷的字节信封。
pub trait ByteEnvelope {
    /// Wrap raw bytes into an envelope. ｜ 把原始字节包进信封。
    fn wrap(bytes: &[u8]) -> Result<Vec<u8>, SmliteError>;

    /// Unwrap an envelope back to raw bytes. ｜ 把信封解回原始字节。
    fn unwrap(envelope: &[u8]) -> Result<Vec<u8>, SmliteError>;
}
