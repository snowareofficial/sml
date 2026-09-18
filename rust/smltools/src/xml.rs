// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0
//! XML → `Value` 的解析器（供 `smltools --from xml` 迁移用）。
//!
//! 目标是把存量 XML（配置、描述文件、SVG、CMSIS-SVD 等）整篇搬进 SML，所以映射
//! 规则刻意保持**无损、可预测**，不与 SML 的块语法做语义联想：
//!
//! - 根元素 → 顶层对象的一个键，值为对象（`<device>…</device>` → `{ device: { … } }`）
//! - 子元素 → 键；**同名兄弟合并为数组**（按文档顺序）
//! - **纯文本元素折叠为字符串**：`<name>PWR</name>` → `name: PWR`（不套 `_text`）
//! - 元素属性 → 该元素的 `_attrs` 对象；元素**同时**有属性/子元素时，文本进 `_text`
//!   （`trim` 后非空才写）
//! - 空元素（`<x/>`、`<x></x>`、只有空白）→ `{}`
//! - **叶子值一律字符串**：XML 没有类型，`<size>0x20</size>` 得字符串 `"0x20"` 而不是
//!   整数，`true` 也不会变布尔（`to_sml` 会给「会被误读成数字/布尔」的文本自动加引号）
//!
//! 忽略 `<?xml …?>`（及其它处理指令）、`<!-- … -->`、`<!DOCTYPE …>`（含内部子集）；
//! CDATA 内容原样进 `_text`。实体支持 `&lt; &gt; &amp; &quot; &apos;` 与
//! `&#nn;` / `&#xnn;`，其余**报错**而非猜。命名空间前缀**保留**
//! （`ns:tag`、`xmlns:xs`、`xs:noNamespaceSchemaLocation` 原样作键名）。
//! 解析前按 XML 1.0 §2.11 做**行尾归一**（`\r\n` / `\r` → `\n`）—— 规范如此要求，
//! 同时也保证了 `xml→sml→json` 往返闭合（成因见 `parse()` 内注释）。
//!
//! ## 与「超大 XML」有关的三个硬约束
//!
//! 1. **深度上限**：递归下降在超深嵌套会爆栈（Rust 栈溢出是 abort，`catch_unwind`
//!    也接不住），故嵌套超过 [`MAX_DEPTH`] 立即返回 `Err`，绝不崩溃。
//! 2. **单趟扫描**：游标只前进不回退，不在大字符串上反复 `find`/拼接，整体 O(n)；
//!    实体解码**仅在遇到 `&` 时**才走解码路径，无 `&` 的文本整段 `push_str`。
//! 3. **报错能定位**：XML 常整篇一行，只报行号会退化成「第 1 行」，故错误同时给出
//!    **字符偏移**：`第 N 行（字符偏移 M）：说明`。
//!
//! ## 已知取舍
//!
//! - `_attrs` / `_text` 的包装让 `Value` 树显著大于源文件（每个非空元素至少多两个
//!   键名 + 一次 `BTreeMap`）。超大文件先跑小样本看内存与耗时；产物侧可用 `--strip`
//!   瘦身。
//! - DTD **内部子集**里自定义的实体不解析（直接跳过 DOCTYPE），用到时按未知实体报错。
//! - 混合内容（`<a>文本<b/>文本</a>`）的多段文本按出现顺序**直接拼接**后 `trim`。
//! - 源文件里若真有名为 `_attrs` / `_text` 的子元素，会与保留键同名并按「同名兄弟
//!   合并」规则并成数组 —— 可预测，且不静默覆盖。

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use sml::Value;
use sml_codes::{
    SmlError, E_LIMIT_001, E_MIGRATE_001, E_MIGRATE_002, E_MIGRATE_003, E_MIGRATE_004,
    E_MIGRATE_005, E_MIGRATE_006, E_MIGRATE_007, E_MIGRATE_008, E_MIGRATE_009, E_MIGRATE_010,
};

/// 值嵌套深度上限。
///
/// 数值与 `sml_value::MAX_VALUE_DEPTH`（解析侧）及 `sml::emit::MAX_VALUE_DEPTH`
/// （输出侧）保持一致（128）。三处刻意各留本地常量而不互相引用：`smltools` 只依赖
/// `swsml`，为一个整数把 `sml-value` 拉成直接依赖并不划算；上游两处也是这么做的。
const MAX_DEPTH: usize = 128;

/// 从 XML 文本解析出 `Value`。顶层是「根元素名 → 元素对象」的单键对象。
///
/// 空文档（只有空白 / 注释 / 声明）返回空对象 —— SML 顶层须为容器。
/// 一切错误都是带码的 [`SmlError`]（`第 N 行（字符偏移 M）：说明 [码]`），本模块不 panic。
pub fn parse(text: &str) -> Result<Value, SmlError> {
    // BOM 不是 XML 内容，但在真实文件里很常见（Windows 记事本另存为）
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);

    // XML 1.0 §2.11 行尾处理：解析前须把 `\r\n` 与单独的 `\r` 一律归一为 `\n`。
    //
    // 这不只是「按规范办事」：`sml-parse` 的指令预扫（scan.rs）用 `str::lines()`
    // 按行重建整篇文本，而 `str::lines()` 会吃掉 CRLF 里的 `\r`，导致**字符串值里的
    // CR 也会丢**。实测 CRLF 源走 `--to sml` 再读回来，`\r\n` 变成 `\n`，
    // `xml→sml→json` 与 `xml→json` 差 485 处 / 970 字节。在这里按 XML 规范归一，
    // 产出的值就不含 CR，往返随之闭合（`--to sml` 的输出在 git diff 里也稳定）。
    let normalized;
    let text = if text.contains('\r') {
        normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        normalized.as_str()
    } else {
        text
    };

    let mut p = Parser {
        text,
        b: text.as_bytes(),
        pos: 0,
    };

    let mut roots: Vec<(String, Value)> = Vec::new();
    loop {
        p.skip_misc()?;
        if p.pos >= p.b.len() {
            break;
        }
        if p.b[p.pos] != b'<' {
            // E-MIGRATE-001：迁入文档在顶层出现了文本内容（只允许空白）。
            return Err(p.err(E_MIGRATE_001, p.pos, "顶层出现文本内容（XML 顶层只允许空白）"));
        }
        roots.push(p.element(0)?);
    }
    Ok(merge_siblings(roots))
}

/// 游标式单趟扫描器。
struct Parser<'a> {
    text: &'a str,
    b: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    // ---- 定位与报错 ----------------------------------------------------------

    /// 把字节位置换算成 `(行号, 字符偏移)`。
    ///
    /// 只在**出错时**调用，故这里允许 O(n) 扫一遍前缀：正常路径零开销，
    /// 而报错路径本来就只走一次。
    fn locate(&self, at: usize) -> (usize, usize) {
        let mut at = at.min(self.text.len());
        while at > 0 && !self.text.is_char_boundary(at) {
            at -= 1;
        }
        let mut line = 1usize;
        let mut chars = 0usize;
        for (i, c) in self.text.char_indices() {
            if i >= at {
                break;
            }
            chars += 1;
            if c == '\n' {
                line += 1;
            }
        }
        (line, chars)
    }

    /// 构造一条带码错误：`第 N 行（字符偏移 M）：说明 [码]`。
    fn err(&self, code: &'static str, at: usize, msg: impl AsRef<str>) -> SmlError {
        let (line, chars) = self.locate(at);
        SmlError::new(
            code,
            format!("第 {line} 行（字符偏移 {chars}）：{}", msg.as_ref()),
        )
    }

    // ---- 基础扫描 ------------------------------------------------------------

    fn skip_ws(&mut self) {
        while self.pos < self.b.len() && is_ws(self.b[self.pos]) {
            self.pos += 1;
        }
    }

    fn starts_with(&self, s: &str) -> bool {
        let n = s.len();
        self.pos + n <= self.b.len() && &self.b[self.pos..self.pos + n] == s.as_bytes()
    }

    fn starts_with_ci(&self, s: &str) -> bool {
        let n = s.len();
        self.pos + n <= self.b.len()
            && self.b[self.pos..self.pos + n].eq_ignore_ascii_case(s.as_bytes())
    }

    /// 跳过空白与三类「非元素」构造：注释、处理指令、DOCTYPE。
    fn skip_misc(&mut self) -> Result<(), SmlError> {
        loop {
            self.skip_ws();
            if self.starts_with("<!--") {
                self.skip_comment()?;
            } else if self.starts_with("<?") {
                self.skip_pi()?;
            } else if self.starts_with_ci("<!DOCTYPE") {
                self.skip_doctype()?;
            } else {
                return Ok(());
            }
        }
    }

    fn skip_comment(&mut self) -> Result<(), SmlError> {
        let start = self.pos;
        self.pos += 4; // "<!--"
        match self.text[self.pos..].find("-->") {
            Some(i) => {
                self.pos += i + 3;
                Ok(())
            }
            // E-MIGRATE-007：注释 / 处理指令 / DOCTYPE / CDATA 未闭合共用此码。
            None => Err(self.err(E_MIGRATE_007, start, "注释未闭合（缺少 `-->`）")),
        }
    }

    fn skip_pi(&mut self) -> Result<(), SmlError> {
        let start = self.pos;
        self.pos += 2; // "<?"
        match self.text[self.pos..].find("?>") {
            Some(i) => {
                self.pos += i + 2;
                Ok(())
            }
            None => Err(self.err(E_MIGRATE_007, start, "处理指令未闭合（缺少 `?>`）")),
        }
    }

    /// 跳过 `<!DOCTYPE …>`，含 `[ … ]` 内部子集（子集里的 `>` 不结束声明）。
    fn skip_doctype(&mut self) -> Result<(), SmlError> {
        let start = self.pos;
        self.pos += 9; // "<!DOCTYPE"
        let mut in_subset = false;
        let mut quote: Option<u8> = None;
        while self.pos < self.b.len() {
            let c = self.b[self.pos];
            match quote {
                Some(q) => {
                    if c == q {
                        quote = None;
                    }
                    self.pos += 1;
                }
                None => match c {
                    b'"' | b'\'' => {
                        quote = Some(c);
                        self.pos += 1;
                    }
                    b'[' => {
                        in_subset = true;
                        self.pos += 1;
                    }
                    b']' => {
                        in_subset = false;
                        self.pos += 1;
                    }
                    b'>' if !in_subset => {
                        self.pos += 1;
                        return Ok(());
                    }
                    _ => self.pos += 1,
                },
            }
        }
        Err(self.err(E_MIGRATE_007, start, "`<!DOCTYPE` 未闭合（缺少 `>`）"))
    }

    /// 解码一个实体（游标停在 `&`）。未知实体按错误处理，不静默吞掉。
    fn decode_entity(&mut self, out: &mut String) -> Result<(), SmlError> {
        let start = self.pos;
        // 实体名极短（最长也就 `&#x10FFFF;` = 10 字符），扫 16 个**字符**即可判定
        // 「没有 `;`」，同时避免在 400KB 单行上整篇 find。
        //
        // ⚠️ 这里必须按**字符**取窗口、不能写成 `&self.text[start..start + 16]`：
        // 按字节切会落在多字节字符中间，`str` 索引直接 panic（本模块承诺不 panic）。
        // `&😀😀😀😀` 这种输入正好能踩中 16 字节边界。
        let mut semi = None;
        for (i, c) in self.text[start + 1..].char_indices().take(16) {
            if c == ';' {
                semi = Some(start + 1 + i);
                break;
            }
        }
        let semi = match semi {
            // E-MIGRATE-009：实体未闭合，或使用了未支持的实体名。
            Some(s) => s,
            None => {
                return Err(self.err(
                    E_MIGRATE_009,
                    start,
                    "`&` 之后未找到实体结束符 `;`（或实体名过长）",
                ))
            }
        };
        let ent = &self.text[start + 1..semi];
        let ch = match ent {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let num = ent.strip_prefix('#').ok_or_else(|| {
                    self.err(
                        E_MIGRATE_009,
                        start,
                        format!("未知实体 `&{ent};`（仅支持 lt/gt/amp/quot/apos 与 &#nn;/&#xnn;）"),
                    )
                })?;
                let (radix, digits) = match num.strip_prefix(['x', 'X']) {
                    Some(h) => (16u32, h),
                    None => (10u32, num),
                };
                // E-MIGRATE-010：实体的数字部分非法（非十六进制，或不是合法码点，含代理区）。
                let code = u32::from_str_radix(digits, radix).map_err(|_| {
                    self.err(E_MIGRATE_010, start, format!("实体 `&{ent};` 的数字部分非法"))
                })?;
                char::from_u32(code).ok_or_else(|| {
                    self.err(
                        E_MIGRATE_010,
                        start,
                        format!("实体 `&{ent};` 不是合法 Unicode 码点"),
                    )
                })?
            }
        };
        out.push(ch);
        self.pos = semi + 1;
        Ok(())
    }

    // ---- 元素 ----------------------------------------------------------------

    /// 解析一个完整元素，返回 `(标签名, 值)`。游标停在 `<`。
    fn element(&mut self, depth: usize) -> Result<(String, Value), SmlError> {
        if depth >= MAX_DEPTH {
            // E-LIMIT-001：XML 迁入超限同报此码（与语言层的嵌套上限共用）。
            return Err(self.err(
                E_LIMIT_001,
                self.pos,
                format!("嵌套深度超过上限 {MAX_DEPTH}（已停止解析，未崩溃）"),
            ));
        }
        let open = self.pos;
        match self.b.get(open + 1) {
            // E-MIGRATE-005：多余的结束标签。
            Some(b'/') => {
                return Err(self.err(E_MIGRATE_005, open, "多余的结束标签（没有与之匹配的开始标签）"))
            }
            // E-MIGRATE-008：此处不支持该声明或处理指令（只认注释与 DOCTYPE）。
            Some(b'!') => {
                return Err(self.err(
                    E_MIGRATE_008,
                    open,
                    "此处不支持 `<!` 声明（只认注释与 DOCTYPE）",
                ))
            }
            Some(b'?') => {
                return Err(self.err(E_MIGRATE_008, open, "此处不支持 `<?` 处理指令"))
            }
            // E-MIGRATE-002：标签未闭合（文件在标签内提前结束）。
            None => return Err(self.err(E_MIGRATE_002, open, "文件在 `<` 处结束，标签不完整")),
            _ => {}
        }
        self.pos = open + 1;

        let name_start = self.pos;
        while self.pos < self.b.len() && !is_name_end(self.b[self.pos]) {
            self.pos += 1;
        }
        if self.pos == name_start {
            // E-MIGRATE-003：标签名为空。
            return Err(self.err(E_MIGRATE_003, open, "标签名为空"));
        }
        let name = self.text[name_start..self.pos].to_string();

        // 属性区：读到 `>` 或 `/>`
        let mut attrs: Vec<(String, String)> = Vec::new();
        let self_closing = loop {
            self.skip_ws();
            if self.pos >= self.b.len() {
                // E-MIGRATE-002：标签未闭合（含属性区结束、标签名后立即结束两种形态）。
                return Err(self.err(
                    E_MIGRATE_002,
                    open,
                    format!("标签 <{name}> 未闭合（文件在属性区结束）"),
                ));
            }
            match self.b[self.pos] {
                b'>' => {
                    self.pos += 1;
                    break false;
                }
                b'/' => {
                    if self.b.get(self.pos + 1) == Some(&b'>') {
                        self.pos += 2;
                        break true;
                    }
                    // E-MIGRATE-006：属性区语法非法（自闭合写成单个斜杠）。
                    return Err(self.err(
                        E_MIGRATE_006,
                        self.pos,
                        "属性区出现单独的 `/`（自闭合应写作 `/>`）",
                    ));
                }
                _ => {
                    let (k, v) = self.attribute(&name)?;
                    attrs.push((k, v));
                }
            }
        };

        let has_attrs = !attrs.is_empty();
        let mut entries: Vec<(String, Value)> = Vec::new();
        // `_attrs` 放在最前：它逻辑上属于「本元素的元信息」，与子元素排序无关
        // （最终落在 `BTreeMap` 里按键名排序，此处只影响同名冲突时的数组顺序）。
        if has_attrs {
            let mut m = BTreeMap::new();
            for (k, v) in attrs {
                m.insert(k, Value::Str(v));
            }
            entries.push(("_attrs".to_string(), Value::Object(m)));
        }

        if self_closing {
            return Ok((name, merge_siblings(entries)));
        }

        let mut text_buf = String::new();
        loop {
            if self.pos >= self.b.len() {
                // E-MIGRATE-002：标签未闭合（文件提前结束）。
                return Err(self.err(
                    E_MIGRATE_002,
                    open,
                    format!("标签 <{name}> 未闭合（文件提前结束）"),
                ));
            }
            match self.b[self.pos] {
                b'<' => {
                    if self.b.get(self.pos + 1) == Some(&b'/') {
                        self.pos += 2;
                        let cs = self.pos;
                        while self.pos < self.b.len() && !is_name_end(self.b[self.pos]) {
                            self.pos += 1;
                        }
                        let close = &self.text[cs..self.pos];
                        if close != name {
                            // E-MIGRATE-004：结束标签与开始标签不匹配。
                            return Err(self.err(
                                E_MIGRATE_004,
                                cs,
                                format!("结束标签 </{close}> 与开始标签 <{name}> 不匹配"),
                            ));
                        }
                        self.skip_ws();
                        if self.b.get(self.pos) != Some(&b'>') {
                            // E-MIGRATE-004：结束标签缺少闭合符号。
                            return Err(self.err(
                                E_MIGRATE_004,
                                self.pos,
                                format!("结束标签 </{name}> 缺少 `>`"),
                            ));
                        }
                        self.pos += 1;
                        break;
                    } else if self.starts_with("<!--") {
                        self.skip_comment()?;
                    } else if self.starts_with("<![CDATA[") {
                        // CDATA 原样入 `_text`：内部不做实体解码、也不认 `<`
                        let start = self.pos;
                        self.pos += 9; // "<![CDATA["
                        match self.text[self.pos..].find("]]>") {
                            Some(i) => {
                                let end = self.pos + i;
                                text_buf.push_str(&self.text[self.pos..end]);
                                self.pos = end + 3;
                            }
                            None => {
                                return Err(self.err(
                                    E_MIGRATE_007,
                                    start,
                                    "CDATA 段未闭合（缺少 `]]>`）",
                                ))
                            }
                        }
                    } else if self.starts_with("<?") {
                        self.skip_pi()?;
                    } else if self.starts_with("<!") {
                        // E-MIGRATE-008：不支持的声明（只认注释；DOCTYPE 只在顶层）。
                        return Err(self.err(
                            E_MIGRATE_008,
                            self.pos,
                            "元素内容里出现不支持的 `<!` 声明",
                        ));
                    } else {
                        let (child, cv) = self.element(depth + 1)?;
                        entries.push((child, cv));
                    }
                }
                b'&' => self.decode_entity(&mut text_buf)?,
                _ => {
                    // 快速路径：整段（可能含中文/换行）直接搬，不做逐字符判断
                    let s = self.pos;
                    while self.pos < self.b.len()
                        && self.b[self.pos] != b'<'
                        && self.b[self.pos] != b'&'
                    {
                        self.pos += 1;
                    }
                    text_buf.push_str(&self.text[s..self.pos]);
                }
            }
        }

        let t = text_buf.trim();

        // **`_text` 折叠**：既无属性、也无子元素时，元素的值就是它的文本本身。
        //
        // `<baseAddress>0x40007000</baseAddress>` 因此写成 `baseAddress: "0x40007000"`，
        // 而不是 `baseAddress: { _text: "0x40007000" }` —— 省掉一层包装。
        //
        // 为什么**不丢信息**：容器一律写成 `{ ... }`，所以回读时「字符串」与「容器」
        // 一眼可分；而且文本仍按**字符串**处理（`0x20` 不会变成整数、`true` 不会变成
        // 布尔），XML 无类型这一点没有被偷偷改掉 —— 这与 `--to sml` 侧的
        // `quote_if_needed` 配合：会被误读成数字/布尔的文本会自动加引号。
        //
        // 为什么值得做：SVD 这类文件里 90% 以上的元素都是「纯文本叶子」，
        // 每个都套一层 `_text` 是纯粹的噪音（也是「文件体积增长快」的主要来源之一）。
        if !has_attrs && entries.is_empty() {
            return Ok((
                name,
                if t.is_empty() {
                    Value::Object(BTreeMap::new())
                } else {
                    Value::Str(t.to_string())
                },
            ));
        }

        if !t.is_empty() {
            entries.push(("_text".to_string(), Value::Str(t.to_string())));
        }
        Ok((name, merge_siblings(entries)))
    }

    /// 解析一个属性，返回 `(名, 值)`。值必须用引号括起（XML 的裸值属性严格说非法）。
    fn attribute(&mut self, tag: &str) -> Result<(String, String), SmlError> {
        let start = self.pos;
        while self.pos < self.b.len() && !is_name_end(self.b[self.pos]) {
            self.pos += 1;
        }
        if self.pos == start {
            // E-MIGRATE-006：属性区语法非法（出现非法字符）。
            return Err(self.err(
                E_MIGRATE_006,
                start,
                format!("<{tag}> 的属性区出现非法字符"),
            ));
        }
        let key = self.text[start..self.pos].to_string();

        self.skip_ws();
        if self.b.get(self.pos) != Some(&b'=') {
            // E-MIGRATE-006：属性区语法非法（缺少等号）。
            return Err(self.err(
                E_MIGRATE_006,
                self.pos,
                format!("<{tag}> 的属性 `{key}` 缺少 `=`"),
            ));
        }
        self.pos += 1;
        self.skip_ws();

        let quote = match self.b.get(self.pos) {
            Some(&q @ (b'"' | b'\'')) => {
                self.pos += 1;
                q
            }
            _ => {
                // E-MIGRATE-006：属性区语法非法（取值未加引号）。
                return Err(self.err(
                    E_MIGRATE_006,
                    self.pos,
                    format!("<{tag}> 的属性 `{key}` 的值必须用引号括起"),
                ))
            }
        };

        let mut buf = String::new();
        loop {
            if self.pos >= self.b.len() {
                // E-MIGRATE-006：属性区语法非法（引号未闭合）。
                return Err(self.err(
                    E_MIGRATE_006,
                    start,
                    format!("<{tag}> 的属性 `{key}` 引号未闭合"),
                ));
            }
            let c = self.b[self.pos];
            if c == quote {
                self.pos += 1;
                break;
            }
            match c {
                b'&' => self.decode_entity(&mut buf)?,
                b'<' => {
                    // E-MIGRATE-006：属性区语法非法（取值里出现尖括号）。
                    return Err(self.err(
                        E_MIGRATE_006,
                        self.pos,
                        format!("<{tag}> 的属性 `{key}` 的值里不允许出现 `<`"),
                    ))
                }
                _ => {
                    let s = self.pos;
                    while self.pos < self.b.len()
                        && self.b[self.pos] != quote
                        && self.b[self.pos] != b'&'
                        && self.b[self.pos] != b'<'
                    {
                        self.pos += 1;
                    }
                    buf.push_str(&self.text[s..self.pos]);
                }
            }
        }
        Ok((key, buf))
    }
}

/// 空白（XML 的 S 产生式：空格 / 制表 / 换行 / 回车）。
fn is_ws(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

/// 名字到此为止（`:`、`.`、`-`、`_` 等仍算名字的一部分，命名空间前缀因此天然保留）。
fn is_name_end(c: u8) -> bool {
    is_ws(c) || matches!(c, b'>' | b'/' | b'=')
}

/// 把 `(键, 值)` 序列折叠成对象：**同名兄弟合并为数组**（保持文档顺序）。
///
/// 用 `Entry` + `mem::replace` 而非按值 `match` 拿出 `Vec`：`Value` 实现了 `Drop`，
/// 从它里面按值移出字段会触发 E0509（见 HANDOFF §4-1）。
fn merge_siblings(entries: Vec<(String, Value)>) -> Value {
    let mut map: BTreeMap<String, Value> = BTreeMap::new();
    for (k, v) in entries {
        match map.entry(k) {
            Entry::Vacant(e) => {
                e.insert(v);
            }
            Entry::Occupied(mut e) => {
                let cur = e.get_mut();
                // 注意这里必须 reborrow（`&mut *cur`）而不是直接 match `cur`：
                // `&mut Value` 不是 `Copy`，按值绑定会把引用移走，之后无法再写回。
                let mut arr = match &mut *cur {
                    // 已经是数组（第 3 个及以后的重名）：直接续在后面
                    Value::Array(a) => std::mem::take(a),
                    // 第 2 个重名：把先前那个标量/对象收进数组首位
                    other => vec![std::mem::replace(other, Value::Null)],
                };
                arr.push(v);
                *cur = Value::Array(arr);
            }
        }
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(v: &Value) -> &BTreeMap<String, Value> {
        match v {
            Value::Object(m) => m,
            other => panic!("期望对象，得 {other:?}"),
        }
    }

    /// `Value` 没有 `as_array()`，数组只能模式匹配取（HANDOFF §4-2）。
    fn arr(v: &Value) -> &Vec<Value> {
        match v {
            Value::Array(a) => a,
            other => panic!("期望数组，得 {other:?}"),
        }
    }

    fn s(v: &Value) -> &str {
        v.as_str().unwrap_or_else(|| panic!("期望字符串，得 {v:?}"))
    }

    fn at<'a>(v: &'a Value, path: &str) -> &'a Value {
        v.get(path)
            .unwrap_or_else(|| panic!("缺少路径 `{path}`：{v:?}"))
    }

    #[test]
    fn nested_elements_and_text() {
        let v = parse("<a><b>1</b><c>x</c></a>").unwrap();
        let a = at(&v, "a");
        assert_eq!(s(at(a, "b")), "1");
        assert_eq!(s(at(a, "c")), "x");
    }

    /// `_text` 折叠的全套形状（这是本模块最核心的一条约定，单独钉住）。
    #[test]
    fn text_collapse_and_its_exceptions() {
        // ① 纯文本、无属性无子元素 → 直接是字符串，没有 `_text` 包装
        let v = parse("<r><name>PWR</name></r>").unwrap();
        assert_eq!(s(at(at(&v, "r"), "name")), "PWR");

        // ② 有属性 → 保持容器，文本留在 `_text`（否则属性没处放）
        let v = parse("<r><a id=\"x\">t</a></r>").unwrap();
        let a = at(at(&v, "r"), "a");
        assert_eq!(s(at(a, "_attrs.id")), "x");
        assert_eq!(s(at(a, "_text")), "t");

        // ③ 有子元素（混合内容）→ 保持容器，文本留在 `_text`
        let v = parse("<r><a>foo<b/>bar</a></r>").unwrap();
        let a = at(at(&v, "r"), "a");
        assert_eq!(s(at(a, "_text")), "foobar");
        assert!(matches!(at(a, "b"), Value::Object(_)));

        // ④ 空元素 → `{}`（折叠不会把它变成空字符串）
        let v = parse("<r><a/><b></b><c>   </c></r>").unwrap();
        for k in ["a", "b", "c"] {
            assert_eq!(obj(at(at(&v, "r"), k)).len(), 0, "{k} 应为空对象");
        }
    }

    /// 折叠是**无损**的：容器一定写成 `{ ... }`，所以回读时二者可分。
    #[test]
    fn collapsed_leaf_is_distinguishable_from_container() {
        let v = parse("<r><a>{}</a><b/></r>").unwrap();
        // `<a>{}</a>` 的文本是字面量 `{}` → 字符串；`<b/>` 是空容器
        assert_eq!(s(at(at(&v, "r"), "a")), "{}");
        assert_eq!(obj(at(at(&v, "r"), "b")).len(), 0);
    }

    #[test]
    fn attributes_go_into_underscore_attrs() {
        let v = parse("<device schemaVersion=\"1.1\" id='d1'/>").unwrap();
        let d = at(&v, "device");
        assert_eq!(s(at(d, "_attrs.schemaVersion")), "1.1");
        assert_eq!(s(at(d, "_attrs.id")), "d1");
    }

    #[test]
    fn namespace_prefix_is_kept() {
        let src = r#"<d xmlns:xs="http://www.w3.org/2001/XMLSchema-instance" xs:noNamespaceSchemaLocation="x.xsd">
                     <xs:name>n</xs:name>
                   </d>"#;
        let v = parse(src).unwrap();
        let d = at(&v, "d");
        assert_eq!(
            s(at(d, "_attrs.xmlns:xs")),
            "http://www.w3.org/2001/XMLSchema-instance"
        );
        assert_eq!(s(at(d, "xs:name")), "n");
    }

    #[test]
    fn repeated_siblings_merge_into_array_in_order() {
        let v = parse("<r><i>1</i><j>skip</j><i>2</i><i>3</i></r>").unwrap();
        let items = arr(at(at(&v, "r"), "i"));
        assert_eq!(items.len(), 3);
        assert_eq!(s(&items[0]), "1");
        assert_eq!(s(&items[1]), "2");
        assert_eq!(s(&items[2]), "3");
    }

    #[test]
    fn entities_named_and_numeric() {
        let v =
            parse("<r><t>&lt;a&gt; &amp; &quot;q&quot; &apos;s&apos; &#65;&#x42;</t></r>").unwrap();
        assert_eq!(s(at(at(&v, "r"), "t")), "<a> & \"q\" 's' AB");
    }

    #[test]
    fn entities_in_attribute_values() {
        let v = parse(r#"<r url="a&amp;b=1"/>"#).unwrap();
        assert_eq!(s(at(at(&v, "r"), "_attrs.url")), "a&b=1");
    }

    /// 实体探测窗口必须按**字符**取：按字节切会落在多字节字符中间，`str` 索引即 panic。
    /// 4 个 emoji = 16 字节，正好踩中窗口边界。
    #[test]
    fn entity_window_does_not_split_multibyte() {
        let e = parse("<r>&😀😀😀😀</r>").unwrap_err();
        assert!(e.message().contains("实体结束符"), "{e}");
        let v = parse("<r>&#x1F600;</r>").unwrap();
        assert_eq!(s(at(&v, "r")), "😀");
    }

    #[test]
    fn unknown_entity_is_an_error() {
        let e = parse("<r>&nbsp;</r>").unwrap_err();
        assert!(e.message().contains("未知实体"), "{e}");
        let e = parse("<r>&#xZZ;</r>").unwrap_err();
        assert!(e.message().contains("数字部分非法"), "{e}");
        let e = parse("<r>&#xD800;</r>").unwrap_err();
        assert!(e.message().contains("不是合法 Unicode"), "{e}");
    }

    #[test]
    fn cdata_is_verbatim() {
        let v = parse("<r><t><![CDATA[a < b & c]]></t></r>").unwrap();
        assert_eq!(s(at(at(&v, "r"), "t")), "a < b & c");
    }

    #[test]
    fn comments_pi_and_doctype_are_skipped() {
        let src = "<?xml version=\"1.0\"?>\n<!-- c -->\n<!DOCTYPE r [ <!ENTITY x \"y\"> ]>\n<r><!--inner--><?pi z?><a>1</a></r>";
        let v = parse(src).unwrap();
        assert_eq!(s(at(at(&v, "r"), "a")), "1");
    }

    #[test]
    fn self_closing_and_empty_element_are_empty_objects() {
        let v = parse("<r><a/><b></b></r>").unwrap();
        assert_eq!(obj(at(at(&v, "r"), "a")).len(), 0);
        assert_eq!(obj(at(at(&v, "r"), "b")).len(), 0);
    }

    /// 空白不构成文本：`<r>\n <a>1</a>\n</r>` 的 `r` 不该被折叠成字符串，
    /// 也不该长出 `_text`。
    #[test]
    fn whitespace_only_text_is_not_written() {
        let v = parse("<r>\n   <a>1</a>\n</r>").unwrap();
        assert_eq!(s(at(at(&v, "r"), "a")), "1");
        let r = obj(at(&v, "r"));
        assert!(!r.contains_key("_text"), "空白不该产生 _text：{r:?}");
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn text_is_trimmed_but_inner_whitespace_kept() {
        let v = parse("<r><t>\n  a\n  b\n</t></r>").unwrap();
        assert_eq!(s(at(at(&v, "r"), "t")), "a\n  b");
    }

    /// XML 1.0 §2.11：`\r\n` 与单独的 `\r` 都要归一为 `\n`。
    ///
    /// 这条不是风格偏好：SML 解析器的指令预扫用 `str::lines()` 重建文本，
    /// 会吃掉 CRLF 的 `\r`，源里留着 CR 就会在 `--to sml` 往返时丢失。
    #[test]
    fn crlf_and_lone_cr_are_normalized() {
        let v = parse("<r><t>a\r\nb\rc</t></r>").unwrap();
        assert_eq!(s(at(at(&v, "r"), "t")), "a\nb\nc");
        let v = parse("<r a=\"1\r\n2\"/>").unwrap();
        assert_eq!(s(at(at(&v, "r"), "_attrs.a")), "1\n2");
        let v = parse("<r><![CDATA[a\r\nb]]></r>").unwrap();
        assert_eq!(s(at(&v, "r")), "a\nb");
    }

    #[test]
    fn empty_document_is_empty_object() {
        assert_eq!(parse("").unwrap(), Value::Object(BTreeMap::new()));
        assert_eq!(
            parse("<?xml version=\"1.0\"?>\n<!-- only -->\n").unwrap(),
            Value::Object(BTreeMap::new())
        );
    }

    /// 叶子仍是**字符串**，没有被折叠动作顺带做成数字/布尔。
    ///
    /// 这一步靠的是 `to_sml` 的 `quote_if_needed`：会被误读成数字/布尔的文本自动加引号。
    /// 这里直接验 `Value` 类型（字符串），而 `--to sml` 的引号行为在集成侧验。
    #[test]
    fn leaves_stay_strings() {
        let v = parse("<r><size>0x20</size><b>true</b><n>17</n></r>").unwrap();
        let r = at(&v, "r");
        for k in ["size", "b", "n"] {
            assert!(
                matches!(at(r, k), Value::Str(_)),
                "{k} 应是字符串，不做类型猜测"
            );
        }
        assert_eq!(s(at(r, "size")), "0x20");
        assert_eq!(s(at(r, "b")), "true");
        assert_eq!(s(at(r, "n")), "17");
    }

    #[test]
    fn mismatched_close_tag_reports_line_and_offset() {
        let e = parse("<r>\n  <a>x</r>\n").unwrap_err();
        assert!(e.message().contains("第 2 行"), "{e}");
        assert!(e.message().contains("字符偏移"), "{e}");
        assert!(e.message().contains("不匹配"), "{e}");
    }

    /// XML 常整篇一行：行号退化成 1，必须靠字符偏移定位。
    #[test]
    fn single_line_document_still_reports_char_offset() {
        let pad = "x".repeat(2000);
        let src = format!("<r><t>{pad}</t></wrong>");
        let e = parse(&src).unwrap_err();
        assert!(e.message().contains("第 1 行"), "{e}");
        assert!(e.message().contains("字符偏移 2"), "{e}");
    }

    #[test]
    fn unclosed_tag_is_an_error_not_a_crash() {
        assert!(parse("<r><a></r>").is_err());
        assert!(parse("<r>").is_err());
        assert!(parse("<r").is_err());
        assert!(parse("</r>").is_err());
        assert!(parse("<r><a>text").is_err());
    }

    #[test]
    fn bad_attributes_are_errors() {
        assert!(parse("<r a=1/>").is_err());
        assert!(parse("<r a/>").is_err());
        assert!(parse("<r a=\"1/>").is_err());
        assert!(parse("<r a=\"<b\"/>").is_err());
    }

    #[test]
    fn top_level_text_is_an_error() {
        assert!(parse("hello <r/>").is_err());
    }

    /// 深度炸弹：必须返回 `Err`，而不是栈溢出 abort。
    #[test]
    fn depth_bomb_returns_error_without_overflow() {
        let open = "<a>".repeat(2000);
        let close = "</a>".repeat(2000);
        let e = parse(&format!("{open}x{close}")).unwrap_err();
        assert!(e.message().contains("嵌套深度超过上限"), "{e}");
    }

    /// 上限之内的嵌套要正常解析：128 层对象（根 = 第 0 层）。
    #[test]
    fn nesting_within_limit_succeeds() {
        let n = MAX_DEPTH;
        let src = format!("{}x{}", "<a>".repeat(n), "</a>".repeat(n));
        let v = parse(&src).unwrap();
        let mut cur = &v;
        for _ in 0..n {
            cur = at(cur, "a");
        }
        assert_eq!(s(cur), "x");
    }

    /// 真实文件冒烟：CMSIS-SVD（432KB）。文件未随仓库分发时自动跳过。
    #[test]
    fn real_svd_file_smoke() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/micro/CH32V103xx.svd");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let v = parse(&text).unwrap();
        let dev = at(&v, "device");
        assert_eq!(s(at(dev, "vendor")), "WCH Ltd.");
        assert_eq!(s(at(dev, "name")), "CH32V103xx");
        // 第二个 <peripheral> 是数组元素（同名兄弟合并）
        let periphs = arr(at(at(dev, "peripherals"), "peripheral"));
        assert!(periphs.len() > 10, "peripheral 数 = {}", periphs.len());
        assert_eq!(s(at(&periphs[0], "name")), "PWR");
        // 有属性的元素不折叠：device 顶层带 schemaVersion
        assert_eq!(s(at(dev, "_attrs.schemaVersion")), "1.1");
    }
}
