// SPDX-License-Identifier: MulanPSL-2.0
//! The SML value model — the **foundation crate** of the whole SML ecosystem, zero-dependency.
//! SML 值模型 —— 整个 SML 生态的**地基 crate**，零依赖。
//!
//! It carries exactly three things: / 只承载三件事：
//!
//! 1. [`Value`]: the 7-variant enum (`Null`/`Bool`/`Int`/`Float`/`Str`/`Array`/`Object`);
//!    [`Value`]：7 种类型的枚举（`Null`/`Bool`/`Int`/`Float`/`Str`/`Array`/`Object`）；
//! 2. `to_sml` (feature `sml`): render a `Value` back to SML text;
//!    `to_sml`（feature `sml`）：把 `Value` 渲染回 SML 文本；
//! 3. the serde bridge (feature `serde`): `Serialize`/`Deserialize` for `Value`.
//!    serde 桥接（feature `serde`）：`Value` 的 `Serialize`/`Deserialize`。
//!
//! The latter two must live here: the **orphan rule** requires `impl Display for Value`,
//! `impl Serialize for Value` and `Value` to be in the same crate, and each of them
//! depends on `to_sml` and the type-description function respectively.
//! 后两者必须留在本 crate：**孤儿规则**要求 `impl Display for Value`、
//! `impl Serialize for Value` 与 `Value` 同 crate，而它们分别依赖
//! `to_sml` 与类型描述函数。
//!
//! # Why a separate crate / # 为什么要独立成 crate
//!
//! The parser, contracts, emit backends, C-ABI and even Flow (layout) all need only
//! `Value`. If it shared a crate with them, downstream crates would be forced to drag
//! in everything (proc macros, C-ABI exports, six translation backends). Standalone:
//! 解析器、契约、emit 后端、C-ABI 乃至 Flow（布局层）都只需要
//! `Value`。若 `Value` 与它们同 crate，下游就被迫拖入全部代码
//! （过程宏、C-ABI 导出、六个转译后端）。独立后：
//!
//! ```toml
//! sml-value = "0.1.0-alpha.3"                                   # 只要值模型（嵌入式）
//! sml-value = { version = "0.1.0-alpha.3", features = ["sml"] } # + 序列化回 SML
//! ```

/// 读/写共用的路径语法（`a.b[0].c`）：`Path::parse` + `Value::get_path`。
/// Path syntax shared by readers and writers; `Value::get` keeps its legacy semantics.
pub mod path;

mod value;
// 保序对象（docs/value-layout.md §3/§5）：只在 `preserve_order` 下编译，
// 未开时 `Map` 就是 `BTreeMap`，本模块不参与构建 —— 不额外占编译时间与体积。
#[cfg(feature = "preserve_order")]
mod object;
#[cfg(feature = "sml")]
mod dump;
#[cfg(feature = "serde")]
mod serde_bridge;
// 直出文本的 serde Serializer：需要 serde（写）与 sml（排版原语）两个 feature。
#[cfg(all(feature = "serde", feature = "sml"))]
mod ser_text;

pub use path::{Path, PathError, Seg};
pub use value::{describe_value, insert_or_merge, Map, MAX_VALUE_DEPTH, Value};

/// The order-preserving object (`Value::Object` bearer under the `preserve_order` feature).
/// 保序对象（`Value::Object` 在 `preserve_order` 档下的载体）。
///
/// This type does **not** exist in the default profile — `Map` is simply `BTreeMap` there.
/// Downstream code should always write `Map` so the same source compiles under both profiles.
/// 默认档**不存在**这个类型 —— 那时 `Map` 就是 `BTreeMap`。下游请一律写 `Map`，
/// 这样两种档位下是同一份源码。
#[cfg(feature = "preserve_order")]
pub use object::Object;

#[cfg(feature = "sml")]
pub use dump::{to_sml, to_sml_canonical, to_sml_checked, to_sml_with, DumpOptions};

/// The serde bridge functions (`Value` ↔ any serde type).
/// serde 桥接函数（`Value` ↔ 任意 serde 类型）。
#[cfg(feature = "serde")]
pub mod serde {
    pub use crate::serde_bridge::{from_value, to_string, to_value};

    /// Emit SML text directly (bypassing `Value`, keeping declaration order, supporting i128 / u128).
    /// 直出 SML 文本（不经 `Value` 中转，保留声明顺序、支持 i128 / u128）。
    #[cfg(feature = "sml")]
    pub use crate::ser_text::to_string_direct;
}