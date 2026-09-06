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
    // 容器保序：SML 对象字段按名字排序存储（BTreeMap），同级写 `h1`/`p`/`ul`
    // 会按字典序输出、正文乱序；只有 `children` 数组能表达**文档顺序**。
    // 故凡带 `children` 的块一律按数组顺序输出子元素 —— 但下列类型自身
    // 已定义了子元素的语义（`items` / `header` / `text` / 行内强调），不参与。
    if block_children(v).is_some()
        && !matches!(
            ty,
            Some("h1") | Some("h2") | Some("h3") | Some("h4") | Some("h5") | Some("h6")
                | Some("p") | Some("ul") | Some("ol") | Some("table") | Some("code")
                | Some("blockquote") | Some("em") | Some("strong") | Some("math")
                | Some("equation")
        )
    {
        return emit_container(v, opt, depth, out);
    }
    match ty {
        Some("h1") => heading(v, "section", opt, depth, out)?,
        Some("h2") => heading(v, "subsection", opt, depth, out)?,
        Some("h3") => heading(v, "subsubsection", opt, depth, out)?,
        Some("h4") => heading(v, "paragraph", opt, depth, out)?,
        Some("h5") => heading(v, "subparagraph", opt, depth, out)?,
        Some("h6") => heading(v, "subparagraph", opt, depth, out)?,
        Some("p") => {
            let c = block_text(v, opt, depth)?;
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
                        if let Some(Value::Bool(b)) = item.get("done") {
                            let mark = if *b { "[x]" } else { "[ ]" };
                            return Err(format!("LaTeX 不支持任务勾选，遇到 done 字段于列表项: {}", mark));
                        }
                        block_text(item, opt, depth)?
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
            let c = block_text(v, opt, depth)?;
            out.push_str("\\begin{quote}\n");
            out.push_str(&c);
            if !c.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("\\end{quote}\n\n");
        }
        Some("table") => emit_latex_table(v, opt, out)?,
        Some("em") => {
            out.push_str(&format!("\\emph{{{}}}", block_text(v, opt, depth)?));
        }
        Some("strong") => {
            out.push_str(&format!("\\textbf{{{}}}", block_text(v, opt, depth)?));
        }
        Some("math") | Some("equation") => emit_math(v, ty == Some("equation"), opt, out)?,
        _ => emit_description(v, opt, depth + 1, out)?,
    }
    Ok(())
}

/// 数学块/行内公式。`opt.math` 关闭时退化为**转义后的纯文本**，
/// 而不是之前的 `description` 环境 —— 后者会把 `$E=mc^2$` 包成
/// `\item[text] ...` 并转义掉 `^`，既不可读也不像公式。
fn emit_math(v: &Value, is_equation: bool, opt: &LatexOptions, out: &mut String) -> Result<(), String> {
    let body = raw_body(v);
    if !opt.math {
        out.push_str(&escape_latex(&body));
        return Ok(());
    }
    // 数学内容无法转义（转义会破坏公式语义），因此改为**拒绝**含
    // 文件读写 / shell 执行 / 包加载原语的内容：这些原语可让
    // 不可信数据读取本地文件或在开启 shell-escape 时执行任意命令。
    check_latex_raw(&body)?;
    if is_equation {
        out.push_str(&format!("\\begin{{equation}}\n{}\n\\end{{equation}}\n", body));
    } else {
        out.push_str(&format!("${}$", body));
    }
    Ok(())
}

/// 取块的原始文本（`text` 优先，其次 `body`），不做任何转义。
fn raw_body(v: &Value) -> String {
    v.get("text")
        .or_else(|| v.get("body"))
        .map(scalar_text)
        .unwrap_or_default()
}

/// 容器：按 `children` 数组顺序输出子元素。
fn emit_container(v: &Value, opt: &LatexOptions, depth: usize, out: &mut String) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("latex: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    for kid in block_children(v).unwrap_or_default() {
        emit_value(kid, None, opt, depth + 1, out)?;
    }
    Ok(())
}

/// 取 `children` 数组的元素引用（保序）。
fn block_children(v: &Value) -> Option<Vec<&Value>> {
    match v.get("children") {
        Some(Value::Array(a)) => Some(a.iter().collect()),
        _ => None,
    }
}

fn heading(v: &Value, cmd: &str, opt: &LatexOptions, depth: usize, out: &mut String) -> Result<(), String> {
    let c = block_text(v, opt, depth)?;
    out.push_str(&format!("\\{}{{{}}}\n\n", cmd, c));
    Ok(())
}

/// 行内类型：可嵌在段落文本里渲染（`p { text: "a" em { text: "b" } }`）。
fn is_inline_ty(t: Option<&str>) -> bool {
    matches!(t, Some("em") | Some("strong") | Some("code") | Some("math") | Some("equation"))
}

/// 把一个值渲染为**行内** LaTeX（`em`/`strong`/`code` 等强调，或标量文本）。
///
/// - `inferred` 是字段名：裸块 `em { }` 解析后只以字段名存在（**不带**
///   `__type`），必须靠它推断类型；`children` 数组里的裸块同理。
/// - 非行内类型的对象返回空串：块级内容（列表/表格）塞进 `\emph{}`
///   会产出无法编译的 LaTeX，宁可丢弃也不产出坏码。
fn inline_text(v: &Value, inferred: Option<&str>, opt: &LatexOptions, depth: usize) -> Result<String, String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("latex: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let Value::Object(_) = v else {
        return Ok(escape_latex(&scalar_text(v)));
    };
    let ty = block_type(v).or(inferred);
    if !is_inline_ty(ty) {
        return Ok(String::new());
    }
    match ty {
        Some("em") => Ok(format!("\\emph{{{}}}", block_text(v, opt, depth + 1)?)),
        Some("strong") => Ok(format!("\\textbf{{{}}}", block_text(v, opt, depth + 1)?)),
        Some("code") => Ok(format!("\\texttt{{{}}}", escape_latex(&raw_body(v)))),
        Some("math") | Some("equation") => {
            let body = raw_body(v);
            if opt.math {
                check_latex_raw(&body)?;
                if ty == Some("equation") {
                    Ok(format!("\\begin{{equation}}\n{}\n\\end{{equation}}", body))
                } else {
                    Ok(format!("${}$", body))
                }
            } else {
                Ok(escape_latex(&body))
            }
        }
        _ => Ok(String::new()),
    }
}

/// 取块的文本内容。
///
/// 拼接顺序：**`text` 标量 → `children` 数组（保序）→ 其余字段（按名字序）**。
/// 之所以把 `text` 提前：SML 对象字段按名字排序存储，若纯按名字序，
/// `li { text: "前缀" em { text: "x" } }` 会渲染成 `\emph{x} 前缀`（`em` < `text`），
/// 与书写顺序相反。`text` 是各后端公认的「主文本」键，置前最符合直觉。
///
/// 需要完全自定义顺序时用 `children` 数组：
/// `p { children: [ "前缀 " em { text: "x" } " 后缀" ] }`。
fn block_text(v: &Value, opt: &LatexOptions, depth: usize) -> Result<String, String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("latex: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let Value::Object(m) = v else {
        return Ok(escape_latex(&scalar_text(v)));
    };
    let mut parts: Vec<String> = Vec::new();
    // 1. 主文本
    if let Some(t) = m.get("text") {
        if !matches!(t, Value::Object(_) | Value::Array(_)) {
            parts.push(escape_latex(&scalar_text(t)));
        }
    }
    // 2. children 数组：唯一保序载体
    let mut kids: Vec<&Value> = Vec::new();
    if let Some(Value::Array(a)) = m.get("children") {
        kids.extend(a.iter());
    }
    for item in kids {
        parts.push(inline_text(item, None, opt, depth + 1)?);
    }
    // 3. 其余字段（含 `text` 的对象/数组形式，如 `text { em { ... } }`）
    for (k, val) in m {
        match k.as_str() {
            "__type" | "__name" | "__args" | "children" => {}
            "text" if !matches!(val, Value::Object(_) | Value::Array(_)) => {}
            _ => match val {
                Value::Null => {}
                Value::Object(_) | Value::Array(_) => {
                    parts.push(inline_text(val, Some(k), opt, depth + 1)?);
                }
                other => parts.push(escape_latex(&scalar_text(other))),
            },
        }
    }
    Ok(parts.join(" ").trim().to_string())
}

/// 列表项来源（按优先级）：显式 `items` 数组 → `children` 数组 → 自身是数组。
///
/// `children` 的加入让 `li { }` 子块写法可用 —— 注意**不能用**多个同名
/// `li { }` 裸块并列：SML 对象字段按名字存储，同名的后者会覆盖前者。
fn list_items(v: &Value) -> Vec<Value> {
    if let Some(items) = v.get("items") {
        if let Value::Array(a) = items {
            return a.clone();
        }
    }
    if let Some(Value::Array(a)) = v.get("children") {
        return a.clone();
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
            // `children` 由 emit_container 负责（保序），此处避免重复输出
            if k == "__type" || k == "__name" || k == "children" {
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
