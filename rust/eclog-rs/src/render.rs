// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! 渲染器：`typical` 与 `json_compatible` 两种 scheme，逐字节对齐 Eclog-CPP。
//! Renderer: `typical` and `json_compatible` schemes, byte-identical to Eclog-CPP.
//!
//! 移植自 C++ `Source/Renderer.cpp` 的状态机（缩进 / 分隔符 / inline 深度），
//! 判据是解析同一份输入后渲染出的字节与 C++ oracle 完全一致。
//! Ported from the C++ `Source/Renderer.cpp` state machine (indent / separator /
//! inline depth); the acceptance criterion is byte equality with the C++ oracle.
//!
//! 数值**不看 `raw`**：C++ 的值树只存数值本身（`int` / `long long` / `double` …），
//! 渲染时按数值重新格式化。因此 `+0` 渲染成 `0`、`-0` 渲染成 `0`（整数零没有符号位），
//! 而 `-0.0` 渲染成 `-0`、`3.14159265358979323846` 渲染成 `3.141592653589793`。
//! Numbers ignore `raw`: the C++ tree keeps only the numeric value, so rendering
//! reformats it. Hence `+0` → `0`, `-0` → `0` (integer zero has no sign), `-0.0` → `-0`.
//!
//! 字符串**看记法**：`Str` 的 `notation` / `delimiter` 决定能否原样输出裸词 / raw /
//! heredoc，不满足条件时降级为 quoted（C++ 同一套判据）。
//! Strings honour the notation: unquoted / raw / heredoc are kept only when the
//! content is still unambiguous, otherwise they degrade to quoted.

use alloc::format;
use alloc::string::{String, ToString};

use crate::value::{Number, NumberInfo, Str, StringNotation, Value};

/// 行分隔符 ｜ Line separator.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Linebreak {
    /// `\r`
    Cr,
    /// `\n`（默认）｜ `\n` (default).
    #[default]
    Lf,
    /// `\r\n`
    CrLf,
}

impl Linebreak {
    fn as_str(self) -> &'static str {
        match self {
            Self::Cr => "\r",
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

/// 渲染 scheme，对应 C++ `RendererConfig::Scheme`。
/// Render scheme, mirroring C++ `RendererConfig::Scheme`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scheme {
    /// 人类可读：根无 `{}`、裸词、无逗号、`inf`/`nan` 裸写、允许注释。
    /// Human-friendly: no root braces, bare words, no commas, bare `inf`/`nan`.
    #[default]
    Typical,
    /// JSON 兼容：根有 `{}`、全 quoted、逗号分隔、`inf`/`nan` 写成字符串。
    /// JSON-compatible: root braces, all quoted, commas, `inf`/`nan` as strings.
    JsonCompatible,
}

/// 排版模式 ｜ Formatting mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Formatting {
    /// 换行 + 缩进 ｜ linebreaks and indentation.
    #[default]
    Regular,
    /// 单行 ｜ single line.
    Compact,
}

/// 缩进字符 ｜ Indent character.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum IndentCharacter {
    /// `\t`（默认，与 C++ 一致）｜ `\t` (default, as in C++).
    #[default]
    Tab,
    /// 空格 ｜ space.
    Space,
}

/// 渲染配置，字段与 C++ `RendererConfig` 一一对应。
/// Render configuration; fields mirror C++ `RendererConfig`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RendererConfig {
    /// scheme ｜ scheme.
    pub scheme: Scheme,
    /// 排版模式 ｜ formatting mode.
    pub formatting: Formatting,
    /// 行分隔符 ｜ line separator.
    pub linebreak: Linebreak,
    /// 缩进字符 ｜ indent character.
    pub indent_character: IndentCharacter,
    /// 每级缩进重复次数 ｜ repetitions per indent level.
    pub indent_size: usize,
    /// 冒号后是否补空格（仅标量成员、regular）｜ space after colon (regular, scalars).
    pub insert_space_after_colon: bool,
    /// 对象的 `{` 是否另起一行 ｜ put `{` on its own line.
    pub place_open_bracket_on_new_line_for_objects: bool,
    /// 数组的 `[` 是否另起一行 ｜ put `[` on its own line.
    pub place_open_bracket_on_new_line_for_arrays: bool,
    /// 根对象是否加 `{}` ｜ wrap the root object in braces.
    pub enclose_root_object_with_curly_brackets: bool,
    /// 是否强制所有字符串加引号 ｜ force quoted strings only.
    pub use_quoted_strings_only: bool,
    /// 是否用逗号分隔 ｜ separate items with commas.
    pub use_commas_to_separate_items: bool,
    /// `inf` / `nan` 是否写成带引号字符串 ｜ render `inf`/`nan` as quoted strings.
    pub use_strings_to_represent_infinities_and_nans: bool,
    /// 是否输出注释 ｜ emit comments.
    pub enable_comments: bool,
}

impl RendererConfig {
    /// 按 scheme 取该 scheme 的默认配置 ｜ defaults for the given scheme.
    pub fn new(scheme: Scheme) -> Self {
        let mut config = Self {
            scheme,
            formatting: Formatting::Regular,
            linebreak: Linebreak::Lf,
            indent_character: IndentCharacter::Tab,
            indent_size: 1,
            insert_space_after_colon: true,
            place_open_bracket_on_new_line_for_objects: true,
            place_open_bracket_on_new_line_for_arrays: true,
            enclose_root_object_with_curly_brackets: false,
            use_quoted_strings_only: false,
            use_commas_to_separate_items: false,
            use_strings_to_represent_infinities_and_nans: false,
            enable_comments: true,
        };

        if scheme == Scheme::JsonCompatible {
            config.enclose_root_object_with_curly_brackets = true;
            config.use_quoted_strings_only = true;
            config.use_commas_to_separate_items = true;
            config.use_strings_to_represent_infinities_and_nans = true;
            config.enable_comments = false;
        }

        config
    }

    /// `typical` 默认配置 ｜ `typical` defaults.
    pub fn typical() -> Self {
        Self::new(Scheme::Typical)
    }

    /// `json_compatible` 默认配置 ｜ `json_compatible` defaults.
    pub fn json_compatible() -> Self {
        Self::new(Scheme::JsonCompatible)
    }
}

impl Default for RendererConfig {
    fn default() -> Self {
        Self::typical()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Separator {
    None,
    Linebreak,
    Comma,
}

/// 流式渲染器；状态字段与 C++ `detail::Renderer` 同名同义。
/// Streaming renderer; state fields mirror C++ `detail::Renderer`.
pub struct Renderer {
    out: String,
    config: RendererConfig,
    nesting_level: usize,
    indent: usize,
    begin_of_line: bool,
    separator: Separator,
    inline: usize,
    has_root: bool,
}

impl Renderer {
    /// 新建渲染器（输出累积在内部缓冲）｜ create a renderer with an internal buffer.
    pub fn new(config: RendererConfig) -> Self {
        Self {
            out: String::new(),
            config,
            nesting_level: 0,
            indent: 0,
            begin_of_line: true,
            separator: Separator::None,
            inline: 0,
            has_root: false,
        }
    }

    /// 当前配置 ｜ the current configuration.
    pub fn config(&self) -> &RendererConfig {
        &self.config
    }

    /// 到目前为止的产物 ｜ output so far.
    pub fn as_str(&self) -> &str {
        &self.out
    }

    /// 取出产物 ｜ take the output.
    pub fn into_string(self) -> String {
        self.out
    }

    /// 开始根对象（`typical` 下不写 `{`）｜ begin the root object.
    pub fn begin_root_object(&mut self) {
        self.begin_object_inner(None);
    }

    /// 结束根对象 ｜ end the root object.
    pub fn end_root_object(&mut self) {
        self.end_object_inner();
        self.has_root = true;
    }

    /// 开始一个数组元素位置的对象 ｜ begin an object in an array slot.
    pub fn begin_object(&mut self) {
        self.begin_object_inner(None);
    }

    /// 开始一个带键的对象 ｜ begin an object with a key.
    pub fn begin_object_with_key(&mut self, key: &Str) {
        self.begin_object_inner(Some(key));
    }

    /// 结束对象 ｜ end an object.
    pub fn end_object(&mut self) {
        self.end_object_inner();
    }

    /// 开始一个数组元素位置的数组 ｜ begin an array in an array slot.
    pub fn begin_array(&mut self) {
        self.begin_array_inner(None);
    }

    /// 开始一个带键的数组 ｜ begin an array with a key.
    pub fn begin_array_with_key(&mut self, key: &Str) {
        self.begin_array_inner(Some(key));
    }

    /// 结束数组 ｜ end an array.
    pub fn end_array(&mut self) {
        self.end_array_inner();
    }

    /// 渲染 `key: value`（值可任意类型，容器会递归）｜ render `key: value`.
    pub fn render_member(&mut self, key: &Str, value: &Value) {
        match value {
            Value::Object(object) => {
                self.begin_object_with_key(key);
                for (child_key, child) in object.iter() {
                    self.render_member(child_key, child);
                }
                self.end_object();
            }
            Value::Array(items) => {
                self.begin_array_with_key(key);
                for child in items {
                    self.render_value(child);
                }
                self.end_array();
            }
            _ => {
                self.begin_item();
                self.render_string(key.text(), key.notation(), key.delimiter());
                self.render_colon();
                if self.config.formatting == Formatting::Regular
                    && self.config.insert_space_after_colon
                {
                    self.render_space();
                }
                self.render_value_internal(value);
                self.end_item();
            }
        }
    }

    /// 渲染一个数组元素位置的值（容器会递归）｜ render a value in an array slot.
    pub fn render_value(&mut self, value: &Value) {
        match value {
            Value::Object(object) => {
                self.begin_object();
                for (child_key, child) in object.iter() {
                    self.render_member(child_key, child);
                }
                self.end_object();
            }
            Value::Array(items) => {
                self.begin_array();
                for child in items {
                    self.render_value(child);
                }
                self.end_array();
            }
            _ => {
                self.begin_item();
                self.render_value_internal(value);
                self.end_item();
            }
        }
    }

    /// 渲染 `count` 个空行（compact / inline 下忽略）｜ render blank lines.
    pub fn render_empty_lines(&mut self, count: usize) {
        if self.config.formatting == Formatting::Compact || self.inline > 0 {
            return;
        }
        self.render_separator();
        for _ in 0..count {
            self.render_linebreak();
        }
    }

    /// 渲染一条 `#` 注释（compact / inline / 关闭注释时忽略）｜ render a `#` comment.
    pub fn render_comment(&mut self, comment: &str) {
        if self.config.formatting == Formatting::Compact
            || self.inline > 0
            || !self.config.enable_comments
        {
            return;
        }

        self.render_separator();
        self.render_indent();
        self.render_char('#');

        let mut chars = comment.chars().peekable();
        let mut linebreak = false;
        let mut begin_of_line = true;

        while let Some(c) = chars.next() {
            if c == '\r' {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                linebreak = true;
            } else if c == '\n' {
                linebreak = true;
            }

            if linebreak {
                self.render_linebreak();
                self.render_indent();
                self.render_char('#');
                linebreak = false;
                begin_of_line = true;
                continue;
            }

            if begin_of_line {
                self.render_space();
                begin_of_line = false;
            }

            self.render_char(c);
        }

        self.render_linebreak();
    }

    /// 进入 inline（不换行）区 ｜ enter an inline (no linebreak) region.
    pub fn begin_inline(&mut self) -> usize {
        self.inline += 1;
        self.inline
    }

    /// 离开 inline 区 ｜ leave an inline region.
    pub fn end_inline(&mut self) -> usize {
        self.inline -= 1;
        if self.inline == 0
            && self.separator == Separator::Comma
            && self.config.formatting == Formatting::Regular
        {
            self.separator = Separator::Linebreak;
        }
        self.inline
    }

    /// 收尾：未渲染过根对象时补一个空对象，再补上待写的换行。
    /// Finish: emit an empty root object when none was rendered, then flush a
    /// pending linebreak (C++ `Renderer::close`).
    pub fn close(&mut self) {
        if !self.has_root {
            self.has_root = true;
            self.begin_object_inner(None);
            self.end_object_inner();
        }
        self.flush();
    }

    fn flush(&mut self) {
        if self.separator == Separator::Linebreak {
            self.render_linebreak();
        }
        self.separator = Separator::None;
    }

    fn begin_object_inner(&mut self, key: Option<&Str>) {
        if let Some(key) = key {
            self.begin_item();
            self.render_string(key.text(), key.notation(), key.delimiter());
            self.render_colon();

            if self.config.formatting == Formatting::Regular {
                if !self.config.place_open_bracket_on_new_line_for_objects || self.inline > 0 {
                    self.render_space();
                } else {
                    self.render_linebreak();
                    self.render_indent();
                }
            }

            self.render_char('{');
            self.begin_struct();
            self.nesting_level += 1;
            return;
        }

        if self.nesting_level > 0 || self.config.enclose_root_object_with_curly_brackets {
            self.begin_item();
            self.render_char('{');
            self.begin_struct();
        }

        self.nesting_level += 1;
    }

    fn end_object_inner(&mut self) {
        self.nesting_level -= 1;

        if self.nesting_level > 0 || self.config.enclose_root_object_with_curly_brackets {
            self.end_struct();
            self.render_char('}');
            self.end_item();
        }
    }

    fn begin_array_inner(&mut self, key: Option<&Str>) {
        if let Some(key) = key {
            self.begin_item();
            self.render_string(key.text(), key.notation(), key.delimiter());
            self.render_colon();

            if self.config.formatting == Formatting::Regular {
                if !self.config.place_open_bracket_on_new_line_for_arrays || self.inline > 0 {
                    self.render_space();
                } else {
                    self.render_linebreak();
                    self.render_indent();
                }
            }

            self.render_char('[');
            self.begin_struct();
            self.nesting_level += 1;
            return;
        }

        self.begin_item();
        self.render_char('[');
        self.begin_struct();
        self.nesting_level += 1;
    }

    fn end_array_inner(&mut self) {
        self.nesting_level -= 1;
        self.end_struct();
        self.render_char(']');
        self.end_item();
    }

    fn begin_item(&mut self) {
        self.render_separator();
        if self.begin_of_line {
            self.render_indent();
        }
    }

    fn end_item(&mut self) {
        if self.config.formatting == Formatting::Regular && self.inline == 0 {
            if !self.begin_of_line {
                self.separator = Separator::Linebreak;
            }
        } else {
            self.separator = Separator::Comma;
        }
    }

    fn begin_struct(&mut self) {
        if self.config.formatting == Formatting::Regular && self.inline == 0 {
            self.render_linebreak();
        }
        self.indent += 1;
    }

    fn end_struct(&mut self) {
        self.indent -= 1;

        if self.config.formatting == Formatting::Regular && self.inline == 0 {
            if !self.begin_of_line {
                self.render_linebreak();
            }
            if self.begin_of_line {
                self.render_indent();
            }
        }
    }

    fn render_value_internal(&mut self, value: &Value) {
        match value {
            Value::Null => self.render_sequence("null"),
            Value::Bool(true) => self.render_sequence("true"),
            Value::Bool(false) => self.render_sequence("false"),
            Value::Str(text) => self.render_string(text.text(), text.notation(), text.delimiter()),
            Value::Number(number) => self.render_number(number),
            // 容器走 begin/end，不会走到这里。
            Value::Object(_) | Value::Array(_) => {}
        }
    }

    fn render_string(&mut self, text: &str, notation: StringNotation, delimiter: &str) {
        if notation == StringNotation::Quoted || self.config.use_quoted_strings_only {
            self.render_quoted_string(text);
            return;
        }

        match notation {
            StringNotation::Unquoted => {
                if matches!(text, "null" | "true" | "false" | "inf" | "nan") {
                    self.render_quoted_string(text);
                    return;
                }

                let mut count = 0usize;
                for byte in text.bytes() {
                    let ok = byte.is_ascii_alphabetic()
                        || byte == b'_'
                        || (count > 0 && (byte == b'-' || byte == b'.' || byte.is_ascii_digit()));
                    if ok {
                        count += 1;
                    } else {
                        break;
                    }
                }

                if !text.is_empty() && count == text.len() {
                    self.render_sequence(text);
                } else {
                    self.render_quoted_string(text);
                }
            }

            StringNotation::Raw => {
                if raw_string_is_emittable(text, delimiter) {
                    self.render_raw_string(text, delimiter);
                } else {
                    self.render_quoted_string(text);
                }
            }

            StringNotation::Heredoc => {
                if self.config.formatting == Formatting::Compact
                    || self.inline > 0
                    || delimiter.is_empty()
                {
                    self.render_quoted_string(text);
                } else if heredoc_is_emittable(text, delimiter) {
                    self.render_heredoc(text, delimiter);
                } else {
                    self.render_quoted_string(text);
                }
            }

            StringNotation::Quoted => self.render_quoted_string(text),
        }
    }

    fn render_quoted_string(&mut self, text: &str) {
        self.render_char('"');
        for c in text.chars() {
            match c {
                '"' => self.render_sequence("\\\""),
                '\\' => self.render_sequence("\\\\"),
                '\u{8}' => self.render_sequence("\\b"),
                '\u{c}' => self.render_sequence("\\f"),
                '\n' => self.render_sequence("\\n"),
                '\r' => self.render_sequence("\\r"),
                '\t' => self.render_sequence("\\t"),
                c if c < ' ' => {
                    let escaped = format!("\\u{:04x}", c as u32);
                    self.render_sequence(&escaped);
                }
                c => self.render_char(c),
            }
        }
        self.render_char('"');
    }

    fn render_raw_string(&mut self, text: &str, delimiter: &str) {
        self.render_char('@');
        for c in delimiter.chars() {
            self.render_char(c);
        }
        self.render_char('"');
        for c in text.chars() {
            self.render_char(c);
        }
        self.render_char('"');
        for c in delimiter.chars() {
            self.render_char(c);
        }
    }

    fn render_heredoc(&mut self, text: &str, delimiter: &str) {
        self.render_char('|');
        for c in delimiter.chars() {
            self.render_char(c);
        }
        self.render_linebreak();

        let mut just_broke = true;
        self.indent += 1;

        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    self.render_linebreak();
                    just_broke = true;
                }
                '\n' => {
                    self.render_linebreak();
                    just_broke = true;
                }
                c => {
                    if self.begin_of_line {
                        self.render_indent();
                    }
                    self.render_char(c);
                    just_broke = false;
                }
            }
        }

        if !just_broke {
            self.render_linebreak();
        }

        self.render_indent();
        for c in delimiter.chars() {
            self.render_char(c);
        }
        self.render_linebreak();
        self.indent -= 1;
    }

    fn render_number(&mut self, number: &Number) {
        let rendered = format_number(number);

        let as_quoted = matches!(rendered.as_str(), s if s.ends_with("inf") || s.ends_with("nan"))
            && self.config.use_strings_to_represent_infinities_and_nans;

        if as_quoted {
            self.render_quoted_string(&rendered);
        } else {
            self.render_sequence(&rendered);
        }
    }

    fn render_separator(&mut self) {
        match self.separator {
            Separator::Linebreak => {
                if self.config.use_commas_to_separate_items {
                    self.render_comma();
                }
                self.render_linebreak();
                self.separator = Separator::None;
            }
            Separator::Comma => {
                self.render_comma();
                if self.config.formatting == Formatting::Regular {
                    self.render_space();
                }
                self.separator = Separator::None;
            }
            Separator::None => {}
        }
    }

    fn render_linebreak(&mut self) {
        let text = self.config.linebreak.as_str();
        self.out.push_str(text);
        self.begin_of_line = true;
    }

    fn render_indent(&mut self) {
        if self.indent == 0 || self.config.indent_size == 0 {
            return;
        }
        let c = match self.config.indent_character {
            IndentCharacter::Tab => '\t',
            IndentCharacter::Space => ' ',
        };
        for _ in 0..self.indent {
            for _ in 0..self.config.indent_size {
                self.out.push(c);
            }
        }
        self.begin_of_line = false;
    }

    fn render_space(&mut self) {
        self.render_char(' ');
    }

    fn render_colon(&mut self) {
        self.render_char(':');
    }

    fn render_comma(&mut self) {
        self.render_char(',');
    }

    fn render_sequence(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.out.push_str(text);
        self.begin_of_line = false;
    }

    fn render_char(&mut self, c: char) {
        self.out.push(c);
        self.begin_of_line = false;
    }
}

/// 把值树渲染成文本（`Document` 入口的等价物）。
/// Render a value tree into text (the equivalent of the `Document` entry point).
///
/// 根是对象时按 C++ `DocumentRenderer` 走（`typical` 不写 `{}`）；其它根类型是本
/// crate 的扩展，按单个值渲染。
/// Object roots follow C++ `DocumentRenderer`; other root kinds are an extension of
/// this crate and render as a single value.
pub fn render(value: &Value, config: &RendererConfig) -> String {
    let mut renderer = Renderer::new(config.clone());

    match value {
        Value::Object(object) => {
            renderer.begin_root_object();
            for (key, child) in object.iter() {
                renderer.render_member(key, child);
            }
            renderer.end_root_object();
        }
        other => {
            renderer.render_value(other);
            // 单个值不是 C++ 的 Document 形态：不让 `close()` 再补一个空根对象。
            renderer.has_root = true;
        }
    }

    renderer.close();
    renderer.into_string()
}

/// 渲染为 `typical` scheme ｜ render with the `typical` scheme.
pub fn to_typical(value: &Value) -> String {
    render(value, &RendererConfig::typical())
}

/// 渲染为 `json_compatible` scheme ｜ render with the `json_compatible` scheme.
pub fn to_json_compatible(value: &Value) -> String {
    render(value, &RendererConfig::json_compatible())
}

/// raw 串能否原样输出：无控制字符（`\t` 除外），且没有会提前终止的 `"` + 定界符。
/// Whether a raw string round-trips: no control characters (except `\t`), and no
/// `"` followed by the delimiter (which would terminate it early).
fn raw_string_is_emittable(text: &str, delimiter: &str) -> bool {
    let bytes = text.as_bytes();
    for (index, &byte) in bytes.iter().enumerate() {
        if byte < 0x20 && byte != b'\t' {
            return false;
        }
        if byte == b'"' && (delimiter.is_empty() || text[index + 1..].starts_with(delimiter)) {
            return false;
        }
    }
    true
}

/// heredoc 能否原样输出：无控制字符（`\t` 除外），且没有一行以定界符开头（后接行尾）。
/// Whether a heredoc round-trips: no control characters (except `\t`), and no line
/// starting with the delimiter followed by a line ending.
fn heredoc_is_emittable(text: &str, delimiter: &str) -> bool {
    let bytes = text.as_bytes();
    let mut begin_of_line = true;
    let mut index = 0usize;

    while index < bytes.len() {
        let byte = bytes[index];

        if begin_of_line && (byte == b' ' || byte == b'\t') {
            index += 1;
        } else if begin_of_line && text[index..].starts_with(delimiter) {
            index += delimiter.len();
            if index == bytes.len() || bytes[index] == b'\r' || bytes[index] == b'\n' {
                return false;
            }
        } else if byte == b'\r' {
            index += 1;
            if index < bytes.len() && bytes[index] == b'\n' {
                index += 1;
            }
            begin_of_line = true;
        } else if byte == b'\n' {
            index += 1;
            begin_of_line = true;
        } else if byte < 0x20 && byte != b'\t' {
            return false;
        } else {
            index += 1;
            begin_of_line = false;
        }
    }

    true
}

/// 把 `Number` 折成 C++ 值树里的数值形态（`Value::asNumber` 的等价物）。
/// Fold a `Number` into the numeric form the C++ tree keeps.
enum NumberForm {
    Signed(i64),
    Unsigned(u64),
    Double(f64),
}

fn number_form(number: &Number) -> NumberForm {
    let info = number.info();
    let has_float_bits =
        info.contains(NumberInfo::FRAC | NumberInfo::EXP | NumberInfo::INF | NumberInfo::NAN);

    if !has_float_bits {
        if let Ok(value) = number.raw().parse::<i64>() {
            return NumberForm::Signed(value);
        }
        if info.contains(NumberInfo::NEG) {
            return NumberForm::Double(number.as_f64());
        }
        if let Ok(value) = number.raw().parse::<u64>() {
            return NumberForm::Unsigned(value);
        }
    }

    NumberForm::Double(number.as_f64())
}

fn format_number(number: &Number) -> String {
    match number_form(number) {
        NumberForm::Signed(value) => value.to_string(),
        NumberForm::Unsigned(value) => value.to_string(),
        NumberForm::Double(value) => format_double(value),
    }
}

/// C++ `doubleToString(d, buffer, -1)` 的等价实现：最短往返数字 + C++ 的排版规则。
/// Equivalent of C++ `doubleToString(d, buffer, -1)`.
///
/// 数字部分复用 Rust 的最短往返格式化（同样是"最短 + 最近 + 偶数优先"），排版规则
/// 照 C++：`[-4, 17)` 用定点，其它用 `e` 指数（**不带 `+`**，不补零）。
/// Digits come from Rust's shortest round-trip formatting; layout follows C++:
/// fixed-point for exponents in `[-4, 17)`, otherwise `e` notation (no `+`, no padding).
fn format_double(value: f64) -> String {
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-inf" } else { "inf" }.to_string();
    }
    if value.is_nan() {
        return if value.is_sign_negative() {
            "-nan"
        } else {
            "nan"
        }
        .to_string();
    }

    let negative = value < 0.0;
    let magnitude = if negative { -value } else { value };

    let scientific = format!("{:e}", magnitude);
    let (mantissa, exponent) = scientific
        .split_once('e')
        .expect("Rust lower-exp formatting always has 'e'");
    let exponent: i32 = exponent.parse().expect("exponent is an integer");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();

    let mut out = String::new();
    if negative {
        out.push('-');
    }

    if (-4..17).contains(&exponent) {
        if exponent < 0 {
            out.push_str("0.");
            for _ in 0..(-exponent - 1) {
                out.push('0');
            }
            out.push_str(&digits);
        } else {
            let point = (exponent + 1) as usize;
            if point < digits.len() {
                out.push_str(&digits[..point]);
                out.push('.');
                out.push_str(&digits[point..]);
            } else {
                out.push_str(&digits);
                for _ in digits.len()..point {
                    out.push('0');
                }
            }
        }
    } else {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push_str(&exponent.to_string());
    }

    out
}
