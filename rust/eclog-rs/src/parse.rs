// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! 解析器：Eclog-CPP `Source/Parse.cpp` 的状态机逐条移植，双出口（树 / 事件）。
//! Parser: a direct port of the Eclog-CPP `Source/Parse.cpp` state machine, with two
//! outlets (tree / events).
//!
//! 为什么照抄状态机而不是重写成"更干净"的递归下降：字段/元素的分隔规则（逗号与
//! 换行等价、尾逗号合法、同一行必须有逗号）与字符串拼接的跨行规则全在那几个
//! `state_*` 里，改写极易与上游行为分叉，而本 crate 的验收判据是逐事件一致。
//! Why port the state machine verbatim instead of rewriting it: the separator rules
//! (comma or newline, trailing commas, same-line requires a comma) and the
//! cross-line `+` concatenation rules all live in those `state_*` blocks; a rewrite
//! diverges easily, and the acceptance criterion here is event-by-event equality.
//!
//! 与 C++ 的两处实现差异（行为等价）：
//! Two intentional implementation differences (behaviourally equivalent):
//!
//! 1. C++ 先把定界符写进解析缓冲区、再由 `Value` 去掉前缀；本实现把正文与定界符
//!    分开返回，因此不需要 `embedDelimiter` 参数。
//!    C++ prefixes the delimiter into the parsing buffer and lets `Value` strip it;
//!    this implementation returns text and delimiter separately.
//! 2. C++ 的 `ParseHandler` 可请求终止；本实现的 `ParseHandler` 没有错误通道。
//!    C++'s handler can ask for termination; ours has no error channel.

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{Error, ErrorKind, ParseErrorKind};
use crate::value::{Number, NumberInfo, Object, Str, StringNotation, Value};

/// 定界符长度上限，与 C++ `InlineArray<char, 16>` 一致。
/// Delimiter length limit, matching C++ `InlineArray<char, 16>`.
const MAX_DELIMITER_LEN: usize = 16;

/// 解析事件接收器：容器只走 begin/end，标量走 `on_value`。
/// Parse event sink: containers through begin/end, scalars through `on_value`.
///
/// 事件序与 `.check` 期望文件一一对应：`object` / `key` / 值 / `object_end`，数组同理。
/// The event order maps one-to-one onto `.check` files.
pub trait ParseHandler {
    /// 进入对象 ｜ entering an object.
    fn on_object_begin(&mut self);
    /// 离开对象 ｜ leaving an object.
    fn on_object_end(&mut self);
    /// 进入数组 ｜ entering an array.
    fn on_array_begin(&mut self);
    /// 离开数组 ｜ leaving an array.
    fn on_array_end(&mut self);
    /// 一个键 ｜ a key.
    fn on_key(&mut self, key: &Str);
    /// 一个标量值 ｜ a scalar value.
    fn on_value(&mut self, value: &Value);
}

/// 解析为值树；根恒为对象，空输入得到空对象。
/// Parse into a value tree; the root is always an object and empty input yields `{}`.
pub fn parse(input: &[u8]) -> Result<Value, Error> {
    let src = decode(input)?;
    let mut sink = TreeSink::new();
    Parser::new(src).parse_document(&mut sink)?;
    Ok(sink.finish())
}

/// 同 [`parse`]，入参已是 `&str`。
/// Same as [`parse`] for callers that already hold a `&str`.
pub fn parse_str(input: &str) -> Result<Value, Error> {
    parse(input.as_bytes())
}

/// 解析并向 handler 发事件，不建树。
/// Parse and drive a handler without building a tree.
pub fn parse_with(input: &[u8], handler: &mut impl ParseHandler) -> Result<(), Error> {
    let src = decode(input)?;
    let mut sink = HandlerSink { handler };
    Parser::new(src).parse_document(&mut sink)
}

/// 同 [`parse_with`]，入参已是 `&str`。
/// Same as [`parse_with`] for callers that already hold a `&str`.
pub fn parse_str_with(input: &str, handler: &mut impl ParseHandler) -> Result<(), Error> {
    parse_with(input.as_bytes(), handler)
}

/// 校验一个字符串是否为完整数值字面量，返回其标志。
/// Validate that `raw` is exactly one number literal and return its flags.
pub(crate) fn validate_number(raw: &str) -> Option<NumberInfo> {
    let mut parser = Parser::new(raw);
    match parser.parse_number_token() {
        Ok((end, info)) if end == raw.len() => Some(info),
        _ => None,
    }
}

fn decode(input: &[u8]) -> Result<&str, Error> {
    match core::str::from_utf8(input) {
        Ok(src) => Ok(src),
        Err(err) => {
            let offset = err.valid_up_to();
            let prefix = core::str::from_utf8(&input[..offset]).unwrap_or("");
            let (line, column) = line_col_at(prefix);
            Err(Error::new(ErrorKind::Decode, line, column, offset))
        }
    }
}

fn line_col_at(prefix: &str) -> (u32, u32) {
    let mut line = 1u32;
    let mut column = 1u32;
    for c in prefix.chars() {
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn is_alpha(c: char) -> bool {
    c.is_ascii_alphabetic()
}

fn is_digit(c: char) -> bool {
    c.is_ascii_digit()
}

fn hex_value(c: char) -> Option<u32> {
    match c {
        '0'..='9' => Some(c as u32 - '0' as u32),
        'a'..='f' => Some(c as u32 - 'a' as u32 + 10),
        'A'..='F' => Some(c as u32 - 'A' as u32 + 10),
        _ => None,
    }
}

/// 一个字符串 token 的三个组成部分 ｜ the three parts of a string token.
struct Token {
    text: String,
    notation: StringNotation,
    delimiter: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Keyword {
    Plain,
    True,
    False,
    Inf,
    Nan,
    Null,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NumState {
    Sign,
    Int,
    Frac,
    Exp,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Linebreak {
    Cr,
    Lf,
    CrLf,
}

trait Sink {
    fn object_begin(&mut self);
    fn object_end(&mut self);
    fn array_begin(&mut self);
    fn array_end(&mut self);
    fn key(&mut self, key: Str);
    fn value(&mut self, value: Value);
}

enum Container {
    Object(Object),
    Array(Vec<Value>),
}

struct Frame {
    key: Option<Str>,
    container: Container,
}

/// 把事件流装配成值树 ｜ assemble the event stream into a value tree.
struct TreeSink {
    stack: Vec<Frame>,
    pending_key: Option<Str>,
    root: Option<Value>,
}

impl TreeSink {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            pending_key: None,
            root: None,
        }
    }

    fn finish(mut self) -> Value {
        self.root
            .take()
            .unwrap_or_else(|| Value::Object(Object::new()))
    }

    fn attach(&mut self, key: Option<Str>, value: Value) {
        match self.stack.last_mut() {
            Some(frame) => match &mut frame.container {
                Container::Object(object) => {
                    let key = key.unwrap_or_else(|| Str::new(""));
                    let _ = object.insert(key, value);
                }
                Container::Array(array) => array.push(value),
            },
            None => self.root = Some(value),
        }
    }
}

impl Sink for TreeSink {
    fn object_begin(&mut self) {
        let key = self.pending_key.take();
        self.stack.push(Frame {
            key,
            container: Container::Object(Object::new()),
        });
    }

    fn object_end(&mut self) {
        if let Some(frame) = self.stack.pop() {
            let value = match frame.container {
                Container::Object(object) => Value::Object(object),
                Container::Array(array) => Value::Array(array),
            };
            self.attach(frame.key, value);
        }
    }

    fn array_begin(&mut self) {
        let key = self.pending_key.take();
        self.stack.push(Frame {
            key,
            container: Container::Array(Vec::new()),
        });
    }

    fn array_end(&mut self) {
        if let Some(frame) = self.stack.pop() {
            let value = match frame.container {
                Container::Object(object) => Value::Object(object),
                Container::Array(array) => Value::Array(array),
            };
            self.attach(frame.key, value);
        }
    }

    fn key(&mut self, key: Str) {
        self.pending_key = Some(key);
    }

    fn value(&mut self, value: Value) {
        let key = self.pending_key.take();
        self.attach(key, value);
    }
}

struct HandlerSink<'h, H: ParseHandler> {
    handler: &'h mut H,
}

impl<H: ParseHandler> Sink for HandlerSink<'_, H> {
    fn object_begin(&mut self) {
        self.handler.on_object_begin();
    }

    fn object_end(&mut self) {
        self.handler.on_object_end();
    }

    fn array_begin(&mut self) {
        self.handler.on_array_begin();
    }

    fn array_end(&mut self) {
        self.handler.on_array_end();
    }

    fn key(&mut self, key: Str) {
        self.handler.on_key(&key);
    }

    fn value(&mut self, value: Value) {
        self.handler.on_value(&value);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum KvState {
    Key,
    Key2,
    KeySeparator,
    Value,
    Value2,
    Next,
    KeywordOrString,
    KeywordOrString2,
    KeywordOrString3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ValueState {
    Value,
    Value2,
    Next,
    KeywordOrString,
    KeywordOrString2,
    KeywordOrString3,
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    line: u32,
    column: u32,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        self.column += 1;
        Some(c)
    }

    fn peek_non_eof(&self) -> Result<char, Error> {
        match self.peek() {
            Some(c) => Ok(c),
            None => Err(self.error(ParseErrorKind::UnexpectedEndOfFile)),
        }
    }

    fn error(&self, kind: ParseErrorKind) -> Error {
        Error::new(ErrorKind::Parse(kind), self.line, self.column, self.pos)
    }

    fn unexpected(&self) -> Error {
        if self.peek().is_none() {
            self.error(ParseErrorKind::UnexpectedEndOfFile)
        } else {
            self.error(ParseErrorKind::UnexpectedCharacter)
        }
    }

    fn skip_spaces_and_tabs(&mut self) -> bool {
        let mut count = 0;
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' {
                self.bump();
                count += 1;
            } else {
                break;
            }
        }
        count > 0
    }

    fn skip_linebreak(&mut self) -> Option<Linebreak> {
        match self.peek() {
            Some('\r') => {
                self.bump();
                if self.peek() == Some('\n') {
                    self.bump();
                    self.new_line();
                    Some(Linebreak::CrLf)
                } else {
                    self.new_line();
                    Some(Linebreak::Cr)
                }
            }
            Some('\n') => {
                self.bump();
                self.new_line();
                Some(Linebreak::Lf)
            }
            _ => None,
        }
    }

    fn new_line(&mut self) {
        self.line += 1;
        self.column = 1;
    }

    fn skip_comment(&mut self) -> Result<bool, Error> {
        if self.peek() != Some('#') {
            return Ok(false);
        }
        self.bump();
        loop {
            match self.peek() {
                None => return Ok(true),
                Some('\r') | Some('\n') => {
                    self.skip_linebreak();
                    return Ok(true);
                }
                Some(c) if c < ' ' && c != '\t' => return Err(self.unexpected()),
                Some(_) => {
                    self.bump();
                }
            }
        }
    }

    fn skip_ws_and_comments(&mut self) -> Result<(), Error> {
        loop {
            let spaces = self.skip_spaces_and_tabs();
            let linebreak = self.skip_linebreak().is_some();
            let comment = self.skip_comment()?;
            if !spaces && !linebreak && !comment {
                return Ok(());
            }
        }
    }

    fn parse_document(&mut self, sink: &mut impl Sink) -> Result<(), Error> {
        self.skip_ws_and_comments()?;

        let curly = if self.peek() == Some('{') {
            self.bump();
            true
        } else {
            false
        };

        sink.object_begin();
        self.parse_key_value_list(sink)?;
        self.skip_ws_and_comments()?;

        if curly {
            if self.peek() == Some('}') {
                self.bump();
                sink.object_end();
                self.skip_ws_and_comments()?;
                if self.peek().is_some() {
                    return Err(self.unexpected());
                }
            } else {
                return Err(self.unexpected());
            }
        } else if self.peek().is_none() {
            sink.object_end();
        } else {
            return Err(self.unexpected());
        }

        Ok(())
    }

    fn parse_object(&mut self, sink: &mut impl Sink) -> Result<(), Error> {
        if self.peek() != Some('{') {
            return Err(self.unexpected());
        }
        self.bump();
        sink.object_begin();
        self.parse_key_value_list(sink)?;
        self.skip_ws_and_comments()?;
        if self.peek() == Some('}') {
            self.bump();
            sink.object_end();
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }

    fn parse_array(&mut self, sink: &mut impl Sink) -> Result<(), Error> {
        if self.peek() != Some('[') {
            return Err(self.unexpected());
        }
        self.bump();
        sink.array_begin();
        self.parse_value_list(sink)?;
        self.skip_ws_and_comments()?;
        if self.peek() == Some(']') {
            self.bump();
            sink.array_end();
            Ok(())
        } else {
            Err(self.unexpected())
        }
    }

    fn parse_key_value_list(&mut self, sink: &mut impl Sink) -> Result<(), Error> {
        let mut state: Vec<KvState> = Vec::with_capacity(4);
        state.push(KvState::Key);

        let mut keyword = Keyword::Plain;
        let mut token: Option<Token> = None;
        let mut key_line = 1u32;
        let mut key_column = 1u32;
        let mut key_offset = 0usize;
        let mut line_of_last_value = 0u32;

        loop {
            self.skip_ws_and_comments()?;
            let ch = self.peek();

            match *state.last().expect("state stack") {
                KvState::Key => {
                    let starts_key = matches!(
                        ch,
                        Some(c) if is_alpha(c) || c == '_' || c == '"' || c == '@' || c == '|'
                    );
                    if starts_key {
                        key_line = self.line;
                        key_column = self.column;
                        key_offset = self.pos;
                        *state.last_mut().expect("state stack") = KvState::Key2;
                        state.push(KvState::KeywordOrString);
                    } else {
                        return Ok(());
                    }
                }

                KvState::Key2 => {
                    if keyword != Keyword::Plain {
                        return Err(Error::new(
                            ErrorKind::Parse(ParseErrorKind::InvalidKey),
                            key_line,
                            key_column,
                            key_offset,
                        ));
                    }
                    let tok = token.take().expect("key token");
                    sink.key(Str::from_parts(tok.text, tok.notation, tok.delimiter));
                    *state.last_mut().expect("state stack") = KvState::KeySeparator;
                }

                KvState::KeySeparator => {
                    if ch == Some(':') {
                        self.bump();
                        *state.last_mut().expect("state stack") = KvState::Value;
                    } else {
                        return Err(self.unexpected());
                    }
                }

                KvState::Value => match ch {
                    Some(c) if is_alpha(c) || c == '_' || c == '"' || c == '@' || c == '|' => {
                        *state.last_mut().expect("state stack") = KvState::Value2;
                        state.push(KvState::KeywordOrString);
                    }
                    Some(c) if c == '+' || c == '-' || is_digit(c) => {
                        let start = self.pos;
                        let (end, info) = self.parse_number_token()?;
                        line_of_last_value = self.line;
                        sink.value(Value::Number(Number::new(&self.src[start..end], info)));
                        *state.last_mut().expect("state stack") = KvState::Next;
                    }
                    Some('{') => {
                        self.parse_object(sink)?;
                        line_of_last_value = self.line;
                        *state.last_mut().expect("state stack") = KvState::Next;
                    }
                    Some('[') => {
                        self.parse_array(sink)?;
                        line_of_last_value = self.line;
                        *state.last_mut().expect("state stack") = KvState::Next;
                    }
                    _ => return Err(self.unexpected()),
                },

                KvState::Value2 => {
                    let tok = token.take();
                    match keyword {
                        Keyword::True => sink.value(Value::Bool(true)),
                        Keyword::False => sink.value(Value::Bool(false)),
                        Keyword::Inf => {
                            let tok = tok.expect("inf token");
                            sink.value(Value::Number(Number::new(tok.text, NumberInfo::INF)));
                        }
                        Keyword::Nan => {
                            let tok = tok.expect("nan token");
                            sink.value(Value::Number(Number::new(tok.text, NumberInfo::NAN)));
                        }
                        Keyword::Null => sink.value(Value::Null),
                        Keyword::Plain => {
                            let tok = tok.expect("string token");
                            sink.value(Value::Str(Str::from_parts(
                                tok.text,
                                tok.notation,
                                tok.delimiter,
                            )));
                        }
                    }
                    *state.last_mut().expect("state stack") = KvState::Next;
                }

                KvState::Next => {
                    if ch == Some(',') {
                        self.bump();
                        *state.last_mut().expect("state stack") = KvState::Key;
                    } else if self.line > line_of_last_value {
                        *state.last_mut().expect("state stack") = KvState::Key;
                    } else {
                        return Ok(());
                    }
                }

                KvState::KeywordOrString => {
                    keyword = Keyword::Plain;
                    let tok = self.parse_string_token()?;
                    line_of_last_value = self.line;

                    if tok.notation == StringNotation::Unquoted {
                        keyword = match tok.text.as_str() {
                            "true" => Keyword::True,
                            "false" => Keyword::False,
                            "inf" => Keyword::Inf,
                            "nan" => Keyword::Nan,
                            "null" => Keyword::Null,
                            _ => Keyword::Plain,
                        };
                        state.pop();
                    } else {
                        *state.last_mut().expect("state stack") = KvState::KeywordOrString2;
                    }
                    token = Some(tok);
                }

                KvState::KeywordOrString2 => {
                    if ch == Some('+') {
                        self.bump();
                        *state.last_mut().expect("state stack") = KvState::KeywordOrString3;
                    } else {
                        state.pop();
                    }
                }

                KvState::KeywordOrString3 => match ch {
                    Some('"') | Some('@') | Some('|') => {
                        let piece = self.parse_string_token()?;
                        token
                            .as_mut()
                            .expect("concatenation head")
                            .text
                            .push_str(&piece.text);
                        line_of_last_value = self.line;
                        *state.last_mut().expect("state stack") = KvState::KeywordOrString2;
                    }
                    _ => return Err(self.unexpected()),
                },
            }
        }
    }

    fn parse_value_list(&mut self, sink: &mut impl Sink) -> Result<(), Error> {
        let mut state: Vec<ValueState> = Vec::with_capacity(4);
        state.push(ValueState::Value);

        let mut keyword = Keyword::Plain;
        let mut token: Option<Token> = None;
        let mut line_of_last_value = 0u32;

        loop {
            self.skip_ws_and_comments()?;
            let ch = self.peek();

            match *state.last().expect("state stack") {
                ValueState::Value => match ch {
                    Some(c) if is_alpha(c) || c == '_' || c == '"' || c == '@' || c == '|' => {
                        *state.last_mut().expect("state stack") = ValueState::Value2;
                        state.push(ValueState::KeywordOrString);
                    }
                    Some(c) if c == '+' || c == '-' || is_digit(c) => {
                        let start = self.pos;
                        let (end, info) = self.parse_number_token()?;
                        line_of_last_value = self.line;
                        sink.value(Value::Number(Number::new(&self.src[start..end], info)));
                        *state.last_mut().expect("state stack") = ValueState::Next;
                    }
                    Some('{') => {
                        self.parse_object(sink)?;
                        line_of_last_value = self.line;
                        *state.last_mut().expect("state stack") = ValueState::Next;
                    }
                    Some('[') => {
                        self.parse_array(sink)?;
                        line_of_last_value = self.line;
                        *state.last_mut().expect("state stack") = ValueState::Next;
                    }
                    _ => return Ok(()),
                },

                ValueState::Value2 => {
                    let tok = token.take();
                    match keyword {
                        Keyword::True => sink.value(Value::Bool(true)),
                        Keyword::False => sink.value(Value::Bool(false)),
                        Keyword::Inf => {
                            let tok = tok.expect("inf token");
                            sink.value(Value::Number(Number::new(tok.text, NumberInfo::INF)));
                        }
                        Keyword::Nan => {
                            let tok = tok.expect("nan token");
                            sink.value(Value::Number(Number::new(tok.text, NumberInfo::NAN)));
                        }
                        Keyword::Null => sink.value(Value::Null),
                        Keyword::Plain => {
                            let tok = tok.expect("string token");
                            sink.value(Value::Str(Str::from_parts(
                                tok.text,
                                tok.notation,
                                tok.delimiter,
                            )));
                        }
                    }
                    *state.last_mut().expect("state stack") = ValueState::Next;
                }

                ValueState::Next => {
                    if ch == Some(',') {
                        self.bump();
                        *state.last_mut().expect("state stack") = ValueState::Value;
                    } else if self.line > line_of_last_value {
                        *state.last_mut().expect("state stack") = ValueState::Value;
                    } else {
                        return Ok(());
                    }
                }

                ValueState::KeywordOrString => {
                    keyword = Keyword::Plain;
                    let tok = self.parse_string_token()?;
                    line_of_last_value = self.line;

                    if tok.notation == StringNotation::Unquoted {
                        keyword = match tok.text.as_str() {
                            "true" => Keyword::True,
                            "false" => Keyword::False,
                            "inf" => Keyword::Inf,
                            "nan" => Keyword::Nan,
                            "null" => Keyword::Null,
                            _ => Keyword::Plain,
                        };
                        state.pop();
                    } else {
                        *state.last_mut().expect("state stack") = ValueState::KeywordOrString2;
                    }
                    token = Some(tok);
                }

                ValueState::KeywordOrString2 => {
                    if ch == Some('+') {
                        self.bump();
                        *state.last_mut().expect("state stack") = ValueState::KeywordOrString3;
                    } else {
                        state.pop();
                    }
                }

                ValueState::KeywordOrString3 => match ch {
                    Some('"') | Some('@') | Some('|') => {
                        let piece = self.parse_string_token()?;
                        token
                            .as_mut()
                            .expect("concatenation head")
                            .text
                            .push_str(&piece.text);
                        line_of_last_value = self.line;
                        *state.last_mut().expect("state stack") = ValueState::KeywordOrString2;
                    }
                    _ => return Err(self.unexpected()),
                },
            }
        }
    }

    fn parse_string_token(&mut self) -> Result<Token, Error> {
        match self.peek() {
            Some(c) if is_alpha(c) || c == '_' => Ok(self.parse_unquoted()),
            Some('"') => self.parse_quoted(),
            Some('@') => self.parse_raw(),
            Some('|') => self.parse_heredoc(),
            _ => Err(self.unexpected()),
        }
    }

    fn parse_unquoted(&mut self) -> Token {
        let start = self.pos;
        self.bump();
        while let Some(c) = self.peek() {
            if is_alpha(c) || is_digit(c) || c == '_' || c == '-' || c == '.' {
                self.bump();
            } else {
                break;
            }
        }
        Token {
            text: String::from(&self.src[start..self.pos]),
            notation: StringNotation::Unquoted,
            delimiter: String::new(),
        }
    }

    fn parse_quoted(&mut self) -> Result<Token, Error> {
        self.bump();

        let mut text = String::new();

        loop {
            let c = self.peek_non_eof()?;
            match c {
                '\\' => {
                    self.bump();
                    let escape = self.peek_non_eof()?;
                    match escape {
                        '"' | '\\' | '/' => {
                            self.bump();
                            text.push(escape);
                        }
                        'b' => {
                            self.bump();
                            text.push('\u{8}');
                        }
                        'f' => {
                            self.bump();
                            text.push('\u{c}');
                        }
                        'n' => {
                            self.bump();
                            text.push('\n');
                        }
                        'r' => {
                            self.bump();
                            text.push('\r');
                        }
                        't' => {
                            self.bump();
                            text.push('\t');
                        }
                        'u' => {
                            let code = self.parse_unicode_escape()?;
                            text.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        _ => {
                            return Err(Error::new(
                                ErrorKind::Parse(ParseErrorKind::UnrecognizedEscape),
                                self.line,
                                self.column.saturating_sub(1),
                                self.pos,
                            ));
                        }
                    }
                }
                c if c < ' ' && c != '\t' => {
                    return Err(Error::new(
                        ErrorKind::Parse(ParseErrorKind::UnescapedControlCharacter),
                        self.line,
                        self.column,
                        self.pos,
                    ));
                }
                '"' => {
                    self.bump();
                    break;
                }
                _ => {
                    self.bump();
                    text.push(c);
                }
            }
        }

        Ok(Token {
            text,
            notation: StringNotation::Quoted,
            delimiter: String::new(),
        })
    }

    fn parse_unicode_escape(&mut self) -> Result<u32, Error> {
        // 调用方已确认 peek() == 'u'
        self.bump();
        let code = self.parse_unicode_hex_digits()?;

        if (code >> 11) == 0x1b {
            if (code >> 10) == 0x36 {
                let high = code;
                if self.skip_sequence("\\u")? {
                    let low = self.parse_unicode_hex_digits()?;
                    if (low >> 10) == 0x37 {
                        return Ok((((high & 0x3ff) << 10) | (low & 0x3ff)) + 0x10000);
                    }
                }
            }
            return Err(self.error(ParseErrorKind::InvalidUtf16Surrogate));
        }

        Ok(code)
    }

    fn parse_unicode_hex_digits(&mut self) -> Result<u32, Error> {
        let variable = if self.peek() == Some('{') {
            self.bump();
            true
        } else {
            false
        };

        let length = if variable { 6 } else { 4 };
        let mut code: u32 = 0;
        let mut i = 0usize;

        while i < length {
            let c = self.peek_non_eof()?;
            let Some(digit) = hex_value(c) else {
                break;
            };
            self.bump();
            code = code * 16 + digit;
            i += 1;
        }

        if code > 0x10ffff {
            return Err(self.error(ParseErrorKind::InvalidUnicodeEscape));
        }
        if !variable && i == 4 {
            return Ok(code);
        }
        if variable && i > 0 && self.peek() == Some('}') {
            self.bump();
            return Ok(code);
        }
        Err(self.error(ParseErrorKind::InvalidUnicodeEscape))
    }

    fn skip_sequence(&mut self, seq: &str) -> Result<bool, Error> {
        let saved = (self.pos, self.line, self.column);
        for expected in seq.chars() {
            match self.peek() {
                Some(c) if c == expected => {
                    self.bump();
                }
                Some(_) => {
                    self.pos = saved.0;
                    self.line = saved.1;
                    self.column = saved.2;
                    return Ok(false);
                }
                None => return Err(self.error(ParseErrorKind::UnexpectedEndOfFile)),
            }
        }
        Ok(true)
    }

    fn parse_raw(&mut self) -> Result<Token, Error> {
        self.bump();

        let delimiter_line = self.line;
        let delimiter_column = self.column;
        let delimiter_offset = self.pos;

        let mut delimiter = String::new();
        loop {
            let c = self.peek_non_eof()?;
            if c.is_ascii_alphanumeric() || c == '_' {
                if delimiter.len() < MAX_DELIMITER_LEN {
                    self.bump();
                    delimiter.push(c);
                } else {
                    return Err(Error::new(
                        ErrorKind::Parse(ParseErrorKind::StringDelimiterTooLong),
                        delimiter_line,
                        delimiter_column,
                        delimiter_offset,
                    ));
                }
            } else {
                break;
            }
        }

        if self.peek() == Some('"') {
            self.bump();
        } else {
            return Err(Error::new(
                ErrorKind::Parse(ParseErrorKind::InvalidStringDelimiter),
                delimiter_line,
                delimiter_column,
                delimiter_offset,
            ));
        }

        let mut text = String::new();
        loop {
            let c = self.peek_non_eof()?;
            if c == '"' {
                self.bump();

                let mut matched = 0usize;
                while matched < delimiter.len() {
                    let next = self.peek_non_eof()?;
                    if next == delimiter.as_bytes()[matched] as char {
                        self.bump();
                        matched += 1;
                    } else {
                        break;
                    }
                }

                if matched == delimiter.len() {
                    break;
                }

                text.push('"');
                text.push_str(&delimiter[..matched]);
            } else if c < ' ' && c != '\t' {
                return Err(self.unexpected());
            } else {
                self.bump();
                text.push(c);
            }
        }

        Ok(Token {
            text,
            notation: StringNotation::Raw,
            delimiter,
        })
    }

    fn parse_heredoc(&mut self) -> Result<Token, Error> {
        self.bump();

        let delimiter_line = self.line;
        let delimiter_column = self.column;
        let delimiter_offset = self.pos;

        let mut delimiter = String::new();
        loop {
            let c = self.peek_non_eof()?;
            if c.is_ascii_alphanumeric() || c == '_' {
                if delimiter.len() < MAX_DELIMITER_LEN {
                    self.bump();
                    delimiter.push(c);
                } else {
                    return Err(Error::new(
                        ErrorKind::Parse(ParseErrorKind::StringDelimiterTooLong),
                        delimiter_line,
                        delimiter_column,
                        delimiter_offset,
                    ));
                }
            } else {
                break;
            }
        }

        match self.peek_non_eof()? {
            '\r' | '\n' => {
                if delimiter.is_empty() {
                    return Err(Error::new(
                        ErrorKind::Parse(ParseErrorKind::EmptyStringDelimiter),
                        delimiter_line,
                        delimiter_column,
                        delimiter_offset,
                    ));
                }
                self.skip_linebreak();
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::Parse(ParseErrorKind::InvalidStringDelimiter),
                    delimiter_line,
                    delimiter_column,
                    delimiter_offset,
                ));
            }
        }

        let mut text = String::new();
        let mut line_begin = 0usize;
        let mut at_line_start = true;
        let mut indent = 0usize;

        loop {
            let c = self.peek_non_eof()?;

            if at_line_start && (c == ' ' || c == '\t') {
                self.bump();
                text.push(c);
                indent += 1;
            } else if at_line_start && c == delimiter.as_bytes()[0] as char {
                self.bump();

                let mut matched = 1usize;
                while matched < delimiter.len() {
                    let next = self.peek_non_eof()?;
                    if next == delimiter.as_bytes()[matched] as char {
                        self.bump();
                        matched += 1;
                    } else {
                        break;
                    }
                }

                if matched == delimiter.len() {
                    let next = self.peek_non_eof()?;
                    if next == '\r' || next == '\n' {
                        if indent > 0 {
                            text.truncate(line_begin);
                        }
                        break;
                    }
                }

                text.push_str(&delimiter[..matched]);
            } else if c == '\r' || c == '\n' {
                match self.skip_linebreak() {
                    Some(Linebreak::CrLf) => text.push_str("\r\n"),
                    Some(Linebreak::Cr) => text.push('\r'),
                    Some(Linebreak::Lf) => text.push('\n'),
                    None => {}
                }
                line_begin = text.len();
                at_line_start = true;
                indent = 0;
            } else if c < ' ' && c != '\t' {
                return Err(self.unexpected());
            } else {
                self.bump();
                text.push(c);
                at_line_start = false;
            }
        }

        if indent > 0 {
            text = unindent(&text, indent);
        }

        Ok(Token {
            text,
            notation: StringNotation::Heredoc,
            delimiter,
        })
    }

    fn transfer_decimal_digits(&mut self) -> Result<(), Error> {
        // C++ `transferDecimalDigits` 先无条件吞掉一个字符，再循环吞数字 —— 照抄。
        self.peek_non_eof()?;
        self.bump();
        while let Some(c) = self.peek() {
            if is_digit(c) {
                self.bump();
            } else {
                break;
            }
        }
        Ok(())
    }

    fn transfer_sequence(&mut self, seq: &str) -> Result<(), Error> {
        for expected in seq.chars() {
            let c = self.peek_non_eof()?;
            if c != expected {
                return Err(self.unexpected());
            }
            self.bump();
        }
        Ok(())
    }

    fn parse_number_token(&mut self) -> Result<(usize, NumberInfo), Error> {
        let mut info = NumberInfo::NONE;
        let mut state = NumState::Sign;

        loop {
            let ch = self.peek();
            match state {
                NumState::Sign => {
                    if let Some(c @ ('+' | '-')) = ch {
                        info |= NumberInfo::SIGN;
                        if c == '-' {
                            info |= NumberInfo::NEG;
                        }
                        self.bump();
                    }
                    state = NumState::Int;
                }

                NumState::Int => match ch {
                    Some('0') => {
                        self.bump();
                        state = NumState::Frac;
                    }
                    Some('1'..='9') => {
                        self.transfer_decimal_digits()?;
                        state = NumState::Frac;
                    }
                    Some('i') => {
                        info |= NumberInfo::INF;
                        self.transfer_sequence("inf")?;
                        state = NumState::End;
                    }
                    Some('n') => {
                        info |= NumberInfo::NAN;
                        self.transfer_sequence("nan")?;
                        state = NumState::End;
                    }
                    _ => return Err(self.unexpected()),
                },

                NumState::Frac => {
                    if ch == Some('.') {
                        info |= NumberInfo::FRAC;
                        self.bump();
                        self.transfer_decimal_digits()?;
                        state = NumState::Exp;
                    } else {
                        state = NumState::Exp;
                    }
                }

                NumState::Exp => {
                    if matches!(ch, Some('e') | Some('E')) {
                        info |= NumberInfo::EXP;
                        self.bump();

                        let mut c = self.peek_non_eof()?;
                        if c == '+' || c == '-' {
                            self.bump();
                            c = self.peek_non_eof()?;
                        }

                        if c == '0' {
                            self.bump();
                        } else if matches!(c, '1'..='9') {
                            self.transfer_decimal_digits()?;
                        } else {
                            return Err(self.unexpected());
                        }

                        state = NumState::End;
                    } else {
                        state = NumState::End;
                    }
                }

                NumState::End => return Ok((self.pos, info)),
            }
        }
    }
}

/// 按「闭标记行的缩进」去掉每行前缀空白，等价 C++ `unindent`。
/// Strip up to `indent` leading spaces/tabs per line, equivalent to C++ `unindent`.
fn unindent(text: &str, indent: usize) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    let mut at_line_start = true;
    let mut stripped = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        if at_line_start && (b == b' ' || b == b'\t') && stripped < indent {
            i += 1;
            stripped += 1;
        } else if b == b'\r' || b == b'\n' {
            out.push(b);
            i += 1;
            if b == b'\r' && i < bytes.len() && bytes[i] == b'\n' {
                out.push(b'\n');
                i += 1;
            }
            at_line_start = true;
            stripped = 0;
        } else {
            out.push(b);
            i += 1;
            at_line_start = false;
        }
    }

    String::from_utf8(out).unwrap_or_default()
}
