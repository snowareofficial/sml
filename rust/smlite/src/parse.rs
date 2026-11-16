// SPDX-License-Identifier: MulanPSL-2.0
//! Minimal parser for the SML pure-data subset: bare words, `#` comments, top-level
//! omitted braces, object / array / scalar values, and optional commas.
//! SML 纯数据子集的最小解析器：裸词、`#` 注释、顶层省略 `{}`、对象 / 数组 / 标量、可选逗号。
//!
//! The concrete token rules are the data-only subset of `sml-value/text_rules.rs`
//! (bare word charset, escape set, float spelling) plus the eclog interop points from
//! I-28. Document-friendly features (featag / weak / `@type` / include) are absent.
//! 词法规则取 `sml-value/text_rules.rs` 的纯数据子集（裸词字符集、转义集、浮点拼写）
//! 加 I-28 的 eclog 互操作点；文档友好特性（featag / weak / `@type` / include）不在此列。

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{ErrorKind, ParseError};
use crate::value::{Object, Value, MAX_VALUE_DEPTH};

/// Parse bytes into a value. ｜ 把字节解析成值。
pub fn parse(bytes: &[u8]) -> Result<Value, ParseError> {
    let mut p = Parser {
        s: bytes,
        pos: 0,
        depth: 0,
    };
    p.skip_ws();
    if p.pos == p.s.len() {
        return Ok(Value::Object(Object::new()));
    }
    let v = match p.peek() {
        Some(b'{') => p.parse_object()?,
        Some(b'[') => p.parse_array()?,
        // 顶层隐式对象要求首个 token 之后紧跟 `:`，否则是纯标量（RFC 7396 允许标量 patch）。
        _ if p.top_level_is_object() => p.parse_implicit_object()?,
        Some(b'"') => Value::Str(p.parse_quoted()?),
        _ => {
            let tok = p.parse_bare()?;
            classify(tok)
        }
    };
    p.skip_ws();
    if p.pos != p.s.len() {
        return Err(p.err(ErrorKind::TrailingData));
    }
    Ok(v)
}

/// Parse a UTF-8 string into a value. ｜ 把 UTF-8 字符串解析成值。
pub fn parse_str(s: &str) -> Result<Value, ParseError> {
    parse(s.as_bytes())
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.pos).copied()
    }

    fn err(&self, kind: ErrorKind) -> ParseError {
        ParseError::new(kind, self.pos)
    }

    /// Whether the first token at top level is a key followed by `:` (implicit object).
    /// 顶层首个 token 是否为后跟 `:` 的键（即隐式对象）。
    fn top_level_is_object(&self) -> bool {
        let mut i = self.pos;
        if self.s.get(i) == Some(&b'"') {
            i += 1;
            while i < self.s.len() {
                match self.s[i] {
                    b'\\' => i += 2,
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
        } else {
            while i < self.s.len() && is_bare_byte(self.s[i]) {
                i += 1;
            }
        }
        while i < self.s.len() && self.s[i].is_ascii_whitespace() {
            i += 1;
        }
        // `key: value` 或 SML 具名块 `key { ... }` 都是隐式对象的条目。
        self.s.get(i) == Some(&b':') || self.s.get(i) == Some(&b'{')
    }

    /// Skip whitespace and `#` line comments; both are pure-data syntax.
    /// 跳过空白与 `#` 行注释；两者都是纯数据语法。
    fn skip_ws(&mut self) {
        loop {
            while matches!(self.peek(), Some(c) if c.is_ascii_whitespace()) {
                self.pos += 1;
            }
            if self.peek() == Some(b'#') {
                while let Some(c) = self.peek() {
                    self.pos += 1;
                    if c == b'\n' {
                        break;
                    }
                }
                continue;
            }
            break;
        }
    }

    fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_VALUE_DEPTH {
            return Err(self.err(ErrorKind::DepthLimit));
        }
        Ok(())
    }

    /// Top-level implicit object: `key: value` pairs without surrounding braces.
    /// 顶层隐式对象：不带外层 `{}` 的 `key: value` 序列。
    fn parse_implicit_object(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        let mut entries: Vec<(alloc::string::String, Value)> = Vec::new();
        loop {
            self.skip_ws();
            if self.pos >= self.s.len() {
                break;
            }
            let key = self.parse_key()?;
            self.skip_ws();
            // SML 具名块 `key { ... }`：值就是紧随的对象，无需冒号。
            let value = if self.peek() == Some(b'{') {
                self.parse_value()?
            } else {
                if self.peek() != Some(b':') {
                    return Err(self.err(ErrorKind::ExpectedColon));
                }
                self.pos += 1;
                self.skip_ws();
                self.parse_value()?
            };
            entries.push((key, value));
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.pos += 1;
            }
        }
        self.depth -= 1;
        Ok(Value::Object(Object::from_entries(entries)))
    }

    fn parse_object(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.pos += 1; // `{`
        let mut entries: Vec<(alloc::string::String, Value)> = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                None => return Err(self.err(ErrorKind::UnexpectedEof)),
                _ => {}
            }
            let key = self.parse_key()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.err(ErrorKind::ExpectedColon));
            }
            self.pos += 1;
            self.skip_ws();
            let value = self.parse_value()?;
            entries.push((key, value));
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.pos += 1;
            }
        }
        self.depth -= 1;
        Ok(Value::Object(Object::from_entries(entries)))
    }

    fn parse_array(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.pos += 1; // `[`
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b']') => {
                    self.pos += 1;
                    break;
                }
                None => return Err(self.err(ErrorKind::UnexpectedEof)),
                _ => {}
            }
            out.push(self.parse_value()?);
            self.skip_ws();
            if self.peek() == Some(b',') {
                self.pos += 1;
            }
        }
        self.depth -= 1;
        Ok(Value::Array(out))
    }

    fn parse_key(&mut self) -> Result<String, ParseError> {
        if self.peek() == Some(b'"') {
            self.parse_quoted()
        } else {
            // 只在写入 Object 拥有型 key 时分配一次。
            Ok(String::from(self.parse_bare()?))
        }
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        self.skip_ws();
        match self.peek() {
            None => Err(self.err(ErrorKind::UnexpectedEof)),
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Value::Str(self.parse_quoted()?)),
            Some(b'}') | Some(b']') | Some(b',') | Some(b':') => {
                Err(self.err(ErrorKind::InvalidSyntax))
            }
            Some(b'\\') => Err(self.err(ErrorKind::InvalidEscape)),
            _ => {
                // 借用 token：数字/关键字零分配，仅 Str 分支在 classify 内分配一次。
                let tok = self.parse_bare()?;
                Ok(classify(tok))
            }
        }
    }

    fn parse_bare(&mut self) -> Result<&'a str, ParseError> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if is_bare_byte(c) {
                self.pos += 1;
            } else {
                break;
            }
        }
        if self.pos == start {
            return Err(self.err(ErrorKind::InvalidSyntax));
        }
        match core::str::from_utf8(&self.s[start..self.pos]) {
            Ok(s) => Ok(s),
            Err(_) => Err(self.err(ErrorKind::InvalidUtf8)),
        }
    }

    /// Quoted string with the data-subset escape set: `\\ \" \n \t \r \0 \uXXXX`.
    /// 引号串，转义集取纯数据子集：`\\ \" \n \t \r \0 \uXXXX`。
    fn parse_quoted(&mut self) -> Result<String, ParseError> {
        self.pos += 1; // opening `"`
        let mut out = Vec::new();
        loop {
            let c = match self.peek() {
                Some(c) => c,
                None => return Err(self.err(ErrorKind::UnexpectedEof)),
            };
            self.pos += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = match self.peek() {
                        Some(e) => e,
                        None => return Err(self.err(ErrorKind::UnexpectedEof)),
                    };
                    self.pos += 1;
                    match e {
                        b'\\' => out.push(b'\\'),
                        b'"' => out.push(b'"'),
                        b'n' => out.push(b'\n'),
                        b't' => out.push(b'\t'),
                        b'r' => out.push(b'\r'),
                        b'0' => out.push(0),
                        b'u' => {
                            let cp = self.parse_hex4()?;
                            let ch = match char::from_u32(cp) {
                                Some(ch) => ch,
                                None => return Err(self.err(ErrorKind::InvalidUnicode)),
                            };
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        _ => return Err(self.err(ErrorKind::InvalidEscape)),
                    }
                }
                _ => out.push(c),
            }
        }
        match String::from_utf8(out) {
            Ok(s) => Ok(s),
            Err(_) => Err(self.err(ErrorKind::InvalidUtf8)),
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, ParseError> {
        let mut v = 0u32;
        for _ in 0..4 {
            let c = match self.peek() {
                Some(c) => c,
                None => return Err(self.err(ErrorKind::UnexpectedEof)),
            };
            self.pos += 1;
            let d = match c {
                b'0'..=b'9' => (c - b'0') as u32,
                b'a'..=b'f' => (c - b'a' + 10) as u32,
                b'A'..=b'F' => (c - b'A' + 10) as u32,
                _ => return Err(self.err(ErrorKind::InvalidUnicode)),
            };
            v = v * 16 + d;
        }
        Ok(v)
    }
}

fn is_bare_byte(c: u8) -> bool {
    !c.is_ascii_whitespace() && !matches!(c, b':' | b'{' | b'}' | b'[' | b']' | b',' | b'#' | b'"' | b'\\')
}

/// Classify a bare token: keyword, integer, float, else string.
/// 裸词归类：关键字、整数、浮点，其余为字符串。
fn classify(tok: &str) -> Value {
    match tok {
        "null" => return Value::Null,
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(i) = tok.parse::<i64>() {
        return Value::Int(i);
    }
    if tok.contains('.') || tok.contains('e') || tok.contains('E') {
        if let Ok(f) = tok.parse::<f64>() {
            return Value::Float(f);
        }
    }
    Value::Str(String::from(tok))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn implicit_object_and_bare_words() {
        let v = parse(b"name: John age: 27 tags: [a b c]").unwrap();
        assert_eq!(v.get("name").and_then(Value::as_str), Some("John"));
        assert_eq!(v.get("age").and_then(Value::as_int), Some(27));
        assert_eq!(v.get("tags").and_then(Value::as_array).unwrap().len(), 3);
    }

    #[test]
    fn comments_and_commas() {
        let v = parse_str("# head\na: 1, # inline\nb: { x: 2, y: hi }\n").unwrap();
        assert_eq!(v.get("a").and_then(Value::as_int), Some(1));
        assert_eq!(
            v.get("b").and_then(Value::as_object).unwrap().get("y").and_then(Value::as_str),
            Some("hi")
        );
    }

    #[test]
    fn quoted_escapes() {
        let v = parse_str(r#"s: "a\nb\t\"c\u0041""#).unwrap();
        assert_eq!(v.get("s").and_then(Value::as_str), Some("a\nb\t\"cA"));
    }

    #[test]
    fn depth_limit() {
        let deep = format!("{}: 1", "a: {".repeat(130));
        let s = format!("{}{}", deep, "}".repeat(130));
        assert_eq!(parse_str(&s).unwrap_err().kind(), ErrorKind::DepthLimit);
    }
}
