// ---------------------------------------------------------------------------
// 自然序列化宏（derive）支持
// ---------------------------------------------------------------------------

use crate::value::Value;

/// 把一个类型「自然地」序列化为 SML 值：
/// 结构体 → 块、newtype → 透明、单元结构体 → 裸词、
/// 枚举单元变体 → 裸词、带数据变体 → `__type` 块。
///
/// 通常用 `#[derive(SmlSerialize)]` 自动实现（`derive` feature 默认开启），
/// 也可手动实现。支持的 `#[sml(...)]` 属性见 `swsml-derive` 的文档。
pub trait SmlSerialize {
    fn to_sml_value(&self) -> Value;

    /// 序列化为 SML 文本（等价于 [`to_sml`] 作用于本类型生成的值）。
    #[cfg(feature = "sml")]
    fn to_sml(&self) -> String {
        crate::to_sml(&self.to_sml_value())
    }
}

/// 从 SML 值反序列化（`#[derive(SmlDeserialize)]` 自动实现）。
pub trait SmlDeserialize: Sized {
    fn from_sml_value(v: &Value) -> Result<Self, String>;

    /// 解析 SML 文本并反序列化。
    fn from_sml(text: &str) -> Result<Self, String> {
        let v = crate::parse(text).map_err(|e| format!("SML 解析失败: {e}"))?;
        Self::from_sml_value(&v)
    }
}

/// 序列化为 SML 文本 —— toml-rs 风格的顶层函数（等价于 [`SmlSerialize::to_sml`]）。
///
/// 用法与 `toml::to_string` 一致（序列化不会失败，故直接返回 `String`）：
///
/// ```rust
/// # use sml::{SmlSerialize, SmlDeserialize};
/// # #[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
/// # struct Server { host: String, port: i32 }
/// # let cfg = Server { host: "web.example".into(), port: 8080 };
/// let text = sml::to_string(&cfg);
/// assert_eq!(text, "host: web.example\nport: 8080\n");
/// ```
#[cfg(feature = "sml")]
pub fn to_string<T: SmlSerialize + ?Sized>(value: &T) -> String {
    crate::to_sml(&value.to_sml_value())
}

/// 解析 SML 文本并反序列化 —— toml-rs 风格的顶层函数（等价于 [`SmlDeserialize::from_sml`]）。
///
/// ```rust
/// # use sml::{SmlSerialize, SmlDeserialize};
/// # #[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
/// # struct Server { host: String, port: i32 }
/// let back: Server = sml::from_str("host: web.example\nport: 8080\n").unwrap();
/// assert_eq!(back.host, "web.example");
/// assert_eq!(back.port, 8080);
/// ```
pub fn from_str<T: SmlDeserialize>(text: &str) -> Result<T, String> {
    T::from_sml(text)
}

/// 宏生成代码引用的内部辅助（请勿直接使用）。
#[doc(hidden)]
pub mod __private {
    use crate::value::Value;
    use super::{SmlDeserialize, SmlSerialize};
    use std::collections::{BTreeMap, HashMap};

    /// 描述值的类型，用于错误信息。
    pub fn describe_value(v: &Value) -> String {
        match v {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Str(s) => format!("字符串 `{s}`"),
            Value::Array(a) => format!("数组（{} 个元素）", a.len()),
            Value::Object(o) => format!("块（{} 个键）", o.len()),
        }
    }

    /// 取出 `_value` 键（枚举单值变体）。
    pub fn take_value(m: &BTreeMap<String, Value>) -> Result<Value, String> {
        m.get("_value")
            .cloned()
            .ok_or_else(|| "缺少 _value 键".to_string())
    }

    /// 取出 `_value` 键并断言为数组（枚举 tuple 变体）。
    pub fn take_array(m: &BTreeMap<String, Value>) -> Result<Vec<Value>, String> {
        match m.get("_value") {
            Some(Value::Array(a)) => Ok(a.clone()),
            Some(other) => Err(format!("_value 期望数组，实际为 {}", describe_value(other))),
            None => Err("缺少 _value 键".to_string()),
        }
    }

    /// `#[sml(flatten)]` 反序列化：把整个块交给子类型。
    pub fn flatten_from<T: SmlDeserialize>(m: &BTreeMap<String, Value>) -> Result<T, String> {
        T::from_sml_value(&Value::Object(m.clone()))
    }

    // ---- 基础类型 ----

    impl SmlSerialize for bool {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Bool(*self)
        }
    }
    impl SmlDeserialize for bool {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Bool(b) => Ok(*b),
                other => Err(format!("期望布尔，实际为 {}", describe_value(other))),
            }
        }
    }

    macro_rules! impl_int {
        ($($t:ty),* $(,)?) => {$(
            impl SmlSerialize for $t {
                #[inline]
                fn to_sml_value(&self) -> Value { Value::Int(*self as i64) }
            }
            impl SmlDeserialize for $t {
                #[inline]
                fn from_sml_value(v: &Value) -> Result<Self, String> {
                    match v {
                        Value::Int(i) => <$t>::try_from(*i)
                            .map_err(|_| format!("整数 {i} 超出 {} 范围", stringify!($t))),
                        Value::Float(f) if f.fract() == 0.0 => {
                            // 整数型浮点：严格上下界校验。
                            // 大整数类型（i64/u64/i128/u128）的 MAX 在 f64 中无法精确表示，
                            // `<$t>::MAX as f64` 会向上舍入为「超出范围」的值，导致边界检查失效
                            // （如 i64 <- 9223372036854775808.0 被误判为合法）。
                            // 因此对「MAX 不可精确表示」的大类型，用 `>=` 拒绝该边界值（fail-closed，
                            // 因为该浮点值本就无法无损回读为整数）；小类型 MAX 可精确表示，用 `>` 即可。
                            const LARGE: bool = (<$t>::MAX as f64) > 9.007_199_254_740_992e15_f64;
                            let max_f = <$t>::MAX as f64;
                            let min_f = <$t>::MIN as f64;
                            if *f < min_f || *f > max_f || (LARGE && *f == max_f) {
                                Err(format!("浮点数 {f} 超出 {} 范围", stringify!($t)))
                            } else {
                                Ok(*f as $t)
                            }
                        }
                        Value::Float(f) => Err(format!("期望整数，实际为小数 {f}")),
                        other => Err(format!("期望整数，实际为 {}", describe_value(other))),
                    }
                }
            }
        )*};
    }
    impl_int!(i8, i16, i32, i64, isize, u8, u16, u32);

    impl SmlSerialize for u64 {
        #[inline]
        fn to_sml_value(&self) -> Value {
            i64::try_from(*self).map(Value::Int).unwrap_or_else(|_| Value::Float(*self as f64))
        }
    }
    impl SmlDeserialize for u64 {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Int(i) => u64::try_from(*i).map_err(|_| format!("整数 {i} 为负数，超出 u64 范围")),
                Value::Float(f) if f.fract() == 0.0 => {
                    const LARGE: bool = (u64::MAX as f64) > 9.007_199_254_740_992e15_f64;
                    let max_f = u64::MAX as f64;
                    if *f < 0.0 || *f > max_f || (LARGE && *f == max_f) {
                        Err(format!("浮点数 {f} 超出 u64 范围"))
                    } else {
                        Ok(*f as u64)
                    }
                }
                Value::Float(f) => Err(format!("期望非负整数，实际为 {f}")),
                other => Err(format!("期望整数，实际为 {}", describe_value(other))),
            }
        }
    }

    impl SmlSerialize for usize {
        #[inline]
        fn to_sml_value(&self) -> Value {
            // 此前 `*self as i64` 在 u64::MAX 等情况下回绕为 -1（静默变负数）。
            // 改用 u64 的饱和逻辑：能放进 i64 就用 Int，否则退化为 Float。
            u64::try_from(*self)
                .map(|u| Value::Int(u as i64))
                .unwrap_or_else(|_| Value::Float(*self as f64))
        }
    }
    impl SmlDeserialize for usize {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Int(i) => usize::try_from(*i)
                    .map_err(|_| format!("整数 {i} 超出 usize 范围")),
                Value::Float(f) if f.fract() == 0.0 => {
                    const LARGE: bool = (usize::MAX as f64) > 9.007_199_254_740_992e15_f64;
                    let max_f = usize::MAX as f64;
                    if *f < 0.0 || *f > max_f || (LARGE && *f == max_f) {
                        Err(format!("浮点数 {f} 超出 usize 范围"))
                    } else {
                        Ok(*f as usize)
                    }
                }
                Value::Float(f) => Err(format!("期望非负整数，实际为 {f}")),
                other => Err(format!("期望整数，实际为 {}", describe_value(other))),
            }
        }
    }

    macro_rules! impl_big {
        ($($t:ty),* $(,)?) => {$(
            impl SmlSerialize for $t {
                #[inline]
                fn to_sml_value(&self) -> Value {
                    i64::try_from(*self).map(Value::Int).unwrap_or_else(|_| Value::Float(*self as f64))
                }
            }
            impl SmlDeserialize for $t {
                #[inline]
                fn from_sml_value(v: &Value) -> Result<Self, String> {
                    match v {
                        // 此前 `Ok(*i as $t)` 对无符号类型会静默饱和
                        // （如 u128 <- Int(-1) 变成 u128::MAX）。改用 try_from 严格校验。
                        Value::Int(i) => <$t>::try_from(*i)
                            .map_err(|_| format!("整数 {i} 超出 {} 范围", stringify!($t))),
                        Value::Float(f) if f.fract() == 0.0 => {
                            const LARGE: bool = (<$t>::MAX as f64) > 9.007_199_254_740_992e15_f64;
                            let max_f = <$t>::MAX as f64;
                            let min_f = <$t>::MIN as f64;
                            if *f < min_f || *f > max_f || (LARGE && *f == max_f) {
                                Err(format!("浮点数 {f} 超出 {} 范围", stringify!($t)))
                            } else {
                                Ok(*f as $t)
                            }
                        }
                        Value::Float(f) => Err(format!("期望整数，实际为小数 {f}")),
                        other => Err(format!("期望整数，实际为 {}", describe_value(other))),
                    }
                }
            }
        )*};
    }
    impl_big!(i128, u128);

    macro_rules! impl_float {
        ($($t:ty),* $(,)?) => {$(
            impl SmlSerialize for $t {
                #[inline]
                fn to_sml_value(&self) -> Value { Value::Float(*self as f64) }
            }
            impl SmlDeserialize for $t {
                #[inline]
                fn from_sml_value(v: &Value) -> Result<Self, String> {
                    match v {
                        Value::Int(i) => Ok(*i as $t),
                        Value::Float(f) => Ok(*f as $t),
                        other => Err(format!("期望数字，实际为 {}", describe_value(other))),
                    }
                }
            }
        )*};
    }
    impl_float!(f32, f64);

    impl SmlSerialize for char {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Str(self.to_string())
        }
    }
    impl SmlDeserialize for char {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Str(s) => {
                    let mut it = s.chars();
                    match (it.next(), it.next()) {
                        (Some(c), None) => Ok(c),
                        _ => Err(format!("期望单个字符，实际为 `{s}`")),
                    }
                }
                other => Err(format!("期望字符串，实际为 {}", describe_value(other))),
            }
        }
    }

    impl SmlSerialize for String {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Str(self.clone())
        }
    }
    impl SmlDeserialize for String {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Str(s) => Ok(s.clone()),
                other => Err(format!("期望字符串，实际为 {}", describe_value(other))),
            }
        }
    }

    impl SmlSerialize for str {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Str(self.to_string())
        }
    }

    impl SmlSerialize for &str {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Str(self.to_string())
        }
    }

    impl SmlSerialize for () {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Null
        }
    }
    impl SmlDeserialize for () {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Null => Ok(()),
                other => Err(format!("期望 null，实际为 {}", describe_value(other))),
            }
        }
    }

    impl SmlSerialize for Value {
        #[inline]
        fn to_sml_value(&self) -> Value {
            self.clone()
        }
    }
    impl SmlDeserialize for Value {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            Ok(v.clone())
        }
    }

    impl<T: SmlSerialize> SmlSerialize for Option<T> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            match self {
                Some(v) => v.to_sml_value(),
                None => Value::Null,
            }
        }
    }
    impl<T: SmlDeserialize> SmlDeserialize for Option<T> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Null => Ok(None),
                other => Ok(Some(T::from_sml_value(other)?)),
            }
        }
    }

    impl<T: SmlSerialize> SmlSerialize for Vec<T> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Array(self.iter().map(SmlSerialize::to_sml_value).collect())
        }
    }
    impl<T: SmlDeserialize> SmlDeserialize for Vec<T> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Array(a) => a.iter().map(SmlDeserialize::from_sml_value).collect(),
                other => Err(format!("期望数组，实际为 {}", describe_value(other))),
            }
        }
    }

    impl<T: SmlSerialize> SmlSerialize for Box<T> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            (**self).to_sml_value()
        }
    }
    impl<T: SmlDeserialize> SmlDeserialize for Box<T> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            Ok(Box::new(T::from_sml_value(v)?))
        }
    }

    impl<V: SmlSerialize> SmlSerialize for BTreeMap<String, V> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Object(
                self.iter()
                    .map(|(k, v)| (k.clone(), v.to_sml_value()))
                    .collect(),
            )
        }
    }
    impl<V: SmlDeserialize> SmlDeserialize for BTreeMap<String, V> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Object(m) => {
                    let mut out = BTreeMap::new();
                    for (k, val) in m {
                        out.insert(k.clone(), V::from_sml_value(val)?);
                    }
                    Ok(out)
                }
                other => Err(format!("期望块（object），实际为 {}", describe_value(other))),
            }
        }
    }

    impl<V: SmlSerialize> SmlSerialize for HashMap<String, V> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Object(
                self.iter()
                    .map(|(k, v)| (k.clone(), v.to_sml_value()))
                    .collect(),
            )
        }
    }
    impl<V: SmlDeserialize> SmlDeserialize for HashMap<String, V> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Object(m) => {
                    let mut out = HashMap::new();
                    for (k, val) in m {
                        out.insert(k.clone(), V::from_sml_value(val)?);
                    }
                    Ok(out)
                }
                other => Err(format!("期望块（object），实际为 {}", describe_value(other))),
            }
        }
    }
}
