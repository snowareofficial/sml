// SPDX-License-Identifier: MulanPSL-2.0
//! Value-layer interop with `sml_value` (feature `swsml-compat`).
//! 与 `sml_value` 的值层互转（feature `swsml-compat`）。
//!
//! Implemented here (in the smlite crate) rather than in sml-value: the local type is
//! smlite's `Value`, so both `From<Value> for sml_value::Value` and
//! `TryFrom<sml_value::Value> for Value` satisfy the orphan rule.
//! 实现放在 smlite 侧而非 sml-value：本地类型是 smlite 的 `Value`，两个方向都满足孤儿规则。

use alloc::string::String;
use alloc::vec::Vec;

use sml_value as sv;

use crate::error::{ConversionError, ErrorKind, SmliteError};
use crate::value::{Object, Value, MAX_VALUE_DEPTH};

/// Convert an smlite value into the sml-value value model.
/// 把 smlite 值转成 sml-value 值模型。
///
/// Objects become `Map::Ordered` so the result stays canonical-signable; floats drop
/// their (absent) raw literal. ｜ 对象转成 `Map::Ordered` 以保持可签 canonical；浮点不带 raw。
impl From<Value> for sv::Value {
    fn from(value: Value) -> Self {
        to_sml_value(value, 0)
    }
}

fn to_sml_value(value: Value, depth: usize) -> sv::Value {
    if depth > MAX_VALUE_DEPTH {
        return sv::Value::Null;
    }
    match value {
        Value::Null => sv::Value::Null,
        Value::Bool(b) => sv::Value::Bool(b),
        Value::Int(i) => sv::Value::Int(i),
        Value::Float(f) => sv::Value::float(f),
        Value::Str(s) => sv::Value::Str(s),
        Value::Array(a) => sv::Value::Array(a.into_iter().map(|v| to_sml_value(v, depth + 1)).collect()),
        Value::Object(o) => {
            let mut map = sv::Map::ordered();
            for (k, v) in o.into_entries() {
                map.insert(k, to_sml_value(v, depth + 1));
            }
            sv::Value::Object(map)
        }
    }
}

/// Convert an sml-value value into an smlite value.
/// 把 sml-value 值转成 smlite 值。
///
/// Float raw literals are dropped; both `Map` states are accepted and flattened into
/// the smlite object backend. ｜ 丢弃浮点 raw；`Map` 两态都接受，压平进 smlite 对象后端。
impl TryFrom<sv::Value> for Value {
    type Error = SmliteError;

    fn try_from(value: sv::Value) -> Result<Self, Self::Error> {
        from_sml_value(&value, 0)
    }
}

impl TryFrom<&sv::Value> for Value {
    type Error = SmliteError;

    fn try_from(value: &sv::Value) -> Result<Self, Self::Error> {
        from_sml_value(value, 0)
    }
}

fn from_sml_value(value: &sv::Value, depth: usize) -> Result<Value, SmliteError> {
    if depth > MAX_VALUE_DEPTH {
        return Err(ConversionError::new(ErrorKind::ConversionDepth).into());
    }
    Ok(match value {
        sv::Value::Null => Value::Null,
        sv::Value::Bool(b) => Value::Bool(*b),
        sv::Value::Int(i) => Value::Int(*i),
        sv::Value::Float(f, _) => Value::Float(*f),
        sv::Value::Str(s) => Value::Str(String::from(s.as_str())),
        sv::Value::Array(a) => {
            let mut out = Vec::with_capacity(a.len());
            for v in a {
                out.push(from_sml_value(v, depth + 1)?);
            }
            Value::Array(out)
        }
        sv::Value::Object(m) => {
            let mut out = Object::with_capacity(m.len());
            for (k, v) in m.iter() {
                out.insert(String::from(k), from_sml_value(v, depth + 1)?);
            }
            Value::Object(out)
        }
    })
}
