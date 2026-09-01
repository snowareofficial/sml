// SPDX-License-Identifier: MulanPSL-2.0
//! LaTeX 转译后端 (`emit-latex`)
//!
//! SML → LaTeX 约定（块类型即语义角色）：
//!
//! | `__type`   | LaTeX 产出                              |
//! |------------|-----------------------------------------|
//! | `h1`..`h6` | `\section` .. `\subparagraph`           |
//! | `p`        | 段落（空行分隔）                        |
//! | `ul`/`ol`  | `itemize` / `enumerate`                 |
//! | `li`       | `\item`                                 |
//! | `code`     | `verbatim` 环境（`lang` 忽略或作注释）  |
//! | `table`    | `tabular`（`header` + `rows`）           |
//! | `blockquote` | `quote` 环境                          |
//! | `em`       | `\emph{...}`                            |
//! | `strong`   | `\textbf{...}`                          |
//! | 无类型对象 | `description` 环境（字段列表）          |
//!
//! v2：`math` 选项开启时，`math`/`equation` 块原样透传 `$...$`/`$$...$$`。

use crate::Value;
use crate::emit::{EmitOptions, escape_latex, scalar_text, block_type, MAX_VALUE_DEPTH};

/// LaTeX 专属选项。
#[derive(Debug, Clone)]
pub struct LatexOptions {
    pub base: EmitOptions,
    /// 文档类。默认 `article`。
    pub documentclass: String,
    /// 生成完整 `document` 环境（含 preamble）。默认 true。
    pub full_document: bool,
    /// v2: 数学块透传（不转义 `$`）。默认 false。
    pub math: bool,
}

impl Default for LatexOptions {
    fn default() -> Self {
        LatexOptions {
            base: EmitOptions::default(),
            documentclass: "article".to_string(),
            full_document: true,
            math: false,
        }
    }
}

impl LatexOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// LaTeX 数学/原始内容中的危险原语：可读写文件、执行命令、加载宏包或改变
/// 类别码。数学内容无法转义（转义会破坏公式语义），只能拒绝。
const LATEX_DANGEROUS: &[&str] = &[
    "write18",
    "immediate",
    "openout",
    "openin",
    "input",
    "include",
    "usepackage",
    "RequirePackage",
    "documentclass",
    "csname",
    "catcode",
    "directlua",
    "latelua",
    "special",
    "read",
    "write",
    "closeout",
    "closein",
    "shipout",
];

/// 检查 LaTeX 原始内容（数学公式等）是否含危险控制序列。
/// 命中即返回 Err——此类内容不能安全地改写，只能拒绝输出。
fn check_latex_raw(body: &str) -> Result<(), String> {
    for seg in body.split('\\').skip(1) {
        let name: String = seg.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        if LATEX_DANGEROUS.contains(&name.as_str()) {
            return Err(format!(
                "latex: 拒绝输出含危险控制序列 `\\{}` 的数学内容",
                name
            ));
        }
    }
    Ok(())
}

/// 中和 verbatim 环境体里的结束标记，防止内容逃逸到文档顶层。
///
/// verbatim 内容无法转义（原样输出正是其语义），攻击者只需在内容里写入
/// `\\end{verbatim}` 就能提前关闭环境，随后注入任意 LaTeX。
/// 这里对所有 `\end{verbatim}` / `\end{verbatim*}`（含 `\end` 与 `{` 之间的
/// 空白变体）在环境名后插入一个空格，使其不再被识别为环境结束符。
fn neutralize_verbatim_end(body: &str) -> String {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len() + 16);
    let mut i = 0usize;
    while i < chars.len() {
        if chars[i] == '\\' && chars[i..].iter().copied().take(4).eq(['\\', 'e', 'n', 'd']) {
            let mut j = i + 4;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            if j < chars.len() && chars[j] == '{' {
                let rest: String = chars[j + 1..].iter().collect();
                if let Some(close) = rest.find('}') {
                    let inner = rest[..close].trim();
                    if inner == "verbatim" || inner == "verbatim*" {
                        out.push_str("\\end{");
                        out.push_str(inner);
                        out.push_str(" }");
                        i = j + 1 + close + 1;
                        continue;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// 文档类白名单字符（类名只允许字母与 `*`/`-`/`.`，且长度受限）。
/// `documentclass` 会被拼进 `\documentclass{...}`，其中可插入 `}` 闭合后
/// 追加任意 preamble 代码（含 `\write18` 等危险原语），必须清洗。
fn sanitize_documentclass(s: &str) -> String {
    let out: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '*' | '-' | '.'))
        .take(64)
        .collect();
    if out.is_empty() {
        "article".to_string()
    } else {
        out
    }
}

pub fn to_latex(v: &Value, opt: &LatexOptions) -> Result<String, String> {
    let mut out = String::new();
    if opt.full_document && opt.base.standalone {
        let class = sanitize_documentclass(&opt.documentclass);
        out.push_str(&format!("\\documentclass{{{}}}\n\\begin{{document}}\n", class));
    }
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
    if opt.full_document && opt.base.standalone {
        out.push_str("\\end{document}\n");
    }
    Ok(out)
}

fn emit_value(v: &Value, inferred: Option<&str>, opt: &LatexOptions, depth: usize, out: &mut String) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("latex: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    match v {
        Value::Object(_) => emit_object(v, inferred, opt, depth + 1, out)?,
        Value::Array(a) => {
            for item in a {
                emit_value(item, inferred, opt, depth + 1, out)?;
            }
        }
        Value::Str(s) => {
            if let Some("p") | Some("h1") | Some("h2") | Some("h3") | Some("h4") | Some("h5") | Some("h6") = inferred {
                let level: usize = inferred.unwrap()[1..].parse().unwrap_or(0);
                if level > 0 {
                    out.push_str(&format!("\\{}{{{}}}\n\n", latex_heading(level), escape_latex(s)));
                } else {
                    out.push_str(&format!("{}\n\n", escape_latex(s)));
                }
            } else {
                out.push_str(&escape_latex(s));
            }
        }
        other => out.push_str(&scalar_text(other)),
    }
    Ok(())
}

fn latex_heading(level: usize) -> &'static str {
    match level {
        1 => "section",
        2 => "subsection",
        3 => "subsubsection",
        4 => "paragraph",
        _ => "subparagraph",
    }
}

fn emit_object(v: &Value, inferred: Option<&str>, opt: &LatexOptions, depth: usize, out: &mut String) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("latex: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let ty = block_type(v).or(inferred);
    match ty {
        Some("h1") => heading(v, "section", opt, out),
        Some("h2") => heading(v, "subsection", opt, out),
        Some("h3") => heading(v, "subsubsection", opt, out),
        Some("h4") => heading(v, "paragraph", opt, out),
        Some("h5") => heading(v, "subparagraph", opt, out),
        Some("h6") => heading(v, "subparagraph", opt, out),
        Some("p") => {
            let c = block_text(v, opt);
            out.push_str(&format!("{}\n\n", c));
        }
        Some("ul") | Some("ol") => {
            let env = if ty == Some("ol") { "enumerate" } else { "itemize" };
            out.push_str(&format!("\\begin{{{}}}\n", env));
            let items = list_items(v);
            for item in &items {
                let body = match item {
                    Value::Str(s) => escape_latex(s),
                    Value::Object(_) => {
                        let t = block_text(item, opt);
                        if let Some(done) = item.get("done") {
                            if let Value::Bool(b) = done {
                                let mark = if *b { "[x]" } else { "[ ]" };
                                return Err(format!("LaTeX 不支持任务勾选，遇到 done 字段于列表项: {}", mark));
                            }
                        }
                        t
                    }
                    other => scalar_text(other),
                };
                out.push_str(&format!("\\item {}", body));
                if !body.ends_with('\n') {
                    out.push('\n');
                }
            }
            out.push_str(&format!("\\end{{{}}}\n\n", env));
        }
        Some("code") => {
            let body = v.get("text").or_else(|| v.get("code"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let lang = v.get("lang").and_then(|x| x.as_str()).unwrap_or("");
            if !lang.is_empty() {
                out.push_str(&format!("% language: {}\n", escape_latex(lang)));
            }
            // verbatim 环境无法转义：把用户内容中的 \end{verbatim} 改成带尾随空格，
            // 使其不再被识别为环境结束符，从而防止「逃逸到文档顶层」。
            let safe_body = neutralize_verbatim_end(body);
            out.push_str("\\begin{verbatim}\n");
            out.push_str(&safe_body);
            if !safe_body.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("\\end{verbatim}\n\n");
        }
        Some("blockquote") => {
            let c = block_text(v, opt);
            out.push_str("\\begin{quote}\n");
            out.push_str(&c);
            if !c.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("\\end{quote}\n\n");
        }
        Some("table") => emit_latex_table(v, opt, out)?,
        Some("em") => {
            out.push_str(&format!("\\emph{{{}}}", block_text(v, opt)));
        }
        Some("strong") => {
            out.push_str(&format!("\\textbf{{{}}}", block_text(v, opt)));
        }
        Some("math") | Some("equation") if opt.math => {
            let body = v.get("text").or_else(|| v.get("body"))
                .and_then(|x| x.as_str())
                .unwrap_or("");
            // 数学内容无法转义（转义会破坏公式语义），因此改为**拒绝**含
            // 文件读写 / shell 执行 / 包加载原语的内容：这些原语可让
            // 不可信数据读取本地文件或在开启 shell-escape 时执行任意命令。
            check_latex_raw(body)?;
            let env = if ty == Some("equation") { "equation" } else { "math" };
            if env == "math" {
                out.push_str(&format!("${}$", body));
            } else {
                out.push_str(&format!("\\begin{{{}}}\n{}\n\\end{{{}}}\n", env, body, env));
            }
        }
        _ => emit_description(v, opt, depth + 1, out)?,
    }
    Ok(())
}

fn heading(v: &Value, cmd: &str, _opt: &LatexOptions, out: &mut String) {
    let c = block_text(v, _opt);
    out.push_str(&format!("\\{}{{{}}}\n\n", cmd, c));
}

fn block_text(v: &Value, _opt: &LatexOptions) -> String {
    if let Some(t) = v.get("text") {
        return escape_latex(&scalar_text(t));
    }
    // 拼接标量字段
    let mut parts = Vec::new();
    if let Value::Object(m) = v {
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            match val {
                Value::Str(s) => parts.push(escape_latex(s)),
                Value::Int(i) => parts.push(i.to_string()),
                Value::Float(_) => parts.push(scalar_text(val)),
                Value::Bool(b) => parts.push(b.to_string()),
                _ => {}
            }
        }
    }
    parts.join(" ")
}

fn list_items(v: &Value) -> Vec<Value> {
    if let Some(items) = v.get("items") {
        if let Value::Array(a) = items {
            return a.clone();
        }
    }
    if let Value::Array(a) = v {
        return a.clone();
    }
    vec![v.clone()]
}

fn emit_latex_table(v: &Value, _opt: &LatexOptions, out: &mut String) -> Result<(), String> {
    let header = match v.get("header") {
        Some(Value::Array(a)) => a.clone(),
        _ => return Err("LaTeX table 缺少 header 数组".to_string()),
    };
    let rows = match v.get("rows") {
        Some(Value::Array(a)) => a.clone(),
        _ => Vec::new(),
    };
    let cols = header.len().max(1);
    let spec = "l".repeat(cols);
    out.push_str("\\begin{tabular}{");
    out.push_str(&spec);
    out.push_str("}\n\\hline\n");
    let hdr: Vec<String> = header.iter().map(|c| escape_latex(&cell_text(c))).collect();
    out.push_str(&format!("{}\\\\\\hline\n", hdr.join(" & ")));
    for row in &rows {
        let cells: Vec<String> = match row {
            Value::Array(a) => a.iter().map(|c| escape_latex(&cell_text(c))).collect(),
            Value::Object(m) => header
                .iter()
                .filter_map(|h| h.as_str())
                .map(|h| match m.get(h) {
                    Some(c) => escape_latex(&cell_text(c)),
                    None => String::new(),
                })
                .collect(),
            other => vec![cell_text(other)],
        };
        out.push_str(&cells.join(" & "));
        out.push_str("\\\\hline\n");
    }
    out.push_str("\\end{tabular}\n\n");
    Ok(())
}

fn cell_text(v: &Value) -> String {
    scalar_text(v)
}

/// 无类型对象 → description 环境。
fn emit_description(v: &Value, opt: &LatexOptions, depth: usize, out: &mut String) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("latex: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    if let Value::Object(m) = v {
        out.push_str("\\begin{description}\n");
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            let body = match val {
                Value::Object(_) => {
                    let mut sub = String::new();
                    emit_object(val, Some(k), opt, depth + 1, &mut sub)?;
                    sub.trim().to_string()
                }
                Value::Array(a) => {
                    let mut s = String::new();
                    for item in a {
                        emit_value(item, None, opt, depth + 1, &mut s)?;
                    }
                    s.trim().to_string()
                }
                _ => escape_latex(&scalar_text(val)),
            };
            out.push_str(&format!("\\item[{}] {}\n", escape_latex(k), body));
        }
        out.push_str("\\end{description}\n\n");
    }
    Ok(())
}
