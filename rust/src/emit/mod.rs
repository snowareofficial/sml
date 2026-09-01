// SPDX-License-Identifier: MulanPSL-2.0
//! 多目标转译后端 (emit)
//!
//! SML 解析为 [`crate::Value`] 后，可经由本模块转译为其它宿主格式：
//!
//! | feature          | 目标            | 入口函数                         |
//! |------------------|-----------------|----------------------------------|
//! | `emit-markdown`  | Markdown/GFM    | [`to_markdown`]                   |
//! | `emit-latex`     | LaTeX           | [`to_latex`]                      |
//! | `emit-xml`       | XML / LVGL UI   | [`to_xml`] / [`to_lvgl`]          |
//! | `emit-svg`       | SVG             | [`to_svg`]                        |
//! | `emit-slint`     | Slint DSL       | [`to_slint`]                      |
//! | `emit-custom`    | 用户 SML 生成器 | [`to_custom`]                     |
//!
//! 默认全部开启。若只需解析/序列化回 SML，可 `default-features = false`
//! （关闭 `sml` 与所有 `emit-*`），此时本模块整体不参与编译。
//!
//! # SML → 宿主格式的通用约定
//!
//! - **对象/块** 通常映射为宿主的「容器/元素/环境」；
//! - **数组** 映射为「列表/序列/重复元素」；
//! - **字符串标量** 默认**自动转义**（`EmitOptions::escape = true`），
//!   防止注入宿主格式保留字（如 XML 的 `<`、`&`）；
//! - **裸块元数据** `__type` / `__name`（`@type name { }` 解析得来）
//!   被后端用来选择语义，而非当作普通字段输出。

use crate::Value;

/// 递归深度上限。与解析侧 `MAX_VALUE_DEPTH` 保持一致：公开 `Value` 类型
/// 可被不可信输入（如 C-ABI `json_to_value` 无深度限制）构造为任意深嵌套，
/// 任一 emit 后端若不设深度上限会在递归序列化时栈溢出（abort 宿主）。
/// 超过此深度时后端返回 `Err` 而非崩溃。
pub(crate) const MAX_VALUE_DEPTH: usize = 128;

/// 转译选项。各后端可解释其关心的字段，未识别字段忽略。
#[derive(Debug, Clone)]
pub struct EmitOptions {
    /// 标量文本是否转义宿主保留字。默认 `true`。
    pub escape: bool,
    /// 顶层包裹元素/文档头是否生成。默认 `true`（生成完整文档）。
    pub standalone: bool,
    /// 缩进单位（空格数）。默认 2。
    pub indent: usize,
}

impl Default for EmitOptions {
    fn default() -> Self {
        EmitOptions {
            escape: true,
            standalone: true,
            indent: 2,
        }
    }
}

impl EmitOptions {
    pub fn new() -> Self {
        Self::default()
    }
    /// 关闭自动转义（用于信任内容、或手动控制）。
    pub fn no_escape(mut self) -> Self {
        self.escape = false;
        self
    }
    /// 仅生成片段（不生成文档头/包裹根元素）。
    pub fn fragment(mut self) -> Self {
        self.standalone = false;
        self
    }
}

// ---------------------------------------------------------------------------
// 转义工具（后端共享）
// ---------------------------------------------------------------------------

/// XML/HTML 文本转义：`&` `<` `>` `"` `'`。
pub fn escape_xml_text(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            _ => o.push(c),
        }
    }
    o
}

/// XML 属性值转义（在文本转义基础上保证引号安全）。
pub fn escape_xml_attr(s: &str) -> String {
    escape_xml_text(s)
}

/// LaTeX 文本转义：保留字 `# $ % & _ { } ~ ^ \`。
pub fn escape_latex(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\textbackslash{}"),
            '#' => o.push_str("\\#"),
            '$' => o.push_str("\\$"),
            '%' => o.push_str("\\%"),
            '&' => o.push_str("\\&"),
            '_' => o.push_str("\\_"),
            '{' => o.push_str("\\{"),
            '}' => o.push_str("\\}"),
            '~' => o.push_str("\\textasciitilde{}"),
            '^' => o.push_str("\\textasciicircum{}"),
            _ => o.push(c),
        }
    }
    o
}

/// 按选项对一段文本做转义（选择 xml 语义）。
pub fn escape_text(s: &str, opt: &EmitOptions) -> String {
    if opt.escape {
        escape_xml_text(s)
    } else {
        s.to_string()
    }
}

/// XML 标签名 / 属性名白名单：仅允许有限安全字符（字母、数字、`:`、`-`、`.`、`_`）。
/// 任何其它字符（空白、`"`、`=`、`<`、`>` 等）替换为 `_`，从根本上杜绝标签/属性名注入。
pub fn sanitize_xml_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, ':' | '-' | '.' | '_') {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    out
}

/// 事件处理器属性名黑名单（`on*`）。XML/SVG 输出里这些属性是**执行点**：
/// 属性值再怎么转义，`onload="..."` 本身也会在渲染时执行，因此只能从名字层面拒绝。
/// 表内为 HTML/SVG 常见事件名，表外仍按「`on` + 全小写字母」通用规则兜底。
const EVENT_ATTRS: &[&str] = &[
    "onabort", "onactivate", "onafterprint", "onanimationend", "onanimationstart",
    "onbeforeprint", "onbeforeunload", "onbegin", "onblur", "oncancel", "oncanplay",
    "onchange", "onclick", "onclose", "oncontextmenu", "oncopy", "oncuechange", "oncut",
    "ondblclick", "ondrag", "ondragend", "ondragenter", "ondragleave", "ondragover",
    "ondragstart", "ondrop", "ondurationchange", "onended", "onerror", "onfocus",
    "onfocusin", "onfocusout", "onhashchange", "oninput", "oninvalid", "onkeydown",
    "onkeypress", "onkeyup", "onload", "onloadeddata", "onloadedmetadata", "onloadstart",
    "onmessage", "onmousedown", "onmouseenter", "onmouseleave", "onmousemove",
    "onmouseout", "onmouseover", "onmouseup", "onmousewheel", "onoffline", "ononline",
    "onpagehide", "onpageshow", "onpaste", "onpause", "onplay", "onplaying", "onpopstate",
    "onprogress", "onratechange", "onrepeat", "onreset", "onresize", "onscroll", "onsearch",
    "onseeked", "onseeking", "onselect", "onshow", "onstalled", "onstorage", "onsubmit",
    "onsuspend", "ontimeupdate", "ontoggle", "ontouchcancel", "ontouchend", "ontouchmove",
    "ontouchstart", "ontransitionend", "onunload", "onvolumechange", "onwaiting", "onwheel",
];

/// XML/SVG **属性名**安全过滤：先做字符清洗（[`sanitize_xml_name`]），再拒绝
/// 事件处理器属性（`on*`）。返回 `None` 表示该属性必须**整条丢弃**。
///
/// 注意：不能用「剔除非法字符」的方式处理事件名——`onload` 全由合法字符组成，
/// 只有拒绝输出才能阻断 `<svg onload="...">` 这类 XSS。
pub fn sanitize_xml_attr_name(name: &str) -> Option<String> {
    let n = sanitize_xml_name(name);
    let lower = n.to_ascii_lowercase();
    if EVENT_ATTRS.contains(&lower.as_str()) {
        return None;
    }
    // 兜底：`on` + 至少 3 个小写字母（覆盖未列出的自定义/未来事件名）。
    if lower.len() >= 5
        && lower.starts_with("on")
        && lower[2..].chars().all(|c| c.is_ascii_lowercase())
    {
        return None;
    }
    Some(n)
}

/// 值语义为 URI 的属性名：这些属性的值是「可导航地址」，必须过 scheme 白名单，
/// 否则 `href="javascript:..."` / `src="file:///etc/passwd"` 会被渲染器直接执行或读取。
pub fn is_uri_attr(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "href" | "src" | "xlink:href" | "action" | "formaction" | "poster" | "background"
    )
}

/// URI scheme 白名单：只放行相对引用、锚点与少数安全 scheme，
/// 阻断 `javascript:` / `vbscript:` / `data:`(非图片) / `file:` 等可执行或本地读取的
/// URI。任何不在白名单内的 URI 一律清空。
///
/// `allow_data_image` 为真时额外放行 `data:image/*`（内联图片的常见需求）。
pub fn sanitize_xml_uri(uri: &str, allow_data_image: bool) -> String {
    let t = uri.trim();
    // 提取 scheme：`:` 之前的部分，且其中不含 `/` `?` `#` `\`（否则是相对路径里的冒号）
    if let Some(i) = t.find(':') {
        let head = &t[..i];
        let is_scheme = i > 0 && !head.chars().any(|c| matches!(c, '/' | '?' | '#' | '\\'));
        if is_scheme {
            let scheme = head.to_ascii_lowercase();
            let rest = t[i + 1..].trim_start().to_ascii_lowercase();
            let ok = matches!(scheme.as_str(), "http" | "https" | "mailto" | "ftp" | "tel")
                || (allow_data_image && scheme == "data" && rest.starts_with("image/"));
            if !ok {
                // 黑名单之外的一切 scheme（javascript:/vbscript:/file:/…）一律丢弃
                return String::new();
            }
        }
    }
    // 无 scheme = 相对引用 / 锚点 / 绝对路径，直接清洗后放行
    strip_uri_ctrl(t)
}

/// 剔除 URI 中会闭合属性或引入 markup 的字符（空白、引号、`<`、`>`、反引号）。
fn strip_uri_ctrl(uri: &str) -> String {
    uri.chars()
        .filter(|c| {
            !c.is_whitespace()
                && !matches!(c, '"' | '\'' | '<' | '>' | '`' | '(' | ')' | '{' | '}')
        })
        .collect()
}

/// Slint 标识符白名单：字母、数字、`_`、`-`、`$`、`.`、`::` 拆分的合法 token。
/// 非法字符替换为 `_`，避免任意 token 注入到 Slint 源码。
pub fn sanitize_slint_ident(name: &str) -> String {
    // 白名单清洗：仅保留合法标识符字符（字母/数字/下划线/连字符/$），其余一律剔除
    // （含空格、`;`、`.`、`:` 等），避免破坏 Slint 语法结构或被用于注入。
    // 清洗后为空、或首字符为数字时，回落到安全默认名。
    let mut out: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '$'))
        .collect();
    // 首字符必须为字母或下划线（Slint 标识符不能以数字开头）
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// 校验 Slint **表达式**（属性值）是否安全：不得含语句分隔符、块定界符、
/// 换行或注释起始符——属性语句形如 `key: expr;`，含上述字符即可逃逸出该语句
/// 并注入任意 DSL 结构。返回 `true` 表示可原样输出。
pub fn slint_expr_safe(code: &str) -> bool {
    if code.chars().any(|c| matches!(c, ';' | '{' | '}' | '\\' | '\n' | '\r')) {
        return false;
    }
    !(code.contains("//") || code.contains("/*") || code.contains("*/"))
}

/// 校验 Slint 事件处理器代码是否「括号配平且不包含会逃逸出闭包的语句分隔」。
/// 返回 `true` 表示安全（花括号配平、无裸 `}` 后接新语句）。
pub fn slint_handler_safe(code: &str) -> bool {
    let mut depth: i64 = 0;
    let bytes = code.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
                i += 1;
            }
            b'"' => {
                // 跳过字符串字面量，避免其中的 {} 干扰
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' {
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                if i < bytes.len() {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    depth == 0
}

// ---------------------------------------------------------------------------
// 标量辅助
// ---------------------------------------------------------------------------

/// 把标量渲染为宿主格式的「纯文本」表达（不含转义，由调用方决定）。
pub(crate) fn scalar_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            if !f.is_finite() {
                // NaN/inf 没有对应的宿主字面量；输出为可被读回的裸文本标记
                // （避免被当成标识符或非法字面量，破坏 round-trip）。
                if f.is_nan() {
                    "nan".to_string()
                } else if *f < 0.0 {
                    "-inf".to_string()
                } else {
                    "inf".to_string()
                }
            } else if *f == f.trunc() && f.abs() < 1e15 {
                format!("{:.1}", f)
            } else {
                f.to_string()
            }
        }
        Value::Str(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => String::new(),
    }
}

/// 从 `__type` / `__name` 提取裸块元数据（若存在）。
pub(crate) fn block_type(v: &Value) -> Option<&str> {
    v.get("__type").and_then(|x| x.as_str())
}
pub(crate) fn block_name(v: &Value) -> Option<&str> {
    v.get("__name").and_then(|x| x.as_str())
}

// ---------------------------------------------------------------------------
// 后端分派（按需 feature 门控）
// ---------------------------------------------------------------------------

#[cfg(feature = "emit-markdown")]
mod markdown;
#[cfg(feature = "emit-markdown")]
pub use markdown::{to_markdown, MarkdownOptions};

#[cfg(feature = "emit-latex")]
mod latex;
#[cfg(feature = "emit-latex")]
pub use latex::{to_latex, LatexOptions};

#[cfg(feature = "emit-xml")]
mod xml;
#[cfg(feature = "emit-xml")]
pub use xml::{to_xml, to_lvgl, XmlOptions};

#[cfg(feature = "emit-svg")]
mod svg;
#[cfg(feature = "emit-svg")]
pub use svg::{to_svg, SvgOptions};

#[cfg(feature = "emit-slint")]
mod slint;
#[cfg(feature = "emit-slint")]
pub use slint::{to_slint, SlintOptions};

#[cfg(feature = "emit-custom")]
mod custom;
#[cfg(feature = "emit-custom")]
pub use custom::{to_custom, CustomRule, CustomOptions};
