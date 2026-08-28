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
//! - `include "path"` 引入外部文件（见 [`parse_file`]）
//! - `$env.VAR` 环境变量内联
//! - `#` 行注释
//! - 类型自识别: true/false -> bool, null -> None, 数字 -> i64/f64, 其余 -> String
//!
//! 值模型: `Value` 枚举 (与 JSON 同构, 另加 `__type`/`__name` 裸块元数据)。
//!
//! # 纯解析 vs 文件解析
//!
//! [`parse`] 是**纯函数**（只吃字符串，不做 IO），因此不含 include 处理。
//! 需要 include 时用 [`parse_file`]，它会先展开指令再交给 `parse`。
//! 这样设计保证了 `parse` 的可嵌入性（如 WASM / 沙箱内无文件系统）。
//!
//! # Cargo features
//!
//! - `serde`（默认关闭）：为 `Value` 实现 `Serialize`/`Deserialize`，
//!   可直接与 serde_json / serde_yaml / toml 等后端互操作。
//!   不启用时本 crate **零依赖**。
//!
//! ```toml
//! sml-rs = { version = "0.2", features = ["serde"] }
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

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

/// SML 语法版本
///
/// SML 源于 eclog，演进中通过 `@version` 声明文档遵循的语法版本，
/// 使解析器能在将来引入 v2 不兼容语法时仍正确读取旧文档。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// v1：初始公开版本
    V1,
}

impl Version {
    /// 当前实现支持的最新版本
    pub const CURRENT: Version = Version::V1;

    /// 解析版本字面量（`v1` / `1`）
    fn from_word(w: &str) -> Option<Version> {
        match w {
            "v1" | "1" => Some(Version::V1),
            _ => None,
        }
    }

    /// 版本名（用于错误信息与序列化回显）
    pub fn name(self) -> &'static str {
        match self {
            Version::V1 => "v1",
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// 若该行是 `@version` 声明，返回版本字面量；否则返回 None。
///
/// `version` 是保留字：不允许作为片段名（`@version { }`）使用。
fn version_directive(line: &str) -> Result<Option<String>, String> {
    let content = strip_line_comment(line).trim();
    // 词法失败的行（如未闭合引号）不是版本声明，交由主解析器报更准确的错
    let toks = match tokenize(content) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    match toks.as_slice() {
        [Tok::At, Tok::Word(w), Tok::Word(v)] if w == "version" => Ok(Some(v.clone())),
        [Tok::At, Tok::Word(w), Tok::Str(v)] if w == "version" => Ok(Some(v.clone())),
        [Tok::At, Tok::Word(w), ..] if w == "version" => Err(
            "`@version` 是版本声明指令，须写作 `@version v1`；`version` 不可作为片段名".into(),
        ),
        _ => Ok(None),
    }
}

/// 剥离 `@version` 声明行，返回剩余文本与声明的版本（未声明则为 None）。
///
/// 允许多次声明（include 进来的文件可各自声明），但必须一致；
/// 声明了实现不支持的版本时报错，避免静默按错误语法解析。
fn strip_version(text: &str) -> Result<(String, Option<Version>), String> {
    let mut declared: Option<Version> = None;
    let mut rest = String::new();
    for line in text.lines() {
        if let Some(lit) = version_directive(line)? {
            let v = Version::from_word(&lit).ok_or_else(|| {
                format!(
                    "不支持的 SML 版本 `{lit}`（本实现支持 {}）",
                    Version::CURRENT.name()
                )
            })?;
            match declared {
                None => declared = Some(v),
                Some(prev) if prev != v => {
                    return Err(format!("@version 冲突：{} 与 {}", prev.name(), v.name()))
                }
                Some(_) => {}
            }
            continue;
        }
        rest.push_str(line);
        rest.push('\n');
    }
    Ok((rest, declared))
}

/// 解析 SML 文本，并返回其声明的语法版本。
///
/// 未声明版本时按 `Version::CURRENT`（v1）处理，**既有文档不受影响**。
pub fn parse_versioned(text: &str) -> Result<(Value, Version), String> {
    let (rest, declared) = strip_version(text)?;
    Ok((parse_impl(&rest)?, declared.unwrap_or(Version::CURRENT)))
}

/// 解析 SML 文件：展开 include，并返回其声明的语法版本
pub fn parse_file_versioned(path: impl AsRef<Path>) -> Result<(Value, Version), String> {
    let path = path.as_ref();
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("读取失败 {}: {e}", path.display()))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let expanded = resolve_includes(&text, &base)?;
    parse_versioned(&expanded)
}

/// 解析 SML 文本
///
/// 会自动识别并剥离 `@version` 声明（需要版本信息时用 [`parse_versioned`]）。
pub fn parse(text: &str) -> Result<Value, String> {
    let (rest, _) = strip_version(text)?;
    parse_impl(&rest)
}

/// 不含版本处理的底层解析
fn parse_impl(text: &str) -> Result<Value, String> {
    let toks = tokenize(text)?;
    let mut p = Parser {
        toks,
        i: 0,
        fragments: BTreeMap::new(),
    };
    p.parse_block(None)
}

// ---------------------------------------------------------------------------
// include 指令：把外部 .sml 文件内联进来
//
// 语法：`include "path.sml"` 或 `@include "path.sml"`（两种等价）
// 语义：**文本内联**（类似 C 的 #include），而非对象合并。
//   这样 include 可以出现在块内部引入一组字段，例如：
//       server web { &base include "common/port.sml" }
//   若做成对象合并就无法表达「注入若干字段到当前块」。
//
// 相对路径按**被包含文件自身所在目录**解析（与 C 预处理器一致），
// 而非进程工作目录，因此嵌套 include 时路径行为可预期。
// ---------------------------------------------------------------------------

/// 嵌套深度上限：既防栈溢出，也让异常深层的引用尽早失败
const MAX_INCLUDE_DEPTH: usize = 32;

/// 剥离行尾注释，正确跳过引号内的 `#`（如 `key: "a#b"` 中的 # 不是注释起点）
fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_quote = false;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_quote = !in_quote,
            // 引号内的反斜杠会转义下一个字符，需整体跳过
            b'\\' if in_quote => i += 1,
            b'#' if !in_quote => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// 若该行是 include 指令，返回目标路径；否则返回 None
fn include_target(line: &str) -> Option<String> {
    let content = strip_line_comment(line).trim();
    let content = content.strip_prefix('@').unwrap_or(content).trim_start();
    // 复用词法器处理路径，使含空格的路径（引号串）能被正确识别
    let toks = tokenize(content).ok()?;
    match toks.as_slice() {
        [Tok::Word(w), Tok::Str(p)] if w == "include" => Some(p.clone()),
        [Tok::Word(w), Tok::Word(p)] if w == "include" => Some(p.clone()),
        _ => None,
    }
}

/// 把 text 中的 include 指令递归展开为不含指令的纯 SML 文本。
///
/// `base` 为相对路径的解析基准目录（通常是当前文件所在目录）。
/// 循环引用与缺失文件都会返回错误，不会静默跳过。
pub fn resolve_includes(text: &str, base: &Path) -> Result<String, String> {
    let mut out = String::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    expand_includes(text, base, &mut out, &mut stack)?;
    Ok(out)
}

fn expand_includes(
    text: &str,
    base: &Path,
    out: &mut String,
    stack: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if stack.len() >= MAX_INCLUDE_DEPTH {
        return Err(format!("include 嵌套超过 {MAX_INCLUDE_DEPTH} 层"));
    }
    for line in text.lines() {
        match include_target(line) {
            Some(rel) => {
                let path = base.join(&rel);
                let canon = path
                    .canonicalize()
                    .map_err(|e| format!("include 无法定位 {}: {e}", path.display()))?;
                // stack 是「当前正在展开的文件链」，命中即成环
                if stack.iter().any(|p| p == &canon) {
                    return Err(format!("include 循环引用: {}", canon.display()));
                }
                let content = std::fs::read_to_string(&canon)
                    .map_err(|e| format!("include 读取失败 {}: {e}", canon.display()))?;
                let child_base = canon
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."));
                stack.push(canon);
                expand_includes(&content, &child_base, out, stack)?;
                stack.pop();
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    Ok(())
}

/// 解析 SML 文件，并展开其中的 include 指令。
///
/// 相对路径以**该文件所在目录**为基准。
pub fn parse_file(path: impl AsRef<Path>) -> Result<Value, String> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读取失败 {}: {e}", path.display()))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let expanded = resolve_includes(&text, &base)?;
    parse(&expanded)
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
// serde 支持（可选 feature：`serde`）
//
// 手写实现而非 `#[derive]`：derive 会把枚举表示为外部标签形式
//   Value::Int(5)  ->  {"Int":5}
// 而配置文件的实际用途希望是自然形状  5。
// 手写后 SML 的 Value 与 JSON 数据形状一致，可直接喂给
// serde_json / serde_yaml / toml 等任意 serde 后端。
//
// 不启用该 feature 时 crate 保持零依赖。
// ---------------------------------------------------------------------------

#[cfg(feature = "serde")]
mod serde_impl {
    use super::Value;
    use serde::de::{self, MapAccess, SeqAccess, Visitor};
    use serde::ser::SerializeMap;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;
    use std::fmt;

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
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------- version ----------------

    #[test]
    fn version_defaults_to_current_when_absent() {
        // 既有文档没有版本声明，必须仍能解析且默认为当前版本
        let (v, ver) = parse_versioned("a: 1\n").unwrap();
        assert_eq!(ver, Version::CURRENT);
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
    }

    #[test]
    fn version_declared_as_v1() {
        let (v, ver) = parse_versioned("@version v1\na: 1\n").unwrap();
        assert_eq!(ver, Version::V1);
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
    }

    #[test]
    fn version_declaration_is_stripped_not_parsed_as_content() {
        // 若未剥离，`@version v1` 会被当成片段定义而解析异常
        let v = parse("@version v1\na: 1\n").unwrap();
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
        assert!(v.get("version").is_none(), "@version 不应进入数据");
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let err = parse_versioned("@version v99\na: 1\n").unwrap_err();
        assert!(err.contains("不支持"), "应拒绝不支持的版本，got: {err}");
        assert!(err.contains("v99"), "错误应含版本号，got: {err}");
    }

    #[test]
    fn conflicting_version_is_rejected() {
        let err = parse_versioned("@version v1\n@version v2\n").unwrap_err();
        // v2 尚未定义，优先报「不支持」
        assert!(!err.is_empty());
        // 两个都支持但不一致时的路径：v1 与 v1 不冲突
        let (_, ver) = parse_versioned("@version v1\n@version v1\n").unwrap();
        assert_eq!(ver, Version::V1, "重复但一致的声明应被接受");
    }

    #[test]
    fn version_is_reserved_as_fragment_name() {
        let err = parse("@version { x: 1 }\n").unwrap_err();
        assert!(err.contains("保留") || err.contains("版本声明"), "got: {err}");
    }

    #[test]
    fn version_works_with_include() {
        let d = tmpdir("version");
        std::fs::write(d.join("p.sml"), "@version v1\nb: 2\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\ninclude \"p.sml\"\n").unwrap();
        let (v, ver) = parse_file_versioned(d.join("main.sml")).unwrap();
        assert_eq!(ver, Version::V1);
        assert_eq!(v.get("b"), Some(&Value::Int(2)), "版本与 include 应协同");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn version_display_matches_name() {
        assert_eq!(Version::V1.name(), "v1");
        assert_eq!(format!("{}", Version::V1), "v1");
    }

    // ---------------- include ----------------

    /// 在临时目录下建文件，返回目录句柄（drop 时自动清理）
    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("sml_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("create tmpdir");
        d
    }

    #[test]
    fn include_inlines_external_file() {
        let d = tmpdir("inline");
        std::fs::write(d.join("part.sml"), "port: 8080\n").unwrap();
        std::fs::write(d.join("main.sml"), "host: local\ninclude \"part.sml\"\n").unwrap();

        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(v.get("host").unwrap().as_str(), Some("local"));
        assert_eq!(v.get("port"), Some(&Value::Int(8080)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_at_prefix_is_equivalent() {
        let d = tmpdir("at");
        std::fs::write(d.join("p.sml"), "b: 2\n").unwrap();
        std::fs::write(d.join("m.sml"), "@include \"p.sml\"\n").unwrap();
        let v = parse_file(d.join("m.sml")).unwrap();
        assert_eq!(v.get("b"), Some(&Value::Int(2)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_resolves_relative_to_including_file() {
        // 关键：相对路径按「被包含文件自身目录」解析，而非进程工作目录
        let d = tmpdir("nested");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("sub/leaf.sml"), "leaf: yes\n").unwrap();
        // mid 在根，include sub/mid2；mid2 在 sub 内，include leaf.sml（相对 sub）
        std::fs::write(d.join("sub/mid2.sml"), "include \"leaf.sml\"\n").unwrap();
        std::fs::write(d.join("main.sml"), "include \"sub/mid2.sml\"\n").unwrap();

        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(
            v.get("leaf").unwrap().as_str(),
            Some("yes"),
            "嵌套 include 的路径应相对各自所在目录解析"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_inside_block_injects_fields() {
        // 文本内联语义：可在块内注入一组字段
        let d = tmpdir("block");
        std::fs::write(d.join("fields.sml"), "region: cn-north-1\nzone: a\n").unwrap();
        std::fs::write(d.join("main.sml"), "server web {\ninclude \"fields.sml\"\nport: 8080\n}\n").unwrap();

        let v = parse_file(d.join("main.sml")).unwrap();
        let server = v.get("server").expect("应有 server 块");
        assert_eq!(server.get("region").unwrap().as_str(), Some("cn-north-1"));
        assert_eq!(server.get("zone").unwrap().as_str(), Some("a"));
        assert_eq!(server.get("port"), Some(&Value::Int(8080)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_detects_cycles() {
        let d = tmpdir("cycle");
        std::fs::write(d.join("a.sml"), "include \"b.sml\"\n").unwrap();
        std::fs::write(d.join("b.sml"), "include \"a.sml\"\n").unwrap();
        let err = parse_file(d.join("a.sml")).unwrap_err();
        assert!(err.contains("循环引用"), "应报循环引用，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_missing_file_is_error() {
        let d = tmpdir("missing");
        std::fs::write(d.join("m.sml"), "include \"nope.sml\"\n").unwrap();
        let err = parse_file(d.join("m.sml")).unwrap_err();
        assert!(err.contains("nope.sml"), "错误应含缺失文件名，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn hash_in_quoted_string_is_not_a_comment() {
        // 引号内的 # 不应被当成注释，否则 `include "a#b.sml"` 会被截断
        assert_eq!(strip_line_comment("k: \"a#b\""), "k: \"a#b\"");
        assert_eq!(strip_line_comment("k: v # comment"), "k: v ");
    }

    #[test]
    fn include_line_is_not_confused_with_key_named_include() {
        // `key: include` 是指令吗？不是——前面有 key 与冒号
        assert_eq!(include_target("key: include"), None);
        assert_eq!(include_target("include \"a.sml\""), Some("a.sml".into()));
        assert_eq!(include_target("@include \"a.sml\""), Some("a.sml".into()));
        assert_eq!(include_target("# include \"a.sml\""), None, "注释行不生效");
    }

    // ---------------- serde ----------------

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_preserves_shape() {
        let v = parse("name: John\nage: 27\ntags: [a b]\nnested { k: v }\n").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        // 自然形状：字符串就是字符串，数字就是数字，而非 {"Int":27}
        assert!(json.contains("\"name\":\"John\""), "got: {json}");
        assert!(json.contains("\"age\":27"), "got: {json}");
        assert!(json.contains("\"tags\":[\"a\",\"b\"]"), "got: {json}");
        assert!(json.contains("\"nested\":{\"k\":\"v\"}"), "got: {json}");

        let back: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v, "serde 往返应还原原值");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_deserializes_json_into_value() {
        let v: Value = serde_json::from_str(r#"{"s":"x","i":5,"f":1.5,"b":true,"n":null,"a":[1,2]}"#).unwrap();
        assert_eq!(v.get("s").unwrap().as_str(), Some("x"));
        assert_eq!(v.get("i"), Some(&Value::Int(5)));
        assert_eq!(v.get("f"), Some(&Value::Float(1.5)));
        assert_eq!(v.get("b"), Some(&Value::Bool(true)));
        assert_eq!(v.get("n"), Some(&Value::Null));
        assert!(matches!(v.get("a"), Some(Value::Array(a)) if a.len() == 2));
    }

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
