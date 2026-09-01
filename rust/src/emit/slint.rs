// SPDX-License-Identifier: MulanPSL-2.0
//! Slint DSL 转译后端 (`emit-slint`)
//!
//! 把 SML 声明层翻译为 [Slint](https://slint.dev) 组件树。这是「声明层」映射，
//! 不生成完整应用骨架；事件回调（handler）按用户约定**留空**或**嵌入 raw 代码**。
//!
//! 约定：
//! - `__type = "component"` + `__name` → `component Name inherits <Base> { }`
//!   （`inherits` 字段指定基类，默认 `Window`）；
//! - 其它 `__type` 即 Slint 元素名（`VerticalLayout`/`Button`/`Text`/...）；
//! - 标量字段 → Slint 属性（`key: value;`）；
//! - 子元素：`children` 数组或内联对象字段递归；
//! - 回调：`on_<ev>`（`on_click`）或 `<ev>`（`clicked`）字段：
//!   - 值为字符串 → 嵌入 raw：`clicked => {{ <raw> }}`；
//!   - 无值/空 → 留空：`clicked => { }`。

use crate::Value;
use crate::emit::{
    EmitOptions, scalar_text, block_type, block_name, sanitize_slint_ident, slint_handler_safe,
    slint_expr_safe, MAX_VALUE_DEPTH,
};

#[derive(Debug, Clone)]
pub struct SlintOptions {
    pub base: EmitOptions,
    /// 组件默认基类（component 无 inherits 时）。默认 `Window`。
    pub default_base: String,
}

impl Default for SlintOptions {
    fn default() -> Self {
        SlintOptions {
            base: EmitOptions::default(),
            default_base: "Window".to_string(),
        }
    }
}

impl SlintOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn to_slint(v: &Value, opt: &SlintOptions) -> Result<String, String> {
    let mut out = String::new();
    if let Value::Object(m) = v {
        for (k, val) in m {
            if k == "__type" || k == "__name" {
                continue;
            }
            emit_slint(val, Some(k), opt, 0, true, &mut out)?;
        }
    } else {
        emit_slint(v, None, opt, 0, true, &mut out)?;
    }
    Ok(out)
}

/// 提取 `` `...` `` 包裹的 raw 表达式（SML 侧约定：反引号内是 Slint 表达式原文）。
///
/// 背景：SML 裸词/引号串一律按「数据」处理，无法表达 Slint 的表达式字面量
/// （颜色 `#1e1e2e`、长度 `16px`、绑定 `root.entry`、调用 `Math.sqrt(2)`）。
/// 用反引号显式标记「原文输出」，可保持 SML 语义纯净（反引号只是普通字符，
/// 不影响词法），同时让转义行为对作者可见。
///
/// 仅当安全校验通过时返回 `Some(inner)`；否则回落到普通字符串字面量。
fn slint_raw_expr(s: &str) -> Option<&str> {
    let inner = s.strip_prefix('`')?.strip_suffix('`')?;
    if inner.is_empty() || !slint_expr_safe(inner) {
        None
    } else {
        Some(inner)
    }
}

/// Slint 字符串字面量转义：`\` 与 `"` 之外还必须转义换行 / 回车 / 制表符。
///
/// 值是**数据**，不是代码：`"` 已转义，故不构成注入（无法提前闭合字面量）；
/// 但未转义的字面换行会产出 Slint 无法编译的跨行字符串（安全审计 P3-2）。
fn slint_escape_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            _ => o.push(c),
        }
    }
    o
}

fn slint_value_str(v: &Value, _opt: &SlintOptions) -> String {
    // Slint 字符串字面量：用双引号包裹，转义反斜杠与双引号（而非 XML 实体）。
    // 特例：`` `expr` `` 是 raw 表达式标记，原样输出（不加引号）。
    if let Value::Str(s) = v {
        if let Some(raw) = slint_raw_expr(s) {
            return raw.to_string();
        }
    }
    match v {
        Value::Str(s) => format!("\"{}\"", slint_escape_str(s)),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(_) => scalar_text(v),
        Value::Null => "null".to_string(),
        other => format!("\"{}\"", slint_escape_str(&scalar_text(other))),
    }
}

fn emit_slint(
    v: &Value,
    inferred: Option<&str>,
    opt: &SlintOptions,
    depth: usize,
    _is_root: bool,
    out: &mut String,
) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("slint: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    // 同名重复元素（SML 把 `Button { } Button { }` 合并为该键下的数组）
    // 展开为同级元素序列，保持书写顺序。
    if let Value::Array(a) = v {
        for item in a {
            emit_slint(item, inferred, opt, depth, _is_root, out)?;
        }
        return Ok(());
    }
    let pad = " ".repeat(depth * opt.base.indent);
    if let Value::Object(_) = v {
        let ty = sanitize_slint_ident(block_type(v).or(inferred).unwrap_or("VerticalLayout"));
        let name = block_name(v).map(sanitize_slint_ident);

        if ty == "component" {
            let base = sanitize_slint_ident(
                v.get("inherits")
                    .and_then(|x| x.as_str())
                    .unwrap_or(&opt.default_base),
            );
            let cname = sanitize_slint_ident(
                v.get("name")
                    .and_then(|x| x.as_str())
                    .or(name.as_deref())
                    .unwrap_or("Main"),
            );
            // 仅 Window 基类组件需要 export（作为 UI 入口）；子组件是内部复用，
            // 导出反而会触发 "doesn't inherit Window" 的 Slint 警告。
            let kw = if base == "Window" { "export component" } else { "component" };
            out.push_str(&format!("{} {} inherits {} {{\n", kw, cname, base));
            emit_slint_body(v, opt, depth + 1, out)?;
            out.push_str(&format!("}}\n"));
            return Ok(());
        }

        // Slint 声明（`property` / `callback` / `function`）：输出 DSL 声明语句，
        // 而不是当作子元素渲染。由 `__type`（即 SML 裸块名）区分。
        if let Some(kind) = SlintDecl::from_type(&ty) {
            emit_slint_decl(kind, v, name.as_deref(), opt, depth, out)?;
            return Ok(());
        }

        // 普通元素。Slint 命名元素语法是 `id := Element { }`，故名字在前。
        let header = match &name {
            Some(n) => format!("{}{} := {}", pad, n, ty),
            None => format!("{}{}", pad, ty),
        };
        out.push_str(&format!("{} {{\n", header));
        emit_slint_body(v, opt, depth + 1, out)?;
        out.push_str(&format!("{}}}\n", pad));
        Ok(())
    } else {
        out.push_str(&format!("{}{}\n", pad, slint_value_str(v, opt)));
        Ok(())
    }
}

/// Slint 声明种类。由 SML 裸块类型名（`__type`）选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlintDecl {
    /// `in-out property <T> name: default;`
    Property,
    /// `callback name(args) -> ret;`
    Callback,
    /// `function name(args) -> ret { ... }`
    Function,
}

impl SlintDecl {
    fn from_type(ty: &str) -> Option<Self> {
        match ty {
            "property" => Some(Self::Property),
            "callback" => Some(Self::Callback),
            "function" => Some(Self::Function),
            _ => None,
        }
    }
}

/// 声明体的参数列表白名单清洗：保留标识符字符、`,`、`:` 与空格
/// （Slint 形参写作 `name: type`），其余字符一律剔除，
/// 避免把任意 token 注入到 Slint 的函数签名里。
fn sanitize_slint_args(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | ',' | ' ' | ':'))
        .collect()
}

/// 类型白名单清洗：仅保留标识符字符与 `<` `>`（如 `length`、`duration`）。
fn sanitize_slint_type(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '<' | '>' | '[' | ']'))
        .collect()
}

fn emit_slint_decl(
    kind: SlintDecl,
    v: &Value,
    name: Option<&str>,
    opt: &SlintOptions,
    depth: usize,
    out: &mut String,
) -> Result<(), String> {
    let pad = " ".repeat(depth * opt.base.indent);
    let n = sanitize_slint_ident(name.unwrap_or("unnamed"));
    let args = sanitize_slint_args(v.get("args").and_then(|x| x.as_str()).unwrap_or(""));
    let ret = match v.get("returns").and_then(|x| x.as_str()) {
        Some(t) if !t.is_empty() => format!(" -> {}", sanitize_slint_type(t)),
        _ => String::new(),
    };
    match kind {
        SlintDecl::Property => {
            let ty = sanitize_slint_type(v.get("type").and_then(|x| x.as_str()).unwrap_or("string"));
            if ty.is_empty() {
                return Err("slint: property 声明缺少合法类型".to_string());
            }
            // access: in / out / in-out / private（默认 in-out，便于宿主语言读写）
            let prefix = match v.get("access").and_then(|x| x.as_str()).unwrap_or("in-out") {
                "in" => "in ",
                "out" => "out ",
                "in-out" => "in-out ",
                _ => "",
            };
            let default = match v.get("default") {
                Some(d) => format!(": {}", slint_value_str(d, opt)),
                None => String::new(),
            };
            out.push_str(&format!(
                "{}{}property <{}> {}{};\n",
                pad, prefix, ty, n, default
            ));
        }
        SlintDecl::Callback => {
            out.push_str(&format!("{}callback {}({}){};\n", pad, n, args, ret));
        }
        SlintDecl::Function => {
            let code = v.get("code").and_then(|x| x.as_str()).unwrap_or("");
            out.push_str(&format!("{}function {}({}){} {{\n", pad, n, args, ret));
            // 与事件处理器一致：源码必须花括号配平，否则丢弃，防止逃逸出函数体
            if !code.is_empty() && slint_handler_safe(code) {
                render_slint_code(code, opt, depth + 1, out);
            }
            out.push_str(&format!("{}}}\n", pad));
        }
    }
    Ok(())
}

/// 渲染对象体：属性 + 子元素 + 回调。
fn emit_slint_body(v: &Value, opt: &SlintOptions, depth: usize, out: &mut String) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("slint: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let pad = " ".repeat(depth * opt.base.indent);
    if let Value::Object(m) = v {
        let mut children: Vec<(Option<String>, &Value)> = Vec::new();
        // 第一遍：属性 + 回调 + 收集子元素
        for (k, val) in m {
            match k.as_str() {
                // `name` 是 component 的元数据（组件名），不能当作属性输出
                // （Slint 组件/元素都没有 `name` 属性，输出会导致编译失败）。
                "__type" | "__name" | "__type_meta" | "inherits" | "name" | "children" => {
                    if k == "children" {
                        if let Value::Array(a) = val {
                            for item in a.iter() {
                                children.push((None, item));
                            }
                        }
                    }
                }
                _ if k.starts_with("on_") => {
                    let ev = &k[3..];
                    emit_callback(ev, val, opt, depth, out)?;
                }
                _ if is_callback_name(k) => {
                    emit_callback(k, val, opt, depth, out)?;
                }
                _ => match val {
                    Value::Object(_) => children.push((Some(k.clone()), val)),
                    Value::Array(a) => {
                        if a.iter().all(|x| matches!(x, Value::Object(_))) {
                            // 同名重复元素：逐个展开为同级元素
                            for item in a {
                                children.push((Some(k.clone()), item));
                            }
                        } else {
                            // 标量数组：渲染为 Slint 数组字面量属性（如 model 数据源）
                            let items: Vec<String> =
                                a.iter().map(|x| slint_value_str(x, opt)).collect();
                            out.push_str(&format!(
                                "{}{}: [{}];\n",
                                pad,
                                sanitize_slint_ident(k),
                                items.join(", ")
                            ));
                        }
                    }
                    _ => {
                        // 属性（序列化为 Slint 字面量）；属性名做标识符白名单清洗
                        out.push_str(&format!("{}{}: {};\n", pad, sanitize_slint_ident(k), slint_value_str(val, opt)));
                    }
                },
            }
        }
        // 第二遍：子元素
        for (ck, child) in &children {
            emit_slint(child, ck.as_deref(), opt, depth, false, out)?;
        }
    }
    Ok(())
}

fn is_callback_name(k: &str) -> bool {
    // Slint 常见事件名
    matches!(k, "clicked" | "toggled" | "pressed" | "released" | "moved" | "touch"
        | "focus" | "key-pressed" | "key-released" | "return-pressed" | "activated"
        | "current-item-changed" | "value-changed" | "text-changed" | "pointer-event")
}

fn emit_callback(ev: &str, val: &Value, opt: &SlintOptions, depth: usize, out: &mut String) -> Result<(), String> {
    if depth > MAX_VALUE_DEPTH {
        return Err(format!("slint: 递归深度超过上限 {}", MAX_VALUE_DEPTH));
    }
    let pad = " ".repeat(depth * opt.base.indent);
    // 把常见事件名映射到 Slint 原生回调名，并做标识符白名单清洗
    let ev = match ev {
        "click" => "clicked".to_string(),
        "toggle" => "toggled".to_string(),
        other => sanitize_slint_ident(other),
    };
    let raw = match val {
        Value::Str(s) if !s.is_empty() => s.clone(),
        Value::Str(_) => String::new(),
        Value::Object(o) => o.get("raw").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        _ => scalar_text(val),
    };
    if raw.is_empty() || !slint_handler_safe(&raw) {
        // 空 handler，或包含会逃逸出闭包的花括号/非法结构：留空，避免注入任意逻辑
        out.push_str(&format!("{}{} => {{ }}\n", pad, ev));
    } else {
        out.push_str(&format!("{}{} => {{\n", pad, ev));
        render_slint_code(&raw, opt, depth + 1, out);
        out.push_str(&format!("{}}}\n", pad));
    }
    Ok(())
}

/// 去掉代码块各行共同的前导空白（保留相对缩进），便于嵌入到任意层级。
fn dedent(code: &str) -> String {
    let lines: Vec<&str> = code.lines().collect();
    let common = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| if l.len() >= common { &l[common..] } else { *l })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 渲染一段 Slint 语句（回调体 / 函数体）：去共同缩进后按 `depth` 重新缩进，
/// 并为未终结的语句行补 `;` —— Slint 要求语句以分号结尾，而 SML 侧的值
/// 通常是裸调用（如 `root.press("7")`），省略分号更符合书写直觉。
fn render_slint_code(code: &str, opt: &SlintOptions, depth: usize, out: &mut String) {
    let pad = " ".repeat(depth * opt.base.indent);
    for line in dedent(code).lines() {
        let t = line.trim_end();
        if t.is_empty() {
            out.push('\n');
            continue;
        }
        let semi = if t.ends_with(';') || t.ends_with('{') || t.ends_with('}') {
            ""
        } else {
            ";"
        };
        out.push_str(&format!("{}{}{}\n", pad, t, semi));
    }
}
