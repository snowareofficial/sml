// SPDX-License-Identifier: MulanPSL-2.0
//! SVG 转译后端 (`emit-svg`)
//!
//! SML → SVG 约定：
//!
//! - 对象 `__type` 即 SVG 元素名（`svg`/`g`/`rect`/`circle`/`path`/`text`/
//!   `line`/`polygon`/`linearGradient` ...）。顶层无类型时默认 `svg`，
//!   并自动补 `xmlns` 与 `viewBox`（可由字段覆盖）。
//! - 标量字段 → 元素属性（如 `x`/`y`/`width`/`fill`/`d`）；
//! - `text` 字段 → 元素文本内容（`<text>hello</text>`）；
//! - `children` 数组或内联对象字段 → 子元素。
//!
//! 例：
//! ```sml
//! svg {
//!   width: 100
//!   height: 100
//!   rect { x: 0 y: 0 width: 50 height: 50 fill: red }
//!   text { x: 10 y: 20 text: "Hi" }
//! }
//! ```

use crate::Value;
use crate::emit::{
    EmitOptions, escape_xml_attr, escape_xml_text, scalar_text, block_type, sanitize_xml_name,
    sanitize_xml_attr_name, sanitize_xml_uri, is_uri_attr, MAX_VALUE_DEPTH,
};

/// 数值属性：仅接受整数/浮点，拒绝任意非数字字符串（防止属性注入）。
/// 返回 `Some(s)` 表示合法数值字符串；`None` 表示该字段不是数字，应跳过。
fn num_attr(v: &Value) -> Option<String> {
    match v {
        Value::Int(i) => Some(i.to_string()),
        Value::Float(f) => Some({
            if *f == f.trunc() && f.is_finite() && f.abs() < 1e15 {
                format!("{:.1}", f)
            } else {
                f.to_string()
            }
        }),
        Value::Str(s) => s.trim().parse::<f64>().ok().map(|_| s.trim().to_string()),
        _ => None,
    }
}

/// 已知为「数值型」的 SVG 属性：这类属性不应接受任意字符串，必须做数值校验，
/// 防止 `x: "1 onload=alert(1)"` 之类的属性注入。其余属性（fill/stroke/class 等）
/// 允许字符串，仅做 XML 转义。
fn is_numeric_svg_attr(k: &str) -> bool {
    matches!(
        k,
        "x" | "y"
            | "width"
            | "height"
            | "cx"
            | "cy"
            | "r"
            | "rx"
            | "ry"
            | "x1"
            | "y1"
            | "x2"
            | "y2"
            | "dx"
            | "dy"
            | "points"
            | "stroke-width"
            | "stroke-dasharray"
            | "opacity"
            | "fill-opacity"
            | "stroke-opacity"
    )
}

#[derive(Debug, Clone)]
pub struct SvgOptions {
    pub base: EmitOptions,
    /// 自动注入 `xmlns="http://www.w3.org/2000/svg"`（顶层 svg 时）。默认 true。
    pub auto_xmlns: bool,
    /// 自动注入 `viewBox`（若顶层未提供）。默认 `0 0 100 100`。
    pub default_view_box: String,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions {
            base: EmitOptions::default(),
            auto_xmlns: true,
            default_view_box: "0 0 100 100".to_string(),
        }
    }
}

impl SvgOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn to_svg(v: &Value, opt: &SvgOptions) -> Result<String, String> {
    let mut out = String::new();
    if opt.base.standalone {
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    }
    if let Value::Object(m) = v {
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            let is_root = k == "svg" || (m.len() == 1);
            emit_svg_node(val, Some(k), "svg", opt, 0, is_root, &mut out)?;
        }
    } else {
        emit_svg_node(v, None, "svg", opt, 0, true, &mut out)?;
    }
    Ok(out)
}

fn emit_svg_node(
    v: &Value,
    inferred: Option<&str>,
    fallback_tag: &str,
    opt: &SvgOptions,
    depth: usize,
    is_root: bool,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("svg: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let pad = " ".repeat(depth * opt.base.indent);
    // SML merges repeated blocks of the same name into an array
    // (e.g. several `circle { }` blocks). Expand them into sibling elements,
    // preserving source order. Without this the array falls through to the
    // scalar branch and the elements are lost (see slint's emit_slint).
    if let Value::Array(a) = v {
        for item in a {
            emit_svg_node(item, inferred, fallback_tag, opt, depth, is_root, out)?;
        }
        return Ok(());
    }
    if let Value::Object(m) = v {
        let tag = sanitize_xml_name(block_type(v).or(inferred).unwrap_or(fallback_tag));
        let mut attrs = String::new();
        if is_root {
            if opt.auto_xmlns && !m.contains_key("xmlns") {
                attrs.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
            }
            // viewBox：优先使用用户提供的 width/height（而非硬编码 100×100）；
            // 若缺字段则回落到 default_view_box。
            if !m.contains_key("viewBox") && opt.auto_xmlns {
                let vb = match (m.get("width"), m.get("height")) {
                    (Some(w), Some(h)) => match (num_attr(w), num_attr(h)) {
                        (Some(ws), Some(hs)) => format!("0 0 {} {}", ws, hs),
                        _ => opt.default_view_box.clone(),
                    },
                    _ => opt.default_view_box.clone(),
                };
                attrs.push_str(&format!(" viewBox=\"{}\"", escape_xml_attr(&vb)));
            }
        }
        let mut children: Vec<(Option<String>, &Value)> = Vec::new();
        let mut text_content: Option<String> = None;

        for (k, val) in m {
            match k.as_str() {
                "__type" => {}
                "__name" => attrs.push_str(&format!(" id=\"{}\"", escape_xml_attr(&scalar_text(val)))),
                "text" => {
                    // 仅当 text 字段为标量时作为元素文本内容；
                    // 若 text 字段是对象/数组，则它是一个 <text> 子元素。
                    match val {
                        Value::Object(_) | Value::Array(_) => children.push((Some(k.clone()), val)),
                        _ => text_content = Some(scalar_text(val)),
                    }
                }
                "children" => {
                    if let Value::Array(a) = val {
                        for item in a.iter() {
                            children.push((None, item));
                        }
                    }
                }
                _ => match val {
                    Value::Object(_) | Value::Array(_) => children.push((Some(k.clone()), val)),
                    _ => {
                        // 事件处理器属性（onload/onclick/…）是执行点，直接丢弃：
                        // 值转义对它们无效。
                        let Some(key) = sanitize_xml_attr_name(k) else {
                            continue;
                        };
                        if is_numeric_svg_attr(k) {
                            // 数值属性：必须真正为数字，否则拒绝（跳过），避免注入
                            if let Some(n) = num_attr(val) {
                                attrs.push_str(&format!(" {}=\"{}\"", key, n));
                            }
                        } else {
                            // 其余属性（fill/stroke/class/href 等）：
                            // URI 类走 scheme 白名单，其余 XML 转义后输出
                            let v = scalar_text(val);
                            let v = if is_uri_attr(&key) {
                                sanitize_xml_uri(&v, true)
                            } else {
                                escape_xml_attr(&v)
                            };
                            attrs.push_str(&format!(" {}=\"{}\"", key, v));
                        }
                    }
                },
            }
        }

        if children.is_empty() && text_content.is_none() {
            out.push_str(&format!("{}<{}{}/>\n", pad, tag, attrs));
        } else if !children.is_empty() {
            out.push_str(&format!("{}<{}{}>\n", pad, tag, attrs));
            for (ck, child) in &children {
                let raw_ctag = ck
                    .clone()
                    .or_else(|| block_type(child).map(|s| s.to_string()))
                    .unwrap_or_else(|| "g".to_string());
                let ctag = sanitize_xml_name(&raw_ctag);
                emit_svg_node(child, Some(&ctag), &ctag, opt, depth + 1, false, out)?;
            }
            out.push_str(&format!("{}</{}>\n", pad, tag));
        } else {
            out.push_str(&format!(
                "{}<{}{}>{}</{}>\n",
                pad,
                tag,
                attrs,
                escape_xml_text(text_content.as_deref().unwrap_or("")),
                tag
            ));
        }
        Ok(())
    } else {
        out.push_str(&format!("{}{}\n", pad, escape_xml_text(&scalar_text(v))));
        Ok(())
    }
}
