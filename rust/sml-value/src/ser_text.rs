//! 直出 SML 文本的 serde `Serializer`（可选 feature：`serde` **且** `sml`）。
//!
//! 与 `serde::to_value` + `to_sml` 那条路（[`crate::serde`]）的区别：
//! 这里**不经过 `Value` 中转**，因此拿到两条 `Value` 给不了的能力：
//!
//! 1. **保留字段声明顺序** —— `Value::Object` 是 `BTreeMap`，走它会按字典序重排；
//! 2. **支持 i128 / u128 与超出 i64 的 `u64`** —— 直接写数字字符，不受 `Value::Int(i64)` 限制。
//!
//! # 已知差异（务必读）
//!
//! - **大整数往返不对称**：超 i64 的整数写出去是裸词，SML 回读按 `coerce_word` 的 B10
//!   规则得到 **`Str`**（不丢精度，但类型变了）。`Value` 那条路是直接**拒收**（`E-DERIVE-002`）。
//! - **浮点没有 `raw`**：serde 只给 `f64`，故不保留 `1e10` 这类书写形式
//!   （`Value::Float` 带 `raw`，走 `to_sml` 能保留）。
//! - **排版与 `to_sml` 逐字节一致**，靠测试锁定 —— 两份发射器必须共用
//!   [`quote_if_needed`] / [`format_float`]，否则会各自漂移。
//! - **非字符串键仍报错**（与 `Value` 那条路一致）：SML 的键是裸词 / 引号串。

use crate::dump::{format_float, quote_if_needed};
use crate::value::MAX_VALUE_DEPTH;

use ::serde::de;
use ::serde::ser::{
    SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};
use ::serde::{Serialize, Serializer};

type Error = ::serde::de::value::Error;

// ---------------------------------------------------------------------------
// Node：保持声明顺序的中间树
// ---------------------------------------------------------------------------

/// 直出用的中间节点。与 [`crate::Value`] 的两点差异：
/// - `Object` 是 `Vec<(String, Node)>`（**声明顺序**），不是 `BTreeMap`；
/// - 多一个 [`Node::BigInt`]，承载放不进 `i64` 的整数。
enum Node {
    Null,
    Bool(bool),
    Int(i64),
    /// 放不进 `i64` 的整数（i128 / u128 / 超 i64 的 u64）：原样写数字字符。
    BigInt(String),
    Float(f64),
    Str(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

// ---------------------------------------------------------------------------
// 渲染：与 `dump.rs` 同规则
// ---------------------------------------------------------------------------

/// 深度超限时的占位（与 `dump.rs` 完全一致，测试依赖这个字面量）。
const DEPTH_OVERFLOW_NULL: &str = "/* …深度超限… */ null";
const DEPTH_OVERFLOW_EMPTY: &str = "/* …深度超限… */ {}";

fn is_scalar(n: &Node) -> bool {
    !matches!(n, Node::Object(_) | Node::Array(_))
}

/// 容器是否「扁平」：**直接子项全是标量**。只看一层，不递归 —— 理由见 `dump.rs`。
fn is_flat(n: &Node) -> bool {
    match n {
        Node::Object(m) => m.iter().all(|(_, v)| is_scalar(v)),
        Node::Array(a) => a.iter().all(is_scalar),
        _ => true,
    }
}

/// 值是否紧跟在 `键:` 之后、同一行开始写（非空的块自己会先换行）。
fn starts_inline(n: &Node) -> bool {
    !matches!(n, Node::Object(m) if !m.is_empty())
}

fn render_scalar(n: &Node) -> String {
    match n {
        Node::Null => "null".to_string(),
        Node::Bool(b) => b.to_string(),
        Node::Int(i) => i.to_string(),
        Node::BigInt(s) => s.clone(),
        Node::Float(f) => format_float(*f, None),
        Node::Str(s) => quote_if_needed(s),
        // `dump_scalar` 的兜底：容器不会走到这里
        Node::Array(_) | Node::Object(_) => String::new(),
    }
}

fn render_inline(n: &Node, depth: usize) -> String {
    if depth > MAX_VALUE_DEPTH {
        return DEPTH_OVERFLOW_NULL.to_string();
    }
    match n {
        Node::Object(m) => {
            let parts: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{}: {}", quote_if_needed(k), render_inline(v, depth + 1)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        Node::Array(a) => {
            let parts: Vec<String> = a.iter().map(|e| render_inline(e, depth + 1)).collect();
            format!("[ {} ]", parts.join(", "))
        }
        other => render_scalar(other),
    }
}

fn render_object_body(m: &[(String, Node)], indent: usize, depth: usize) -> String {
    let mut out = String::new();
    let pad = "  ".repeat(indent + 1);
    for (k, v) in m {
        out.push_str(&format!("\n{pad}{}:", quote_if_needed(k)));
        if starts_inline(v) {
            out.push(' ');
        }
        out.push_str(&render_value(v, indent + 1, depth + 1));
    }
    out.push_str(&format!("\n{}}}", "  ".repeat(indent)));
    out
}

fn render_block(m: &[(String, Node)], indent: usize, depth: usize) -> String {
    if depth > MAX_VALUE_DEPTH {
        return DEPTH_OVERFLOW_EMPTY.to_string();
    }
    if m.is_empty() {
        return "{}".to_string();
    }
    format!("\n{}{{", "  ".repeat(indent)) + &render_object_body(m, indent, depth)
}

fn render_value(n: &Node, indent: usize, depth: usize) -> String {
    if depth > MAX_VALUE_DEPTH {
        return DEPTH_OVERFLOW_NULL.to_string();
    }
    match n {
        Node::Array(a) => {
            if a.is_empty() {
                return "[]".to_string();
            }
            let mut out = String::from("[");
            for e in a {
                out.push('\n');
                out.push_str(&"  ".repeat(indent + 1));
                out.push_str(&render_element(e, indent + 1, depth + 1));
            }
            out.push_str(&format!("\n{}]", "  ".repeat(indent)));
            out
        }
        Node::Object(m) => render_block(m, indent, depth + 1),
        other => render_scalar(other),
    }
}

fn render_element(n: &Node, indent: usize, depth: usize) -> String {
    // 深度守卫必须在最前：数组分支会递归自己，少了它深嵌套会爆栈
    if depth > MAX_VALUE_DEPTH {
        return DEPTH_OVERFLOW_NULL.to_string();
    }
    if is_flat(n) {
        return render_inline(n, depth);
    }
    match n {
        Node::Object(m) => format!("{{") + &render_object_body(m, indent, depth),
        Node::Array(a) => {
            let mut out = String::from("[");
            for e in a {
                out.push('\n');
                out.push_str(&"  ".repeat(indent + 1));
                out.push_str(&render_element(e, indent + 1, depth + 1));
            }
            out.push_str(&format!("\n{}]", "  ".repeat(indent)));
            out
        }
        other => render_inline(other, depth),
    }
}

/// 顶层渲染，规则与 `to_sml` 一致（含「根块含 `__type` 走 `dump_block`」的特例）。
fn render_root(n: &Node) -> String {
    match n {
        Node::Object(m) if m.iter().any(|(k, _)| k == "__type") => render_block(m, 0, 0),
        Node::Object(m) => {
            let mut out = String::new();
            for (k, v) in m {
                out.push_str(&format!("{}:", quote_if_needed(k)));
                if starts_inline(v) {
                    out.push(' ');
                }
                out.push_str(&render_value(v, 0, 0));
                out.push('\n');
            }
            out
        }
        other => render_element(other, 0, 0),
    }
}

// ---------------------------------------------------------------------------
// Serializer：T: Serialize -> Node -> String
// ---------------------------------------------------------------------------

/// 迭代求节点最大深度（显式栈，不递归 —— 深嵌套会把栈写爆）。
fn max_depth(root: &Node) -> usize {
    let mut stack: Vec<(&Node, usize)> = vec![(root, 1)];
    let mut max = 1usize;
    while let Some((cur, d)) = stack.pop() {
        if d > max {
            max = d;
        }
        match cur {
            Node::Object(m) => {
                for (_, v) in m {
                    stack.push((v, d + 1));
                }
            }
            Node::Array(a) => {
                for v in a {
                    stack.push((v, d + 1));
                }
            }
            _ => {}
        }
    }
    max
}

/// 任意 serde 类型序列化为 SML 文本，**不经 `Value` 中转**。
///
/// 与 [`crate::serde::to_string`] 的差别见模块文档（顺序 / 大整数 / 浮点 raw）。
///
/// 深度超过 [`MAX_VALUE_DEPTH`] 直接返回 `Err`（对齐 `to_sml_checked` 的口径），
/// 而不是像 `to_sml` 那样静默写占位符 —— 本函数是可失败的，没必要吞掉。
pub fn to_string_direct<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
    let node = value.serialize(NodeSerializer).map_err(|e| e.to_string())?;
    let d = max_depth(&node);
    if d > MAX_VALUE_DEPTH {
        return Err(format!(
            "嵌套深度 {d} 超过上限 {MAX_VALUE_DEPTH}（序列化中止）"
        ));
    }
    Ok(render_root(&node))
}

struct NodeSerializer;

/// 建树用的辅助：键存在则替换值并**保持首次出现的位置**（贴近 BTreeMap 的行为）。
fn upsert(entries: &mut Vec<(String, Node)>, key: String, value: Node) {
    if let Some(slot) = entries.iter_mut().find(|(k, _)| *k == key) {
        slot.1 = value;
    } else {
        entries.push((key, value));
    }
}

impl Serializer for NodeSerializer {
    type Ok = Node;
    type Error = Error;
    type SerializeSeq = SeqBuilder;
    type SerializeTuple = SeqBuilder;
    type SerializeTupleStruct = SeqBuilder;
    type SerializeTupleVariant = TupleVariantBuilder;
    type SerializeMap = MapBuilder;
    type SerializeStruct = MapBuilder;
    type SerializeStructVariant = StructVariantBuilder;

    fn serialize_bool(self, v: bool) -> Result<Node, Error> {
        Ok(Node::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<Node, Error> {
        Ok(Node::Int(v as i64))
    }
    fn serialize_i16(self, v: i16) -> Result<Node, Error> {
        Ok(Node::Int(v as i64))
    }
    fn serialize_i32(self, v: i32) -> Result<Node, Error> {
        Ok(Node::Int(v as i64))
    }
    fn serialize_i64(self, v: i64) -> Result<Node, Error> {
        Ok(Node::Int(v))
    }
    // `Value` 那条路对超范围的整数是**拒收**；这里直出文本，可以保真写出。
    // 代价是 SML 回读得到 Str（见模块文档）。
    fn serialize_i128(self, v: i128) -> Result<Node, Error> {
        Ok(match i64::try_from(v) {
            Ok(i) => Node::Int(i),
            Err(_) => Node::BigInt(v.to_string()),
        })
    }
    fn serialize_u8(self, v: u8) -> Result<Node, Error> {
        Ok(Node::Int(v as i64))
    }
    fn serialize_u16(self, v: u16) -> Result<Node, Error> {
        Ok(Node::Int(v as i64))
    }
    fn serialize_u32(self, v: u32) -> Result<Node, Error> {
        Ok(Node::Int(v as i64))
    }
    fn serialize_u64(self, v: u64) -> Result<Node, Error> {
        Ok(match i64::try_from(v) {
            Ok(i) => Node::Int(i),
            Err(_) => Node::BigInt(v.to_string()),
        })
    }
    fn serialize_u128(self, v: u128) -> Result<Node, Error> {
        Ok(match i64::try_from(v) {
            Ok(i) => Node::Int(i),
            Err(_) => Node::BigInt(v.to_string()),
        })
    }
    fn serialize_f32(self, v: f32) -> Result<Node, Error> {
        Ok(Node::Float(v as f64))
    }
    fn serialize_f64(self, v: f64) -> Result<Node, Error> {
        Ok(Node::Float(v))
    }
    fn serialize_char(self, v: char) -> Result<Node, Error> {
        Ok(Node::Str(v.to_string()))
    }
    fn serialize_str(self, v: &str) -> Result<Node, Error> {
        Ok(Node::Str(v.to_string()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Node, Error> {
        Ok(Node::Array(v.iter().map(|&b| Node::Int(b as i64)).collect()))
    }
    fn serialize_none(self) -> Result<Node, Error> {
        Ok(Node::Null)
    }
    fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<Node, Error> {
        v.serialize(NodeSerializer)
    }
    fn serialize_unit(self) -> Result<Node, Error> {
        Ok(Node::Null)
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<Node, Error> {
        Ok(Node::Null)
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
    ) -> Result<Node, Error> {
        Ok(Node::Str(variant.to_string()))
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        v: &T,
    ) -> Result<Node, Error> {
        v.serialize(NodeSerializer)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Node, Error> {
        Ok(Node::Object(vec![
            ("__type".to_string(), Node::Str(variant.to_string())),
            ("_value".to_string(), value.serialize(NodeSerializer)?),
        ]))
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Ok(SeqBuilder { items: Vec::new() })
    }
    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        Ok(SeqBuilder { items: Vec::new() })
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Ok(SeqBuilder { items: Vec::new() })
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Ok(TupleVariantBuilder {
            variant: variant.to_string(),
            values: Vec::new(),
        })
    }
    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Ok(MapBuilder {
            entries: Vec::new(),
            key: None,
        })
    }
    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct, Error> {
        Ok(MapBuilder {
            entries: Vec::new(),
            key: None,
        })
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _idx: u32,
        variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Ok(StructVariantBuilder {
            variant: variant.to_string(),
            entries: Vec::new(),
        })
    }
}

struct SeqBuilder {
    items: Vec<Node>,
}

impl SerializeSeq for SeqBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.items.push(value.serialize(NodeSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Array(self.items))
    }
}

impl SerializeTuple for SeqBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Node, Error> {
        SerializeSeq::end(self)
    }
}

impl SerializeTupleStruct for SeqBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<Node, Error> {
        SerializeSeq::end(self)
    }
}

struct MapBuilder {
    entries: Vec<(String, Node)>,
    key: Option<String>,
}

impl SerializeMap for MapBuilder {
    type Ok = Node;
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
        upsert(&mut self.entries, k, value.serialize(NodeSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Object(self.entries))
    }
}

impl SerializeStruct for MapBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        // 声明顺序 = 调用顺序，这是本模块存在的意义（`BTreeMap` 会重排）
        upsert(
            &mut self.entries,
            key.to_string(),
            value.serialize(NodeSerializer)?,
        );
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Object(self.entries))
    }
}

struct TupleVariantBuilder {
    variant: String,
    values: Vec<Node>,
}

impl SerializeTupleVariant for TupleVariantBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
        self.values.push(value.serialize(NodeSerializer)?);
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        Ok(Node::Object(vec![
            ("__type".to_string(), Node::Str(self.variant)),
            ("_value".to_string(), Node::Array(self.values)),
        ]))
    }
}

struct StructVariantBuilder {
    variant: String,
    entries: Vec<(String, Node)>,
}

impl SerializeStructVariant for StructVariantBuilder {
    type Ok = Node;
    type Error = Error;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Error> {
        upsert(
            &mut self.entries,
            key.to_string(),
            value.serialize(NodeSerializer)?,
        );
        Ok(())
    }
    fn end(self) -> Result<Node, Error> {
        let mut entries = vec![("__type".to_string(), Node::Str(self.variant))];
        entries.extend(self.entries);
        Ok(Node::Object(entries))
    }
}

/// 对象键必须能转成字符串（SML 的键是裸词/字符串）—— 与 `serde_bridge.rs` 同口径。
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
        fn serialize_i128(_v: i128) -> Result<String, Error>;
        fn serialize_u8(_v: u8) -> Result<String, Error>;
        fn serialize_u16(_v: u16) -> Result<String, Error>;
        fn serialize_u32(_v: u32) -> Result<String, Error>;
        fn serialize_u64(_v: u64) -> Result<String, Error>;
        fn serialize_u128(_v: u128) -> Result<String, Error>;
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
