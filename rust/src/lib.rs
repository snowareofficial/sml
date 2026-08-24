//! SML — SNOWARE Markup Language (Rust 实现, crate 名 `sml`)
//!
//! 声明式数据/配置格式, JSON/YAML 的替代品。语法与 Soup 生态的
//! `lib/sml.soup` (Lua) 对齐:
//!
//! ```sml
//! firstName: John
//! age: 27
//! address:
//! {
//!     streetAddress: "21 2nd Street"
//!     state: NY
//! }
//! phoneNumbers: [ { type: home } { type: office } ]
//! @base { region: cn-north-1 }
//! server web { &base port: 8080 }
//! ```
//!
//! 特性:
//! - 引号可选 (裸词即字符串)
//! - 块冒号可省 (`address { }` ≡ `address: { }`)
//! - 数组分隔灵活 (逗号可选)
//! - 片段继承 (`@name { }` 定义 / `&name` 引用)
//! - `$env.VAR` 环境变量内联
//! - `#` 行注释
//! - 类型自识别: true/false -> bool, null -> None, 数字 -> i64/f64, 其余 -> String
//!
//! 值模型: `Value` 枚举 (与 JSON 同构, 另加 `__type`/`__name` 裸块元数据)。

use std::collections::BTreeMap;
use std::fmt;

// ---------------------------------------------------------------------------
// 值模型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    /// 对象/块; `__type` / `__name` 裸块元数据以保留字键存放
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// 对象字段按需取 (支持 "." 点路径)
    pub fn get(&self, path: &str) -> Option<&Value> {
        let mut cur = self;
        for seg in path.split('.') {
            match cur {
                Value::Object(m) => cur = m.get(seg)?,
                _ => return None,
            }
        }
        Some(cur)
    }
    /// 字符串视图 (字符串直接返回; 其它返回 None)
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", to_sml(self))
    }
}

// ---------------------------------------------------------------------------
// 解析: 词法 + 递归下降
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    LBrace,  // {
    RBrace,  // }
    LBrack,  // [
    RBrack,  // ]
    Comma,   // ,
    Colon,   // :
    At,      // @
    Str(String),   // 引号串 (已解码)
    Word(String),  // 裸词
}

fn tokenize(text: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    let n = bytes.len();
    let mut buf = String::new();
    let mut flush = |buf: &mut String, toks: &mut Vec<Tok>| {
        if !buf.is_empty() {
            toks.push(Tok::Word(std::mem::take(buf)));
        }
    };
    while i < n {
        let c = bytes[i] as char;
        match c {
            '#' => {
                while i < n && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            '"' => {
                flush(&mut buf, &mut toks);
                i += 1;
                let mut s = String::new();
                while i < n {
                    let cc = bytes[i];
                    if cc == b'"' {
                        i += 1;
                        break;
                    }
                    if cc == b'\\' && i + 1 < n {
                        i += 1;
                        let e = bytes[i];
                        s.push(match e {
                            b'n' => '\n',
                            b't' => '\t',
                            b'r' => '\r',
                            b'0' => '\0',
                            b'"' => '"',
                            b'\\' => '\\',
                            other => other as char,
                        });
                        i += 1;
                    } else {
                        s.push(cc as char);
                        i += 1;
                    }
                }
                toks.push(Tok::Str(s));
            }
            '{' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::RBrace);
                i += 1;
            }
            '[' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::LBrack);
                i += 1;
            }
            ']' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::RBrack);
                i += 1;
            }
            ',' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::Comma);
                i += 1;
            }
            ':' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::Colon);
                i += 1;
            }
            '@' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::At);
                i += 1;
            }
            ' ' | '\t' | '\n' | '\r' => {
                flush(&mut buf, &mut toks);
                i += 1;
            }
            _ => {
                buf.push(c);
                i += 1;
            }
        }
    }
    flush(&mut buf, &mut toks);
    Ok(toks)
}

fn coerce_word(w: &str, fragments: &BTreeMap<String, Value>) -> Value {
    match w {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        "null" => return Value::Null,
        _ => {}
    }
    // $env.VAR 内联
    if let Some(ev) = w.strip_prefix("$env.") {
        return Value::Str(std::env::var(ev).unwrap_or_default());
    }
    // 片段引用 &name
    if let Some(name) = w.strip_prefix('&') {
        if let Some(v) = fragments.get(name) {
            return v.clone();
        }
        return Value::Str(w.to_string());
    }
    // 数字: int / float / 科学计数
    if let Ok(i) = w.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = w.parse::<f64>() {
        return Value::Float(f);
    }
    Value::Str(w.to_string())
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
    fragments: BTreeMap<String, Value>,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.i).cloned();
        if t.is_some() {
            self.i += 1;
        }
        t
    }

    /// 解析对象/块, 直到遇到 closing (None=顶层)
    fn parse_block(&mut self, closing: Option<Tok>) -> Result<Value, String> {
        let mut node: BTreeMap<String, Value> = BTreeMap::new();
        loop {
            let tok = match self.peek().cloned() {
                None => break,
                Some(t) => t,
            };
            match tok {
                Tok::RBrace | Tok::RBrack => {
                    if let Some(cl) = &closing {
                        if *cl == tok {
                            self.next();
                            break;
                        }
                    }
                    // 顶层遇右括号也停
                    break;
                }
                Tok::Comma => {
                    self.next();
                }
                Tok::At => {
                    // @name { ... } 片段定义 (不进主树)
                    self.next();
                    let fname = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        _ => return Err("sml: @ 后需片段名".into()),
                    };
                    if self.peek() == Some(&Tok::Colon) {
                        self.next();
                    }
                    // 可选 type [name] 参数
                    let mut ftype: Option<String> = None;
                    let mut farg: Option<String> = None;
                    if let Some(Tok::Word(s)) = self.peek().cloned() {
                        if *self.peek().unwrap() != Tok::LBrace {
                            self.next();
                            ftype = Some(s);
                            if let Some(Tok::Word(s2)) = self.peek().cloned() {
                                if *self.peek().unwrap() != Tok::LBrace {
                                    self.next();
                                    farg = Some(s2);
                                }
                            }
                        }
                    }
                    if self.peek() == Some(&Tok::LBrace) {
                        self.next();
                        let mut sub = match self.parse_block(Some(Tok::RBrace))? {
                            Value::Object(m) => m,
                            other => {
                                let mut m = BTreeMap::new();
                                m.insert("_value".into(), other);
                                m
                            }
                        };
                        if let Some(t) = ftype {
                            sub.insert("__type".into(), Value::Str(t));
                        }
                        if let Some(a) = farg {
                            sub.insert("__name".into(), Value::Str(a));
                        }
                        self.fragments.insert(fname, Value::Object(sub));
                    }
                }
                _ => {
                    // key
                    let key = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        other => return Err(format!("sml: 期望键, 得 {:?}", other)),
                    };
                    let colon = self.peek() == Some(&Tok::Colon);
                    if colon {
                        self.next();
                    }
                    let val = self.parse_value(&key, colon)?;
                    // 同名冲突 -> 提升为数组
                    if let Some(existing) = node.get_mut(&key) {
                        match existing {
                            Value::Array(a) => a.push(val),
                            _ => {
                                let old = node.remove(&key).unwrap();
                                node.insert(key, Value::Array(vec![old, val]));
                            }
                        }
                    } else {
                        node.insert(key, val);
                    }
                }
            }
        }
        Ok(Value::Object(node))
    }

    /// 解析一个值 (在 key 之后)
    fn parse_value(&mut self, key: &str, colon: bool) -> Result<Value, String> {
        // 无冒号且后继是裸词: 可能是裸块 `type [name] { }`
        if !colon && matches!(self.peek(), Some(Tok::Word(_))) {
            // 预扫描: 收集参数直到 { / 结束; 若发现 { 则按裸块处理
            let mut probe = self.i;
            let mut found_block = false;
            while probe < self.toks.len() {
                match &self.toks[probe] {
                    Tok::Word(_) | Tok::Str(_) => probe += 1,
                    Tok::LBrace => {
                        found_block = true;
                        break;
                    }
                    _ => break,
                }
            }
            if found_block {
                // 裸块: key 为类型, 参数在 { 前
                let mut args: Vec<Value> = Vec::new();
                while let Some(t) = self.peek().cloned() {
                    match t {
                        Tok::Word(w) => {
                            args.push(coerce_word(&w, &self.fragments));
                            self.next();
                        }
                        Tok::Str(_) => {
                            if let Some(Tok::Str(s)) = self.next() {
                                args.push(Value::Str(s));
                            }
                        }
                        _ => break,
                    }
                }
                if self.peek() == Some(&Tok::LBrace) {
                    self.next();
                    let mut sub = self.parse_block(Some(Tok::RBrace))?;
                    if let Value::Object(m) = &mut sub {
                        m.insert("__type".into(), Value::Str(key.to_string()));
                        if args.len() == 1 {
                            m.insert("__name".into(), args.remove(0));
                        }
                    }
                    return Ok(sub);
                }
            }
        }
        match self.peek().cloned() {
            Some(Tok::LBrace) => {
                self.next();
                self.parse_block(Some(Tok::RBrace))
            }
            Some(Tok::LBrack) => {
                self.next();
                self.parse_array()
            }
            Some(tok @ (Tok::Word(_) | Tok::Str(_))) => {
                let v = match tok {
                    Tok::Word(w) => coerce_word(&w, &self.fragments),
                    Tok::Str(s) => {
                        let ev = s.strip_prefix("$env.");
                        match ev {
                            Some(name) => Value::Str(std::env::var(name).unwrap_or_default()),
                            None => Value::Str(s),
                        }
                    }
                    _ => unreachable!(),
                };
                self.next();
                Ok(v)
            }
            // 键后无值: `key }` / `key ]` / `key ,` / 行尾 —— key 本身即值 (片段引用/裸词)
            Some(Tok::RBrace) | Some(Tok::RBrack) | Some(Tok::Comma) | None => {
                if colon {
                    // 有冒号但无值: 空值
                    Ok(Value::Null)
                } else {
                    Ok(coerce_word(key, &self.fragments))
                }
            }
            _ => Err("sml: 语法错误".into()),
        }
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        let mut arr = Vec::new();
        loop {
            match self.peek().cloned() {
                None => break,
                Some(Tok::RBrack) => {
                    self.next();
                    break;
                }
                Some(Tok::Comma) => {
                    self.next();
                }
                Some(Tok::LBrace) => {
                    self.next();
                    arr.push(self.parse_block(Some(Tok::RBrace))?);
                }
                Some(Tok::Word(w)) => {
                    arr.push(coerce_word(&w, &self.fragments));
                    self.next();
                }
                Some(Tok::Str(_)) => {
                    if let Some(Tok::Str(s)) = self.next() {
                        arr.push(Value::Str(s));
                    }
                }
                _ => break,
            }
        }
        Ok(Value::Array(arr))
    }
}

/// 解析 SML 文本
pub fn parse(text: &str) -> Result<Value, String> {
    let toks = tokenize(text)?;
    let mut p = Parser {
        toks,
        i: 0,
        fragments: BTreeMap::new(),
    };
    p.parse_block(None)
}

/// 解析到对象 (失败抛 `ParseError`)
pub fn loads(text: &str) -> Result<Value, ParseError> {
    parse(text).map_err(ParseError)
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sml parse error: {}", self.0)
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// 序列化
// ---------------------------------------------------------------------------

fn quote_if_needed(s: &str) -> String {
    if s.is_empty() || s.contains([' ', '\t', '\n', '\r', ':', '#', '{', '}']) {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

fn dump_value(v: &Value, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&format!("{}", f)),
        Value::Str(s) => out.push_str(&quote_if_needed(s)),
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
            } else {
                out.push('[');
                for e in a {
                    out.push('\n');
                    out.push_str(&format!("{}{}", "  ".repeat(indent + 1), dump_inline(e)));
                }
                out.push_str(&format!("\n{}]", pad));
            }
        }
        Value::Object(m) => {
            let has_body = m.iter().any(|(k, _)| k != "__type" && k != "__name");
            if !has_body {
                out.push_str("{}");
                return;
            }
            out.push_str(&format!("\n{}{{", pad));
            for (k, val) in m {
                if k == "__type" || k == "__name" {
                    continue;
                }
                out.push_str(&format!("\n{}{}: ", "  ".repeat(indent + 1), k));
                dump_value(val, indent + 1, out);
            }
            out.push_str(&format!("\n{}}}", pad));
        }
    }
}

fn dump_scalar(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Str(s) => quote_if_needed(s),
        _ => "".into(),
    }
}

fn dump_inline(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            let parts: Vec<String> = m
                .iter()
                .filter(|(k, _)| k.as_str() != "__type" && k.as_str() != "__name")
                .map(|(k, val)| {
                    let vs = match val {
                        Value::Str(s) => quote_if_needed(s),
                        Value::Int(i) => i.to_string(),
                        Value::Float(f) => f.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Null => "null".to_string(),
                        Value::Array(_) => "[..]".to_string(),
                        Value::Object(_) => "{..}".to_string(),
                    };
                    format!("{}: {}", k, vs)
                })
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(dump_inline).collect();
            format!("[ {} ]", parts.join(", "))
        }
        other => dump_scalar(other),
    }
}

/// 序列化回 SML 文本 (round-trip)
pub fn to_sml(v: &Value) -> String {
    let mut out = String::new();
    if let Value::Object(m) = v {
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            out.push_str(&format!("{}: ", k));
            dump_value(val, 0, &mut out);
            out.push('\n');
        }
    } else {
        out.push_str(&dump_inline(v));
    }
    out
}

// ---------------------------------------------------------------------------
// C-ABI (cdylib, 供 C / 其它语言调用)
// ---------------------------------------------------------------------------

use std::os::raw::{c_char, c_int};
use std::ptr;

fn cstr(s: &str) -> *mut c_char {
    let c = std::ffi::CString::new(s).unwrap_or_default();
    c.into_raw()
}

/// sml_parse(text) -> 返回 JSON 字符串 (调用方 sml_free 释放); 失败返回 NULL
#[no_mangle]
pub extern "C" fn sml_parse(text: *const c_char) -> *mut c_char {
    if text.is_null() {
        return ptr::null_mut();
    }
    let t = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned();
    match parse(&t) {
        Ok(v) => cstr(&jsonify(&v)),
        Err(_) => ptr::null_mut(),
    }
}

/// sml_dump(json) -> 接受 JSON 字符串, 序列化为 SML; 调用方 sml_free
#[no_mangle]
pub extern "C" fn sml_dump(json: *const c_char) -> *mut c_char {
    if json.is_null() {
        return ptr::null_mut();
    }
    let j = unsafe { std::ffi::CStr::from_ptr(json) }.to_string_lossy().into_owned();
    match json_to_value(&j) {
        Some(v) => cstr(&to_sml(&v)),
        None => ptr::null_mut(),
    }
}

/// sml_free(p): 释放由 sml_parse / sml_dump 返回的字符串
#[no_mangle]
pub unsafe extern "C" fn sml_free(p: *mut c_char) {
    if !p.is_null() {
        drop(unsafe { std::ffi::CString::from_raw(p) });
    }
}

/// sml_version() -> 版本字符串 (调用方 sml_free)
#[no_mangle]
pub extern "C" fn sml_version() -> *mut c_char {
    cstr(concat!("sml ", env!("CARGO_PKG_VERSION")))
}

// ---------------------------------------------------------------------------
// 内部: JSON <-> Value (供 C-ABI 便捷桥)
// ---------------------------------------------------------------------------

fn jsonify(v: &Value) -> String {
    fn esc(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Str(s) => format!("\"{}\"", esc(s)),
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(jsonify).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Object(m) => {
            let parts: Vec<String> = m
                .iter()
                .map(|(k, val)| format!("\"{}\":{}", esc(k), jsonify(val)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

fn json_to_value(s: &str) -> Option<Value> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let _n = bytes.len();
    let mut skip_ws = |b: &[u8], i: &mut usize| {
        while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
            *i += 1;
        }
    };
    let mut parse_str = |b: &[u8], i: &mut usize| -> Option<String> {
        skip_ws(b, i);
        if *i >= b.len() || b[*i] != b'"' {
            return None;
        }
        *i += 1;
        let mut out = String::new();
        while *i < b.len() {
            let c = b[*i];
            if c == b'"' {
                *i += 1;
                return Some(out);
            }
            if c == b'\\' && *i + 1 < b.len() {
                *i += 1;
                let e = b[*i];
                out.push(match e {
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    b'"' => '"',
                    b'\\' => '\\',
                    _ => e as char,
                });
            } else {
                out.push(c as char);
            }
            *i += 1;
        }
        None
    };
    fn parse_val_impl(
        b: &[u8],
        i: &mut usize,
        s: &str,
        parse_str: &dyn Fn(&[u8], &mut usize) -> Option<String>,
    ) -> Option<Value> {
        let mut skip_ws = |b: &[u8], i: &mut usize| {
            while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
                *i += 1;
            }
        };
        skip_ws(b, i);
        if *i >= b.len() {
            return None;
        }
        match b[*i] {
            b'{' => {
                *i += 1;
                let mut m = BTreeMap::new();
                skip_ws(b, i);
                if *i < b.len() && b[*i] == b'}' {
                    *i += 1;
                    return Some(Value::Object(m));
                }
                loop {
                    skip_ws(b, i);
                    let k = parse_str(b, i)?;
                    skip_ws(b, i);
                    if *i < b.len() && b[*i] == b':' {
                        *i += 1;
                    }
                    let v = parse_val_impl(b, i, s, parse_str)?;
                    m.insert(k, v);
                    skip_ws(b, i);
                    if *i < b.len() && b[*i] == b',' {
                        *i += 1;
                    } else if *i < b.len() && b[*i] == b'}' {
                        *i += 1;
                        break;
                    }
                }
                Some(Value::Object(m))
            }
            b'[' => {
                *i += 1;
                let mut a = Vec::new();
                skip_ws(b, i);
                if *i < b.len() && b[*i] == b']' {
                    *i += 1;
                    return Some(Value::Array(a));
                }
                loop {
                    a.push(parse_val_impl(b, i, s, parse_str)?);
                    skip_ws(b, i);
                    if *i < b.len() && b[*i] == b',' {
                        *i += 1;
                    } else if *i < b.len() && b[*i] == b']' {
                        *i += 1;
                        break;
                    }
                }
                Some(Value::Array(a))
            }
            b'"' => parse_str(b, i).map(Value::Str),
            b't' => {
                if s[*i..].starts_with("true") {
                    *i += 4;
                    Some(Value::Bool(true))
                } else {
                    None
                }
            }
            b'f' => {
                if s[*i..].starts_with("false") {
                    *i += 5;
                    Some(Value::Bool(false))
                } else {
                    None
                }
            }
            b'n' => {
                if s[*i..].starts_with("null") {
                    *i += 4;
                    Some(Value::Null)
                } else {
                    None
                }
            }
            _ => {
                let start = *i;
                while *i < b.len()
                    && (b[*i].is_ascii_digit()
                        || matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E'))
                {
                    *i += 1;
                }
                let tok = s[start..*i].to_string();
                if let Ok(iv) = tok.parse::<i64>() {
                    Some(Value::Int(iv))
                } else if let Ok(fv) = tok.parse::<f64>() {
                    Some(Value::Float(fv))
                } else {
                    None
                }
            }
        }
    }
    parse_val_impl(bytes, &mut i, s, &parse_str)
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic() {
        let text = "firstName: John\nage: 27\nisAlive: true\nspouse: null\n";
        let v = parse(text).unwrap();
        assert_eq!(v.get("firstName"), Some(&Value::Str("John".into())));
        assert_eq!(v.get("age"), Some(&Value::Int(27)));
        assert_eq!(v.get("isAlive"), Some(&Value::Bool(true)));
        assert_eq!(v.get("spouse"), Some(&Value::Null));
    }

    #[test]
    fn parse_nested() {
        let text = "address:\n{\n    streetAddress: \"21 2nd Street\"\n    state: NY\n}\n";
        let v = parse(text).unwrap();
        assert_eq!(
            v.get("address.streetAddress"),
            Some(&Value::Str("21 2nd Street".into()))
        );
        assert_eq!(v.get("address.state"), Some(&Value::Str("NY".into())));
    }

    #[test]
    fn parse_array() {
        let text = "phoneNumbers:\n[\n    { type: home }\n    { type: office }\n]\n";
        let v = parse(text).unwrap();
        if let Some(Value::Array(a)) = v.get("phoneNumbers") {
            assert_eq!(a.len(), 2);
            assert_eq!(a[0].get("type"), Some(&Value::Str("home".into())));
        } else {
            panic!("not array");
        }
    }

    #[test]
    fn parse_fragment() {
        let text = "@base { region: cn-north-1 }\nserver web { &base }\n";
        let v = parse(text).unwrap();
        // &base 展开为字段 (键名 "&base", 值=片段对象), 与 Lua 实现一致
        assert_eq!(
            v.get("server.&base.region"),
            Some(&Value::Str("cn-north-1".into()))
        );
        assert_eq!(v.get("server.__type"), Some(&Value::Str("server".into())));
        assert_eq!(v.get("server.__name"), Some(&Value::Str("web".into())));
    }

    #[test]
    fn roundtrip() {
        let text = "name: myapp\nport: 8080\nflags: [ a b c ]\n";
        let v = parse(text).unwrap();
        let out = to_sml(&v);
        let v2 = parse(&out).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn env_inline() {
        std::env::set_var("SML_TEST_VAR", "hello");
        let text = "greeting: $env.SML_TEST_VAR\n";
        let v = parse(text).unwrap();
        assert_eq!(v.get("greeting"), Some(&Value::Str("hello".into())));
    }

    #[test]
    fn c_abi_json_bridge() {
        let text = "name: John\nage: 27\n";
        let v = parse(text).unwrap();
        let j = jsonify(&v);
        assert!(j.contains("\"name\":\"John\""));
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }
}
