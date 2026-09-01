// SPDX-License-Identifier: MulanPSL-2.0
//! 用户自定义生成器后端 (`emit-custom`)
//!
//! 让用户用一份 SML 文档描述「规则表 + 模板」，把任意 SML 值转译为任意文本。
//!
//! # 生成器文档约定
//!
//! ```sml
//! rules: [
//!   { match: "h1"       template: "# {value}\n" }
//!   { match: "p"        template: "{value}\n\n" }
//!   { match: "*"        template: "{key}: {value}\n" }
//! ]
//! ```
//!
//! - `match`：要匹配的块类型（`__type`）；`"*"` 表示兜底规则；
//!   也支持 `match-key: "name"`（匹配字段名而非类型）。
//! - `template`：含占位符的模板字符串。
//!
//! # 占位符
//!
//! | 占位符          | 含义                                        |
//! |-----------------|---------------------------------------------|
//! | `{value}`       | 当前节点文本（标量/块主文本）               |
//! | `{raw}`         | 当前节点的原始 SML 序列化（需 `sml` feature）|
//! | `{key}`         | 当前字段名（对象遍历时）                    |
//! | `{item}`        | 数组元素文本（配合 `items` 循环）           |
//! | `{nested}`      | 递归渲染子节点                              |
//! | `{items:TPL}`   | 对数组每个元素套用子模板 TPL               |
//!
//! 安全限制：递归深度上限 `MAX_VALUE_DEPTH`(128)，单节点循环上限 100000 元素；
//! 规则解析失败返回 `Err(String)`。

use crate::Value;
use crate::emit::{EmitOptions, scalar_text, block_type, MAX_VALUE_DEPTH};
use std::collections::HashSet;

#[cfg(feature = "sml")]
use crate::core::to_sml;

const MAX_LOOP: usize = 100_000;

/// 输出总长度上限（字节）。
///
/// `{nested}` 可在同一模板里出现多次，因此每层递归都会把子树输出放大 m 倍；
/// 配合 128 层深度上限，一个不到 1KB 的深层嵌套文档就能产出 m^128 字节，
/// 直接把宿主 OOM。仅限制「深度」和「单层元素数」都挡不住这种放大，
/// 故额外对**累计输出长度**设上限。
const MAX_OUTPUT: usize = 8 * 1024 * 1024;

/// 带上限地追加文本：超过 [`MAX_OUTPUT`] 返回 Err（而非继续吃内存）。
fn push_capped(out: &mut String, s: &str) -> Result<(), String> {
    if out.len().saturating_add(s.len()) > MAX_OUTPUT {
        return Err(format!("custom: 输出超过长度上限 {} 字节（模板存在放大，请检查 `{{nested}}` 是否重复出现）", MAX_OUTPUT));
    }
    out.push_str(s);
    Ok(())
}

/// 单条规则。
#[derive(Debug, Clone)]
pub struct CustomRule {
    /// 要匹配的块类型；`None` 表示匹配任意类型（但优先级最低）。
    pub match_type: Option<String>,
    /// 要匹配的字段名（`match-key`）；与 `match_type` 二选一。
    pub match_key: Option<String>,
    /// 模板字符串。
    pub template: String,
}

/// 自定义生成器选项。
#[derive(Debug, Clone)]
pub struct CustomOptions {
    pub base: EmitOptions,
    pub rules: Vec<CustomRule>,
    /// 渲染顶层字段时跳过的字段名（凭据过滤，如 password/token/secret）。
    pub exclude: HashSet<String>,
    /// 若设置，仅渲染该集合内的顶层字段（白名单优先于 `exclude`）。
    pub include_only: Option<HashSet<String>>,
}

impl CustomOptions {
    pub fn new() -> Self {
        CustomOptions {
            base: EmitOptions::default(),
            rules: Vec::new(),
            exclude: HashSet::new(),
            include_only: None,
        }
    }
    /// 设置需排除的字段名（如 `password`, `token`, `secret`）。
    pub fn exclude_fields(mut self, names: &[&str]) -> Self {
        self.exclude = names.iter().map(|s| s.to_string()).collect();
        self
    }
    /// 设置仅包含的字段名白名单。
    pub fn include_fields(mut self, names: &[&str]) -> Self {
        self.include_only = Some(names.iter().map(|s| s.to_string()).collect());
        self
    }
    /// 从生成器文档（SML 解析出的 Value）构建规则表。
    pub fn from_generator(gen: &Value) -> Result<Self, String> {
        let mut opt = CustomOptions::new();
        let rules = gen.get("rules").and_then(|x| match x {
            Value::Array(a) => Some(a.clone()),
            _ => None,
        }).ok_or_else(|| "custom 生成器缺少 rules 数组".to_string())?;

        for (i, r) in rules.iter().enumerate() {
            let template = r.get("template").and_then(|x| x.as_str())
                .ok_or_else(|| format!("规则 #{} 缺少 template 字符串", i))?;
            let match_type = r.get("match").and_then(|x| x.as_str()).map(|s| s.to_string());
            let match_key = r.get("match-key").and_then(|x| x.as_str()).map(|s| s.to_string());
            opt.rules.push(CustomRule {
                match_type,
                match_key,
                template: template.to_string(),
            });
        }
        if opt.rules.is_empty() {
            return Err("custom 生成器 rules 为空".to_string());
        }
        Ok(opt)
    }
}

/// 字段是否允许渲染（考虑 include_only 白名单与 exclude 黑名单）。
fn field_allowed(k: &str, opt: &CustomOptions) -> bool {
    if let Some(inc) = &opt.include_only {
        return inc.contains(k);
    }
    !opt.exclude.contains(k)
}

/// 应用自定义规则把 SML 值转译为文本。
pub fn to_custom(v: &Value, opt: &CustomOptions) -> Result<String, String> {
    let mut out = String::new();
    if let Value::Object(m) = v {
        // 1) 优先按 rules 的出现顺序输出，保证顶层字段有确定性顺序
        //    （例如 Dockerfile 要求 FROM 在最前）。未被命中的字段再补渲。
        //    select_rule 会用字段名（key）匹配 match_type，因此这里把
        //    match_type 也当作字段名来定位，使 `match: "base"` 既能匹配又能定序。
        let mut emitted: HashSet<String> = HashSet::new();
        for r in &opt.rules {
            for cand in [r.match_key.as_ref(), r.match_type.as_ref()]
                .into_iter()
                .flatten()
            {
                if let Some(val) = m.get(cand) {
                    if field_allowed(cand, opt) {
                        render(val, Some(cand), opt, 0, 0, &mut out)?;
                        emitted.insert(cand.clone());
                    }
                }
            }
        }
        // 2) 其余未命中字段按原顺序补上
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            if emitted.contains(k) || !field_allowed(k, opt) {
                continue;
            }
            render(val, Some(k), opt, 0, 0, &mut out)?;
        }
    } else {
        render(v, None, opt, 0, 0, &mut out)?;
    }
    Ok(out)
}

fn render(
    v: &Value,
    key: Option<&str>,
    opt: &CustomOptions,
    depth: usize,
    _loop: usize,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("custom: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let rule = match select_rule(v, key, opt) {
        Some(r) => r,
        None => return Ok(()), // 无匹配规则（空 rules 等）：不输出，避免 panic
    };

    // 先渲染子节点（若有），收集到 {nested} / {items:...} 占位符替换用
    let mut nested = String::new();
    let mut item_loops: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    match v {
        Value::Object(m) => {
            for (k, val) in m {
                if k == "__type" || k == "__name" {
                    continue;
                }
                if !field_allowed(k, opt) {
                    continue;
                }
                // 普通子节点 → 递归进 nested
                // 注意：数组字段作为独立值渲染时由 Array 分支处理 {items:}，
                // 这里仅对对象内的嵌套对象/标量做递归（供 {nested} 使用）。
                if !matches!(val, Value::Array(_)) {
                    let mut sub = String::new();
                    render(val, Some(k), opt, depth + 1, 0, &mut sub)?;
                    push_capped(&mut nested, &sub)?;
                }
            }
        }
        Value::Array(a) => {
            if a.len() > MAX_LOOP {
                return Err(format!("custom: 数组超过循环上限 {}", MAX_LOOP));
            }
            // 数组本身若被规则模板用 {items:TPL} 描述，则按循环渲染
            for tpl_key in collect_item_templates(&rule.template) {
                let mut buf = String::new();
                for item in a {
                    push_capped(&mut buf, &render_item(item, &tpl_key, opt, depth + 1)?)?;
                }
                item_loops.insert(tpl_key, buf);
            }
            // 其余情况（无 {items:} 模板）按元素递归进 nested
            if item_loops.is_empty() {
                for item in a {
                    let mut sub = String::new();
                    render(item, key, opt, depth + 1, 0, &mut sub)?;
                    push_capped(&mut nested, &sub)?;
                }
            }
        }
        _ => {}
    }

    // 值文本：对象优先取 text 字段（如 h1 { text: "..." }），否则取标量
    let value_text = match v {
        // `text` 本身也是一个字段名，必须同样过 exclude / include_only 白名单：
        // 子节点遍历处已过滤 text，若这里直接取出来填进 `{value}`，
        // 则 `exclude: ["text"]` 形同虚设（安全审计 P3-3）。
        Value::Object(_) => match v.get("text") {
            Some(t) if field_allowed("text", opt) => scalar_text(t),
            _ => scalar_text(v),
        },
        _ => scalar_text(v),
    };
    let key_text = key.unwrap_or("").to_string();

    // 单遍占位符替换：先把所有占位符收集齐，统一替换，避免已插入内容里的
    // 占位符（如 value="{key}"）被二次展开。需按占位符长度降序，避免 {items:...} 被 {item} 截断。
    let mut subs: Vec<(String, String)> = Vec::new();
    subs.push(("{nested}".to_string(), nested));
    subs.push(("{raw}".to_string(), raw_sml(v)));
    subs.push(("{value}".to_string(), value_text.clone()));
    subs.push(("{key}".to_string(), key_text));
    subs.push(("{item}".to_string(), value_text));
    for (tpl, rendered) in &item_loops {
        subs.push((format!("{{items:{}}}", tpl), rendered.clone()));
    }
    // 长占位符优先替换
    subs.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
    let result = replace_all_once(&rule.template, &subs);

    push_capped(out, &result)
}

/// 单遍替换：扫描输入，命中任一占位符即写入对应替换值，绝不回头重新扫描已写入内容。
fn replace_all_once(template: &str, subs: &[(String, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let mut matched: Option<&String> = None;
            let mut matched_val: Option<&String> = None;
            for (pat, val) in subs {
                if template[i..].starts_with(pat.as_str()) {
                    matched = Some(pat);
                    matched_val = Some(val);
                    break;
                }
            }
            if let (Some(pat), Some(val)) = (matched, matched_val) {
                out.push_str(val);
                i += pat.len();
                continue;
            }
        }
        // 拷贝单个字符（处理 UTF-8）
        let ch = template[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// 渲染数组元素的子模板（{items:TPL} 中的 TPL 部分）。
fn render_item(item: &Value, tpl: &str, opt: &CustomOptions, depth: usize) -> Result<String, String> {
    let item_text = scalar_text(item);
    // 嵌套的 {nested} 对数组元素也支持
    let mut nested = String::new();
    if let Value::Object(_) | Value::Array(_) = item {
        render(item, None, opt, depth + 1, 0, &mut nested)?;
    }
    // 单遍替换，避免 {value}="{item}" 之类二次展开
    let subs: Vec<(String, String)> = vec![
        ("{nested}".to_string(), nested),
        ("{raw}".to_string(), raw_sml(item)),
        ("{value}".to_string(), item_text.clone()),
        ("{item}".to_string(), item_text),
    ];
    Ok(replace_all_once(tpl, &subs))
}

/// 从模板中收集 `{items:...}` 的子模板名（即 `...` 部分）。
/// 使用大括号配对，因此子模板内部可以包含 `{value}`/`{item}` 等带花括号的占位符。
fn collect_item_templates(tpl: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = tpl.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"{items:") {
            // 括号配对：从 `{items:` 之后开始，depth 计数直到匹配的外层 `}`
            let mut depth = 0usize;
            let mut j = i + 7; // 跳过 "{items:"
            let mut found = None;
            while j < bytes.len() {
                match bytes[j] {
                    b'{' => depth += 1,
                    b'}' => {
                        if depth == 0 {
                            found = Some(j);
                            break;
                        }
                        depth -= 1;
                    }
                    _ => {}
                }
                j += 1;
            }
            if let Some(end) = found {
                let inner = &tpl[i + 7..end];
                out.push(inner.to_string());
                i = end + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn select_rule<'a>(v: &Value, key: Option<&str>, opt: &'a CustomOptions) -> Option<&'a CustomRule> {
    let ty = block_type(v);
    // 优先：__type 或 字段名（key）精确匹配 match_type
    if let Some(t) = ty.or(key) {
        for r in &opt.rules {
            if r.match_type.as_deref() == Some(t) {
                return Some(r);
            }
        }
    }
    // 其次：match_key 精确匹配字段名
    if let Some(k) = key {
        for r in &opt.rules {
            if r.match_key.as_deref() == Some(k) {
                return Some(r);
            }
        }
    }
    // 兜底 "*"
    for r in &opt.rules {
        if r.match_type.as_deref() == Some("*") {
            return Some(r);
        }
    }
    // 实在没有且没有规则可兜底：返回 None（调用方据此跳过，避免越界 panic）
    opt.rules.first()
}

#[cfg(feature = "sml")]
fn raw_sml(v: &Value) -> String {
    to_sml(v)
}
#[cfg(not(feature = "sml"))]
fn raw_sml(_v: &Value) -> String {
    String::new()
}
