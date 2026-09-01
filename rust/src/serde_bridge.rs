//! serde 桥接（可选 feature: `serde`）
//! 本文件内容原为 `pub mod serde { ... }`，拆分后由 lib.rs 重新 `pub mod serde { pub use ... }`。
use crate::core::*;
use crate::value::*;
use crate::derive_macro::__private;

use ::serde::de::{self, MapAccess, SeqAccess, Visitor};
use ::serde::ser::{
    SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
};
use ::serde::{Deserialize, Deserializer, Serialize, Serializer};
use ::std::collections::BTreeMap;
use ::std::fmt;

/// serde 错误类型（自定义消息，实现 ser/de 两个 Error trait）
type Error = ::serde::de::value::Error;

fn type_err(v: &Value, expected: &str) -> Error {
    de::Error::custom(format!(
        "期望 {expected}，实际为 {}",
        crate::derive_macro::__private::describe_value(v)
    ))
}

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Null => serializer.serialize_unit(),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Int(i) => serializer.serialize_i64(*i),
            Value::Float(f) => serializer.serialize_f64(*f),
            Value::Str(s) => serializer.serialize_str(s),
            // Vec<Value> / 逐项委托，递归依赖 Value 自身的 impl
            Value::Array(a) => a.serialize(serializer),
            Value::Object(m) => {
                let mut map = serializer.serialize_map(Some(m.len()))?;
                for (k, v) in m {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 交给格式自行判断类型（JSON 的数字/字符串/数组/对象都能落到对应变体）
        deserializer.deserialize_any(ValueVisitor)
    }
}

struct ValueVisitor;

impl<'de> Visitor<'de> for ValueVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any valid SML/JSON value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_some<D>(self, d: D) -> Result<Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Deserialize::deserialize(d)
    }
    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(Value::Bool(v))
    }
    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(Value::Int(v))
    }
    // 超出 i64 的大整数退化为 Float，避免直接报错丢失数据
    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(i64::try_from(v)
            .map(Value::Int)
            .unwrap_or_else(|_| Value::Float(v as f64)))
    }
    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        Ok(Value::Float(v))
    }
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        Ok(Value::Str(v.to_string()))
    }
    fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
        Ok(Value::Str(v))
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut v = Vec::new();
        while let Some(x) = seq.next_element()? {
            v.push(x);
        }
        Ok(Value::Array(v))
    }
    fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut m = BTreeMap::new();
        while let Some((k, v)) = map.next_entry::<String, Value>()? {
            m.insert(k, v);
        }
        Ok(Value::Object(m))
    }
}

// -----------------------------------------------------------------------
// serde 桥：任意 `serde::Serialize / Deserialize` 类型 <-> SML
// -----------------------------------------------------------------------

/// 解析 SML 文本并一键反序列化到任意 serde 类型（等价于 `toml::from_str`）。
///
/// ```rust
/// # use serde::Deserialize;
/// # #[derive(Deserialize, Debug)]
/// # struct Server { host: String, port: i32 }
/// let s: Server = sml::serde::from_str("host: web.example\nport: 8080\n").unwrap();
/// assert_eq!(s.host, "web.example");
/// ```
pub fn from_str<T: de::DeserializeOwned>(text: &str) -> Result<T, String> {
    let value = crate::parse(text)?;
    from_value(value)
}

/// 从任意 [`Value`] 反序列化到任意 serde 类型。
pub fn from_value<T: de::DeserializeOwned>(value: Value) -> Result<T, String> {
    T::deserialize(ValueDeserializer(value)).map_err(|e| e.to_string())
}

/// 任意 serde 类型序列化为 [`Value`]（等价于 `serde_json::to_value`）。
pub fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<Value, String> {
    value.serialize(ValueSerializer).map_err(|e| e.to_string())
}

/// 任意 serde 类型序列化为 SML 文本（等价于 `toml::to_string`）。
#[cfg(feature = "sml")]
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    Ok(crate::to_sml(&to_value(value)?))
}

#[cfg(not(feature = "sml"))]
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let _ = value;
    Err("sml feature not enabled".to_string())
}

// ---- Serializer: T: Serialize -> Value ----

struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SeqSerializer;
    type SerializeTuple = SeqSerializer;
    type SerializeTupleStruct = SeqSerializer;
    type SerializeTupleVariant = TupleVariantSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = MapSerializer;
    type SerializeStructVariant = StructVariantSerializer;

    fn serialize_bool(self, v: bool) -> Result<Value, Error> {
        Ok(Value::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<Value, Error> {
        Ok(Value::Int(v as i64))
    }
    fn serialize_i16(self, v: i16) -> Result<Value, Error> {
        Ok(Value::Int(v as i64))
    }
    fn serialize_i32(self, v: i32) -> Result<Value, Error> {
        Ok(Value::Int(v as i64))
    }
    fn serialize_i64(self, v: i64) -> Result<Value, Error> {
        Ok(Value::Int(v))
    }
    fn serialize_u8(self, v: u8) -> Result<Value, Error> {
        Ok(Value::Int(v as i64))
    }
    fn serialize_u16(self, v: u16) -> Result<Value, Error> {
        Ok(Value::Int(v as i64))
    }
    fn serialize_u32(self, v: u32) -> Result<Value, Error> {
        Ok(Value::Int(v as i64))
    }
    fn serialize_u64(self, v: u64) -> Result<Value, Error> {
        Ok(i64::try_from(v)
            .map(Value::Int)
            .unwrap_or_else(|_| Value::Float(v as f64)))
    }
    fn serialize_f32(self, v: f32) -> Result<Value, Error> {
        Ok(Value::Float(v as f64))
    }
    fn serialize_f64(self, v: f64) -> Result<Value, Error> {
        Ok(Value::Float(v))
    }
    fn serialize_char(self, v: char) -> Result<Value, Error> {
        Ok(Value::Str(v.to_string()))
    }
    fn serialize_str(self, v: &str) -> Result<Value, Error> {
        Ok(Value::Str(v.to_string()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Value, Error> {
        Ok(Value::Array(v.iter().map(|&b| Value::Int(b as i64)).collect()))
    }
    fn serialize_none(self) -> Result<Value, Error> {
        Ok(Value::Null)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<Value, Error> {
        v.serialize(ValueSerializer)
    }
    fn serialize_unit(self) -> Result<Value, Error> {
        Ok(Value::Null)
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Error> {
        Ok(Value::Null)
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
    ) -> Result<Value, Error> {
        Ok(Value::Str(variant.to_string()))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        v: &T,
    ) -> Result<Value, Error> {
        v.serialize(ValueSerializer)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Value, Error> {
        Ok(Value::Object(BTreeMap::from([
            ("__type".into(), Value::Str(variant.to_string())),
            ("_value".into(), value.serialize(ValueSerializer)?),
        ])))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Ok(SeqSerializer(Vec::new()))
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Ok(TupleVariantSerializer {
            variant: variant.to_string(),
            values: Vec::new(),
        })
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Ok(MapSerializer {
            map: BTreeMap::new(),
            key: None,
        })
    }
    fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct, Error> {
        self.serialize_map(Some(len))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Ok(StructVariantSerializer {
            variant: variant.to_string(),
            map: BTreeMap::new(),
        })
    }
}

struct SeqSerializer(Vec<Value>);

impl SerializeSeq for SeqSerializer {
    type Ok = Value;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.0.push(value.serialize(ValueSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        Ok(Value::Array(self.0))
    }
}
impl SerializeTuple for SeqSerializer {
    type Ok = Value;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Value, Error> {
        SerializeSeq::end(self)
    }
}
impl SerializeTupleStruct for SeqSerializer {
    type Ok = Value;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Value, Error> {
        SerializeSeq::end(self)
    }
}

struct MapSerializer {
    map: BTreeMap<String, Value>,
    key: Option<String>,
}

impl SerializeMap for MapSerializer {
    type Ok = Value;
    type Error = Error;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
        self.key = Some(key.serialize(KeySerializer)?);
        Ok(())
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        let k = self
            .key
            .take()
            .ok_or_else(|| de::Error::custom("serialize_value 前需先 serialize_key"))?;
        self.map.insert(k, value.serialize(ValueSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        Ok(Value::Object(self.map))
    }
}

impl SerializeStruct for MapSerializer {
    type Ok = Value;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.map
            .insert(key.to_string(), value.serialize(ValueSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        Ok(Value::Object(self.map))
    }
}

/// 对象键必须能转成字符串（SML 的键是裸词/字符串）
struct KeySerializer;

macro_rules! key_unsupported {
    ($(fn $m:ident($($a:ident : $t:ty),*) -> Result<String, Error>;)*) => {
        $(
            fn $m(self, $($a: $t),*) -> Result<String, Error> {
                Err(de::Error::custom("SML 对象的键必须是字符串"))
            }
        )*
    };
}

impl Serializer for KeySerializer {
    type Ok = String;
    type Error = Error;
    type SerializeSeq = ::serde::ser::Impossible<String, Error>;
    type SerializeTuple = ::serde::ser::Impossible<String, Error>;
    type SerializeTupleStruct = ::serde::ser::Impossible<String, Error>;
    type SerializeTupleVariant = ::serde::ser::Impossible<String, Error>;
    type SerializeMap = ::serde::ser::Impossible<String, Error>;
    type SerializeStruct = ::serde::ser::Impossible<String, Error>;
    type SerializeStructVariant = ::serde::ser::Impossible<String, Error>;

    fn serialize_str(self, v: &str) -> Result<String, Error> {
        Ok(v.to_string())
    }
    fn serialize_char(self, v: char) -> Result<String, Error> {
        Ok(v.to_string())
    }
    key_unsupported! {
        fn serialize_bool(_v: bool) -> Result<String, Error>;
        fn serialize_i8(_v: i8) -> Result<String, Error>;
        fn serialize_i16(_v: i16) -> Result<String, Error>;
        fn serialize_i32(_v: i32) -> Result<String, Error>;
        fn serialize_i64(_v: i64) -> Result<String, Error>;
        fn serialize_u8(_v: u8) -> Result<String, Error>;
        fn serialize_u16(_v: u16) -> Result<String, Error>;
        fn serialize_u32(_v: u32) -> Result<String, Error>;
        fn serialize_u64(_v: u64) -> Result<String, Error>;
        fn serialize_f32(_v: f32) -> Result<String, Error>;
        fn serialize_f64(_v: f64) -> Result<String, Error>;
        fn serialize_bytes(_v: &[u8]) -> Result<String, Error>;
        fn serialize_none() -> Result<String, Error>;
        fn serialize_unit() -> Result<String, Error>;
        fn serialize_unit_struct(_n: &'static str) -> Result<String, Error>;
        fn serialize_unit_variant(_n: &'static str, _i: u32, _v: &'static str) -> Result<String, Error>;
    }
    fn serialize_some<T: Serialize + ?Sized>(self, _v: &T) -> Result<String, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _n: &'static str,
        _v: &T,
    ) -> Result<String, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _n: &'static str,
        _i: u32,
        _v: &'static str,
        _x: &T,
    ) -> Result<String, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
    // 以下方法返回关联类型（Impossible），一律报错——SML 键只能是字符串
    fn serialize_seq(self, _l: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
    fn serialize_tuple(self, _l: usize) -> Result<Self::SerializeTuple, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
    fn serialize_tuple_struct(
        self,
        _n: &'static str,
        _l: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
    fn serialize_tuple_variant(
        self,
        _n: &'static str,
        _i: u32,
        _v: &'static str,
        _l: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
    fn serialize_map(self, _l: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
    fn serialize_struct(self, _n: &'static str, _l: usize) -> Result<Self::SerializeStruct, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
    fn serialize_struct_variant(
        self,
        _n: &'static str,
        _i: u32,
        _v: &'static str,
        _l: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(de::Error::custom("SML 对象的键必须是字符串"))
    }
}

struct TupleVariantSerializer {
    variant: String,
    values: Vec<Value>,
}

impl SerializeTupleVariant for TupleVariantSerializer {
    type Ok = Value;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.values.push(value.serialize(ValueSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        Ok(Value::Object(BTreeMap::from([
            ("__type".into(), Value::Str(self.variant)),
            ("_value".into(), Value::Array(self.values)),
        ])))
    }
}

struct StructVariantSerializer {
    variant: String,
    map: BTreeMap<String, Value>,
}

impl SerializeStructVariant for StructVariantSerializer {
    type Ok = Value;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        self.map
            .insert(key.to_string(), value.serialize(ValueSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Value, Error> {
        let mut m = BTreeMap::new();
        m.insert("__type".into(), Value::Str(self.variant));
        m.extend(self.map);
        Ok(Value::Object(m))
    }
}

// ---- Deserializer: Value -> T: Deserialize ----

macro_rules! deser_int {
    ($(fn $m:ident($v:ident, $call:ident);)*) => {
        $(
            fn $m<V>(self, $v: V) -> Result<V::Value, Self::Error>
            where V: Visitor<'de> {
                match self.0 {
                    Value::Int(i) => $v.$call(i as _),
                    Value::Float(f)
                        if f.fract() == 0.0
                            && f >= i64::MIN as f64
                            && f <= i64::MAX as f64 =>
                    {
                        $v.$call(f as _)
                    }
                    other => Err(type_err(&other, stringify!($m).trim_start_matches("deserialize_"))),
                }
            }
        )*
    };
}

struct ValueDeserializer(Value);

impl<'de> Deserializer<'de> for ValueDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        // 按引用匹配：`Value` 实现了 `Drop`，按值绑定会触发 E0509（无法部分移出）。
        // 需要所有权的分支（Vec / BTreeMap）显式 clone。
        match &self.0 {
            Value::Null => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(*b),
            Value::Int(i) => visitor.visit_i64(*i),
            Value::Float(f) => visitor.visit_f64(*f),
            Value::Str(s) => visitor.visit_str(s),
            Value::Array(a) => visitor.visit_seq(SeqDeserializer { items: a.clone(), idx: 0 }),
            Value::Object(m) => {
                visitor.visit_map(MapDeserializer { map: m.clone(), pending: None })
            }
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Bool(b) => visitor.visit_bool(*b),
            other => Err(type_err(other, "布尔")),
        }
    }

    deser_int! {
        fn deserialize_i8(v, visit_i8);
        fn deserialize_i16(v, visit_i16);
        fn deserialize_i32(v, visit_i32);
        fn deserialize_i64(v, visit_i64);
        fn deserialize_u8(v, visit_u8);
        fn deserialize_u16(v, visit_u16);
        fn deserialize_u32(v, visit_u32);
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Int(i) if *i >= 0 => visitor.visit_u64(*i as u64),
            Value::Float(f)
                if f.fract() == 0.0 && *f >= 0.0 && *f <= u64::MAX as f64 =>
            {
                visitor.visit_u64(*f as u64)
            }
            other => Err(type_err(other, "u64")),
        }
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Int(i) => visitor.visit_f32(*i as f32),
            Value::Float(f) => visitor.visit_f32(*f as f32),
            other => Err(type_err(other, "f32")),
        }
    }
    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Int(i) => visitor.visit_f64(*i as f64),
            Value::Float(f) => visitor.visit_f64(*f),
            other => Err(type_err(other, "f64")),
        }
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Str(s) if s.chars().count() == 1 => {
                visitor.visit_char(s.chars().next().unwrap())
            }
            other => Err(type_err(other, "字符")),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Str(s) => visitor.visit_str(s),
            other => Err(type_err(other, "字符串")),
        }
    }
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Array(items) => {
                let mut buf = Vec::with_capacity(items.len());
                for it in items {
                    match it {
                        Value::Int(i) if (0..=255).contains(i) => buf.push(*i as u8),
                        other => return Err(type_err(other, "字节")),
                    }
                }
                visitor.visit_byte_buf(buf)
            }
            other => Err(type_err(&other, "字节数组")),
        }
    }
    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Null => visitor.visit_none(),
            other => visitor.visit_some(ValueDeserializer(other)),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.0 {
            Value::Null => visitor.visit_unit(),
            other => Err(type_err(&other, "unit")),
        }
    }
    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Array(a) => visitor.visit_seq(SeqDeserializer { items: a.clone(), idx: 0 }),
            other => Err(type_err(other, "数组")),
        }
    }
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Object(m) => {
                visitor.visit_map(MapDeserializer { map: m.clone(), pending: None })
            }
            other => Err(type_err(other, "块/对象")),
        }
    }
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match &self.0 {
            Value::Str(s) => visitor.visit_enum(EnumDeserializer {
                variant: s.clone(),
                kind: EnumKind::Unit,
            }),
            Value::Object(m) => {
                // 取副本后按需 `remove`：`Value` 实现 `Drop`，不可从原值部分移出。
                let mut m = m.clone();
                // 1) SML 专有约定：`__type` 键（与 SmlSerialize 输出一致）
                if let Some(ty) = m.remove("__type") {
                    let variant = match &ty {
                        Value::Str(s) => s.clone(),
                        _ => return Err(de::Error::custom("`__type` 的值必须是字符串")),
                    };
                    let kind = match m.remove("_value") {
                        Some(v) => match &v {
                            Value::Array(items) => EnumKind::Tuple(items.clone()),
                            _ => EnumKind::Newtype(v),
                        },
                        None if m.is_empty() => EnumKind::Unit,
                        None => EnumKind::Struct(m),
                    };
                    return visitor.visit_enum(EnumDeserializer { variant, kind });
                }
                // 2) serde 外部标签（含 SML 裸词包裹形态）：
                //    {"in-maintenance": "in-maintenance"} -> 单元变体
                //    {"Circle": 3}                        -> 单值变体
                if m.len() == 1 {
                    let (k, v) = m.pop_first().expect("len==1 必有键");
                    let kind = match &v {
                        Value::Str(s) if *s == k => EnumKind::Unit,
                        _ => EnumKind::Newtype(v),
                    };
                    return visitor.visit_enum(EnumDeserializer { variant: k, kind });
                }
                Err(de::Error::custom(
                    "枚举块需要 `__type` 键（SML 约定）或单键外部标签 `{ VariantName: ... }`",
                ))
            }
            other => Err(type_err(other, "枚举")),
        }
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

struct SeqDeserializer {
    items: Vec<Value>,
    idx: usize,
}

impl<'de> SeqAccess<'de> for SeqDeserializer {
    type Error = Error;
    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        if self.idx >= self.items.len() {
            return Ok(None);
        }
        let item = self.items[self.idx].clone();
        self.idx += 1;
        seed.deserialize(ValueDeserializer(item)).map(Some)
    }
}

struct MapDeserializer {
    map: BTreeMap<String, Value>,
    pending: Option<Value>,
}

impl<'de> MapAccess<'de> for MapDeserializer {
    type Error = Error;
    fn next_key_seed<K: de::DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        let Some((k, v)) = self.map.pop_first() else {
            return Ok(None);
        };
        self.pending = Some(v);
        seed.deserialize(KeyDeserializer(&k)).map(Some)
    }
    fn next_value_seed<V: de::DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Error> {
        let v = self.pending.take().ok_or_else(|| {
            de::Error::custom("value 缺失：需先调用 next_key_seed")
        })?;
        seed.deserialize(ValueDeserializer(v))
    }
}

/// 字段名 / 变体名的轻量反序列化器（只认字符串）
struct KeyDeserializer<'a>(&'a str);

macro_rules! key_delegate {
    ($($m:ident),* $(,)?) => {
        $(
            fn $m<V>(self, visitor: V) -> Result<V::Value, Error>
            where V: Visitor<'de> {
                self.deserialize_any(visitor)
            }
        )*
    };
}

impl<'de, 'a> Deserializer<'de> for KeyDeserializer<'a> {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.0)
    }
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.0)
    }
    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.0)
    }
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_str(self.0)
    }
    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(EnumDeserializer {
            variant: self.0.to_string(),
            kind: EnumKind::Unit,
        })
    }
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_some(self)
    }
    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }
    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }
    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
    key_delegate! {
        deserialize_bool, deserialize_i8, deserialize_i16, deserialize_i32,
        deserialize_i64, deserialize_u8, deserialize_u16, deserialize_u32,
        deserialize_u64, deserialize_f32, deserialize_f64, deserialize_char,
        deserialize_bytes, deserialize_byte_buf, deserialize_unit,
        deserialize_seq, deserialize_map,
    }
}

// ---- 枚举（SML `__type` 约定，与 SmlDeserialize 一致）----

#[derive(Debug)]
enum EnumKind {
    Unit,
    Newtype(Value),
    Tuple(Vec<Value>),
    Struct(BTreeMap<String, Value>),
}

struct EnumDeserializer {
    variant: String,
    kind: EnumKind,
}

impl<'de> de::EnumAccess<'de> for EnumDeserializer {
    type Error = Error;
    type Variant = VariantAccess;
    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let variant = seed.deserialize(KeyDeserializer(&self.variant))?;
        Ok((variant, VariantAccess { kind: self.kind }))
    }
}

struct VariantAccess {
    kind: EnumKind,
}

impl<'de> de::VariantAccess<'de> for VariantAccess {
    type Error = Error;
    fn unit_variant(self) -> Result<(), Error> {
        match self.kind {
            EnumKind::Unit => Ok(()),
            _ => Err(de::Error::custom("该变体携带数据，不能按单元变体解析")),
        }
    }
    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, Error> {
        match self.kind {
            EnumKind::Newtype(v) => seed.deserialize(ValueDeserializer(v)),
            EnumKind::Tuple(items) => {
                seed.deserialize(ValueDeserializer(Value::Array(items)))
            }
            _ => Err(de::Error::custom("该变体没有单值数据")),
        }
    }
    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.kind {
            EnumKind::Tuple(items) => {
                visitor.visit_seq(SeqDeserializer { items, idx: 0 })
            }
            _ => Err(de::Error::custom("该变体不是元组形态")),
        }
    }
    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.kind {
            EnumKind::Struct(m) => {
                visitor.visit_map(MapDeserializer { map: m, pending: None })
            }
            _ => Err(de::Error::custom("该变体不是结构体形态")),
        }
    }
}
