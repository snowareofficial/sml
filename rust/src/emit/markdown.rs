// SPDX-License-Identifier: MulanPSL-2.0
//! Markdown / GFM 转译后端 (`emit-markdown`)
//!
//! SML → Markdown 的语义约定（SML 风格声明层，块类型即原生 Markdown 名）：
//!
//! | SML 块类型 (`__type`) | Markdown 产出                         |
//! |-----------------------|----------------------------------------|
//! | `h1`..`h6`            | 对应级别标题（取 `text` 或首个标量）    |
//! | `p`                   | 段落                                    |
//! | `ul` / `ol`           | 无序 / 有序列表（`items` 或自身为数组）|
//! | `li`                  | 列表项（支持 `done` 任务勾选）          |
//! | `table`               | GFM 表格（`header` + `rows`）           |
//! | `code`                | 围栏代码块（`lang` 指定语言）           |
//! | `blockquote`          | 引用块                                  |
//! | `hr`                  | 分隔线 `---`                            |
//! | `a`                   | 链接（`text` + `href`）                 |
//! | `img`                 | 图片（`src` + `alt`）                   |
//! | `em` / `strong` / `del` | 行内强调/加粗/删除线                  |
//! | （无类型对象）        | 字段以「`## key` + 内容」渲染，或定义列表 |
//!
//! v2 扩展（由 [`MarkdownOptions`] 开启）：脚注、数学 `$..$`、
//! 以及把未知 HTML 标签作为透传 raw。

use crate::Value;
use crate::emit::{
    EmitOptions, escape_text, escape_xml_attr, is_uri_attr, scalar_text, block_type, block_name,
    sanitize_xml_name, sanitize_xml_attr_name, sanitize_xml_uri, MAX_VALUE_DEPTH,
};

/// 过滤危险 URI scheme：`javascript:` / `vbscript:` / `data:`（图片除外）一律清空，
/// 防止 Markdown 链接/图片触发 XSS。
/// 同时剔除会破坏 Markdown 链接/图片语法的字符（`"`、`)`、`<`、`>`），
/// 避免 `src="y" onerror="..."` 这类属性注入。
fn sanitize_uri(uri: &str, allow_data_image: bool) -> String {
    let trimmed = uri.trim();
    let lower = trimmed.to_ascii_lowercase();
    let dangerous = lower.starts_with("javascript:")
        || lower.starts_with("vbscript:")
        || lower.starts_with("data:")
        || lower.starts_with("file:");
    if dangerous && !(allow_data_image && lower.starts_with("data:image/")) {
        return String::new();
    }
    // 只保留安全 URL 字符集，剥离任何可用于闭合链接目标或注入属性的字符
    // （空格、`=`、`"`、`(`、`)`、`<`、`>` 等），避免 `src="y" onerror="..."` 类注入。
    trimmed
        .chars()
        .filter(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '.' | '_' | ':' | '/' | '?' | '#' | '@' | '%' | '&' | '-' | '+' | ',' | ';')
        })
        .collect()
}

/// 图片替代文本 `![alt]`：先做 HTML 实体转义（渲染成 `alt="…"` 后不可闭合属性），
/// 再把会提前闭合 Markdown 图片语法的 `[` `]` 转为实体。
fn md_img_alt(s: &str) -> String {
    md_escape_inline(s)
        .replace('[', "&#91;")
        .replace(']', "&#93;")
        .replace('"', "&quot;")
}

/// 图片标题 `![..](.. "title")`：剔除会闭合标题的引号与换行。
fn md_img_title(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '"' | '\n' | '\r'))
        .collect::<String>()
        .trim()
        .to_string()
}

/// HTML 透传模式下禁止出现的标签：这些标签能执行脚本或加载外部资源，
/// 即使标签体已转义也不安全（事件属性、`src` 导航等注入面过多），一律拒绝。
const HTML_PASSTHROUGH_DENY: &[&str] = &[
    "script", "iframe", "object", "embed", "applet", "style", "link", "meta", "base", "svg",
    "math", "form", "input", "button", "textarea", "select", "option", "frame", "frameset",
    "template", "noscript", "canvas", "audio", "video", "source", "track", "portal", "dialog",
];

/// 检查 HTML 透传标签名是否安全。返回 `Err` 表示该标签被安全策略拒绝。
fn check_html_passthrough_tag(name: &str) -> Result<String, String> {
    let tag = sanitize_xml_name(name);
    if HTML_PASSTHROUGH_DENY.contains(&tag.to_ascii_lowercase().as_str()) {
        return Err(format!("markdown: HTML 透传拒绝危险标签 `<{tag}>`"));
    }
    Ok(tag)
}

/// 选择比**所有给定片段**中任意连续反引号都长的围栏，避免其中任一片段提前闭合围栏。
///
/// 必须同时考虑代码体与语言标注：只按代码体计算时，语言标注里的反引号
/// 会提前闭合围栏，把它后面的内容泄到代码块之外（安全审计 P1-1）。
fn code_fence(parts: &[&str]) -> String {
    let mut max_run = 0usize;
    for part in parts {
        let mut run = 0usize;
        for ch in part.chars() {
            if ch == '`' {
                run += 1;
                if run > max_run {
                    max_run = run;
                }
            } else {
                run = 0;
            }
        }
    }
    "`".repeat((max_run + 1).max(3))
}

/// Markdown 代码块的 info string（语言标注）清洗。
///
/// info string 在语法上只能是**单行**的标识符，且会被渲染器原样写进
/// `<code class="language-…">`：换行会提前结束围栏起始行、反引号会提前
/// 闭合围栏（二者都能把内容泄到代码块之外），`<` `>` `"` 等则可破坏
/// class 属性。故采用**白名单**——只保留语言标识符字符集，其余一律剔除。
/// 见安全审计 P1-1。
fn sanitize_code_lang(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '_' | '#' | '.'))
        .collect()
}

/// 行内文本里 `&`、`<`、`>` 转义（Markdown 上下文），避免 HTML 标签注入。
///
/// 换行同样需中和：它会中断当前块，使后续内容被解析为标题/新段落等，
/// 从而逃逸出 `- **k**: v`、`![alt]` 这类单行结构的定界（安全审计 P2-2）。
fn md_escape_inline(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '\n' | '\r' => o.push(' '),
            _ => o.push(c),
        }
    }
    o
}

/// 字段名渲染（`### name` / `- **key**: value`）：在 HTML 转义之外中和
/// Markdown **结构字符**。
///
/// - 换行会让字段逃逸出当前块，伪造标题或新列表项；
/// - `*` `_` `` ` `` `[` `]` `#` 会破坏 `**key**` 的定界或引入新标记。
///
/// 见安全审计 P2-2。
fn md_escape_key(s: &str) -> String {
    let mut t = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            // 先加反斜杠中和 Markdown 标记，再做实体转义（顺序不可颠倒）。
            // 刻意**不转义 `_`**：词内下划线在 CommonMark 中不构成强调，
            // 而它是标识符的高频字符（`max_retries`），转义会损害可读性。
            '\\' | '*' | '`' | '[' | ']' | '#' => {
                t.push('\\');
                t.push(c);
            }
            '\n' | '\r' => t.push(' '),
            _ => t.push(c),
        }
    }
    md_escape_inline(&t)
}

/// 表格单元格文本：在 HTML 转义之外中和 Markdown **表格结构字符**。
///
/// `|` 会伪造额外列，换行会伪造额外行——二者都能让不可信数据「看起来像」
/// 表头或另一条记录。故 `|` 转义为 `\|`，换行折叠为空格（表格内无法表达真换行）。
/// 见安全审计 P2-1。
fn md_cell_text(v: &Value, opt: &EmitOptions) -> String {
    let raw = cell_text(v);
    let mut t = String::with_capacity(raw.len() + 2);
    for c in raw.chars() {
        match c {
            '|' => t.push_str("\\|"),
            '\n' | '\r' => t.push(' '),
            _ => t.push(c),
        }
    }
    escape_text(&t, opt)
}

/// Markdown 专属选项。
#[derive(Debug, Clone)]
pub struct MarkdownOptions {
    pub base: EmitOptions,
    /// v2: 允许 `$...$` / `$$...$$` 数学块透传（不转义 `$`）。默认 false。
    pub math: bool,
    /// v2: 允许未知块类型作为 HTML 原始标签透传。默认 false。
    pub html_passthrough: bool,
    /// GFM 任务列表（`- [ ]` / `- [x]`）。默认 true。
    pub task_list: bool,
}

impl Default for MarkdownOptions {
    fn default() -> Self {
        MarkdownOptions {
            base: EmitOptions::default(),
            math: false,
            html_passthrough: false,
            task_list: true,
        }
    }
}

impl MarkdownOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// SML 值 → Markdown 文本。
pub fn to_markdown(v: &Value, opt: &MarkdownOptions) -> Result<String, String> {
    let mut out = String::new();
    // 顶层若是对象（文档），遍历其字段，字段名即 SML 风格块类型
    if let Value::Object(m) = v {
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            emit_value(val, Some(k), opt, 0, &mut out)?;
        }
    } else {
        emit_value(v, None, opt, 0, &mut out)?;
    }
    // 收尾规范化：保证末尾单个换行
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

fn indent_str(n: usize, opt: &MarkdownOptions) -> String {
    " ".repeat(n * opt.base.indent)
}

/// 渲染单个值（顶层或嵌套）。`inferred` 为字段名推断的块类型。
fn emit_value(
    v: &Value,
    inferred: Option<&str>,
    opt: &MarkdownOptions,
    depth: usize,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("markdown: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    match v {
        Value::Null => {}
        Value::Bool(b) => out.push_str(&b.to_string()),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(_) => out.push_str(&scalar_text(v)),
        Value::Str(s) => {
            // 带推断类型（如 p: "text"）时渲染为对应块
            if let Some(ty) = inferred {
                match ty {
                    "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                        let level = if ty.starts_with('h') { ty[1..].parse().unwrap_or(1) } else { 0 };
                        if level > 0 {
                            out.push_str(&format!("{} {}\n\n", "#".repeat(level), escape_text(s, &opt.base)));
                        } else {
                            out.push_str(&format!("{}\n\n", escape_text(s, &opt.base)));
                        }
                        return Ok(());
                    }
                    "em" => out.push_str(&format!("*{}*", escape_text(s, &opt.base))),
                    "strong" => out.push_str(&format!("**{}**", escape_text(s, &opt.base))),
                    "del" => out.push_str(&format!("~~{}~~", escape_text(s, &opt.base))),
                    _ => out.push_str(&escape_text(s, &opt.base)),
                }
            } else {
                out.push_str(&escape_text(s, &opt.base));
            }
        }
        Value::Array(a) => {
            for item in a {
                emit_value(item, inferred, opt, depth + 1, out)?;
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
        Value::Object(_) => emit_object(v, inferred, opt, depth + 1, out)?,
    }
    Ok(())
}

fn emit_object(
    v: &Value,
    inferred: Option<&str>,
    opt: &MarkdownOptions,
    depth: usize,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("markdown: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let ty = block_type(v).or(inferred);
    let pad = indent_str(depth, opt);

    match ty {
        Some("h1") | Some("h2") | Some("h3") | Some("h4") | Some("h5") | Some("h6") => {
            let level: usize = ty.unwrap()[1..].parse().unwrap_or(1);
            let content = block_text_content(v, opt);
            out.push_str(&format!("{} {}\n\n", "#".repeat(level), content));
        }
        Some("p") => {
            let content = block_text_content(v, opt);
            out.push_str(&format!("{}{}\n\n", pad, content));
        }
        Some("hr") => {
            out.push_str(&format!("{}---\n\n", pad));
        }
        Some("code") => {
            let lang = sanitize_code_lang(v.get("lang").and_then(|x| x.as_str()).unwrap_or(""));
            let body = v.get("text").or_else(|| v.get("code"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            // 围栏长度须同时覆盖 lang 与 body：只按 body 计算时，lang 里的
            // 反引号/换行会提前闭合围栏，把 `# INJECTED`、`<script>` 之类内容
            // 泄到代码块之外（安全审计 P1-1）。
            let fence = code_fence(&[body, &lang]);
            out.push_str(&format!("{} {}{}\n{}\n{}\n\n", pad, fence, lang, body, fence));
        }
        Some("blockquote") => {
            let content = block_text_content(v, opt);
            for line in content.lines() {
                out.push_str(&format!("{}> {}\n", pad, line));
            }
            out.push('\n');
        }
        Some("ul") | Some("ol") => {
            let items = list_items(v);
            let ordered = ty == Some("ol");
            for (idx, item) in items.iter().enumerate() {
                let marker = if ordered {
                    format!("{}. ", idx + 1)
                } else {
                    "- ".to_string()
                };
                emit_list_item(item, opt, depth + 1, &marker, &pad, out)?;
            }
            out.push('\n');
        }
        Some("table") => {
            emit_table(v, opt, &pad, out)?;
            out.push('\n');
        }
        Some("a") => {
            let text = v.get("text").and_then(|x| x.as_str()).unwrap_or("");
            let href = sanitize_uri(v.get("href").and_then(|x| x.as_str()).unwrap_or(""), false);
            out.push_str(&format!("[{}]({})", escape_text(text, &opt.base), href));
        }
        Some("img") => {
            let src = sanitize_uri(v.get("src").and_then(|x| x.as_str()).unwrap_or(""), true);
            // alt / title 同样是注入面：渲染结果为 `<img alt="…" title="…">`，
            // 未转义的 `"` 可闭合属性并追加事件处理器；`[` `]` 可提前闭合
            // Markdown 图片语法，把后续文本重开为链接。故二者都需清洗。
            let alt = md_img_alt(v.get("alt").and_then(|x| x.as_str()).unwrap_or(""));
            let title = md_img_title(v.get("title").and_then(|x| x.as_str()).unwrap_or(""));
            // title 必须放在**括号内**（`![alt](src "title")`）——
            // 写成 `![alt](src) "title"` 时 title 会被当作普通正文渲染，
            // 既不生效也会把内容泄漏到文档流里。
            if title.is_empty() {
                out.push_str(&format!("![{}]({})", alt, src));
            } else {
                out.push_str(&format!("![{}]({} \"{}\")", alt, src, title));
            }
        }
        Some("em") => {
            let c = block_text_content(v, opt);
            out.push_str(&format!("*{}*", c));
        }
        Some("strong") => {
            let c = block_text_content(v, opt);
            out.push_str(&format!("**{}**", c));
        }
        Some("del") => {
            let c = block_text_content(v, opt);
            out.push_str(&format!("~~{}~~", c));
        }
        Some(other) if opt.html_passthrough => {
            // v2 HTML 透传：标签名过危险标签黑名单（script/iframe/…），
            // 标签体一律转义——透传定位是「安全标签 + 文本体」，
            // 不做任意 raw HTML 直通，否则等价于开放 XSS。
            let name = check_html_passthrough_tag(other)?;
            let attrs = object_attrs(v, &["__type", "__name", "text"]);
            let inner = v.get("text").and_then(|x| x.as_str()).unwrap_or("");
            out.push_str(&format!(
                "<{}{}>{}</{}>\n\n",
                name,
                attrs,
                escape_text(inner, &opt.base),
                name
            ));
        }
        _ => {
            // 无类型或未知类型：作为「字段分组」渲染为小节
            emit_generic_object(v, opt, depth + 1, out)?;
        }
    }
    Ok(())
}

/// 取块的主文本：优先 `text` 字段，否则第一个标量字段，否则把子块逐行展开。
fn block_text_content(v: &Value, opt: &MarkdownOptions) -> String {
    if let Some(t) = v.get("text") {
        return escape_text(&scalar_text(t), &opt.base);
    }
    // 遍历对象字段，拼接标量；忽略元数据
    let mut parts: Vec<String> = Vec::new();
    if let Value::Object(m) = v {
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            match val {
                Value::Str(s) => parts.push(escape_text(s, &opt.base)),
                Value::Int(i) => parts.push(i.to_string()),
                Value::Float(_) => parts.push(scalar_text(val)),
                Value::Bool(b) => parts.push(b.to_string()),
                _ => {}
            }
        }
    }
    parts.join(" ")
}

/// 列表项提取：优先 `items` 数组，否则对象自身为数组，否则单行文本。
fn list_items(v: &Value) -> Vec<Value> {
    if let Some(items) = v.get("items") {
        if let Value::Array(a) = items {
            return a.clone();
        }
    }
    if let Value::Array(a) = v {
        return a.clone();
    }
    // 单个对象作为唯一项
    vec![v.clone()]
}

fn emit_list_item(
    item: &Value,
    opt: &MarkdownOptions,
    depth: usize,
    marker: &str,
    pad: &str,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("markdown: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    // 任务列表项：{ text, done }
    if opt.task_list {
        if let Some(done) = item.get("done") {
            if let Value::Bool(b) = done {
                let text = item.get("text").and_then(|x| x.as_str()).unwrap_or("");
                let boxchar = if *b { "x" } else { " " };
                out.push_str(&format!("{}{}- [{}] {}\n", pad, marker, boxchar, escape_text(text, &opt.base)));
                return Ok(());
            }
        }
    }
    match item {
        Value::Str(s) => {
            out.push_str(&format!("{}{}{}\n", pad, marker, escape_text(s, &opt.base)));
        }
        Value::Object(_) => {
            // 子块：渲染其文本，保持缩进
            let mut sub = String::new();
            emit_object(item, None, opt, depth + 1, &mut sub)?;
            // 去掉尾部空行，合并到同一列表项
            let sub = sub.trim_end();
            if sub.is_empty() {
                let t = block_text_content(item, opt);
                out.push_str(&format!("{}{}{}\n", pad, marker, t));
            } else {
                // 多行子块：第一行接 marker，后续行加缩进
                let sub_indent = format!("{}{}", pad, "  ");
                for (i, line) in sub.lines().enumerate() {
                    if i == 0 {
                        out.push_str(&format!("{}{}{}\n", pad, marker, line));
                    } else {
                        out.push_str(&format!("{}{}\n", sub_indent, line));
                    }
                }
            }
        }
        other => {
            out.push_str(&format!("{}{}{}\n", pad, marker, scalar_text(other)));
        }
    }
    Ok(())
}

fn emit_table(v: &Value, opt: &MarkdownOptions, pad: &str, out: &mut String) -> Result<(), String> {
    let header = v.get("header").and_then(|x| match x {
        Value::Array(a) => Some(a.clone()),
        _ => None,
    });
    let rows = v.get("rows").and_then(|x| match x {
        Value::Array(a) => Some(a.clone()),
        _ => None,
    });

    let header = match header {
        Some(h) => h,
        None => return Err("table 缺少 header 数组".to_string()),
    };
    let rows = rows.unwrap_or_default();

    let hdr_cells: Vec<String> = header.iter().map(|c| md_cell_text(c, &opt.base)).collect();
    out.push_str(&format!("{}| {} |\n", pad, hdr_cells.join(" | ")));
    let sep: Vec<String> = hdr_cells.iter().map(|_| "---".to_string()).collect();
    out.push_str(&format!("{}| {} |\n", pad, sep.join(" | ")));
    for row in &rows {
        let cells: Vec<String> = match row {
            Value::Array(a) => a.iter().map(|c| md_cell_text(c, &opt.base)).collect(),
            Value::Object(m) => header
                .iter()
                .filter_map(|h| h.as_str())
                .map(|h| match m.get(h) {
                    Some(c) => md_cell_text(c, &opt.base),
                    None => String::new(),
                })
                .collect(),
            other => vec![md_cell_text(other, &opt.base)],
        };
        out.push_str(&format!("{}| {} |\n", pad, cells.join(" | ")));
    }
    Ok(())
}

fn cell_text(v: &Value) -> String {
    scalar_text(v)
}

/// 渲染未知/无类型对象为「字段小节」。
fn emit_generic_object(
    v: &Value,
    opt: &MarkdownOptions,
    depth: usize,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("markdown: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let pad = indent_str(depth, opt);
    if let Value::Object(m) = v {
        let name = block_name(v);
        if let Some(n) = name {
            out.push_str(&format!("{}### {}\n\n", pad, md_escape_key(n)));
        }
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            // 字段名是数据结构的一部分、也可能来自不可信输入，必须中和其中的
            // Markdown 结构字符与 HTML，否则可伪造标题/新列表项（安全审计 P2-2）。
            let key = md_escape_key(k);
            match val {
                Value::Str(_) | Value::Int(_) | Value::Float(_) | Value::Bool(_) => {
                    out.push_str(&format!("{}- **{}**: {}\n", pad, key, md_escape_inline(&scalar_text(val))));
                }
                Value::Null => {
                    out.push_str(&format!("{}- **{}**: (空)\n", pad, key));
                }
                Value::Array(a) => {
                    out.push_str(&format!("{}- **{}**:\n", pad, key));
                    for item in a {
                        let mut sub = String::new();
                        emit_value(item, Some(k), opt, depth + 1, &mut sub)?;
                        for line in sub.lines() {
                            out.push_str(&format!("{}{}\n", pad, line));
                        }
                    }
                }
                Value::Object(_) => {
                    out.push_str(&format!("{}- **{}**:\n", pad, key));
                    emit_generic_object(val, opt, depth + 1, out)?;
                }
            }
        }
        out.push('\n');
    }
    Ok(())
}

/// 把对象中除忽略键外的键值渲染为 ` k="v"` 属性串（属性名/值均做安全处理）。
fn object_attrs(v: &Value, skip: &[&str]) -> String {
    let mut s = String::new();
    if let Value::Object(m) = v {
        for (k, val) in m {
            if skip.contains(&k.as_str()) {
                continue;
            }
            // 事件属性（onclick/onload/…）丢弃：值转义对执行点无效
            let Some(name) = sanitize_xml_attr_name(k) else {
                continue;
            };
            let raw = scalar_text(val);
            // href/src 等 URI 属性走 scheme 白名单，阻断 `javascript:`
            let valstr = if is_uri_attr(&name) {
                sanitize_xml_uri(&raw, true)
            } else {
                escape_xml_attr(&raw)
            };
            s.push_str(&format!(" {}=\"{}\"", name, valstr));
        }
    }
    s
}
