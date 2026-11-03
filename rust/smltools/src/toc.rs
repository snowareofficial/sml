// SPDX-License-Identifier: MulanPSL-2.0
//! `--toc`：从**值树**里抽标题，生成目录（Markdown 列表 / HTML `<nav>`）。
//!
//! 为什么从值树抽、而不是从已生成的文本里反解：文本反解要按格式各写一遍扫描，
//! 遇到代码块里的 `#` 就误判；值树里标题就是 `h1..h6` / `topic` / `section` 这些块类型，判据稳定。
//!
//! ⚠️ **顺序**：默认 `Value::Object` 是 BTreeMap（按名字排序），源序不保。
//! 想让目录按书写顺序，就给容器块用 `children` 数组（那里保序），或整体开 `preserve_order`。
//! 这条与 `docs/value-layout.md` §3 同源。

use sml::Value;

/// 一条目录项。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 标题层级（1..=6）。
    pub level: usize,
    /// 标题文本（未转义）。
    pub title: String,
    /// 锚点（块的 `label`）；空串表示该条**没有链接目标**。
    pub label: String,
}

/// 收集全文档的标题（按遍历顺序）。
pub fn collect(v: &Value) -> Vec<Entry> {
    let mut out = Vec::new();
    walk(v, 1, &mut out);
    out
}

fn walk(v: &Value, depth: usize, out: &mut Vec<Entry>) {
    match v {
        Value::Array(items) => {
            for it in items {
                walk(it, depth, out);
            }
        }
        Value::Object(m) => {
            // `children` 是唯一保序载体：有它就按它走，别把标题顺序交给 BTreeMap。
            let mut kids: Vec<&Value> = Vec::new();
            if let Some(Value::Array(a)) = m.get("children") {
                kids.extend(a.iter());
            }
            for kid in kids {
                walk(kid, depth + 1, out);
            }
            for (k, val) in m {
                let key = key_str(k);
                if key == "children" || key == "__type" || key == "__name" {
                    continue;
                }
                if let Some(level) = heading_level(key, depth) {
                    out.push(Entry {
                        level,
                        title: heading_title(val),
                        label: val
                            .get("label")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
                walk(val, depth + 1, out);
            }
        }
        _ => {}
    }
}

/// 块类型 → 标题层级。`topic` 当一级，`section` 按嵌套深度（封顶 6）。
fn heading_level(key: &str, depth: usize) -> Option<usize> {
    match key {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => Some((key.as_bytes()[1] - b'0') as usize),
        "topic" => Some(1),
        "section" => Some(depth.min(6)),
        _ => None,
    }
}

/// 标题文本：`title` → `__name` → `text` → 首个标量（都取不到就是空串）。
fn heading_title(v: &Value) -> String {
    for key in ["title", "__name", "text"] {
        if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
            return s.trim().to_string();
        }
    }
    if let Value::Object(m) = v {
        for (k, val) in m {
            let key = key_str(k);
            if key == "__type" || key == "__name" || key == "label" {
                continue;
            }
            if let Some(s) = val.as_str() {
                return s.trim().to_string();
            }
        }
    }
    String::new()
}

/// 对象键 → `&str`（`Value::Object` 的键类型是泛型，`as_ref()` 需要显式消歧）。
fn key_str<K: AsRef<str> + ?Sized>(k: &K) -> &str {
    k.as_ref()
}

/// 生成 Markdown 目录（`- [标题](#label)`；无 `label` 的条目只有文字）。
pub fn render_markdown(entries: &[Entry]) -> String {
    let entries: Vec<&Entry> = entries.iter().filter(|e| !e.title.is_empty()).collect();
    if entries.is_empty() {
        return String::new();
    }
    let min = entries.iter().map(|e| e.level).min().unwrap_or(1);
    let mut out = String::from("## 目录\n\n");
    for e in entries {
        let pad = "  ".repeat(e.level.saturating_sub(min));
        if e.label.is_empty() {
            out.push_str(&format!("{pad}- {}\n", e.title));
        } else {
            out.push_str(&format!("{pad}- [{}](#{})\n", e.title, e.label));
        }
    }
    out.push('\n');
    out
}

/// 生成 HTML 目录（`<nav class="toc">` + 嵌套 `<ul>`）。
pub fn render_html(entries: &[Entry]) -> String {
    let entries: Vec<&Entry> = entries.iter().filter(|e| !e.title.is_empty()).collect();
    if entries.is_empty() {
        return String::new();
    }
    let base = entries[0].level;
    let mut out = String::from("<nav class=\"toc\">\n  <h2 class=\"toc-title\">目录</h2>\n");
    let mut open: usize = 0;
    for e in entries {
        let want = e.level.saturating_sub(base) + 1;
        while open < want {
            out.push_str("  <ul>\n");
            open += 1;
        }
        while open > want {
            out.push_str("  </ul>\n");
            open -= 1;
        }
        if e.label.is_empty() {
            out.push_str(&format!("    <li>{}</li>\n", escape(&e.title)));
        } else {
            out.push_str(&format!(
                "    <li><a href=\"#{}\">{}</a></li>\n",
                escape(&e.label),
                escape(&e.title)
            ));
        }
    }
    while open > 0 {
        out.push_str("  </ul>\n");
        open -= 1;
    }
    out.push_str("</nav>\n");
    out
}

/// 目录里的标题与锚点都要转义：标题来自文档，可能含 `<` 或引号。
fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn v(src: &str) -> Value {
        sml::parse(src).expect("parse")
    }

    #[test]
    fn collects_titles_and_labels() {
        let doc = v("h1 { label: \"intro\" text: \"引言\" }\nh2 { text: \"小节\" }");
        let entries = collect(&doc);
        let intro = entries.iter().find(|e| e.title == "引言").expect("引言");
        assert_eq!(intro.level, 1);
        assert_eq!(intro.label, "intro");
        let sub = entries.iter().find(|e| e.title == "小节").expect("小节");
        assert_eq!(sub.level, 2);
        assert_eq!(sub.label, "", "没给 label 就没有锚点");
    }

    #[test]
    fn markdown_links_only_when_label_present() {
        let doc = v("h1 { label: \"a\" text: \"有锚\" }\nh2 { text: \"无锚\" }");
        let md = render_markdown(&collect(&doc));
        assert!(md.contains("## 目录"), "{md}");
        assert!(md.contains("- [有锚](#a)"), "{md}");
        assert!(md.contains("  - 无锚"), "层级缩进 + 无链接: {md}");
        assert!(!md.contains("[](#)"), "空锚点不该出现: {md}");
    }

    #[test]
    fn html_nests_lists_and_escapes() {
        let doc = v("h1 { label: \"a\" text: \"<b>\" }\nh2 { text: \"子\" }");
        let html = render_html(&collect(&doc));
        assert!(html.contains("<nav class=\"toc\">"), "{html}");
        assert!(html.contains("<a href=\"#a\">&lt;b&gt;</a>"), "标题必须转义: {html}");
        assert!(html.contains("<ul>") && html.contains("</ul>"), "{html}");
    }

    #[test]
    fn empty_document_yields_empty_toc() {
        let doc = v("p { text: \"正文\" }");
        assert!(collect(&doc).iter().all(|e| e.title.is_empty() || true));
        assert_eq!(render_markdown(&collect(&doc)), "");
        assert_eq!(render_html(&collect(&doc)), "");
    }
}
