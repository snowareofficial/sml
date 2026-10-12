// SPDX-License-Identifier: MulanPSL-2.0
//! SML 词法分析：把文本切成 [`Tok`] 序列，并提供裸词 → 值的强制转换。
//!
//! 只依赖值模型与特性集（裸词是否可当字符串由 `Feature::BarewordStr` 决定），
//! 不涉及语法结构，便于单独测试与复用。

use std::collections::BTreeMap;

use sml_codes::{
    E_FEATURE_001, E_FEATURE_002, E_FEATURE_005, E_INCLUDE_006, E_LEX_001, E_LEX_002, E_LEX_003,
    E_LEX_004, E_LEX_005, E_LEX_006, SmlError,
};
use sml_feature::{Feature, FeatureSet};
use sml_value::Value;

// ---------------------------------------------------------------------------
// 解析: 词法 + 递归下降
// ---------------------------------------------------------------------------

/// 词法单元。`pub` 仅为让 `cond` 等子模块复用解析器游标，
/// 不对外暴露（核心模块本身是私有的）。
#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    LBrace,  // {
    RBrace,  // }
    LBrack,  // [
    RBrack,  // ]
    Comma,   // ,
    Colon,   // :
    At,      // @（紧邻其后内容，如 `@name` / `@contract`）
    /// 孤立的 `@`：其后紧跟空白或行尾，没有片段名/指令名。
    ///
    /// 必须与 `At` 区分——若统一成 `At`，孤立 `@` 会把**其后紧跟的块**
    /// 当成片段体消费掉（`@` + `mailer { .. }` 与 `@mailer { .. }`
    /// 的 token 流完全相同），导致内容被静默丢弃且不报错。
    BareAt,
    Str(String),   // 引号串 (已解码)
    Word(String),  // 裸词
}

/// 词法单元在原文中的区间（**字节**偏移，左闭右开）。
///
/// 为什么单独存、而不塞进 `Tok`：`Tok` 是解析器与 `cond` 共用的形状定义，
/// 给它加字段会把改动扩散到每一处 match。旁路表让「要不要位置」成为调用方的
/// 选择 —— 不关心位置的路径（[`tokenize`]）零成本、行为逐字不变。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

/// 带位置记录的字符游标：每消费一个字符就累加它的 UTF-8 字节数。
///
/// 这样扫描逻辑里的 `next()` / `peek()` / `by_ref()` **一处都不用改**，
/// 位置自动跟着走 —— 手写「每个消费点都 +len」迟早漏一处，漏了就是静默错位。
struct PosChars<'a> {
    it: std::str::Chars<'a>,
    pos: usize,
    peeked: Option<char>,
}

impl<'a> PosChars<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            it: s.chars(),
            pos: 0,
            peeked: None,
        }
    }
    fn bump(&mut self) -> Option<char> {
        let c = match self.peeked.take() {
            Some(c) => c,
            None => self.it.next()?,
        };
        self.pos += c.len_utf8();
        Some(c)
    }
    fn peek(&mut self) -> Option<&char> {
        if self.peeked.is_none() {
            self.peeked = self.it.next();
        }
        self.peeked.as_ref()
    }
    /// 已消费的字节数（即下一个字符的起始偏移）。
    fn pos(&self) -> usize {
        self.pos
    }
    fn next(&mut self) -> Option<char> {
        self.bump()
    }
}

impl<'a> Iterator for PosChars<'a> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
        self.bump()
    }
}

/// 切词，**并**返回每个 token 在原文中的字节区间。
///
/// 与 [`tokenize`] 是同一个扫描器、同一套判据，只是多记一份旁路区间；
/// 需要错误定位的入口（解析器 / CLI / 编辑器）用它。
pub fn tokenize_spans(text: &str) -> Result<(Vec<Tok>, Vec<Span>), SmlError> {
    let mut toks = Vec::new();
    let mut spans = Vec::new();
    let mut chars = PosChars::new(text);
    let mut buf = String::new();
    // 闭包只按参数操作，不捕获可变状态，故无需 mut。
    //
    // `end` 由调用方给：裸词的字符是**连续**消费的（注释/定界符都会先把 buf 冲掉），
    // 故起点 = `end - buf 的字节长度`，不必额外维护「词起始」状态。
    let flush = |buf: &mut String, toks: &mut Vec<Tok>, spans: &mut Vec<Span>, end: usize| {
        if !buf.is_empty() {
            let len: usize = buf.chars().map(|c| c.len_utf8()).sum();
            let start = end.saturating_sub(len);
            toks.push(Tok::Word(std::mem::take(buf)));
            spans.push(Span { start, end });
        }
    };
    while let Some(c) = chars.next() {
        // 本字符的起始偏移：`next()` 之后 pos 已越过它，故回退它的字节长度
        let c_start = chars.pos() - c.len_utf8();
        match c {
            '#' => {
                // 单行注释到行尾
                for c2 in chars.by_ref() {
                    if c2 == '\n' {
                        break;
                    }
                }
            }
            '-' => {
                // `--` 单行注释到行尾；否则作为普通字符
                if chars.peek() == Some(&'-') {
                    chars.next(); // 吃掉第二个 -
                    for c2 in chars.by_ref() {
                        if c2 == '\n' {
                            break;
                        }
                    }
                } else {
                    buf.push(c);
                }
            }
            '/' => {
                match chars.peek() {
                    // `//` 单行注释到行尾
                    Some('/') => {
                        chars.next(); // 吃掉第二个 /
                        for c2 in chars.by_ref() {
                            if c2 == '\n' {
                                break;
                            }
                        }
                    }
                    // `/*` 多行注释，直到 `*/`；EOF 未闭合则报错（与未闭合字符串一致）
                    Some('*') => {
                        chars.next(); // 吃掉 *
                        loop {
                            match chars.next() {
                                Some('*') => {
                                    if chars.peek() == Some(&'/') {
                                        chars.next();
                                        break;
                                    }
                                }
                                Some(_) => {}
                                None => {
                                    return Err(SmlError::new(
                                        E_LEX_002,
                                        "sml: 未闭合的块注释 /* ... */（遇到文件结尾）",
                                    ))
                                }
                            }
                        }
                    }
                    // 否则作为普通字符（如路径 a/b/c）
                    _ => buf.push(c),
                }
            }
            '_' => {
                // `_*` 多行注释，直到 `*_`；否则作为普通字符
                if chars.peek() == Some(&'*') {
                    chars.next(); // 吃掉 *
                    loop {
                        match chars.next() {
                            Some('*') => {
                                if chars.peek() == Some(&'_') {
                                    chars.next();
                                    break;
                                }
                            }
                            Some(_) => {}
                            None => {
                                return Err(SmlError::new(
                                    E_LEX_003,
                                    "sml: 未闭合的块注释 _* ... *_（遇到文件结尾）",
                                ))
                            }
                        }
                    }
                } else {
                    buf.push(c);
                }
            }
            '"' => {
                flush(&mut buf, &mut toks, &mut spans, c_start);
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => {
                            // 转义：\n \t \r \0 \" \\ \u{XXXX} \uXXXX
                            match chars.next() {
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some('r') => s.push('\r'),
                                Some('0') => s.push('\0'),
                                Some('"') => s.push('"'),
                                Some('\\') => s.push('\\'),
                                Some('u') => {
                                    let mut hex = String::new();
                                    // 支持 \u{XXXX} 或 \uXXXX
                                    if chars.peek() == Some(&'{') {
                                        chars.next();
                                        for c2 in chars.by_ref() {
                                            if c2 == '}' {
                                                break;
                                            }
                                            hex.push(c2);
                                        }
                                    } else {
                                        for _ in 0..4 {
                                            if let Some(c2) = chars.next() {
                                                hex.push(c2);
                                            } else {
                                                // B8：\uXXXX 定长读 4 字符，
                                                // 不足 4 位说明输入截断/非法，
                                                // 必须把已读的 hex 当作失败处理
                                                // 而非静默丢弃（否则会吃掉后续引号）。
                                                return Err(SmlError::new(
                                                    E_LEX_005,
                                                    format!(
                                                        "sml: 字符串转义 \\u 缺少足够的十六进制数字（期望 4 位，得 {hex:?}）"
                                                    ),
                                                ));
                                            }
                                        }
                                    }
                                    // B8：非法码点（如代理区 \uD800、空 hex、非 hex）
                                    // 必须报错，不能静默丢弃并吞掉闭合引号。
                                    if hex.is_empty() {
                                        return Err(SmlError::new(
                                            E_LEX_005,
                                            "sml: 字符串转义 \\u 后缺少十六进制数字",
                                        ));
                                    }
                                    let cp = u32::from_str_radix(&hex, 16).map_err(|_| {
                                        SmlError::new(
                                            E_LEX_005,
                                            format!("sml: 字符串转义 \\u 含非十六进制数字：{hex:?}"),
                                        )
                                    })?;
                                    let ch = char::from_u32(cp).ok_or_else(|| {
                                        SmlError::new(
                                            E_LEX_005,
                                            format!(
                                                "sml: 字符串转义 \\u 得到非法 Unicode 码点：U+{cp:04X}"
                                            ),
                                        )
                                    })?;
                                    s.push(ch);
                                }
                                Some(other) => {
                                    // 未知转义（非 n/t/r/0/"/\/u）：必须报错，而非静默丢弃
                                    // 反斜杠（P1-5：\U \d \z 等会让路径/正则静默损坏）。
                                    // 与 \u 系列一致的严格策略：非法转义即失败。
                                    return Err(SmlError::new(
                                        E_LEX_004,
                                        format!(
                                            "sml: 字符串含未知转义符 \\{}（仅支持 \\n \\t \\r \\0 \\\" \\\\ \\uXXXX）",
                                            other
                                        ),
                                    ));
                                }
                                // B9：转义符后遇 EOF，未闭合的反斜杠报错
                                None => {
                                    return Err(SmlError::new(
                                        E_LEX_006,
                                        "sml: 字符串中的转义符 \\ 后遇到文件结束",
                                    ))
                                }
                            }
                        }
                        Some(other) => s.push(other),
                        // B9：未闭合字符串（EOF 前没有闭合引号）必须报错，
                        // 否则后续整行/整个文件会被静默吞并。
                        None => {
                            return Err(SmlError::new(
                                E_LEX_001,
                                "sml: 字符串未闭合（缺少结束引号 \"）",
                            ))
                        }
                    }
                }
                // 字符串区间含首尾引号（左闭右开）
                spans.push(Span { start: c_start, end: chars.pos() });
                toks.push(Tok::Str(s));
            }
            '{' => {
                flush(&mut buf, &mut toks, &mut spans, c_start);
                toks.push(Tok::LBrace);
                spans.push(Span { start: c_start, end: chars.pos() });
            }
            '}' => {
                flush(&mut buf, &mut toks, &mut spans, c_start);
                toks.push(Tok::RBrace);
                spans.push(Span { start: c_start, end: chars.pos() });
            }
            '[' => {
                flush(&mut buf, &mut toks, &mut spans, c_start);
                toks.push(Tok::LBrack);
                spans.push(Span { start: c_start, end: chars.pos() });
            }
            ']' => {
                flush(&mut buf, &mut toks, &mut spans, c_start);
                toks.push(Tok::RBrack);
                spans.push(Span { start: c_start, end: chars.pos() });
            }
            ',' => {
                flush(&mut buf, &mut toks, &mut spans, c_start);
                toks.push(Tok::Comma);
                spans.push(Span { start: c_start, end: chars.pos() });
            }
            ':' => {
                flush(&mut buf, &mut toks, &mut spans, c_start);
                toks.push(Tok::Colon);
                spans.push(Span { start: c_start, end: chars.pos() });
            }
            '@' => {
                // `@` 仅当位于**词首**时才是片段定义标记（`@base { ... }`）。
                // 出现在词中间时（典型如邮箱 `a@b.c`）必须作为普通字符保留：
                // 否则 `a@b.c` 会被切成 `Word("a")` + `At` + `Word("b.c")`，
                // 后半段在解析时被丢弃，导致邮箱静默损坏为 `a`。
                if buf.is_empty() {
                    // 仅当 `@` 与后随内容紧邻（无空白）时才是片段/指令标记。
                    // 孤立 `@`（后接空白或行尾）单独记为 BareAt，交由解析器报错，
                    // 避免其后的块被误认作片段体而静默丢弃。
                    let adjacent = match chars.peek() {
                        None => false,
                        Some(c) => !c.is_whitespace(),
                    };
                    toks.push(if adjacent { Tok::At } else { Tok::BareAt });
                    spans.push(Span { start: c_start, end: chars.pos() });
                } else {
                    buf.push(c);
                }
            }
            ' ' | '\t' | '\n' | '\r' => {
                // 空白本身不是 token：词在此处结束，故 end 取空白的起始偏移
                flush(&mut buf, &mut toks, &mut spans, c_start);
            }
            _ => {
                buf.push(c);
            }
        }
    }
    flush(&mut buf, &mut toks, &mut spans, chars.pos());
    Ok((toks, spans))
}

/// 切词（不带位置）。
///
/// 行为与加入 spans 之前**逐字一致**：内部就是 [`tokenize_spans`] 丢掉区间。
/// 需要错误定位的入口改用 [`tokenize_spans`]。
pub fn tokenize(text: &str) -> Result<Vec<Tok>, SmlError> {
    tokenize_spans(text).map(|(t, _)| t)
}

/// 查环境变量：优先查本次解析的**覆盖表**（`env_overrides`），未命中才回落
/// 到真实进程环境。覆盖表仅作用于本次解析，不修改进程环境（避免线程不安全的
/// `std::env::set_var` 与跨解析污染）。
pub fn lookup_env(overrides: Option<&BTreeMap<String, String>>, name: &str) -> String {
    if let Some(m) = overrides {
        if let Some(v) = m.get(name) {
            return v.clone();
        }
    }
    std::env::var(name).unwrap_or_default()
}

/// 把裸词 `w` 转为 Value。
///
/// 受 `features` 控制：关闭 `BarewordStr` 后纯字符串裸词（如 `John`）被拒绝，
/// 必须写作 `"John"`；仍允许的非字符串裸词：bool / null / 数字 /
/// 片段引用 `&x`（需 `fragment`）/ 环境变量 `$env.X`（需 `env`）。
///
/// `env` 为本次解析的环境变量覆盖表（见 [`lookup_env`]）。
pub fn coerce_word(
    w: &str,
    fragments: &BTreeMap<String, Value>,
    features: FeatureSet,
    ns_prefix: &str,
    env: Option<&BTreeMap<String, String>>,
) -> Result<Value, SmlError> {
    match w {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        "null" => return Ok(Value::Null),
        _ => {}
    }
    // $env.VAR 内联（需 env 特性）
    if let Some(ev) = w.strip_prefix("$env.") {
        if !features.has(Feature::Env) {
            return Err(SmlError::new(
                E_FEATURE_002,
                format!("sml: 当前特性集禁用了 `$env`（env），裸词 `{}` 无法解析", w),
            ));
        }
        return Ok(Value::Str(lookup_env(env, ev)));
    }
    // 片段引用 &name（需 fragment 特性）。命名空间隔离：先查裸名，再逐级查 ns 前缀。
    if let Some(name) = w.strip_prefix('&') {
        if !features.has(Feature::Fragment) {
            return Err(SmlError::new(
                E_FEATURE_001,
                format!("sml: 当前特性集禁用了片段引用（fragment），`{}` 无法解析", w),
            ));
        }
        if let Some(v) = fragments.get(name) {
            return Ok(v.clone());
        }
        // 逐级回退：ui.form.foo → form.foo → foo
        if !ns_prefix.is_empty() {
            let mut probe = ns_prefix.to_string();
            loop {
                let full = format!("{probe}.{name}");
                if let Some(v) = fragments.get(&full) {
                    return Ok(v.clone());
                }
                match probe.rfind('.') {
                    Some(idx) => probe.truncate(idx),
                    None => break,
                }
            }
        }
        // 片段特性已开启但名字未定义：必须报错，不能静默降级为字符串
        // （否则拼错的片段名会得到 Str("&name")，下游 .get 取到 None，难以排查）。
        return Err(SmlError::new(
            E_INCLUDE_006,
            format!("sml: 未定义的片段引用 `{}`", w),
        ));
    }
    // 数字: int / float / 科学计数
    if let Ok(i) = w.parse::<i64>() {
        //预先摘除符号
        let digits = w.strip_prefix(['+','-']).unwrap_or(w);
        // 保留前导零语义：以 `0` 开头且非 0x/0b 的纯数字（如 0755、007）必须保留为
        // 字符串，否则权限位/编号会被静默改写（P1-4：mode: 0755 不应变成 755）。
        // 0xFF / 0b101 等显式进制前缀不在其列，按原逻辑（非 i64 十进制）走下方字符串。
        if digits.starts_with('0') && digits.len() > 1 && !w.starts_with("0x") && !w.starts_with("0b") {
            if digits.chars().all(|c| c.is_ascii_digit()) {
                return Ok(Value::Str(w.to_string()));
            }
        }
        return Ok(Value::Int(i));
    }
    // B10：整数超 i64 范围时，不能静默降级为 Float（会丢精度，
    // 如 9223372036854775808 这类 uint64 上界 ID / 纳秒时间戳）。
    // 若为纯整数形态则保留为字符串（round-trip 安全、零精度损失）；
    // 带小数点/科学计数符的才走 f64。
    // 负号开头的数也需要处理可能的负溢出
    let looks_int = !w.contains(['.', 'e', 'E']) && w.chars().all(|c| c.is_ascii_digit() || c == '+' || c == '-');
    if looks_int {
        // 处理大整数情况
        return Ok(Value::Str(w.to_string()));
    }
    // 只有「首字符为数字或小数点」的词才承认是数字字面量。
    //
    // Rust 的 f64 解析器接受 `inf` / `infinity` / `nan` 且大小写不敏感，
    // 若不设此闸，`status: inf`、`ratio: nan` 这类裸词会被静默解析成浮点数
    // （与 YAML 1.1 把 `NO` 识别成 false 是同一类问题）。
    //
    // 此处刻意**不**在分支内 return 字符串，而是跳过 f64 解析继续下走：
    // 这样 v2/v3 严格模式（Feature::BarewordStr 关闭）仍会对这些裸词报错，
    // 而不是静默降级为字符串。
    let numeric_head = w
        .trim_start_matches(['+', '-'])
        .chars()
        .next()
        .map(|c| c.is_ascii_digit() || c == '.')
        .unwrap_or(false);

    if numeric_head {
        if let Ok(f) = w.parse::<f64>() {
            // 记录原始字面量，使 dump 能还原书写形式（P3：1e10 不再变成
            // 10000000000.0）。这是 raw 的**唯一生产点** —— 其余所有构造
            // 路径一律 raw = None，保证「raw 只在解析后未改动时有效」。
            return Ok(Value::float_with_raw(f, w));
        }
    }
    if !features.has(Feature::BarewordStr) {
        return Err(SmlError::new(
            E_FEATURE_005,
            format!(
                "sml: 字符串必须加引号，裸词 `{}` 应写作 `\"{}\"`（特性 bareword-string 已禁用）",
                w, w
            ),
        ));
    }
    Ok(Value::Str(w.to_string()))
}


// --- 字符串跨度工具 -------------------------------------------------------
// 下沉自 `sml-parse::scan`：include 展开需要判断某位置是否在字符串内，
// 而 sml-parse 已依赖 sml-include，留在 scan 会成环。

/// 计算文本中所有字符串字面量的字节区间（含单引号/双引号、单行与多行三引号）。
/// 用于让「按行剥离指令」在字符串内部时跳过，避免破坏多行字符串数据
/// （如 `note: "line1\n@version v1\nline2"` 中的 @version 被误当指令）。
pub fn compute_string_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' || c == b'\'' {
            let quote = c;
            // 三引号（多行）？
            let triple = i + 2 < bytes.len() && bytes[i + 1] == quote && bytes[i + 2] == quote;
            let end = if triple {
                // 找到下一个 """" / ''' 作为结束
                let mut j = i + 3;
                let mut found = None;
                while j + 2 < bytes.len() {
                    if bytes[j] == quote && bytes[j + 1] == quote && bytes[j + 2] == quote {
                        found = Some(j + 3);
                        break;
                    }
                    j += 1;
                }
                found.unwrap_or(bytes.len())
            } else {
                // 单行：遇到未转义的同类引号结束
                let mut j = i + 1;
                let mut found = None;
                while j < bytes.len() {
                    if bytes[j] == b'\\' {
                        j += 2;
                        continue;
                    }
                    if bytes[j] == quote {
                        found = Some(j + 1);
                        break;
                    }
                    j += 1;
                }
                found.unwrap_or(bytes.len())
            };
            spans.push((i, end));
            i = end;
        } else {
            i += 1;
        }
    }
    spans
}

/// 判断 `line` 起始字节 `start` 是否落在任一字符串区间内（字符串内的行不算指令）。
pub fn line_starts_in_string(_text: &str, start: usize, spans: &[(usize, usize)]) -> bool {
    // 指令通常位于行首（可有缩进）。检测起始位置是否在字符串内即可。
    spans.iter().any(|(s, e)| start >= *s && start < *e)
}

/// 计算文本中所有**块注释**（`/* … */` 与 `_* … *_`）的字节区间。
///
/// 与 [`compute_string_spans`] 并列存在，理由也一样：**跨行**的块注释会让"逐行"处理失明。
/// `sml-include` 现在按"跨行单元合并后的段"来词法（见 `expand.rs` 的 `segments`），
/// 它必须知道某一行是否落在块注释里 —— 否则注释里写的一行 `include "x.sml"` 会被**真的展开**，
/// 既错又危险（等同字符串里伪造 include 的那条任意文件读取路径）。
///
/// 与词法器保持一致：块注释**不嵌套**（`/*` 到最近一个 `*/`），且
/// 行注释（`#` / `//` / `--` 到行尾）与字符串内部的 `/*` 都**不**开启块注释。
pub fn compute_block_comment_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let strings = compute_string_spans(text);
    let in_string = |i: usize| strings.iter().any(|(s, e)| i >= *s && i < *e);
    let mut spans = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if in_string(i) {
            i += 1;
            continue;
        }
        let b = bytes[i];
        let skip_to_eol = b == b'#'
            || (b == b'/' && bytes.get(i + 1) == Some(&b'/'))
            || (b == b'-' && bytes.get(i + 1) == Some(&b'-'));
        if skip_to_eol {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // `/* … */` 与 `_* … *_`：这两对都是块注释，按同样方式找最近的收尾。
        let closer = match (b, bytes.get(i + 1)) {
            (b'/', Some(b'*')) => Some((b'*', b'/')),
            (b'_', Some(b'*')) => Some((b'*', b'_')),
            _ => None,
        };
        if let Some((c1, c2)) = closer {
            let mut j = i + 2;
            while j + 1 < bytes.len() && !(bytes[j] == c1 && bytes[j + 1] == c2) {
                j += 1;
            }
            let end = if j + 1 < bytes.len() { j + 2 } else { bytes.len() };
            spans.push((i, end));
            i = end;
            continue;
        }
        i += 1;
    }
    spans
}

/// 推进到下一行的起始字节偏移（处理 \n；CRLF 也兼容）。
pub fn advance_line(mut start: usize, line: &str, text: &str) -> usize {
    start += line.len();
    if text[start..].starts_with('\n') {
        start += 1;
        if text[start..].starts_with('\r') {
            start += 1;
        }
    } else if text[start..].starts_with('\r') {
        start += 1;
        if text[start..].starts_with('\n') {
            start += 1;
        }
    }
    start
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `tokenize` 与 `tokenize_spans` 必须**逐字一致** —— 这是「加位置不改行为」的钉子：
    /// 任一端漂移，这里就红。
    #[test]
    fn tokenize_matches_tokenize_spans() {
        let cases = [
            "a: 1",
            "k: 中",
            r#"s: "ab""#,
            "# 注释\na: 1\n",
            "m: [ 1, 2 ]\n",
            "@base { x: 1 }\ny: &base\n",
            "a@b.c: 1\n",
            "/* c */ k: 1\n",
            "",
            "42\n",
        ];
        for src in cases {
            let a = tokenize(src).unwrap();
            let (b, spans) = tokenize_spans(src).unwrap();
            assert_eq!(a, b, "输入 {src:?}：两个入口的结果不一致");
            assert_eq!(
                b.len(),
                spans.len(),
                "输入 {src:?}：spans 与 toks 数量不相等"
            );
        }
    }

    /// 区间是**字节**偏移，且指向原文中该 token 的真实位置。
    #[test]
    fn spans_point_at_source_bytes() {
        let (toks, spans) = tokenize_spans("a: 1").unwrap();
        assert_eq!(
            toks,
            vec![
                Tok::Word("a".into()),
                Tok::Colon,
                Tok::Word("1".into())
            ]
        );
        assert_eq!(
            spans,
            vec![
                Span { start: 0, end: 1 },
                Span { start: 1, end: 2 },
                Span { start: 3, end: 4 },
            ]
        );
    }

    /// 多字节字符按 UTF-8 字节计（`中` 占 3 字节）—— 位置一旦按 char 计就会错位。
    #[test]
    fn spans_are_byte_offsets_not_char_offsets() {
        let src = "k: 中";
        let (toks, spans) = tokenize_spans(src).unwrap();
        assert_eq!(toks.len(), 3);
        assert_eq!(spans[2], Span { start: 3, end: 6 });
        assert_eq!(&src[spans[2].start..spans[2].end], "中");
    }

    /// 字符串区间含首尾引号。
    #[test]
    fn string_span_includes_quotes() {
        let src = r#"s: "ab""#;
        let (toks, spans) = tokenize_spans(src).unwrap();
        assert_eq!(toks[2], Tok::Str("ab".into()));
        assert_eq!(spans[2], Span { start: 3, end: 7 });
    }

    /// 注释不产生 token，其后的键位置仍要正确。
    #[test]
    fn key_after_comment_is_located() {
        let src = "# 注释\na: 1\n";
        let (toks, spans) = tokenize_spans(src).unwrap();
        assert_eq!(toks[0], Tok::Word("a".into()));
        assert_eq!(&src[spans[0].start..spans[0].end], "a");
    }
}
