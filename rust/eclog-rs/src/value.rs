// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! 值模型：`Value` / `Number` / `NumberInfo` / `Str` / `StringNotation` / `Object`。
//! Value model: `Value` / `Number` / `NumberInfo` / `Str` / `StringNotation` / `Object`.
//!
//! 两条与 smlite / serde_json 不同的取舍：
//! Two deliberate differences from smlite / serde_json:
//!
//! 1. `Number` 不分 int/float，只保留**原始字面量** `raw` 加位标志 `info`，因此
//!    `+0` / `-0` / `1e100` / `+inf` / `-nan` 都能逐字节对齐。
//!    `Number` does not split int/float: it keeps the raw lexeme plus `info` flags,
//!    so `+0` / `-0` / `1e100` / `+inf` / `-nan` stay byte-exact.
//! 2. `Str` 保留来源记法与定界符（heredoc 的 TAG、raw 的定界符），渲染阶段据此还原。
//!    `Str` keeps the source notation and delimiter (heredoc TAG, raw delimiter) so the
//!    renderer can reproduce the bytes.
//!
//! 相等语义（有意不对称）：`Str` 只比正文（记法是表现层），`Number` 比 `raw + info`
//! （raw 就是身份，不做数值折算）。
//! Equality is intentionally asymmetric: `Str` compares text only (notation is
//! presentation); `Number` compares `raw + info` (the lexeme *is* the identity).

use alloc::string::String;
use alloc::vec::Vec;
use core::borrow::Borrow;
use core::fmt;
use core::ops::{BitOr, BitOrAssign, Deref};

/// 数值位标志，对应 C++ `enum NumberInfo`。
/// Number bit flags, mirroring C++ `enum NumberInfo`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct NumberInfo(u8);

impl NumberInfo {
    /// 无任何标志 ｜ no flags.
    pub const NONE: Self = Self(0);
    /// 出现了显式正负号 ｜ an explicit `+`/`-` sign was present.
    pub const SIGN: Self = Self(1 << 0);
    /// 符号为负（隐含 `SIGN`）｜ the sign was `-` (implies [`Self::SIGN`]).
    pub const NEG: Self = Self(1 << 1);
    /// 有小数部分 ｜ has a fractional part.
    pub const FRAC: Self = Self(1 << 2);
    /// 有指数部分 ｜ has an exponent part.
    pub const EXP: Self = Self(1 << 3);
    /// `inf` ｜ infinity.
    pub const INF: Self = Self(1 << 4);
    /// `nan` ｜ not-a-number.
    pub const NAN: Self = Self(1 << 5);

    /// 原始位 ｜ raw bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// 只保留已定义的位 ｜ keep only defined bits.
    pub const fn from_bits_truncate(bits: u8) -> Self {
        Self(bits & 0b11_1111)
    }

    /// 是否没有任何标志 ｜ whether no flag is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// 是否包含全部给定位 ｜ whether all given flags are present.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl BitOr for NumberInfo {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for NumberInfo {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl fmt::Display for NumberInfo {
    /// 与 `.check` 期望文本同形：`none` 或 `sign|neg|frac|exp|inf|nan`。
    /// Same shape as the `.check` expectations.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return f.write_str("none");
        }
        let mut first = true;
        for (flag, name) in [
            (Self::SIGN, "sign"),
            (Self::NEG, "neg"),
            (Self::FRAC, "frac"),
            (Self::EXP, "exp"),
            (Self::INF, "inf"),
            (Self::NAN, "nan"),
        ] {
            if self.contains(flag) {
                if !first {
                    f.write_str("|")?;
                }
                f.write_str(name)?;
                first = false;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for NumberInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// 一个 Eclog 数值：原始字面量 + 位标志。
/// One Eclog number: raw lexeme plus flags.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Number {
    raw: String,
    info: NumberInfo,
}

impl Number {
    /// 由原始字面量与标志构造（`raw` 必须已经是 Eclog 数值字面量）。
    /// Build from a raw lexeme and flags (`raw` must already be an Eclog literal).
    pub fn new(raw: impl Into<String>, info: NumberInfo) -> Self {
        Self {
            raw: raw.into(),
            info,
        }
    }

    /// 校验并解析一个字面量；形状不合法返回 `None`。
    /// Validate and parse a literal; `None` when the shape is not a number.
    pub fn from_raw(raw: &str) -> Option<Self> {
        crate::parse::validate_number(raw).map(|info| Self {
            raw: String::from(raw),
            info,
        })
    }

    /// 由整数构造；负数自动置 `SIGN | NEG`。
    /// Build from an integer; negative values set `SIGN | NEG`.
    pub fn from_i64(value: i64) -> Self {
        let raw = alloc::format!("{}", value);
        let info = if value < 0 {
            NumberInfo::SIGN | NumberInfo::NEG
        } else {
            NumberInfo::NONE
        };
        Self { raw, info }
    }

    /// 原始字面量 ｜ the raw lexeme.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// 位标志 ｜ flags.
    pub const fn info(&self) -> NumberInfo {
        self.info
    }

    /// 是否有显式符号 ｜ whether an explicit sign was written.
    pub const fn has_sign(&self) -> bool {
        self.info.contains(NumberInfo::SIGN)
    }

    /// 符号是否为负 ｜ whether the sign is negative.
    pub const fn is_negative(&self) -> bool {
        self.info.contains(NumberInfo::NEG)
    }

    /// 是否有小数部分 ｜ whether a fractional part is present.
    pub const fn has_frac(&self) -> bool {
        self.info.contains(NumberInfo::FRAC)
    }

    /// 是否有指数部分 ｜ whether an exponent is present.
    pub const fn has_exp(&self) -> bool {
        self.info.contains(NumberInfo::EXP)
    }

    /// 是否 `inf` ｜ whether it is an infinity.
    pub const fn is_inf(&self) -> bool {
        self.info.contains(NumberInfo::INF)
    }

    /// 是否 `nan` ｜ whether it is a NaN.
    pub const fn is_nan(&self) -> bool {
        self.info.contains(NumberInfo::NAN)
    }

    /// 是否为可无损转换成整数的字面量（无小数、无指数、非 inf/nan）。
    /// Whether the literal converts to an integer losslessly.
    pub const fn is_integer_literal(&self) -> bool {
        !(self.has_frac() || self.has_exp() || self.is_inf() || self.is_nan())
    }

    /// 转 `i64`；不是整数字面量或溢出时 `None`。
    /// Convert to `i64`; `None` when not an integer literal or out of range.
    pub fn as_i64(&self) -> Option<i64> {
        if !self.is_integer_literal() {
            return None;
        }
        self.raw.parse::<i64>().ok()
    }

    /// 转 `u64`；不是整数字面量、为负或溢出时 `None`。
    /// Convert to `u64`; `None` when not an integer literal, negative, or out of range.
    pub fn as_u64(&self) -> Option<u64> {
        if !self.is_integer_literal() {
            return None;
        }
        self.raw.parse::<u64>().ok()
    }

    /// 转 `f64`；`inf` / `nan` 按标志映射（负号保留在 `inf` 与 `nan` 的符号位上）。
    /// Convert to `f64`; `inf` / `nan` map through the flags (negative sign kept).
    pub fn as_f64(&self) -> f64 {
        if self.is_nan() {
            return if self.is_negative() {
                -f64::NAN
            } else {
                f64::NAN
            };
        }
        if self.is_inf() {
            return if self.is_negative() {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        self.raw.parse::<f64>().unwrap_or(f64::NAN)
    }
}

impl fmt::Debug for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Number({:?}, {})", self.raw, self.info)
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl From<i64> for Number {
    fn from(value: i64) -> Self {
        Self::from_i64(value)
    }
}

/// 字符串记法，对应 C++ `enum StringNotation`。
/// String notation, mirroring C++ `enum StringNotation`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum StringNotation {
    /// 裸词 ｜ bare word.
    #[default]
    Unquoted,
    /// 双引号 ｜ double-quoted.
    Quoted,
    /// `@delim"..."delim` ｜ raw string.
    Raw,
    /// `|TAG ... TAG` ｜ heredoc.
    Heredoc,
}

/// 一个 Eclog 字符串：解码后的正文 + 来源记法 + 定界符。
/// One Eclog string: decoded text, source notation, and delimiter.
///
/// `delimiter` 仅对 [`StringNotation::Raw`] / [`StringNotation::Heredoc`] 非空，
/// 上限 16 个 ASCII 字母数字或 `_`。
/// `delimiter` is non-empty only for raw / heredoc strings: at most 16 ASCII
/// alphanumerics or `_`.
#[derive(Clone, Default)]
pub struct Str {
    text: String,
    notation: StringNotation,
    delimiter: String,
}

impl Str {
    /// 裸词（默认记法）｜ bare word (the default notation).
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            notation: StringNotation::Unquoted,
            delimiter: String::new(),
        }
    }

    /// 双引号记法 ｜ quoted notation.
    pub fn quoted(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            notation: StringNotation::Quoted,
            delimiter: String::new(),
        }
    }

    /// raw 记法 ｜ raw notation.
    pub fn raw(text: impl Into<String>, delimiter: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            notation: StringNotation::Raw,
            delimiter: delimiter.into(),
        }
    }

    /// heredoc 记法 ｜ heredoc notation.
    pub fn heredoc(text: impl Into<String>, tag: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            notation: StringNotation::Heredoc,
            delimiter: tag.into(),
        }
    }

    /// 任意组合构造 ｜ build from parts.
    pub fn from_parts(
        text: impl Into<String>,
        notation: StringNotation,
        delimiter: impl Into<String>,
    ) -> Self {
        Self {
            text: text.into(),
            notation,
            delimiter: delimiter.into(),
        }
    }

    /// 解码后的正文 ｜ decoded text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 来源记法 ｜ source notation.
    pub const fn notation(&self) -> StringNotation {
        self.notation
    }

    /// 定界符（raw / heredoc 专用）｜ delimiter (raw / heredoc only).
    pub fn delimiter(&self) -> &str {
        &self.delimiter
    }

    /// 取出正文，丢弃记法 ｜ take the text, dropping notation.
    pub fn into_string(self) -> String {
        self.text
    }

    /// 定界符是否合法（≤16 个 ASCII 字母数字或 `_`）。
    /// Whether a delimiter is valid (≤16 ASCII alphanumerics or `_`).
    pub fn is_valid_delimiter(delimiter: &str) -> bool {
        delimiter.len() <= 16
            && delimiter
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }
}

impl Deref for Str {
    type Target = str;

    fn deref(&self) -> &str {
        &self.text
    }
}

impl AsRef<str> for Str {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl Borrow<str> for Str {
    fn borrow(&self) -> &str {
        &self.text
    }
}

impl PartialEq for Str {
    /// 只比正文：记法与定界符属表现层 ｜ text only: notation is presentation.
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for Str {}

impl core::hash::Hash for Str {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl PartialEq<str> for Str {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for Str {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

impl PartialEq<Str> for str {
    fn eq(&self, other: &Str) -> bool {
        self == other.text
    }
}

impl From<&str> for Str {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Str {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<Str> for String {
    fn from(value: Str) -> Self {
        value.text
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Str")
            .field("text", &self.text)
            .field("notation", &self.notation)
            .field("delimiter", &self.delimiter)
            .finish()
    }
}

/// 插入序对象：`Vec<(Str, Value)>`，键唯一。
/// Insertion-ordered object: `Vec<(Str, Value)>` with unique keys.
///
/// 重复键语义与 C++ `insertForce` 一致：**位置取首次出现，键与值取最后一次**。
/// Duplicate-key semantics match C++ `insertForce`: the position is the first
/// occurrence, the key and value are the last occurrence.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Object {
    entries: Vec<(Str, Value)>,
}

impl Object {
    /// 空对象 ｜ empty object.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 预分配容量 ｜ pre-allocate for `capacity` entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
        }
    }

    /// 条目数 ｜ number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空 ｜ whether it has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 按插入序遍历 ｜ iterate in insertion order.
    pub fn iter(&self) -> core::slice::Iter<'_, (Str, Value)> {
        self.entries.iter()
    }

    /// 按插入序可变遍历 ｜ mutable iteration in insertion order.
    pub fn iter_mut(&mut self) -> core::slice::IterMut<'_, (Str, Value)> {
        self.entries.iter_mut()
    }

    /// 键序列 ｜ keys in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &Str> {
        self.entries.iter().map(|(k, _)| k)
    }

    /// 值序列 ｜ values in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.entries.iter().map(|(_, v)| v)
    }

    /// 按键取值（键唯一，取首个匹配）｜ look up by key (keys are unique).
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries
            .iter()
            .find(|(k, _)| k.text() == key)
            .map(|(_, v)| v)
    }

    /// 按键取可变值 ｜ look up by key, mutably.
    pub fn get_mut(&mut self, key: &str) -> Option<&mut Value> {
        self.entries
            .iter_mut()
            .find(|(k, _)| k.text() == key)
            .map(|(_, v)| v)
    }

    /// 下标取条目 ｜ entry by index.
    pub fn get_index(&self, index: usize) -> Option<(&Str, &Value)> {
        self.entries.get(index).map(|(k, v)| (k, v))
    }

    /// 键是否存在 ｜ whether the key exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// 插入或替换，返回被替换的旧值。
    /// Insert or replace, returning the previous value.
    ///
    /// 撞键时**原位替换**（键与值都换、位置不动），即 C++ `appendForce`。
    /// On collision the entry is replaced in place (both key and value), i.e. C++
    /// `appendForce`.
    pub fn insert(&mut self, key: impl Into<Str>, value: Value) -> Option<Value> {
        let key = key.into();
        if let Some(slot) = self
            .entries
            .iter_mut()
            .find(|(k, _)| k.text() == key.text())
        {
            let old = core::mem::replace(&mut slot.1, value);
            slot.0 = key;
            return Some(old);
        }
        self.entries.push((key, value));
        None
    }

    /// 仅当键不存在时追加，返回是否写入。
    /// Append only when the key is absent; returns whether it was written.
    ///
    /// 撞键时整体不动（连键的记法也不改），即 C++ `append`。
    /// On collision nothing changes, not even the key notation (C++ `append`).
    pub fn append(&mut self, key: impl Into<Str>, value: Value) -> bool {
        let key = key.into();
        if self.contains_key(key.text()) {
            return false;
        }
        self.entries.push((key, value));
        true
    }

    /// 按键删除，返回值 ｜ remove by key.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let index = self.entries.iter().position(|(k, _)| k.text() == key)?;
        Some(self.entries.remove(index).1)
    }

    /// 清空 ｜ drop all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl FromIterator<(Str, Value)> for Object {
    fn from_iter<I: IntoIterator<Item = (Str, Value)>>(iter: I) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}

impl IntoIterator for Object {
    type Item = (Str, Value);
    type IntoIter = alloc::vec::IntoIter<(Str, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

impl<'a> IntoIterator for &'a Object {
    type Item = &'a (Str, Value);
    type IntoIter = core::slice::Iter<'a, (Str, Value)>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

/// 一个 Eclog 值 ｜ an Eclog value.
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    /// `null`
    Null,
    /// `true` / `false`
    Bool(bool),
    /// 数值（保留原始字面量）｜ number (raw lexeme kept).
    Number(Number),
    /// 字符串（保留记法）｜ string (notation kept).
    Str(Str),
    /// 数组 ｜ array.
    Array(Vec<Value>),
    /// 对象 ｜ object.
    Object(Object),
}

impl Value {
    /// 类型名，与 C++ `DebugParseHandler::toString(ValueType)` 相同。
    /// Type name, identical to C++ `DebugParseHandler::toString(ValueType)`.
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "boolean",
            Self::Number(_) => "number",
            Self::Str(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }

    /// 是否 `null` ｜ whether it is null.
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// 是否布尔 ｜ whether it is a boolean.
    pub const fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// 是否数值 ｜ whether it is a number.
    pub const fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }

    /// 是否字符串 ｜ whether it is a string.
    pub const fn is_str(&self) -> bool {
        matches!(self, Self::Str(_))
    }

    /// 是否数组 ｜ whether it is an array.
    pub const fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    /// 是否对象 ｜ whether it is an object.
    pub const fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    /// 取布尔 ｜ the boolean, if any.
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// 取数值 ｜ the number, if any.
    pub const fn as_number(&self) -> Option<&Number> {
        match self {
            Self::Number(n) => Some(n),
            _ => None,
        }
    }

    /// 取字符串节点（含记法）｜ the string node (notation included).
    pub const fn as_str(&self) -> Option<&Str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    /// 取字符串正文 ｜ the string text.
    pub fn as_text(&self) -> Option<&str> {
        self.as_str().map(Str::text)
    }

    /// 取数组切片 ｜ the array slice.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// 取可变数组 ｜ the array, mutably.
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    /// 取对象 ｜ the object.
    pub const fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }

    /// 取可变对象 ｜ the object, mutably.
    pub fn as_object_mut(&mut self) -> Option<&mut Object> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }

    /// 对象按键取值；非对象返回 `None`。
    /// Object lookup; `None` for non-objects.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.as_object().and_then(|o| o.get(key))
    }

    /// 数组下标取值；非数组或越界返回 `None`。
    /// Array index; `None` for non-arrays or out-of-range.
    pub fn index(&self, index: usize) -> Option<&Value> {
        self.as_array().and_then(|a| a.get(index))
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<Number> for Value {
    fn from(value: Number) -> Self {
        Self::Number(value)
    }
}

impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Number(Number::from_i64(value))
    }
}

impl From<Str> for Value {
    fn from(value: Str) -> Self {
        Self::Str(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Str(Str::new(value))
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Str(Str::new(value))
    }
}

impl From<Vec<Value>> for Value {
    fn from(value: Vec<Value>) -> Self {
        Self::Array(value)
    }
}

impl From<Object> for Value {
    fn from(value: Object) -> Self {
        Self::Object(value)
    }
}
