// SPDX-License-Identifier: MulanPSL-2.0
//! `TYPED` segment (0x70): typed variables keyed by `var_id` (N04 / spec §25).
//! `TYPED` 段（0x70）：按 `var_id` 取的强类型变量（N04 / §25）。
//!
//! # What it is for ｜ 解决什么
//!
//! `VALUE` stores a named tree: every field carries its key in `STRTAB`. A struct that is
//! known at compile time can instead store **numeric ids + a fixed layout**, which skips the
//! names entirely and decodes straight into the type. It is an **additive side channel**:
//! `VALUE` is always written, canonical equality and signatures still look at `VALUE` only
//! (§25.1), and readers that do not know `TYPED` skip it by the segment header's `len`.
//! ｜ `VALUE` 存的是带名字的树；编译期已知的结构体可以只存「数字 id + 固定布局」。它是**附加通道**：
//! `VALUE` 必写、canonical 与签名只看 `VALUE`、不认的读取方按段头 `len` 跳过。
//!
//! # Body layout ｜ 段体布局
//!
//! ```text
//! TYPED   = count:varint ‖ entries[count] ‖ payload
//! entries = (var_id:varint ‖ type_id:u64le ‖ off:u32le ‖ len:u32le)*   sorted by var_id
//! payload[off..off+len] = the encoded value (no header)
//! ```
//!
//! `type_id` sits in the **directory**, not in the payload: §25.5 requires a read-time
//! comparison, and §25.2's three-field entry had nowhere to put it. No file existed when this
//! was added, so the layout was extended instead of shipping an unverifiable contract.
//! ｜ `type_id` 放在**目录**里：§25.5 要求读时比对，而 §25.2 的三字段项没地方放。
//! 加这一笔时还没有任何文件，故直接扩了布局，而不是发一份无法校验的契约。
//!
//! # Wire format ｜ 线格式
//!
//! Integers are LEB128 (signed: zigzag), `f32`/`f64` are fixed little-endian, strings/bytes
//! are `len:varint ‖ bytes`, `Option` is a 1-byte tag, arrays are `count:varint ‖ items`.
//! Structures are written **in full, field id ascending** — no presence bitset, because the
//! type is not allowed to change (§25.5).
//! ｜ 结构体全量写、字段 id 升序 —— 类型不许变，故不需要 presence bitset。

use std::collections::{BTreeMap, HashMap};

use sml_value::{Map, Value};

/// Variable id; `0` is reserved for "unspecified" ｜ 变量 id；`0` 保留给「未指定」。
pub type VarId = u32;

/// Decode depth cap (defensive; also bounds hostile nesting) ｜ 解码深度上限。
const MAX_DEPTH: u32 = 64;
/// Element-count cap per array (defensive; stops "declare 4 G elements" bombs) ｜ 单个数组的元素上限。
const MAX_ELEMS: u64 = 1_000_000;

/// Stable fingerprint of a type description ｜ 类型描述的稳定指纹（FNV-1a 64）。
///
/// Use names of **fields and variants**, not the type's own name alone:
/// `type_hash(&["struct", "host", "port"])`. A proc-macro will generate these later; hand
/// impls write the constant directly. ｜ 用字段/变体名；将来由过程宏生成，手写实现直接写常量。
pub const fn type_hash(parts: &[&str]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < parts.len() {
        let bytes = parts[i].as_bytes();
        let mut j = 0;
        while j < bytes.len() {
            h ^= bytes[j] as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
            j += 1;
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

/// Mix a tag with a child type id (const so it can feed `TYPE_ID`) ｜ 把标签与子类型 id 混起来。
const fn mix(tag: u64, child: u64) -> u64 {
    let mut h = tag ^ 0x9e37_79b9_7f4a_7c15;
    h ^= child;
    h = h.wrapping_mul(0x0000_0100_0000_01b3);
    h ^= h >> 29;
    h
}

const TAG_VEC: u64 = 0x01;
const TAG_OPTION: u64 = 0x02;

/// Encode into a `TYPED` payload ｜ 编进 `TYPED` 载荷。
pub trait Encode {
    /// Stable type id; compare on read ｜ 稳定类型 id；读时比对。
    const TYPE_ID: u64;
    /// Append the encoded bytes ｜ 追加编码后的字节。
    fn encode(&self, e: &mut Encoder<'_>);
}

/// Decode from a `TYPED` payload ｜ 从 `TYPED` 载荷解码。
pub trait Decode: Sized {
    /// Stable type id; must equal the directory's ｜ 稳定类型 id；必须与目录里的一致。
    const TYPE_ID: u64;
    /// Decode one value ｜ 解一个值。
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError>;
}

/// Write-side buffer. ｜ 写入端缓冲。
pub struct Encoder<'a> {
    out: &'a mut Vec<u8>,
}

impl<'a> Encoder<'a> {
    /// Write an unsigned LEB128 ｜ 写无符号 LEB128。
    pub fn put_varint(&mut self, mut v: u64) {
        loop {
            let byte = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.out.push(byte);
                return;
            }
            self.out.push(byte | 0x80);
        }
    }

    /// Write a zigzag-encoded signed integer ｜ 写 zigzag 有符号整数。
    pub fn put_i64(&mut self, v: i64) {
        self.put_varint(((v << 1) ^ (v >> 63)) as u64);
    }

    /// Write raw bytes with a length prefix ｜ 写「长度 + 原始字节」。
    pub fn put_bytes(&mut self, b: &[u8]) {
        self.put_varint(b.len() as u64);
        self.out.extend_from_slice(b);
    }

    /// Write a `u64` little-endian (used for the type id) ｜ 写小端 `u64`。
    pub fn put_u64le(&mut self, v: u64) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }
}

/// Read-side cursor with defensive budgets ｜ 带防御性预算的读取游标。
pub struct Decoder<'a> {
    b: &'a [u8],
    pos: usize,
    depth: u32,
}

impl<'a> Decoder<'a> {
    /// Wrap a payload slice ｜ 包一段载荷。
    pub fn new(b: &'a [u8]) -> Self {
        Decoder { b, pos: 0, depth: 0 }
    }

    /// Bytes left ｜ 剩余字节数。
    pub fn remaining(&self) -> usize {
        self.b.len() - self.pos
    }

    /// Read an unsigned LEB128 ｜ 读无符号 LEB128。
    pub fn get_varint(&mut self) -> Result<u64, TypedError> {
        let start = self.pos;
        let mut shift = 0u32;
        let mut out = 0u64;
        loop {
            if shift > 63 {
                return Err(TypedError::BadVarint { at: start });
            }
            let byte = *self
                .b
                .get(self.pos)
                .ok_or(TypedError::Truncated { at: start })?;
            self.pos += 1;
            out |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return Ok(out);
            }
            shift += 7;
        }
    }

    /// Read a zigzag signed integer ｜ 读 zigzag 有符号整数。
    pub fn get_i64(&mut self) -> Result<i64, TypedError> {
        let z = self.get_varint()?;
        Ok(((z >> 1) as i64) ^ -((z & 1) as i64))
    }

    /// Read `len:varint ‖ bytes` ｜ 读「长度 + 原始字节」。
    pub fn get_bytes(&mut self) -> Result<&'a [u8], TypedError> {
        let start = self.pos;
        let len = self.get_varint()?;
        let len = usize::try_from(len).map_err(|_| TypedError::TooLong { at: start })?;
        if self.remaining() < len {
            return Err(TypedError::Truncated { at: start });
        }
        let out = &self.b[self.pos..self.pos + len];
        self.pos += len;
        Ok(out)
    }

    fn enter(&mut self) -> Result<(), TypedError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(TypedError::DepthExceeded);
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }
}

macro_rules! impl_uint {
    ($($t:ty),*) => {$(
        impl Encode for $t {
            const TYPE_ID: u64 = type_hash(&[stringify!($t)]);
            fn encode(&self, e: &mut Encoder<'_>) { e.put_varint(*self as u64); }
        }
        impl Decode for $t {
            const TYPE_ID: u64 = type_hash(&[stringify!($t)]);
            fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
                let v = d.get_varint()?;
                <$t>::try_from(v).map_err(|_| TypedError::OutOfRange { type_name: stringify!($t) })
            }
        }
    )*};
}

impl_uint!(u8, u16, u32, u64);

macro_rules! impl_int {
    ($($t:ty),*) => {$(
        impl Encode for $t {
            const TYPE_ID: u64 = type_hash(&[stringify!($t)]);
            fn encode(&self, e: &mut Encoder<'_>) { e.put_i64(*self as i64); }
        }
        impl Decode for $t {
            const TYPE_ID: u64 = type_hash(&[stringify!($t)]);
            fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
                let v = d.get_i64()?;
                <$t>::try_from(v).map_err(|_| TypedError::OutOfRange { type_name: stringify!($t) })
            }
        }
    )*};
}

impl_int!(i8, i16, i32, i64);

impl Encode for bool {
    const TYPE_ID: u64 = type_hash(&["bool"]);
    fn encode(&self, e: &mut Encoder<'_>) {
        e.put_varint(u64::from(*self));
    }
}

impl Decode for bool {
    const TYPE_ID: u64 = type_hash(&["bool"]);
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
        match d.get_varint()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(TypedError::BadBool { value: other }),
        }
    }
}

impl Encode for f32 {
    const TYPE_ID: u64 = type_hash(&["f32"]);
    fn encode(&self, e: &mut Encoder<'_>) {
        e.out.extend_from_slice(&self.to_le_bytes());
    }
}

impl Decode for f32 {
    const TYPE_ID: u64 = type_hash(&["f32"]);
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
        if d.remaining() < 4 {
            return Err(TypedError::Truncated { at: d.pos });
        }
        let raw = [d.b[d.pos], d.b[d.pos + 1], d.b[d.pos + 2], d.b[d.pos + 3]];
        d.pos += 4;
        Ok(f32::from_le_bytes(raw))
    }
}

impl Encode for f64 {
    const TYPE_ID: u64 = type_hash(&["f64"]);
    fn encode(&self, e: &mut Encoder<'_>) {
        e.out.extend_from_slice(&self.to_le_bytes());
    }
}

impl Decode for f64 {
    const TYPE_ID: u64 = type_hash(&["f64"]);
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
        if d.remaining() < 8 {
            return Err(TypedError::Truncated { at: d.pos });
        }
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&d.b[d.pos..d.pos + 8]);
        d.pos += 8;
        Ok(f64::from_le_bytes(raw))
    }
}

impl Encode for String {
    const TYPE_ID: u64 = type_hash(&["String"]);
    fn encode(&self, e: &mut Encoder<'_>) {
        e.put_bytes(self.as_bytes());
    }
}

impl Decode for String {
    const TYPE_ID: u64 = type_hash(&["String"]);
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
        let raw = d.get_bytes()?;
        std::str::from_utf8(raw)
            .map(str::to_owned)
            .map_err(|_| TypedError::BadUtf8)
    }
}

impl Encode for str {
    const TYPE_ID: u64 = type_hash(&["str"]);
    fn encode(&self, e: &mut Encoder<'_>) {
        e.put_bytes(self.as_bytes());
    }
}

impl Encode for &str {
    const TYPE_ID: u64 = <str as Encode>::TYPE_ID;
    fn encode(&self, e: &mut Encoder<'_>) {
        e.put_bytes(self.as_bytes());
    }
}

impl<T: Encode> Encode for Vec<T> {
    const TYPE_ID: u64 = mix(TAG_VEC, T::TYPE_ID);
    fn encode(&self, e: &mut Encoder<'_>) {
        e.put_varint(self.len() as u64);
        for item in self {
            item.encode(e);
        }
    }
}

impl<T: Decode> Decode for Vec<T> {
    const TYPE_ID: u64 = mix(TAG_VEC, T::TYPE_ID);
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
        let start = d.pos;
        let count = d.get_varint()?;
        if count > MAX_ELEMS {
            return Err(TypedError::TooManyElements { at: start, count });
        }
        d.enter()?;
        let mut out = Vec::with_capacity((count as usize).min(1024));
        for _ in 0..count {
            out.push(T::decode(d)?);
        }
        d.leave();
        Ok(out)
    }
}

/// Raw bytes, encoded as `len:varint ‖ bytes` (**not** one varint per byte).
/// 原始字节：`len:varint ‖ bytes`（不是每个字节一个 varint）。
///
/// A newtype is needed because `Vec<u8>` already has the generic `Vec<T>` impl; Rust has no
/// specialization, so "bytes" gets its own type instead of a conflicting impl.
/// ｜ `Vec<u8>` 已被泛型 `Vec<T>` 占用且无特化，故用独立类型而不是冲突实现。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Bytes(pub Vec<u8>);

impl Encode for Bytes {
    const TYPE_ID: u64 = type_hash(&["bytes"]);
    fn encode(&self, e: &mut Encoder<'_>) {
        e.put_bytes(&self.0);
    }
}

impl Decode for Bytes {
    const TYPE_ID: u64 = type_hash(&["bytes"]);
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
        Ok(Bytes(d.get_bytes()?.to_vec()))
    }
}

impl<T: Encode> Encode for Option<T> {
    const TYPE_ID: u64 = mix(TAG_OPTION, T::TYPE_ID);
    fn encode(&self, e: &mut Encoder<'_>) {
        match self {
            None => e.put_varint(0),
            Some(v) => {
                e.put_varint(1);
                v.encode(e);
            }
        }
    }
}

impl<T: Decode> Decode for Option<T> {
    const TYPE_ID: u64 = mix(TAG_OPTION, T::TYPE_ID);
    fn decode(d: &mut Decoder<'_>) -> Result<Self, TypedError> {
        match d.get_varint()? {
            0 => Ok(None),
            1 => {
                d.enter()?;
                let v = T::decode(d);
                d.leave();
                v.map(Some)
            }
            other => Err(TypedError::BadOptionTag { tag: other }),
        }
    }
}

/// Builder for the `TYPED` segment body ｜ `TYPED` 段体的构建器。
#[derive(Default)]
pub struct Typed {
    vars: BTreeMap<VarId, (u64, Vec<u8>)>,
}

impl Typed {
    /// Empty builder ｜ 空构建器。
    pub fn new() -> Self {
        Self::default()
    }

    /// Store one variable ｜ 存一个变量。
    ///
    /// `id = 0` and duplicate ids are rejected — never auto-assigned, never guessed (§25.2).
    /// ｜ `id = 0` 与重复 id 直接报错，不自动分配、不猜。
    pub fn put<T: Encode>(&mut self, id: VarId, v: &T) -> Result<(), TypedError> {
        if id == 0 {
            return Err(TypedError::ReservedVarId);
        }
        if self.vars.contains_key(&id) {
            return Err(TypedError::DuplicateVarId { id });
        }
        let mut buf = Vec::new();
        v.encode(&mut Encoder { out: &mut buf });
        self.vars.insert(id, (T::TYPE_ID, buf));
        Ok(())
    }

    /// Whether anything was stored ｜ 是否存过东西。
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// Number of variables ｜ 变量个数。
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// Serialise the segment body (entries sorted by `var_id`, so `get` can bisect).
    /// 序列化段体（条目按 `var_id` 升序，读端才能二分）。
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_varint_into(&mut out, self.vars.len() as u64);
        let mut off = 0u32;
        for (id, (type_id, body)) in &self.vars {
            put_varint_into(&mut out, *id as u64);
            out.extend_from_slice(&type_id.to_le_bytes());
            out.extend_from_slice(&off.to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            off += body.len() as u32;
        }
        for (_, body) in self.vars.values() {
            out.extend_from_slice(body);
        }
        out
    }
}

fn put_varint_into(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// One directory entry ｜ 一条目录项。
#[derive(Debug, Clone, Copy)]
pub struct VarEntry {
    /// Variable id ｜ 变量 id。
    pub id: VarId,
    /// Type fingerprint recorded at write time ｜ 写入时记的类型指纹。
    pub type_id: u64,
    /// Offset into the payload ｜ 载荷内偏移。
    pub off: u32,
    /// Payload length ｜ 载荷长度。
    pub len: u32,
}

/// Read the directory of a `TYPED` body ｜ 读 `TYPED` 段体的目录。
pub fn directory(segment: &[u8]) -> Result<Vec<VarEntry>, TypedError> {
    let mut d = Decoder::new(segment);
    let count = d.get_varint()?;
    if count > MAX_ELEMS {
        return Err(TypedError::TooManyElements { at: 0, count });
    }
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let id64 = d.get_varint()?;
        let id = VarId::try_from(id64).map_err(|_| TypedError::BadVarId { id: id64 })?;
        let mut t = [0u8; 8];
        if d.remaining() < 8 {
            return Err(TypedError::Truncated { at: d.pos });
        }
        t.copy_from_slice(&d.b[d.pos..d.pos + 8]);
        d.pos += 8;
        let type_id = u64::from_le_bytes(t);
        // off/len are fixed `u32le` in the entry (§25.2), **not** varints.
        // ｜ 目录项里的 off/len 是定长 `u32le`，不是 varint。
        if d.remaining() < 8 {
            return Err(TypedError::Truncated { at: d.pos });
        }
        let off = u32::from_le_bytes([d.b[d.pos], d.b[d.pos + 1], d.b[d.pos + 2], d.b[d.pos + 3]]);
        let len = u32::from_le_bytes([
            d.b[d.pos + 4],
            d.b[d.pos + 5],
            d.b[d.pos + 6],
            d.b[d.pos + 7],
        ]);
        d.pos += 8;
        out.push(VarEntry {
            id,
            type_id,
            off,
            len,
        });
    }
    Ok(out)
}

/// Decode variable `id` as `T`, checking its type id first ｜ 按 `T` 解变量 `id`，先比对类型 id。
pub fn read_var<T: Decode>(segment: &[u8], id: VarId) -> Result<T, TypedError> {
    let dir = directory(segment)?;
    let entry = dir
        .binary_search_by_key(&id, |e| e.id)
        .ok()
        .map(|i| dir[i])
        .ok_or(TypedError::NoSuchVar { id })?;
    if entry.type_id != T::TYPE_ID {
        return Err(TypedError::TypeMismatch {
            id,
            found: entry.type_id,
            expected: T::TYPE_ID,
        });
    }
    // Payload starts right after the directory; `off` is relative to it.
    let mut d = Decoder::new(segment);
    let count = d.get_varint()?;
    for _ in 0..count {
        d.get_varint()?; // id
        d.pos += 8; // type_id:u64le
        d.pos += 8; // off:u32le ‖ len:u32le
    }
    let payload_at = d.pos + entry.off as usize;
    let end = payload_at
        .checked_add(entry.len as usize)
        .ok_or(TypedError::Truncated { at: payload_at })?;
    if end > segment.len() {
        return Err(TypedError::Truncated { at: payload_at });
    }
    let mut v = Decoder::new(&segment[payload_at..end]);
    let out = T::decode(&mut v)?;
    if v.remaining() != 0 {
        return Err(TypedError::Trailing { at: v.pos });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// 内嵌 schema（D45）：用 `SCHEMA` 值把 `TYPED` 动态渲染成「变量 → 值」。
// Embedded schema (D45): render `TYPED` without compile-time types.
// ---------------------------------------------------------------------------

/// 一条动态渲染出来的变量。 ｜ One dynamically rendered variable.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentedVar {
    /// Variable id ｜ 变量 id。
    pub id: VarId,
    /// Type name from `SCHEMA` ｜ `SCHEMA` 里的类型名。
    pub name: String,
    /// Decoded value (fields in schema order) ｜ 解出的值（字段按 schema 顺序）。
    pub value: Value,
}

enum TypeRef {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Str,
    Bytes,
    Vec(Box<TypeRef>),
    Opt(Box<TypeRef>),
    Named(u64),
}

struct FieldDef {
    name: String,
    ty: TypeRef,
}

struct TypeDef {
    name: String,
    fields: Vec<FieldDef>,
}

fn schema_err(why: impl Into<String>) -> TypedError {
    TypedError::Schema(why.into())
}

fn as_obj<'a>(v: &'a Value, what: &str) -> Result<&'a Map, TypedError> {
    match v {
        Value::Object(m) => Ok(m),
        _ => Err(schema_err(format!("{what} 必须是对象"))),
    }
}

fn as_arr<'a>(v: &'a Value, what: &str) -> Result<&'a [Value], TypedError> {
    match v {
        Value::Array(a) => Ok(a),
        _ => Err(schema_err(format!("{what} 必须是数组"))),
    }
}

fn as_str<'a>(v: &'a Value, what: &str) -> Result<&'a str, TypedError> {
    match v {
        Value::Str(s) => Ok(s),
        _ => Err(schema_err(format!("{what} 必须是字符串"))),
    }
}

fn as_u64(v: &Value, what: &str) -> Result<u64, TypedError> {
    match v {
        Value::Int(i) if *i >= 0 => Ok(*i as u64),
        _ => Err(schema_err(format!("{what} 必须是非负整数"))),
    }
}

fn parse_type_id(s: &str) -> Result<u64, TypedError> {
    if s.len() != 16 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(schema_err("type id 必须是 16 位十六进制"));
    }
    u64::from_str_radix(s, 16).map_err(|_| schema_err("type id 不是合法十六进制"))
}

fn parse_ty(v: &Value) -> Result<TypeRef, TypedError> {
    match v {
        Value::Str(s) => match s.as_str() {
            "bool" => Ok(TypeRef::Bool),
            "u8" => Ok(TypeRef::U8),
            "u16" => Ok(TypeRef::U16),
            "u32" => Ok(TypeRef::U32),
            "u64" => Ok(TypeRef::U64),
            "i8" => Ok(TypeRef::I8),
            "i16" => Ok(TypeRef::I16),
            "i32" => Ok(TypeRef::I32),
            "i64" => Ok(TypeRef::I64),
            "f32" => Ok(TypeRef::F32),
            "f64" => Ok(TypeRef::F64),
            "str" => Ok(TypeRef::Str),
            "bytes" => Ok(TypeRef::Bytes),
            other => Err(schema_err(format!("未知 ty `{other}`"))),
        },
        Value::Object(m) => {
            if let Some(inner) = m.get("vec") {
                return Ok(TypeRef::Vec(Box::new(parse_ty(inner)?)));
            }
            if let Some(inner) = m.get("opt") {
                return Ok(TypeRef::Opt(Box::new(parse_ty(inner)?)));
            }
            if let Some(inner) = m.get("type") {
                return Ok(TypeRef::Named(parse_type_id(as_str(inner, "type")?)?));
            }
            Err(schema_err("ty 对象要有 vec / opt / type 之一"))
        }
        _ => Err(schema_err("ty 必须是字符串或对象")),
    }
}

fn parse_schema(schema: &Value) -> Result<HashMap<u64, TypeDef>, TypedError> {
    let top = as_obj(schema, "SCHEMA")?;
    let version = top
        .get("schema_version")
        .ok_or_else(|| schema_err("缺 schema_version"))?;
    if as_u64(version, "schema_version")? != 1 {
        return Err(schema_err("schema_version 只支持 1"));
    }
    let types = as_arr(
        top.get("types").ok_or_else(|| schema_err("缺 types"))?,
        "types",
    )?;
    let mut out = HashMap::new();
    for t in types {
        let o = as_obj(t, "types[]")?;
        let id = parse_type_id(as_str(
            o.get("id").ok_or_else(|| schema_err("type 缺 id"))?,
            "id",
        )?)?;
        let name = as_str(
            o.get("name").ok_or_else(|| schema_err("type 缺 name"))?,
            "name",
        )?
        .to_string();
        let kind = match o.get("kind") {
            Some(v) => as_str(v, "kind")?,
            None => "struct",
        };
        if kind != "struct" {
            return Err(schema_err("v1 只支持 kind: struct"));
        }
        let fields_v = as_arr(
            o.get("fields").ok_or_else(|| schema_err("type 缺 fields"))?,
            "fields",
        )?;
        let mut fields = Vec::new();
        let mut last = 0u64;
        for f in fields_v {
            let fo = as_obj(f, "fields[]")?;
            let fid = as_u64(fo.get("id").ok_or_else(|| schema_err("field 缺 id"))?, "field id")?;
            if fid <= last {
                return Err(schema_err("fields 必须按 id 严格升序"));
            }
            last = fid;
            let fname = as_str(
                fo.get("name").ok_or_else(|| schema_err("field 缺 name"))?,
                "field name",
            )?
            .to_string();
            let ty = parse_ty(fo.get("ty").ok_or_else(|| schema_err("field 缺 ty"))?)?;
            fields.push(FieldDef { name: fname, ty });
        }
        if out.insert(id, TypeDef { name, fields }).is_some() {
            return Err(schema_err("type id 重复"));
        }
    }
    Ok(out)
}

fn read_fixed<const N: usize>(d: &mut Decoder<'_>) -> Result<[u8; N], TypedError> {
    let at = d.pos;
    if d.remaining() < N {
        return Err(TypedError::Truncated { at });
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&d.b[d.pos..d.pos + N]);
    d.pos += N;
    Ok(out)
}

fn hex_string(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push_str(&format!("{x:02x}"));
    }
    s
}

fn decode_struct(
    d: &mut Decoder<'_>,
    t: &TypeDef,
    types: &HashMap<u64, TypeDef>,
) -> Result<Value, TypedError> {
    d.enter()?;
    let mut m = Map::ordered();
    for f in &t.fields {
        let v = decode_ty(d, &f.ty, types)?;
        m.insert(f.name.clone(), v);
    }
    d.leave();
    Ok(Value::Object(m))
}

fn decode_ty(
    d: &mut Decoder<'_>,
    ty: &TypeRef,
    types: &HashMap<u64, TypeDef>,
) -> Result<Value, TypedError> {
    let uint = |d: &mut Decoder<'_>, max: u64, name: &'static str| -> Result<Value, TypedError> {
        let v = d.get_varint()?;
        if v > max {
            return Err(TypedError::OutOfRange { type_name: name });
        }
        Ok(Value::Int(v as i64))
    };
    match ty {
        TypeRef::Bool => match d.get_varint()? {
            0 => Ok(Value::Bool(false)),
            1 => Ok(Value::Bool(true)),
            v => Err(TypedError::BadBool { value: v }),
        },
        TypeRef::U8 => uint(d, u8::MAX as u64, "u8"),
        TypeRef::U16 => uint(d, u16::MAX as u64, "u16"),
        TypeRef::U32 => uint(d, u32::MAX as u64, "u32"),
        TypeRef::U64 => {
            let v = d.get_varint()?;
            i64::try_from(v)
                .map(Value::Int)
                .map_err(|_| TypedError::OutOfRange { type_name: "u64" })
        }
        TypeRef::I8 => {
            let v = d.get_i64()?;
            i8::try_from(v).map_err(|_| TypedError::OutOfRange { type_name: "i8" })?;
            Ok(Value::Int(v))
        }
        TypeRef::I16 => {
            let v = d.get_i64()?;
            i16::try_from(v).map_err(|_| TypedError::OutOfRange { type_name: "i16" })?;
            Ok(Value::Int(v))
        }
        TypeRef::I32 => {
            let v = d.get_i64()?;
            i32::try_from(v).map_err(|_| TypedError::OutOfRange { type_name: "i32" })?;
            Ok(Value::Int(v))
        }
        TypeRef::I64 => Ok(Value::Int(d.get_i64()?)),
        TypeRef::F32 => {
            let raw: [u8; 4] = read_fixed(d)?;
            Ok(Value::float(f32::from_le_bytes(raw) as f64))
        }
        TypeRef::F64 => {
            let raw: [u8; 8] = read_fixed(d)?;
            Ok(Value::float(f64::from_le_bytes(raw)))
        }
        TypeRef::Str => {
            let b = d.get_bytes()?;
            let s = std::str::from_utf8(b).map_err(|_| TypedError::BadUtf8)?;
            Ok(Value::Str(s.to_string()))
        }
        TypeRef::Bytes => Ok(Value::Str(hex_string(d.get_bytes()?))),
        TypeRef::Vec(inner) => {
            let count = d.get_varint()?;
            if count > MAX_ELEMS {
                return Err(TypedError::TooManyElements { at: d.pos, count });
            }
            d.enter()?;
            let mut out = Vec::with_capacity((count as usize).min(1024));
            for _ in 0..count {
                out.push(decode_ty(d, inner, types)?);
            }
            d.leave();
            Ok(Value::Array(out))
        }
        TypeRef::Opt(inner) => match d.get_varint()? {
            0 => Ok(Value::Null),
            1 => decode_ty(d, inner, types),
            tag => Err(TypedError::BadOptionTag { tag }),
        },
        TypeRef::Named(id) => {
            let t = types
                .get(id)
                .ok_or(TypedError::UnknownType { id: *id })?;
            decode_struct(d, t, types)
        }
    }
}

fn payload_base(segment: &[u8]) -> Result<usize, TypedError> {
    let mut d = Decoder::new(segment);
    let count = d.get_varint()?;
    if count > MAX_ELEMS {
        return Err(TypedError::TooManyElements { at: 0, count });
    }
    for _ in 0..count {
        d.get_varint()?;
        if d.remaining() < 16 {
            return Err(TypedError::Truncated { at: d.pos });
        }
        d.pos += 16;
    }
    Ok(d.pos)
}

/// 用内嵌 `SCHEMA` 渲染 `TYPED`：字段名与顺序都来自 schema（D45）。
/// Render `TYPED` via an embedded `SCHEMA`; field names and order come from the schema.
pub fn documented(segment: &[u8], schema: &Value) -> Result<Vec<DocumentedVar>, TypedError> {
    let types = parse_schema(schema)?;
    let dir = directory(segment)?;
    let base = payload_base(segment)?;
    let mut out = Vec::with_capacity(dir.len());
    for e in dir {
        let t = types
            .get(&e.type_id)
            .ok_or(TypedError::UnknownType { id: e.type_id })?;
        let start = base
            .checked_add(e.off as usize)
            .ok_or(TypedError::Truncated { at: base })?;
        let end = start
            .checked_add(e.len as usize)
            .ok_or(TypedError::Truncated { at: start })?;
        if end > segment.len() {
            return Err(TypedError::Truncated { at: start });
        }
        let mut d = Decoder::new(&segment[start..end]);
        let value = decode_struct(&mut d, t, &types)?;
        if d.remaining() != 0 {
            return Err(TypedError::Trailing { at: d.pos });
        }
        out.push(DocumentedVar {
            id: e.id,
            name: t.name.clone(),
            value,
        });
    }
    Ok(out)
}

/// `TYPED` failures ｜ `TYPED` 失败原因。
#[derive(Debug)]
pub enum TypedError {
    /// `var_id = 0` is reserved ｜ `var_id = 0` 保留。
    ReservedVarId,
    /// Same id stored twice ｜ 同一个 id 存了两次。
    DuplicateVarId { id: VarId },
    /// No such variable in this file ｜ 文件里没有这个变量。
    NoSuchVar { id: VarId },
    /// Stored type id differs from the requested one ｜ 存的类型 id 与要的不一致。
    TypeMismatch {
        id: VarId,
        found: u64,
        expected: u64,
    },
    /// Id does not fit `u32` ｜ id 超出 `u32`。
    BadVarId { id: u64 },
    /// Payload ended early ｜ 载荷提前结束。
    Truncated { at: usize },
    /// Bytes left after decoding one value ｜ 解完一个值后还有剩余字节。
    Trailing { at: usize },
    /// LEB128 longer than 10 bytes ｜ LEB128 超过 10 字节。
    BadVarint { at: usize },
    /// Declared length does not fit the buffer ｜ 声明长度超出缓冲。
    TooLong { at: usize },
    /// Integer out of range for the target type ｜ 整数超出目标类型范围。
    OutOfRange { type_name: &'static str },
    /// Bool tag other than 0/1 ｜ 布尔的 tag 不是 0/1。
    BadBool { value: u64 },
    /// Option tag other than 0/1 ｜ Option 的 tag 不是 0/1。
    BadOptionTag { tag: u64 },
    /// Invalid UTF-8 in a `String` ｜ 字符串不是合法 UTF-8。
    BadUtf8,
    /// Array/entry count above the cap ｜ 元素数超过上限。
    TooManyElements { at: usize, count: u64 },
    /// Nesting deeper than [`MAX_DEPTH`] ｜ 嵌套超过上限。
    DepthExceeded,
    /// Not a `TYPED` body (empty or too short) ｜ 不是 `TYPED` 段体。
    NotTyped,
    /// `SCHEMA` is malformed ｜ `SCHEMA` 不合法。
    Schema(String),
    /// `SCHEMA` has no definition for this `type_id` ｜ `SCHEMA` 里没有这个 type_id。
    UnknownType { id: u64 },
}

impl std::fmt::Display for TypedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypedError::ReservedVarId => write!(f, "var_id = 0 是保留值"),
            TypedError::DuplicateVarId { id } => write!(f, "var_id {id} 重复写入"),
            TypedError::NoSuchVar { id } => write!(f, "文件里没有 var_id {id}"),
            TypedError::TypeMismatch {
                id,
                found,
                expected,
            } => write!(
                f,
                "var_id {id} 的类型不符：文件里是 {found:016x}，代码里要 {expected:016x}"
            ),
            TypedError::BadVarId { id } => write!(f, "var_id {id} 超出 u32"),
            TypedError::Truncated { at } => write!(f, "TYPED 载荷在偏移 {at} 处结束"),
            TypedError::Trailing { at } => write!(f, "解完一个值后仍有剩余字节（偏移 {at}）"),
            TypedError::BadVarint { at } => write!(f, "偏移 {at} 处的 LEB128 超过 10 字节"),
            TypedError::TooLong { at } => write!(f, "偏移 {at} 处声明的长度超出缓冲"),
            TypedError::OutOfRange { type_name } => write!(f, "整数超出 {type_name} 的范围"),
            TypedError::BadBool { value } => write!(f, "布尔的 tag 只能是 0/1，读到 {value}"),
            TypedError::BadOptionTag { tag } => write!(f, "Option 的 tag 只能是 0/1，读到 {tag}"),
            TypedError::BadUtf8 => write!(f, "字符串不是合法 UTF-8"),
            TypedError::TooManyElements { at, count } => {
                write!(f, "偏移 {at} 处声明 {count} 个元素，超过上限")
            }
            TypedError::DepthExceeded => write!(f, "嵌套深度超过 {MAX_DEPTH}"),
            TypedError::NotTyped => write!(f, "不是 TYPED 段体"),
            TypedError::Schema(why) => write!(f, "SCHEMA 不合法：{why}"),
            TypedError::UnknownType { id } => write!(f, "SCHEMA 里没有 type_id {id:016x}"),
        }
    }
}

impl std::error::Error for TypedError {}

impl From<TypedError> for crate::container::ContainerError {
    fn from(e: TypedError) -> Self {
        crate::container::ContainerError::Typed(e.to_string())
    }
}
