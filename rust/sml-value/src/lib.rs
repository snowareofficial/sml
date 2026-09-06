// SPDX-License-Identifier: MulanPSL-2.0
//! SML 值模型 —— 整个 SML 生态的**地基 crate**，零依赖。
//!
//! 只承载三件事：
//!
//! 1. [`Value`]：7 种类型的枚举（`Null`/`Bool`/`Int`/`Float`/`Str`/`Array`/`Object`）；
//! 2. `to_sml`（feature `sml`）：把 `Value` 渲染回 SML 文本；
//! 3. serde 桥接（feature `serde`）：`Value` 的 `Serialize`/`Deserialize`。
//!
//! 后两者必须留在本 crate：**孤儿规则**要求 `impl Display for Value`、
//! `impl Serialize for Value` 与 `Value` 同 crate，而它们分别依赖
//! `to_sml` 与类型描述函数。
//!
//! # 为什么要独立成 crate
//!
//! 解析器、契约、emit 后端、C-ABI 乃至 Crystalic（布局层）都只需要
//! `Value`。若 `Value` 与它们同 crate，下游就被迫拖入全部代码
//! （过程宏、C-ABI 导出、六个转译后端）。独立后：
//!
//! ```toml
//! sml-value = "0.5"                                   # 只要值模型（嵌入式）
//! sml-value = { version = "0.5", features = ["sml"] } # + 序列化回 SML
//! ```

mod value;
#[cfg(feature = "sml")]
mod dump;
#[cfg(feature = "serde")]
mod serde_bridge;

pub use value::{describe_value, MAX_VALUE_DEPTH, Value};

#[cfg(feature = "sml")]
pub use dump::to_sml;

/// serde 桥接函数（`Value` ↔ 任意 serde 类型）。
#[cfg(feature = "serde")]
pub mod serde {
    pub use crate::serde_bridge::{from_value, to_string, to_value};
}
