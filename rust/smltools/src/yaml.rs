//! YAML → `Value` 的**最小可用子集**解析器（供 `smltools --from yaml` 迁移用）。
//!
//! 目标不是实现完整 YAML 1.2（那是很大的规范，已有成熟实现），而是覆盖
//! **配置文件里真正常见的写法**，把存量 YAML 迁进 SML：
//!
//! **支持**
//! - 块映射 / 块序列（含 `- key: value` 内联映射及其后续同族行）
//! - 流式 `[a, b]` / `{k: v}`
//! - 标量：裸词、单/双引号（含转义）、整数（十进制 / `0x` / `0o` / `0b` / `_` 分隔）、
//!   浮点、`true|false`、`null|~`
//! - 注释（`#` 前须为空白或行首，故 `a#b` 不是注释）、空行、`---` 起始标记
//! - 块标量 `|`（保留换行）与 `>`（折叠为空格），含 `-` / `+` 收尾修饰
//! - 锚点 `&name` 与别名 `*name`（同一文档内）
//!
//! **不支持**（会报错而不是猜）
//! - `? 复杂键`、`!!` 标签、合并键 `<<`、多文档（只取首文档）
//! - 显式缩进指示 `|2` 按默认缩进处理
//!
//! **一个刻意的行为选择**：按 YAML 1.2 只把 `true/false` 认作布尔，
//! `yes/no/on/off` 一律当**字符串** —— 避免著名的「挪威问题」
//! （国家代码 `NO` 被 YAML 1.1 解析成 `false`）。

use std::collections::{BTreeMap, HashMap};

use sml::Value;

/// 预处理后的一行。
#[derive(Debug, Clone)]
struct Line {
    /// 前导空白宽度（前导只可能是空格/制表符，都是 ASCII，按字节计安全）
    indent: usize,
    /// 去掉行尾注释并 trim 后的内容
    text: String,
    /// 原始行：块标量 `|` / `>` 的内容里 `#` 是正文，必须用原始行
    raw: String,
    /// 1 起的行号，用于报错定位
    no: usize,
}

/// 从文本解析 YAML 子集。空文档按空对象返回（SML 顶层须为容器）。
pub fn parse(text: &str) -> Result<Value, String> {
    let mut p = Parser {
        lines: preprocess(text),
        i: 0,
        anchors: HashMap::new(),
    };
    p.skip_blank();
    if p.i >= p.lines.len() {
        return Ok(Value::Object(BTreeMap::new()));
    }
    let indent = p.lines[p.i].indent;
    let v = p.parse_node(indent)?;
    p.skip_blank();
    if p.i < p.lines.len() {
        return Err(format!(
            "第 {} 行：意外的内容 `{}`（缩进不一致，或存在多份文档）",
            p.lines[p.i].no, p.lines[p.i].text
        ));
    }
    Ok(v)
}

/// 预处理：去 BOM、跳过文档标记、算缩进、去行尾注释（同时保留原始行）。
fn preprocess(text: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let s = if idx == 0 {
            raw.trim_start_matches('\u{feff}')
        } else {
            raw
        };
        let trimmed = s.trim_start();
        if trimmed == "---" || trimmed == "..." {
            continue;
        }
        out.push(Line {
            indent: s.len() - trimmed.len(),
            text: strip_comment(trimmed).trim_end().to_string(),
            raw: raw.to_string(),
            no: idx + 1,
        });
    }
    out
}

/// 去掉行尾注释：`#` 仅在行首或前面是空白时才算注释（YAML 规则），
/// 因此 `a#b`、`url: http://x#y` 里的 `#` 是正文。
fn strip_comment(s: &str) -> &str {
    let b = s.as_bytes();
    let (mut in_s, mut in_d) = (false, false);
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !in_d => in_s = !in_s,
            b'"' if !in_s => in_d = !in_d,
            b'#' if !in_s && !in_d => {
                if i == 0 || b[i - 1] == b' ' || b[i - 1] == b'\t' {
                    return &s[..i];
                }
            }
            _ => {}
        }
    }
    s
}

/// 切分 `键: 值`。
///
/// 冒号须后跟空白或位于行尾（YAML 规则），否则 `url: http://x` 里
/// `http` 后面的冒号会被误当成分隔符。引号内与流式括号内的冒号不算。
fn split_map_entry(s: &str) -> Option<(String, String)> {
    let b = s.as_bytes();
    let (mut in_s, mut in_d) = (false, false);
    let mut depth = 0usize;
    for i in 0..b.len() {
        match b[i] {
            b'\'' if !in_d => in_s = !in_s,
            b'"' if !in_s => in_d = !in_d,
            b'[' | b'{' if !in_s && !in_d => depth += 1,
            b']' | b'}' if !in_s && !in_d => depth = depth.saturating_sub(1),
            b':' if !in_s && !in_d && depth == 0 => {
                let next_ok = i + 1 >= b.len() || b[i + 1] == b' ' || b[i + 1] == b'\t';
                if next_ok {
                    let k = s[..i].trim();
                    if k.is_empty() {
                        return None;
                    }
                    return Some((k.to_string(), s[i + 1..].trim().to_string()));
                }
            }
            _ => {}
        }
    }
    None
}

fn is_map_entry(s: &str) -> bool {
    split_map_entry(s).is_some()
}

fn is_seq_item(s: &str) -> bool {
    s == "-" || s.starts_with("- ")
}

/// 块标量的收尾修饰。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chomp {
    /// 默认：保留单个结尾换行
    Clip,
    /// `-`：去掉结尾换行
    Strip,
    /// `+`：保留全部结尾空行
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BlockScalar {
    /// `>` 折叠为空格；`|` 保留换行
    fold: bool,
    chomp: Chomp,
}

/// 识别 `|` / `>` / `|-` / `>-` / `|+` / `>+` 等。
fn block_scalar_kind(s: &str) -> Option<BlockScalar> {
    let t = s.trim();
    let mut it = t.chars();
    let fold = match it.next()? {
        '|' => false,
        '>' => true,
        _ => return None,
    };
    let rest: String = it.collect();
    let chomp = if rest.contains('-') {
        Chomp::Strip
    } else if rest.contains('+') {
        Chomp::Keep
    } else {
        Chomp::Clip
    };
    Some(BlockScalar { fold, chomp })
}

/// 去掉键名外层的引号。
///
/// 这里不绕道 `parse_scalar`：`Value` 实现了 `Drop`，按值 match 其内部 `String`
/// 会触发 E0509（不能从 Drop 类型里移出字段），直接处理引号更省事也少一次分配。
fn unquote_key(k: &str) -> String {
    let t = k.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        return unescape_double(&t[1..t.len() - 1]);
    }
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        return t[1..t.len() - 1].replace("''", "'");
    }
    t.to_string()
}

/// 标量取值：引号 / 布尔 / null / 数字 / 裸词。
fn parse_scalar(t: &str) -> Value {
    let t = t.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        return Value::Str(unescape_double(&t[1..t.len() - 1]));
    }
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        // 单引号里只有 `''` 一种转义（表示一个单引号）
        return Value::Str(t[1..t.len() - 1].replace("''", "'"));
    }
    match t {
        "null" | "Null" | "NULL" | "~" | "" => Value::Null,
        "true" | "True" | "TRUE" => Value::Bool(true),
        "false" | "False" | "FALSE" => Value::Bool(false),
        _ => {
            if let Some(i) = parse_int(t) {
                return Value::Int(i);
            }
            if let Some(f) = parse_float(t) {
                return Value::float(f);
            }
            Value::Str(t.to_string())
        }
    }
}

/// 双引号内的转义。
fn unescape_double(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('/') => out.push('/'),
            Some(other) => {
                // 未知转义：原样保留（YAML 规范也应报错，这里宽松处理）
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// 整数：十进制（可带正负号与 `_` 分隔）、`0x` / `0o` / `0b`。
fn parse_int(t: &str) -> Option<i64> {
    let s = t.replace('_', "");
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s.as_str()),
    };
    let body = body.strip_prefix('+').unwrap_or(body);
    let v = if let Some(h) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if let Some(o) = body.strip_prefix("0o").or_else(|| body.strip_prefix("0O")) {
        i64::from_str_radix(o, 8).ok()?
    } else if let Some(b) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        i64::from_str_radix(b, 2).ok()?
    } else {
        body.parse::<i64>().ok()?
    };
    Some(if neg { -v } else { v })
}

/// 浮点：必须含 `.` 或指数标记，否则交给 `parse_int`。
fn parse_float(t: &str) -> Option<f64> {
    let s = t.replace('_', "");
    if !(s.contains('.') || s.contains('e') || s.contains('E')) {
        return None;
    }
    s.parse::<f64>().ok()
}

/// 把块标量的内容行拼成字符串。
fn join_lines(lines: &[String], fold: bool) -> String {
    if !fold {
        return lines.join("\n");
    }
    // 折叠：连续非空行用空格连接，空行保留为换行
    let mut out = String::new();
    let mut prev_empty = true;
    for l in lines {
        if l.is_empty() {
            out.push('\n');
            prev_empty = true;
        } else {
            if !prev_empty && !out.is_empty() {
                out.push(' ');
            }
            out.push_str(l);
            prev_empty = false;
        }
    }
    out
}

/// 流式结构（`[...]` / `{...}`）解析器。
struct FlowParser<'a> {
    s: &'a [u8],
    i: usize,
    anchors: &'a mut HashMap<String, Value>,
}

impl<'a> FlowParser<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn value(&mut self, no: usize) -> Result<Value, String> {
        self.skip_ws();
        if self.i >= self.s.len() {
            return Ok(Value::Null);
        }
        match self.s[self.i] {
            b'[' => {
                self.i += 1;
                let mut out = Vec::new();
                loop {
                    self.skip_ws();
                    if self.i >= self.s.len() {
                        return Err(format!("第 {no} 行：`[` 未闭合"));
                    }
                    if self.s[self.i] == b']' {
                        self.i += 1;
                        break;
                    }
                    out.push(self.value(no)?);
                    self.skip_ws();
                    if self.i < self.s.len() && self.s[self.i] == b',' {
                        self.i += 1;
                    }
                }
                Ok(Value::Array(out))
            }
            b'{' => {
                self.i += 1;
                let mut m = BTreeMap::new();
                loop {
                    self.skip_ws();
                    if self.i >= self.s.len() {
                        return Err(format!("第 {no} 行：`{{` 未闭合"));
                    }
                    if self.s[self.i] == b'}' {
                        self.i += 1;
                        break;
                    }
                    let key = self.raw_token(no)?;
                    self.skip_ws();
                    if self.i < self.s.len() && self.s[self.i] == b':' {
                        self.i += 1;
                        let v = self.value(no)?;
                        m.insert(unquote_key(&key), v);
                    } else {
                        // `{a, b}` 形态：无值键按 null
                        m.insert(unquote_key(&key), Value::Null);
                    }
                    self.skip_ws();
                    if self.i < self.s.len() && self.s[self.i] == b',' {
                        self.i += 1;
                    }
                }
                Ok(Value::Object(m))
            }
            b'"' => {
                self.i += 1;
                let start = self.i;
                while self.i < self.s.len() && self.s[self.i] != b'"' {
                    if self.s[self.i] == b'\\' {
                        self.i += 1;
                    }
                    self.i += 1;
                }
                let raw = std::str::from_utf8(&self.s[start..self.i.min(self.s.len())])
                    .unwrap_or("")
                    .to_string();
                if self.i < self.s.len() {
                    self.i += 1; // 收尾引号
                }
                Ok(Value::Str(unescape_double(&raw)))
            }
            _ => {
                let t = self.raw_token(no)?;
                if let Some(name) = t.strip_prefix('*') {
                    return self
                        .anchors
                        .get(name.trim())
                        .cloned()
                        .ok_or_else(|| format!("第 {no} 行：别名 `*{}` 未定义", name.trim()));
                }
                Ok(parse_scalar(&t))
            }
        }
    }

    /// 读到分隔符（`,` / `]` / `}` / `:`）或引号结束为止的原始片段。
    fn raw_token(&mut self, no: usize) -> Result<String, String> {
        self.skip_ws();
        if self.i < self.s.len() && (self.s[self.i] == b'"' || self.s[self.i] == b'\'') {
            let quote = self.s[self.i];
            self.i += 1;
            let start = self.i;
            while self.i < self.s.len() && self.s[self.i] != quote {
                self.i += 1;
            }
            let t = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("").to_string();
            if self.i < self.s.len() {
                self.i += 1;
            }
            return Ok(t);
        }
        let start = self.i;
        while self.i < self.s.len() && !matches!(self.s[self.i], b',' | b']' | b'}' | b':') {
            self.i += 1;
        }
        std::str::from_utf8(&self.s[start..self.i])
            .map(|s| s.trim().to_string())
            .map_err(|_| format!("第 {no} 行：流式结构里有非法 UTF-8"))
    }
}

/// 块解析器。
struct Parser {
    lines: Vec<Line>,
    i: usize,
    anchors: HashMap<String, Value>,
}

impl Parser {
    fn skip_blank(&mut self) {
        while self.i < self.lines.len() && self.lines[self.i].text.is_empty() {
            self.i += 1;
        }
    }

    /// 解析一个节点：序列 / 映射 / 标量。
    fn parse_node(&mut self, indent: usize) -> Result<Value, String> {
        self.skip_blank();
        if self.i >= self.lines.len() {
            return Ok(Value::Null);
        }
        let line = &self.lines[self.i];
        if line.indent < indent {
            return Ok(Value::Null);
        }
        let text = line.text.clone();
        let no = line.no;
        if is_seq_item(&text) {
            self.parse_seq(indent)
        } else if text.starts_with('[') || text.starts_with('{') {
            self.i += 1;
            let mut f = FlowParser {
                s: text.as_bytes(),
                i: 0,
                anchors: &mut self.anchors,
            };
            let v = f.value(no)?;
            f.skip_ws();
            if f.i < f.s.len() {
                return Err(format!("第 {no} 行：流式结构后有多余字符"));
            }
            Ok(v)
        } else if is_map_entry(&text) {
            self.parse_map(indent)
        } else {
            self.i += 1;
            let v = parse_scalar(&text);
            Ok(v)
        }
    }

    /// 块映射。
    fn parse_map(&mut self, indent: usize) -> Result<Value, String> {
        let mut m = BTreeMap::new();
        loop {
            self.skip_blank();
            if self.i >= self.lines.len() {
                break;
            }
            let line = &self.lines[self.i];
            let (lindent, lno) = (line.indent, line.no);
            if lindent < indent {
                break;
            }
            let text = line.text.clone();
            if is_seq_item(&text) {
                break; // 交给上层的序列解析
            }
            if lindent > indent {
                return Err(format!("第 {lno} 行：缩进过深 `{text}`"));
            }
            let Some((raw_key, rest)) = split_map_entry(&text) else {
                return Err(format!("第 {lno} 行：不是 `键: 值` 形式 `{text}`"));
            };
            let key = unquote_key(&raw_key);
            self.i += 1;

            let val = if !rest.is_empty() {
                if let Some(kind) = block_scalar_kind(&rest) {
                    self.parse_block_scalar(indent, kind)?
                } else {
                    let (anchor, body) = split_anchor(&rest);
                    // `key: &锚点` 且锚点后无值 → 值在后续缩进块里（锚点指向该子节点）
                    let v = if anchor.is_some() && body.trim().is_empty() {
                        self.skip_blank();
                        if self.i < self.lines.len() && self.lines[self.i].indent > indent {
                            let child = self.lines[self.i].indent;
                            self.parse_node(child)?
                        } else {
                            Value::Null
                        }
                    } else {
                        self.scalar_or_flow(&body, lno)?
                    };
                    if let Some(a) = anchor {
                        self.anchors.insert(a, v.clone());
                    }
                    v
                }
            } else {
                // 值在后续行：更深的缩进 → 子节点；同缩进的 `- ` → 序列
                self.skip_blank();
                if self.i < self.lines.len() && self.lines[self.i].indent > indent {
                    let child = self.lines[self.i].indent;
                    self.parse_node(child)?
                } else if self.i < self.lines.len()
                    && self.lines[self.i].indent == indent
                    && is_seq_item(&self.lines[self.i].text)
                {
                    self.parse_seq(indent)?
                } else {
                    Value::Null
                }
            };
            m.insert(key, val);
        }
        Ok(Value::Object(m))
    }

    /// 块序列。
    fn parse_seq(&mut self, indent: usize) -> Result<Value, String> {
        let mut items = Vec::new();
        loop {
            self.skip_blank();
            if self.i >= self.lines.len() {
                break;
            }
            let line = &self.lines[self.i];
            if line.indent != indent || !is_seq_item(&line.text) {
                break;
            }
            let text = line.text.clone();
            let no = line.no;
            self.i += 1;
            let rest = text[1..].trim_start().to_string();
            if rest.is_empty() {
                // 该项在后续行
                self.skip_blank();
                if self.i < self.lines.len() && self.lines[self.i].indent > indent {
                    let child = self.lines[self.i].indent;
                    items.push(self.parse_node(child)?);
                } else {
                    items.push(Value::Null);
                }
            } else if let Some(kind) = block_scalar_kind(&rest) {
                items.push(self.parse_block_scalar(indent, kind)?);
            } else if is_map_entry(&rest) {
                items.push(self.parse_inline_map(indent, rest, no)?);
            } else {
                let (anchor, body) = split_anchor(&rest);
                let v = self.scalar_or_flow(&body, no)?;
                if let Some(a) = anchor {
                    self.anchors.insert(a, v.clone());
                }
                items.push(v);
            }
        }
        Ok(Value::Array(items))
    }

    /// `- key: value` 形态：首行的键值对 + 后续缩进更深的行同属这一项。
    ///
    /// 做法是把它们拼成一个**临时行集**（首行作为缩进 0 的虚拟行，后续行按
    /// `seq_indent + 2` 重基准），再交给普通的映射解析，避免为内联映射单写一套逻辑。
    fn parse_inline_map(&mut self, seq_indent: usize, first: String, no: usize) -> Result<Value, String> {
        let mut sub = vec![Line {
            indent: 0,
            text: first,
            raw: String::new(),
            no,
        }];
        let base = seq_indent + 2;
        while self.i < self.lines.len() {
            let l = &self.lines[self.i];
            if !l.text.is_empty() && l.indent <= seq_indent {
                break;
            }
            sub.push(Line {
                indent: l.indent.saturating_sub(base),
                text: l.text.clone(),
                raw: l.raw.clone(),
                no: l.no,
            });
            self.i += 1;
        }
        // 去掉尾部空行（后面的项不属于本映射）
        while sub.last().map(|l| l.text.is_empty()).unwrap_or(false) {
            sub.pop();
        }
        let saved_lines = std::mem::replace(&mut self.lines, sub);
        let saved_i = self.i;
        self.i = 0;
        let v = self.parse_map(0);
        self.lines = saved_lines;
        self.i = saved_i;
        v
    }

    /// 块标量 `|` / `>`：内容为后续所有缩进大于当前键的行。
    fn parse_block_scalar(&mut self, key_indent: usize, kind: BlockScalar) -> Result<Value, String> {
        let mut collected: Vec<(usize, String)> = Vec::new();
        let mut min_indent: Option<usize> = None;
        while self.i < self.lines.len() {
            let raw = self.lines[self.i].raw.clone();
            let no = self.lines[self.i].no;
            let stripped = raw.trim_start();
            if stripped.is_empty() {
                collected.push((usize::MAX, String::new())); // 空行属于块内容
                self.i += 1;
                continue;
            }
            let ind = raw.len() - stripped.len();
            if ind <= key_indent {
                break;
            }
            min_indent = Some(min_indent.map_or(ind, |c: usize| c.min(ind)));
            collected.push((ind, stripped.trim_end().to_string()));
            let _ = no;
            self.i += 1;
        }
        let base = min_indent.unwrap_or(key_indent + 2);
        let mut body_lines: Vec<String> = Vec::new();
        for (ind, s) in &collected {
            if *ind == usize::MAX {
                body_lines.push(String::new());
            } else {
                let pad = " ".repeat((*ind).saturating_sub(base));
                body_lines.push(format!("{pad}{s}"));
            }
        }
        // 尾部空行的处理由收尾修饰决定
        let mut trailing = 0usize;
        while body_lines.last().map(|s| s.is_empty()).unwrap_or(false) {
            body_lines.pop();
            trailing += 1;
        }
        let body = join_lines(&body_lines, kind.fold);
        let out = match kind.chomp {
            Chomp::Strip => body,
            Chomp::Clip => {
                if body.is_empty() {
                    String::new()
                } else {
                    format!("{body}\n")
                }
            }
            Chomp::Keep => {
                let mut s = body;
                if !s.is_empty() {
                    s.push('\n');
                }
                for _ in 1..trailing {
                    s.push('\n');
                }
                s
            }
        };
        Ok(Value::Str(out))
    }

    /// 标量或流式结构。
    fn scalar_or_flow(&mut self, t: &str, no: usize) -> Result<Value, String> {
        let t = t.trim();
        if t.is_empty() {
            return Ok(Value::Null);
        }
        if let Some(name) = t.strip_prefix('*') {
            return self
                .anchors
                .get(name.trim())
                .cloned()
                .ok_or_else(|| format!("第 {no} 行：别名 `*{}` 未定义", name.trim()));
        }
        if t.starts_with('[') || t.starts_with('{') {
            let mut f = FlowParser {
                s: t.as_bytes(),
                i: 0,
                anchors: &mut self.anchors,
            };
            let v = f.value(no)?;
            f.skip_ws();
            if f.i < f.s.len() {
                return Err(format!("第 {no} 行：流式结构后有多余字符"));
            }
            return Ok(v);
        }
        Ok(parse_scalar(t))
    }
}

/// 拆出 `&锚点名` 前缀，返回（锚点名, 剩余文本）。
fn split_anchor(s: &str) -> (Option<String>, String) {
    let t = s.trim();
    if let Some(rest) = t.strip_prefix('&') {
        let mut it = rest.splitn(2, char::is_whitespace);
        let name = it.next().unwrap_or("").to_string();
        let body = it.next().unwrap_or("").to_string();
        if !name.is_empty() {
            return (Some(name), body);
        }
    }
    (None, t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv<'a>(v: &'a Value, k: &str) -> &'a Value {
        v.get(k).unwrap_or_else(|| panic!("缺少键 `{k}`：{v:?}"))
    }

    /// `Value` 只提供 `as_str` / `as_float`，数组用模式匹配取。
    fn arr(v: &Value) -> &Vec<Value> {
        match v {
            Value::Array(a) => a,
            other => panic!("期望数组，得 {other:?}"),
        }
    }

    #[test]
    fn simple_mapping_and_scalars() {
        let v = parse("name: John\nage: 27\nratio: 1.5\nok: true\nnone: null\n").unwrap();
        assert_eq!(kv(&v, "name"), &Value::Str("John".into()));
        assert_eq!(kv(&v, "age"), &Value::Int(27));
        assert_eq!(kv(&v, "ratio"), &Value::float(1.5));
        assert_eq!(kv(&v, "ok"), &Value::Bool(true));
        assert_eq!(kv(&v, "none"), &Value::Null);
    }

    #[test]
    fn nested_mapping() {
        let v = parse("a:\n  b: 1\n  c:\n    d: 2\n").unwrap();
        let a = v.get("a").unwrap();
        assert_eq!(kv(a, "b"), &Value::Int(1));
        assert_eq!(kv(kv(a, "c"), "d"), &Value::Int(2));
    }

    #[test]
    fn block_sequence() {
        let v = parse("items:\n  - a\n  - b\n  - c\n").unwrap();
        assert_eq!(arr(v.get("items").unwrap()).len(), 3);
    }

    #[test]
    fn sequence_of_mappings() {
        let src = "servers:\n  - name: a\n    port: 1\n  - name: b\n    port: 2\n";
        let v = parse(src).unwrap();
        let servers = arr(v.get("servers").unwrap());
        assert_eq!(servers.len(), 2);
        assert_eq!(kv(&servers[0], "name"), &Value::Str("a".into()));
        assert_eq!(kv(&servers[0], "port"), &Value::Int(1));
        assert_eq!(kv(&servers[1], "name"), &Value::Str("b".into()));
        assert_eq!(kv(&servers[1], "port"), &Value::Int(2));
    }

    #[test]
    fn k8s_style_document() {
        let src = "\
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web
  labels:
    app: web
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: nginx
          image: nginx:1.25
          ports:
            - containerPort: 80
";
        let v = parse(src).unwrap();
        assert_eq!(kv(&v, "kind"), &Value::Str("Deployment".into()));
        let spec = v.get("spec").unwrap();
        assert_eq!(kv(spec, "replicas"), &Value::Int(3));
        let containers = arr(kv(kv(kv(spec, "template"), "spec"), "containers"));
        assert_eq!(kv(&containers[0], "image"), &Value::Str("nginx:1.25".into()));
    }

    #[test]
    fn flow_collections() {
        let v = parse("a: [1, 2, 3]\nb: {x: 1, y: two}\nc: []\n").unwrap();
        assert_eq!(arr(kv(&v, "a")).len(), 3);
        assert_eq!(kv(kv(&v, "b"), "y"), &Value::Str("two".into()));
        assert_eq!(arr(kv(&v, "c")).len(), 0);
    }

    #[test]
    fn comments_and_hash_in_value() {
        let v = parse("# 头部注释\nurl: http://x#frag  # 行尾注释\nplain: a#b\n").unwrap();
        assert_eq!(kv(&v, "url"), &Value::Str("http://x#frag".into()));
        assert_eq!(kv(&v, "plain"), &Value::Str("a#b".into()));
    }

    #[test]
    fn quoted_strings() {
        let v = parse("a: \"hello\\nworld\"\nb: 'it''s'\n").unwrap();
        assert_eq!(kv(&v, "a"), &Value::Str("hello\nworld".into()));
        assert_eq!(kv(&v, "b"), &Value::Str("it's".into()));
    }

    #[test]
    fn block_scalar_literal_and_folded() {
        let src = "lit: |\n  line1\n  line2\nfold: >\n  a\n  b\nstrip: |-\n  x\n";
        let v = parse(src).unwrap();
        assert_eq!(kv(&v, "lit"), &Value::Str("line1\nline2\n".into()));
        assert_eq!(kv(&v, "fold"), &Value::Str("a b\n".into()));
        assert_eq!(kv(&v, "strip"), &Value::Str("x".into()));
    }

    #[test]
    fn anchors_and_aliases() {
        let src = "base: &b\n  x: 1\nuse: *b\n";
        let v = parse(src).unwrap();
        assert_eq!(kv(kv(&v, "use"), "x"), &Value::Int(1));
    }

    #[test]
    fn doc_marker_is_skipped() {
        let v = parse("---\na: 1\n").unwrap();
        assert_eq!(kv(&v, "a"), &Value::Int(1));
    }

    #[test]
    fn numbers_bases_and_underscores() {
        let v = parse("hex: 0xff\noct: 0o17\nbin: 0b1010\nbig: 1_000\nneg: -3\n").unwrap();
        assert_eq!(kv(&v, "hex"), &Value::Int(255));
        assert_eq!(kv(&v, "oct"), &Value::Int(15));
        assert_eq!(kv(&v, "bin"), &Value::Int(10));
        assert_eq!(kv(&v, "big"), &Value::Int(1000));
        assert_eq!(kv(&v, "neg"), &Value::Int(-3));
    }

    /// 挪威问题：`no` / `yes` 必须是字符串，不能变成布尔。
    #[test]
    fn norway_problem_avoided() {
        let v = parse("country: no\nagreed: yes\non: off\n").unwrap();
        assert_eq!(kv(&v, "country"), &Value::Str("no".into()));
        assert_eq!(kv(&v, "agreed"), &Value::Str("yes".into()));
        assert_eq!(kv(&v, "on"), &Value::Str("off".into()));
    }

    #[test]
    fn type_discriminator_like_value_stays_string() {
        let v = parse("image: nginx:1.25\nnodePort: 30080\n").unwrap();
        assert_eq!(kv(&v, "image"), &Value::Str("nginx:1.25".into()));
    }

    #[test]
    fn undefined_alias_errors() {
        assert!(parse("a: *nope\n").is_err());
    }

    #[test]
    fn empty_document_is_empty_object() {
        assert_eq!(parse("").unwrap(), Value::Object(BTreeMap::new()));
        assert_eq!(parse("# 只有注释\n").unwrap(), Value::Object(BTreeMap::new()));
    }
}
