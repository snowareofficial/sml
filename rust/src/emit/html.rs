// SPDX-License-Identifier: MulanPSL-2.0
//! HTML5 转译后端 (`emit-html`)
//!
//! SML → 排版精美的 HTML5 小说/文档。块名即语义类型，直接映射为语义标签：
//!
//! | SML 块类型 (`__type`) | HTML 产出                                  |
//! |-----------------------|-------------------------------------------|
//! | `topic`               | `<article class="book">`（书/文档根）+ 标题 |
//! | `section`             | `<section class="chapter">` + 标题（按嵌套层级 h2..h6） |
//! | `para`                | `<p>` 段落                                 |
//! | `quote`               | `<blockquote>` 引文（诗/对话）            |
//! | `img`                 | `<figure><img><figcaption>` 插图          |
//! | `hr`                  | `<hr>` 分隔                                |
//! | `em`/`strong`/`del`   | 行内 `<em>`/`<strong>`/`<del>`            |
//! | `h1`..`h6`            | 对应级别标题                              |
//!
//! 正文内联角色 `{ref:}`/`{term:}`/`{math:}`/`{em:}`/`{strong:}` 渲染为对应 HTML。
//! `label` 字段渲染为锚点 `id`，供 `{ref: label}` 交叉引用跳转。
//!
//! 输出可选 `standalone`（完整 `<!DOCTYPE html>` + 内嵌 CSS 排版样式），或 `fragment`
//! （仅 `<article>` 内容片段，便于嵌入既有页面）。

use crate::Value;
use crate::emit::{
    EmitOptions, MAX_VALUE_DEPTH, block_name, block_type, escape_xml_attr, escape_xml_text,
    sanitize_xml_uri, scalar_text,
};

/// HTML 专属选项。
#[derive(Debug, Clone)]
pub struct HtmlOptions {
    pub base: EmitOptions,
    /// 生成完整 HTML 文档（含 `<head>` 与内嵌排版 CSS）。默认 `true`。
    pub standalone: bool,
    /// `<html lang>` 属性（语言代码）。默认 `"zh-CN"`。
    pub lang: String,
    /// 文档标题（写入 `<title>` 与书封面标题）。缺省取顶层 `title` 字段/书名。
    pub title: String,
    /// 内嵌 CSS 排版样式（仅 `standalone` 时使用）。
    pub css: String,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        HtmlOptions {
            base: EmitOptions::default(),
            standalone: true,
            lang: "zh-CN".to_string(),
            title: String::new(),
            css: default_css().to_string(),
        }
    }
}

impl HtmlOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// SML 值 → HTML 文本。
pub fn to_html(v: &Value, opt: &HtmlOptions) -> Result<String, String> {
    let mut body = String::new();
    if let Value::Object(_) = v {
        emit_object(v, None, opt, 0, 0, &mut body)?;
    } else {
        let mut sub = String::new();
        emit_value(v, None, opt, 0, 0, &mut sub)?;
        body.push_str(&format!("<article class=\"book\">\n{sub}</article>\n"));
    }

    if !opt.standalone {
        return Ok(body);
    }

    // 提取书的标题与作者（顶层 topic 的 title/author）。
    let (doc_title, author) = if let Value::Object(m) = v {
        let title = if !opt.title.is_empty() {
            opt.title.clone()
        } else {
            m.get("title")
                .and_then(|x| x.as_str())
                .map(str::to_string)
                .or_else(|| block_name(v).map(str::to_string))
                .unwrap_or_else(|| "未命名".to_string())
        };
        let author = m
            .get("author")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        (title, author)
    } else {
        (
            if opt.title.is_empty() {
                "未命名".to_string()
            } else {
                opt.title.clone()
            },
            String::new(),
        )
    };

    let safe_title = escape_xml_text(&doc_title);
    let auth_meta = if author.is_empty() {
        String::new()
    } else {
        format!("\n  <meta name=\"author\" content=\"{}\">", escape_xml_attr(&author))
    };

    let html = format!(
        "<!DOCTYPE html>\n\
<html lang=\"{lang}\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n\
  <title>{title}</title>{auth_meta}\n\
  <style>\n{css}\n  </style>\n\
</head>\n\
<body>\n{body}</body>\n\
</html>\n",
        lang = escape_xml_attr(&opt.lang),
        title = safe_title,
        auth_meta = auth_meta,
        css = opt.css,
        body = body,
    );
    Ok(html)
}

fn indent_str(n: usize) -> String {
    "  ".repeat(n)
}

/// 渲染单个值（顶层或嵌套）。`inferred` 为字段名推断的块类型。
fn emit_value(
    v: &Value,
    inferred: Option<&str>,
    opt: &HtmlOptions,
    depth: usize,
    hlevel: usize,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("html: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    match v {
        Value::Null => {}
        Value::Bool(b) => out.push_str(&b.to_string()),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(_, _) => out.push_str(&scalar_text(v)),
        Value::Str(s) => {
            if let Some(ty) = inferred {
                match ty {
                    "em" => out.push_str(&format!("<em>{}</em>", escape_xml_text(s))),
                    "strong" => out.push_str(&format!("<strong>{}</strong>", escape_xml_text(s))),
                    "del" => out.push_str(&format!("<del>{}</del>", escape_xml_text(s))),
                    _ => out.push_str(&escape_xml_text(s)),
                }
            } else {
                out.push_str(&escape_xml_text(s));
            }
        }
        Value::Array(a) => {
            for item in a {
                emit_value(item, inferred, opt, depth + 1, hlevel, out)?;
            }
        }
        Value::Object(_) => emit_object(v, inferred, opt, depth + 1, hlevel, out)?,
    }
    Ok(())
}

/// 渲染对象（块）。
fn emit_object(
    v: &Value,
    inferred: Option<&str>,
    opt: &HtmlOptions,
    depth: usize,
    hlevel: usize,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("html: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let ty = block_type(v).or(inferred);
    let pad = indent_str(depth);

    match ty {
        Some("topic") => {
            // 书/文档根：<article class="book">，含封面图、书名、作者。
            out.push_str(&format!("{pad}<article class=\"book\">\n"));
            // 封面图（顶层 img 子块，或 cover 字段）
            if let Some(Value::Object(_)) = v.get("cover") {
                render_img(v.get("cover").unwrap(), opt, depth + 1, out)?;
            }
            let title = block_heading_text(v, opt);
            let id = anchor(v);
            out.push_str(&format!(
                "{pad}  <header class=\"book-header\">\n{pad}    <h1 class=\"book-title\"{id}>{title}</h1>\n",
                id = id,
                title = title
            ));
            if let Some(a) = v.get("author").and_then(|x| x.as_str()) {
                out.push_str(&format!(
                    "{pad}    <p class=\"book-author\">{} 著</p>\n",
                    escape_xml_text(a)
                ));
            }
            out.push_str(&format!("{pad}  </header>\n"));
            emit_children(v, opt, depth, hlevel, out)?;
            out.push_str(&format!("{pad}</article>\n"));
        }
        Some("section") => {
            // 顶层单块会被 parse 包一层（emit_generic_object 会 +1 层 hlevel），
            // 故 topic 实际位于 hlevel=1、一级 section 位于 hlevel=2；
            // 直接用 hlevel 作标题级（封顶 h6）：一级 section→h2，逐层加深。
            let lvl = hlevel.min(6); // topic(书)→h1，一级 section→h2，逐层加深
            let title = block_heading_text(v, opt);
            let id = anchor(v);
            out.push_str(&format!(
                "{pad}<section class=\"chapter\"{id}>\n{pad}  <h{lvl} class=\"chapter-title\">{title}</h{lvl}>\n",
                lvl = lvl,
                id = id,
                title = title
            ));
            // 章节插图（结构文件里的 illustration 字段，渲染在标题与正文之间）
            if let Some(Value::Object(_)) = v.get("illustration") {
                render_img(v.get("illustration").unwrap(), opt, depth + 1, out)?;
            }
            emit_children(v, opt, depth, hlevel, out)?;
            out.push_str(&format!("{pad}</section>\n"));
        }
        Some("para") => {
            let content = block_text_content(v, opt);
            out.push_str(&format!("{pad}<p>{}</p>\n", content));
        }
        Some("quote") => {
            let content = block_text_content(v, opt);
            out.push_str(&format!("{pad}<blockquote>\n{pad}  <p>{}</p>\n{pad}</blockquote>\n", content));
        }
        Some("img") => {
            render_img(v, opt, depth, out)?;
        }
        Some("hr") => {
            out.push_str(&format!("{pad}<hr class=\"scene-break\">\n"));
        }
        Some("em") => {
            let c = block_text_content(v, opt);
            out.push_str(&format!("<em>{}</em>", c));
        }
        Some("strong") => {
            let c = block_text_content(v, opt);
            out.push_str(&format!("<strong>{}</strong>", c));
        }
        Some("del") => {
            let c = block_text_content(v, opt);
            out.push_str(&format!("<del>{}</del>", c));
        }
        Some("h1" | "h2" | "h3" | "h4" | "h5" | "h6") => {
            let level: usize = ty.unwrap()[1..].parse().unwrap_or(2);
            let content = block_text_content(v, opt);
            out.push_str(&format!(
                "{pad}<h{level} class=\"heading\">{content}</h{level}>\n",
                level = level,
                content = content
            ));
        }
        Some("math") => {
            let content = block_text_content(v, opt);
            out.push_str(&format!("{pad}<div class=\"math\">\\[{}\\]</div>\n", content));
        }
        _ => {
            // 无类型或未知类型：作为字段分组渲染
            emit_generic_object(v, opt, depth, hlevel, out)?;
        }
    }
    Ok(())
}

/// 渲染插图块 `img { src, alt, caption? }` 为 `<figure>`。
fn render_img(v: &Value, _opt: &HtmlOptions, depth: usize, out: &mut String) -> Result<(), String> {
    let pad = indent_str(depth);
    let src = sanitize_xml_uri(
        v.get("src").and_then(|x| x.as_str()).unwrap_or(""),
        true,
    );
    let alt = v.get("alt").and_then(|x| x.as_str()).unwrap_or("");
    if src.is_empty() {
        // 无有效 src 时至少保留 alt 文本，避免信息丢失
        out.push_str(&format!("{pad}<p class=\"missing-img\">{}</p>\n", escape_xml_text(alt)));
        return Ok(());
    }
    out.push_str(&format!("{pad}<figure class=\"illus\">\n"));
    out.push_str(&format!(
        "{pad}  <img src=\"{}\" alt=\"{}\" loading=\"lazy\">\n",
        src,
        escape_xml_attr(alt)
    ));
    if !alt.is_empty() {
        out.push_str(&format!(
            "{pad}  <figcaption>{}</figcaption>\n",
            escape_xml_text(alt)
        ));
    }
    out.push_str(&format!("{pad}</figure>\n"));
    Ok(())
}

/// 取块的主文本：优先 `text` 字段，否则首个标量字段，否则拼接子块。
fn block_text_content(v: &Value, _opt: &HtmlOptions) -> String {
    if let Some(t) = v.get("text") {
        return apply_roles(&escape_xml_text(&scalar_text(t)));
    }
    let mut parts: Vec<String> = Vec::new();
    if let Value::Object(m) = v {
        for (k, val) in m {
            if DOC_META.contains(&k.as_str()) {
                continue;
            }
            match val {
                Value::Str(s) => parts.push(escape_xml_text(s)),
                Value::Int(i) => parts.push(i.to_string()),
                Value::Float(_, _) => parts.push(scalar_text(val)),
                Value::Bool(b) => parts.push(b.to_string()),
                _ => {}
            }
        }
    }
    apply_roles(&parts.join(" "))
}

/// 容器块渲染时跳过的元数据键。
const DOC_META: &[&str] = &["__type", "__name", "title", "label", "children", "type", "cover", "illustration", "author", "src", "alt", "caption"];

/// 取容器块标题文本（用于 `topic`/`section`）。
fn block_heading_text(v: &Value, opt: &HtmlOptions) -> String {
    if let Some(t) = v.get("title").and_then(|x| x.as_str()) {
        return apply_roles(&escape_xml_text(t));
    }
    if let Some(n) = block_name(v) {
        return apply_roles(&escape_xml_text(n));
    }
    if let Some(t) = v.get("text").and_then(|x| x.as_str()) {
        return apply_roles(&escape_xml_text(t));
    }
    block_text_content(v, opt)
}

/// 取 `label` 渲染为 ` id="..."` 属性（含前导空格），无则空串。
fn anchor(v: &Value) -> String {
    match v.get("label").and_then(|x| x.as_str()) {
        Some(l) if !l.is_empty() => format!(" id=\"{}\"", escape_xml_attr(l)),
        _ => String::new(),
    }
}

/// 渲染容器子内容：优先 `children` 数组；再遍历其余对象/数组字段。
fn emit_children(
    v: &Value,
    opt: &HtmlOptions,
    depth: usize,
    hlevel: usize,
    out: &mut String,
) -> Result<(), String> {
    if let Some(Value::Array(children)) = v.get("children") {
        for child in children {
            emit_value(child, None, opt, depth + 1, hlevel + 1, out)?;
        }
    }
    if let Value::Object(m) = v {
        for (k, val) in m {
            if DOC_META.contains(&k.as_str()) {
                continue;
            }
            match val {
                Value::Object(_) => {
                    emit_object(val, Some(k), opt, depth + 1, hlevel + 1, out)?;
                }
                Value::Array(a) => {
                    if a.iter().all(|x| matches!(x, Value::Object(_))) {
                        for item in a {
                            emit_object(item, Some(k), opt, depth + 1, hlevel + 1, out)?;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// 未知/无类型对象渲染为字段小节（兜底，保证不丢信息）。
fn emit_generic_object(
    v: &Value,
    opt: &HtmlOptions,
    depth: usize,
    hlevel: usize,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("html: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let pad = indent_str(depth);
    if let Value::Object(m) = v {
        let name = block_name(v);
        if let Some(n) = name {
            out.push_str(&format!("{pad}<h3 class=\"group\">{}</h3>\n", escape_xml_text(n)));
        }
        for (k, val) in m {
            if DOC_META.contains(&k.as_str()) {
                continue;
            }
            let key = escape_xml_text(k);
            match val {
                Value::Str(_) | Value::Int(_) | Value::Float(_, _) | Value::Bool(_) => {
                    out.push_str(&format!(
                        "{pad}<p class=\"field\"><strong>{}</strong>: {}</p>\n",
                        key,
                        escape_xml_text(&scalar_text(val))
                    ));
                }
                Value::Null => {
                    out.push_str(&format!("{pad}<p class=\"field\"><strong>{}</strong>: (空)</p>\n", key));
                }
                _ => {
                    emit_object(val, Some(k), opt, depth + 1, hlevel + 1, out)?;
                }
            }
            let _ = hlevel;
        }
    }
    Ok(())
}

/// 把正文里的内联语义角色 `{role: arg}` 转换为 HTML。
fn apply_roles(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' {
            let mut inner = String::new();
            let mut depth = 1usize;
            let mut closed = false;
            while let Some(d) = chars.next() {
                if d == '{' {
                    depth += 1;
                    inner.push(d);
                } else if d == '}' {
                    depth -= 1;
                    if depth == 0 {
                        closed = true;
                        break;
                    } else {
                        inner.push(d);
                    }
                } else {
                    inner.push(d);
                }
            }
            if closed {
                if let Some(html) = role_to_html(&inner) {
                    out.push_str(&html);
                    continue;
                }
                out.push('{');
                out.push_str(&inner);
                out.push('}');
            } else {
                out.push('{');
                out.push_str(&inner);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn role_to_html(inner: &str) -> Option<String> {
    let inner = inner.trim();
    let mut it = inner.splitn(2, ':');
    let role = it.next()?.trim();
    let arg = it.next()?.trim();
    if arg.is_empty() {
        return None;
    }
    match role {
        "ref" => {
            let (target, label) = match arg.split_once('|') {
                Some((t, l)) => (t.trim(), l.trim()),
                None => (arg, arg),
            };
            Some(format!(
                "<a class=\"ref\" href=\"#{}\">{}</a>",
                escape_xml_attr(target),
                escape_xml_text(label)
            ))
        }
        "term" => Some(format!("<dfn>{}</dfn>", escape_xml_text(strip_quotes(arg)))),
        "math" => Some(format!("<span class=\"math\">\\({}\\)</span>", escape_xml_text(strip_quotes(arg)))),
        "em" => Some(format!("<em>{}</em>", escape_xml_text(arg))),
        "strong" => Some(format!("<strong>{}</strong>", escape_xml_text(arg))),
        _ => None,
    }
}

fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    let n = s.len();
    if n >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[n - 1] == b'"') || (b[0] == b'\'' && b[n - 1] == b'\'') {
            return &s[1..n - 1];
        }
    }
    s
}

/// 默认排版 CSS：宣纸底、衬线中文、舒适行距、首段下沉、章节饰线与插图样式。
fn default_css() -> &'static str {
    ":root{\n\
  --paper:#f7f3ea;\n\
  --ink:#23201b;\n\
  --ink-soft:#5b554b;\n\
  --rule:#c9bfa8;\n\
  --accent:#8a2b2b;\n\
  --maxw:38rem;\n\
}\n\
* { box-sizing:border-box; }\n\
html { background:var(--paper); }\n\
body {\n\
  margin:0;\n\
  background:var(--paper);\n\
  color:var(--ink);\n\
  font-family:\"Noto Serif SC\",\"Source Han Serif SC\",\"Songti SC\",STSong,\"SimSun\",Georgia,serif;\n\
  font-size:18px;\n\
  line-height:1.95;\n\
  -webkit-font-smoothing:antialiased;\n\
  text-rendering:optimizeLegibility;\n\
}\n\
.book {\n\
  max-width:var(--maxw);\n\
  margin:0 auto;\n\
  padding:5rem 1.5rem 7rem;\n\
}\n\
.book-header { text-align:center; margin-bottom:3.5rem; }\n\
.book-title {\n\
  font-size:2.6rem;\n\
  letter-spacing:.4rem;\n\
  margin:0 0 .6rem;\n\
  font-weight:700;\n\
}\n\
.book-author {\n\
  color:var(--ink-soft);\n\
  font-size:1rem;\n\
  letter-spacing:.2rem;\n\
  margin:0;\n\
}\n\
.chapter { margin-top:3.2rem; }\n\
.chapter-title {\n\
  font-size:1.7rem;\n\
  text-align:center;\n\
  letter-spacing:.3rem;\n\
  margin:0 0 1.8rem;\n\
  padding-bottom:1rem;\n\
  position:relative;\n\
}\n\
.chapter-title::after {\n\
  content:\"\";\n\
  display:block;\n\
  width:3.5rem;\n\
  height:2px;\n\
  background:var(--accent);\n\
  margin:.9rem auto 0;\n\
  opacity:.8;\n\
}\n\
p { margin:0 0 1.15rem; text-align:justify; text-justify:inter-character; }\n\
.chapter > p:first-of-type::first-letter {\n\
  font-size:3.1rem;\n\
  line-height:1;\n\
  float:left;\n\
  padding:.25rem .5rem 0 0;\n\
  color:var(--accent);\n\
  font-weight:700;\n\
}\n\
blockquote {\n\
  margin:1.6rem 0;\n\
  padding:.4rem 1.4rem;\n\
  border-left:3px solid var(--rule);\n\
  color:var(--ink-soft);\n\
  font-style:italic;\n\
  background:rgba(0,0,0,.02);\n\
}\n\
blockquote p { margin:.4rem 0; text-align:left; white-space:pre-line; }\n\
hr.scene-break {\n\
  border:none;\n\
  text-align:center;\n\
  margin:2.6rem 0;\n\
}\n\
hr.scene-break::before {\n\
  content:\"❦\";\n\
  color:var(--rule);\n\
  font-size:1.2rem;\n\
  letter-spacing:1rem;\n\
}\n\
figure.illus {\n\
  margin:2.4rem 0;\n\
  text-align:center;\n\
}\n\
figure.illus img {\n\
  max-width:100%;\n\
  border-radius:6px;\n\
  box-shadow:0 6px 22px rgba(0,0,0,.18);\n\
  display:inline-block;\n\
}\n\
figcaption {\n\
  margin-top:.7rem;\n\
  font-size:.85rem;\n\
  color:var(--ink-soft);\n\
  letter-spacing:.05rem;\n\
}\n\
em, dfn { color:var(--accent); font-style:normal; border-bottom:1px dotted var(--rule); }\n\
a.ref { color:var(--accent); text-decoration:none; border-bottom:1px solid var(--rule); }\n\
a.ref:hover { border-bottom-color:var(--accent); }\n\
.math { font-family:\"Cambria Math\",Georgia,serif; }\n\
.field { color:var(--ink-soft); }\n\
@media (max-width:480px){\n\
  body { font-size:16.5px; }\n\
  .book { padding:3rem 1.1rem 4rem; }\n\
  .book-title { font-size:2rem; }\n\
}\n"
}
