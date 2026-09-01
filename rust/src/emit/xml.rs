// SPDX-License-Identifier: MulanPSL-2.0
//! XML 转译后端 (`emit-xml`)，含 LVGL UI 适配。
//!
//! ## 通用 XML (`to_xml`)
//!
//! - 对象 (`Value::Object`) → XML 元素；标签名取 `__type`，否则 `root`；
//!   对象的 `__name` 字段映射为 `name` 属性；其余标量字段映射为属性；
//!   嵌套对象/数组映射为子元素（数组元素重复同一标签，标签取元素自身
//!   `__type` 或父字段名）。
//! - 标量 → 文本节点（自动 XML 转义）。
//!
//! ## LVGL UI (`to_lvgl`)
//!
//! 生成 [LVGL UI 文件](https://docs.lvgl.io/master/overview/xml.html) 风格的 XML：
//!
//! - 根屏幕：块类型 `screen`（或顶层对象） → `<screen name="...">`；
//! - 部件：块类型即 LVGL 部件名（`lv_button`/`lv_label`/`lv_slider`/...）
//!   经 `to_lvgl` 归一为短名（`lv_label` → `label`）；
//! - 父子关系：`children` 数组（或对象内联的部件字段）递归为子元素；
//! - 属性（含 `x`/`y`/`width`/`align`/`text` 等）原样输出为 LVGL 属性；
//! - 事件回调：`on_*` 字段映射为 `<event .../>` 子元素（handler 留空
//!   或嵌入 raw 函数名，符合 Slint/LVGL「handler 留空」约定）。

use crate::Value;
use crate::emit::{
    EmitOptions, escape_xml_attr, escape_xml_text, scalar_text, block_type, block_name,
    sanitize_xml_name, sanitize_xml_attr_name, sanitize_xml_uri, is_uri_attr, MAX_VALUE_DEPTH,
};

/// 写入一条属性：属性名过事件处理器黑名单，URI 类属性值过 scheme 白名单。
/// 返回 false 表示该属性被安全策略丢弃（事件属性）。
fn push_attr(attrs: &mut String, name: &str, value: &str) -> bool {
    let Some(k) = sanitize_xml_attr_name(name) else {
        return false;
    };
    let v = if is_uri_attr(&k) {
        sanitize_xml_uri(value, true)
    } else {
        escape_xml_attr(value)
    };
    attrs.push_str(&format!(" {}=\"{}\"", k, v));
    true
}

/// XML 专属选项。
#[derive(Debug, Clone)]
pub struct XmlOptions {
    pub base: EmitOptions,
    /// 根元素标签名（未指定 `__type` 时）。默认 `"root"`。
    pub root_tag: String,
}

impl Default for XmlOptions {
    fn default() -> Self {
        XmlOptions {
            base: EmitOptions::default(),
            root_tag: "root".to_string(),
        }
    }
}

impl XmlOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// SML → 通用 XML。
pub fn to_xml(v: &Value, opt: &XmlOptions) -> Result<String, String> {
    let mut out = String::new();
    if opt.base.standalone {
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    }
    if let Value::Object(m) = v {
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            emit_node(val, Some(k), &opt.root_tag, opt, 0, &mut out)?;
        }
    } else {
        emit_node(v, None, &opt.root_tag, opt, 0, &mut out)?;
    }
    Ok(out)
}

fn emit_node(
    v: &Value,
    inferred: Option<&str>,
    fallback_tag: &str,
    opt: &XmlOptions,
    depth: usize,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("xml: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let pad = " ".repeat(depth * opt.base.indent);
    match v {
        Value::Object(_) => {
            let tag = sanitize_xml_name(block_type(v).or(inferred).unwrap_or(fallback_tag));
            let mut attrs = String::new();
            // 持有引用而非克隆子树：深层嵌套时递归 clone 会先于深度检查栈溢出
            let mut child_nodes: Vec<(String, &Value)> = Vec::new();
            let mut text_content: Option<String> = None;

            if let Value::Object(m) = v {
                for (k, val) in m {
                    match k.as_str() {
                        "__type" => {}
                        "__name" => {
                            push_attr(&mut attrs, "name", &scalar_text(val));
                        }
                        _ => match val {
                            Value::Object(_) | Value::Array(_) => {
                                child_nodes.push((k.clone(), val));
                            }
                            _ => {
                                // 标量默认作为属性；但若字段名为 text，则作为文本内容
                                if k == "text" {
                                    text_content = Some(scalar_text(val));
                                } else {
                                    push_attr(&mut attrs, k, &scalar_text(val));
                                }
                            }
                        },
                    }
                }
            }

            if child_nodes.is_empty() && text_content.is_none() {
                out.push_str(&format!("{}<{}{}/>\n", pad, tag, attrs));
            } else if !child_nodes.is_empty() {
                out.push_str(&format!("{}<{}{}>\n", pad, tag, attrs));
                for (ck, cv) in &child_nodes {
                    emit_node(cv, Some(ck), ck, opt, depth + 1, out)?;
                }
                if let Some(tc) = &text_content {
                    out.push_str(&format!("{}{}\n", " ".repeat((depth + 1) * opt.base.indent), escape_xml_text(tc)));
                }
                out.push_str(&format!("{}</{}>\n", pad, tag));
            } else {
                out.push_str(&format!(
                    "{}<{}>{}</{}>\n",
                    pad,
                    tag,
                    escape_xml_text(text_content.as_deref().unwrap_or("")),
                    tag
                ));
            }
        }
        Value::Array(a) => {
            for item in a {
                emit_node(item, None, fallback_tag, opt, depth + 1, out)?;
            }
        }
        Value::Str(s) => {
            out.push_str(&format!("{}{}\n", pad, escape_xml_text(s)));
        }
        other => {
            out.push_str(&format!("{}{}\n", pad, scalar_text(other)));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// LVGL UI XML
// ---------------------------------------------------------------------------

/// SML → LVGL UI XML。
///
/// 约定：
/// - 屏幕：`__type = "screen"`（或根对象无类型）→ `<screen name="...">`；
/// - 部件：`__type` 形如 `lv_label`/`lv_button` → 去掉 `lv_` 前缀作标签；
///   也接受已简化名（`label`/`button`）；
/// - 子部件：`children` 数组，或对象内联的「非属性」对象字段；
/// - 属性：除保留键外所有标量字段 → LVGL 属性；
/// - 事件：`on_<event>` 字段（如 `on_click`）→ `<event name="click" handler="..."/>`。
pub fn to_lvgl(v: &Value, opt: &XmlOptions) -> Result<String, String> {
    let mut out = String::new();
    if opt.base.standalone {
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    }
    if let Value::Object(m) = v {
        // 文档对象：遍历字段，字段名即部件/屏幕类型
        let explicit = block_type(v);
        if let Some(t) = explicit {
            let tag = if t == "screen" { "screen" } else { lv_short(t) };
            emit_lvgl_node(v, tag, opt, 0, &mut out)?;
        } else {
            for (k, val) in m {
                if k == "__type" || k == "__name" {
                    continue;
                }
                let tag = lv_short(k);
                emit_lvgl_node(val, tag, opt, 0, &mut out)?;
            }
        }
    } else {
        emit_lvgl_node(v, "screen", opt, 0, &mut out)?;
    }
    Ok(out)
}

fn lv_short(ty: &str) -> &str {
    ty.strip_prefix("lv_").unwrap_or(ty)
}

fn emit_lvgl_node(v: &Value, tag: &str, opt: &XmlOptions, depth: usize, out: &mut String) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("lvgl: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let pad = " ".repeat(depth * opt.base.indent);
    let tag = sanitize_xml_name(tag);
    if let Value::Object(m) = v {
        let name = block_name(v).or_else(|| m.get("id").and_then(|x| x.as_str()));
        let mut attrs = String::new();
        let mut children: Vec<(Option<String>, &Value)> = Vec::new();
        let mut events: Vec<(String, String)> = Vec::new();

        for (k, val) in m {
            match k.as_str() {
                "__type" | "__name" | "id" | "children" => {
                    if k == "children" {
                        if let Value::Array(a) = val {
                            for item in a.iter() {
                                children.push((None, item));
                            }
                        }
                    }
                }
                _ if k.starts_with("on_") => {
                    let ev = sanitize_xml_name(&k[3..]);
                    let handler = scalar_text(val);
                    events.push((ev, handler));
                }
                _ => {
                    if let Value::Object(_) = val {
                        children.push((Some(k.clone()), val));
                    } else if let Value::Array(_) = val {
                        // 数组字段也视为子部件集合（标签取元素 __type，无字段名）
                        if let Value::Array(a) = val {
                            for item in a.iter() {
                                children.push((None, item));
                            }
                        }
                    } else {
                        push_attr(&mut attrs, k, &scalar_text(val));
                    }
                }
            }
        }
        if let Some(n) = name {
            push_attr(&mut attrs, "name", n);
        }

        if children.is_empty() && events.is_empty() {
            out.push_str(&format!("{}<{}{}/>\n", pad, tag, attrs));
        } else {
            out.push_str(&format!("{}<{}{}>\n", pad, tag, attrs));
            for (ck, child) in &children {
                let ctag = match ck.as_deref().map(lv_short) {
                    Some(t) => sanitize_xml_name(t),
                    None => match block_type(child) {
                        Some(t) => sanitize_xml_name(lv_short(t)),
                        None => "obj".to_string(),
                    },
                };
                emit_lvgl_node(child, &ctag, opt, depth + 1, out)?;
            }
            for (ev, handler) in &events {
                out.push_str(&format!("{}  <event name=\"{}\" handler=\"{}\"/>\n", pad, ev, escape_xml_attr(handler)));
            }
            out.push_str(&format!("{}</{}>\n", pad, tag));
        }
        Ok(())
    } else {
        out.push_str(&format!("{}{}\n", pad, escape_xml_text(&scalar_text(v))));
        Ok(())
    }
}
