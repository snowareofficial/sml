// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! `.ambr` 容器对接的**契约**（feature `amberice`）：只定接口，不接实依赖。
//! Contract for the `.ambr` container (feature `amberice`): interface only, no real
//! dependency wired in.
//!
//! 为什么只有接口：`amberice` 的 no_std 子集（P17）与 `sml-value` 的 no_std（P16）都还没
//! 落地，容器现在只提供 std 构建；提前把依赖接进来会让本 crate 的核心承诺（`no_std` +
//! 零硬依赖）失效。等两边就绪后，由**本 crate 自己的适配器**实现这个 trait，核心代码
//! 一行都不用改。
//! Why interface only: the no_std subset of `amberice` (P17) and of `sml-value` (P16) are
//! not ready; wiring the dependency now would break this crate's no_std / zero-dependency
//! core. Once both land, an adapter inside this crate implements the trait and the core
//! stays untouched.
//!
//! 两条边界，别越界：
//! Two boundaries to keep:
//!
//! 1. **不经 smlite**：对接在 eclog-rs 侧完成，smlite 的值模型不参与。
//!    No detour through smlite: the adapter lives here.
//! 2. **不定义私有二进制格式**：容器不认识 eclog 文本语法（见
//!    `docs/amberice/plan/nostd-subset-eval.md` §6.3），值树的编码由适配器决定 ——
//!    在 `sml-value` 就绪前自行发明一套字节格式，只会和将来那份重复。
//!    No private binary format: the container does not know the eclog syntax, and the
//!    value encoding is the adapter's business; inventing one now would duplicate the
//!    future `sml-value` representation.
//!
//! 已知的损失面（实现适配器时必须显式决定）：`Value::Number` 的 `raw` 与
//! `Str` 的 `notation` / `delimiter` 是 eclog 特有的表现层信息，`sml-value` 的
//! canonical 表示不承载它们 —— 往返后渲染字节不再逐字节相同，只有值相等。
//! Known loss: `Number::raw` and `Str`'s notation/delimiter are eclog-specific
//! presentation data that a canonical `sml-value` form does not carry; a round trip
//! preserves values, not rendered bytes.

use alloc::vec::Vec;

use crate::value::Value;

/// 值树 ↔ `.ambr` 字节的适配器契约。
/// Adapter contract between the value tree and `.ambr` bytes.
///
/// 对应 amberice 字节面的两组入口：写侧 `dump_with` / `pack_with`（`&[u8] -> Vec<u8>`），
/// 读侧 `load` / `get`（`&[u8] -> 值`）。
/// Mirrors amberice's two byte-face groups: the write side (`dump_with` / `pack_with`)
/// and the read side (`load` / `get`).
///
/// ```
/// use eclog::amberice::AmbericeBridge;
/// use eclog::{parse, to_typical, Value};
///
/// /// 最小的退化适配器：载荷就是 eclog 自己的文本。
/// struct TextPayload;
///
/// impl AmbericeBridge for TextPayload {
///     type Error = eclog::Error;
///
///     fn dump(value: &Value) -> Result<Vec<u8>, Self::Error> {
///         Ok(to_typical(value).into_bytes())
///     }
///
///     fn load(bytes: &[u8]) -> Result<Value, Self::Error> {
///         parse(bytes)
///     }
/// }
///
/// let doc = parse(b"a: 1").unwrap();
/// let bytes = TextPayload::dump(&doc).unwrap();
/// assert_eq!(TextPayload::load(&bytes).unwrap(), doc);
/// ```
pub trait AmbericeBridge {
    /// 失败类型由适配器决定（可以是 `eclog::Error`，也可以是容器自己的错误）。
    /// The error type is the adapter's choice.
    type Error;

    /// 把值树写成容器字节 ｜ write the value tree into container bytes.
    fn dump(value: &Value) -> Result<Vec<u8>, Self::Error>;

    /// 从容器字节读回值树 ｜ read the value tree back from container bytes.
    fn load(bytes: &[u8]) -> Result<Value, Self::Error>;
}
