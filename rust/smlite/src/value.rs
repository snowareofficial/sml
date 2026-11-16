// SPDX-License-Identifier: MulanPSL-2.0
//! The smlite value model: a compact, ordered, no-std-ready data tree.
//! smlite 值模型：紧凑、保序、可在 no_std 下使用的数据树。
//!
//! Deliberately smaller than `sml_value::Value`: `Float(f64)` carries no raw literal,
//! integers are signed-only, and objects have no reserved `__type` / `__name` keys.
//! 相比 `sml_value::Value` 刻意更轻：`Float(f64)` 不带 raw 字面量、整数只有有符号、
//! 对象没有 `__type` / `__name` 保留键。

use alloc::string::String;
use alloc::vec::Vec;

/// Maximum nesting depth; aligned with sml-value so conversions carry no depth gap.
/// 最大嵌套深度；与 sml-value 对齐，互转不引入深度差。
pub const MAX_VALUE_DEPTH: usize = 128;

/// The seven value kinds of the SML pure-data subset. ｜ SML 纯数据子集的七种值。
#[derive(Debug, Clone)]
pub enum Value {
    /// No value (`null`). ｜ 空值（`null`）。
    Null,
    /// Boolean. ｜ 布尔值。
    Bool(bool),
    /// Signed 64-bit integer; no unsigned type. ｜ 有符号 64 位整数；无无符号类型。
    Int(i64),
    /// IEEE-754 double, no raw literal. ｜ 双精度浮点，不带原始字面量。
    Float(f64),
    /// UTF-8 string. ｜ UTF-8 字符串。
    Str(String),
    /// Ordered sequence of values. ｜ 有序值序列。
    Array(Vec<Value>),
    /// Object; key order per the `object-sort` feature (default: insertion order).
    /// 对象；键序由 `object-sort` feature 决定（默认插入序）。
    Object(Object),
}

impl Value {
    /// View as bool, or `None`. ｜ 取布尔值，非布尔返回 `None`。
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// View as signed integer, or `None`. ｜ 取有符号整数，类型不符返回 `None`。
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// View as float, or `None`. ｜ 取浮点数，类型不符返回 `None`。
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// View as string slice, or `None`. ｜ 取字符串切片，类型不符返回 `None`。
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    /// View as array slice, or `None`. ｜ 取数组切片，类型不符返回 `None`。
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// View as object, or `None`. ｜ 取对象，类型不符返回 `None`。
    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Look up a key on an object; `None` for any other kind. ｜ 对象的键查找；非对象返回 `None`。
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(o) => o.get(key),
            _ => None,
        }
    }
}

impl PartialEq for Value {
    /// Equality of values; `NaN != NaN` follows IEEE-754. ｜ 值相等性；`NaN != NaN` 遵循 IEEE-754。
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Object(a), Value::Object(b)) => a == b,
            _ => false,
        }
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(String::from(v))
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Str(v)
    }
}

impl From<Vec<Value>> for Value {
    fn from(v: Vec<Value>) -> Self {
        Value::Array(v)
    }
}

impl From<Object> for Value {
    fn from(v: Object) -> Self {
        Value::Object(v)
    }
}

/// An ordered object. ｜ 有序对象。
///
/// Default backend is insertion-ordered `Vec<(String, Value)>`; the mutually exclusive
/// `object-sort` feature swaps in `BTreeMap` for sorted, deterministic byte order.
/// 默认后端是插入序 `Vec<(String, Value)>`；互斥的 `object-sort` feature 换成
/// `BTreeMap`，得到确定性的排序字节序。
#[derive(Debug, Clone)]
pub struct Object {
    #[cfg(not(feature = "object-sort"))]
    entries: Vec<(String, Value)>,
    #[cfg(feature = "object-sort")]
    entries: alloc::collections::BTreeMap<String, Value>,
}

impl Object {
    /// Empty object. ｜ 空对象。
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Empty object with a capacity hint. ｜ 带容量提示的空对象。
    pub fn with_capacity(n: usize) -> Self {
        #[cfg(not(feature = "object-sort"))]
        {
            Self {
                entries: Vec::with_capacity(n),
            }
        }
        #[cfg(feature = "object-sort")]
        {
            let _ = n;
            Self {
                entries: alloc::collections::BTreeMap::new(),
            }
        }
    }

    /// Number of entries. ｜ 条目数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the object is empty. ｜ 对象是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Look up a key. ｜ 按键查找。
    pub fn get(&self, key: &str) -> Option<&Value> {
        #[cfg(not(feature = "object-sort"))]
        {
            self.entries
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v)
        }
        #[cfg(feature = "object-sort")]
        {
            self.entries.get(key)
        }
    }

    /// Insert or replace a key, keeping its position on overwrite. ｜ 插入或覆盖键；覆盖不改变位置。
    pub fn insert(&mut self, key: String, value: Value) -> Option<Value> {
        #[cfg(not(feature = "object-sort"))]
        {
            if let Some((_, old)) = self.entries.iter_mut().find(|(k, _)| *k == key) {
                return Some(core::mem::replace(old, value));
            }
            self.entries.push((key, value));
            None
        }
        #[cfg(feature = "object-sort")]
        {
            self.entries.insert(key, value)
        }
    }

    /// Remove a key. ｜ 删除键。
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        #[cfg(not(feature = "object-sort"))]
        {
            let pos = self.entries.iter().position(|(k, _)| k == key)?;
            Some(self.entries.remove(pos).1)
        }
        #[cfg(feature = "object-sort")]
        {
            self.entries.remove(key)
        }
    }

    /// Iterate in the backend's order. ｜ 按后端顺序迭代。
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Build an object from owned entries; duplicate keys keep the first position and
    /// take the last value. Common case (no duplicates) is zero extra allocation — a
    /// borrow-only sorted scan detects duplicates before deciding to merge.
    /// 从 owned 条目建对象；重名键保留首次位置、取最后的值。无重名的常见路径零额外分配：
    /// 先用借用排序扫描检测重名，再决定是否需要合并。
    pub fn from_entries(entries: Vec<(String, Value)>) -> Self {
        #[cfg(not(feature = "object-sort"))]
        {
            let dup = {
                let mut keys: alloc::vec::Vec<&str> =
                    entries.iter().map(|(k, _)| k.as_str()).collect();
                keys.sort_unstable();
                keys.windows(2).any(|w| w[0] == w[1])
            };
            if !dup {
                return Self { entries };
            }
            let mut index: alloc::collections::BTreeMap<String, usize> =
                alloc::collections::BTreeMap::new();
            let mut out: Vec<(String, Value)> = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                if let Some(&i) = index.get(&k) {
                    out[i].1 = v;
                } else {
                    index.insert(k.clone(), out.len());
                    out.push((k, v));
                }
            }
            Self { entries: out }
        }
        #[cfg(feature = "object-sort")]
        {
            Self {
                entries: entries.into_iter().collect(),
            }
        }
    }

    /// Consume into owned `(key, value)` pairs in backend order.
    /// 按后端顺序消费成-owned `(键, 值)` 对。
    pub fn into_entries(self) -> Vec<(String, Value)> {
        #[cfg(not(feature = "object-sort"))]
        {
            self.entries
        }
        #[cfg(feature = "object-sort")]
        {
            self.entries.into_iter().collect()
        }
    }
}

impl Default for Object {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for Object {
    /// Key/value equality; insertion order does not affect equality. ｜ 键值相等性；插入序不影响相等。
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .all(|(k, v)| other.get(k).map_or(false, |ov| ov == v))
    }
}

impl<const N: usize> From<[(String, Value); N]> for Object {
    fn from(entries: [(String, Value); N]) -> Self {
        let mut out = Self::with_capacity(N);
        for (k, v) in entries {
            out.insert(k, v);
        }
        out
    }
}
