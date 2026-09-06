//! SML 文本序列化（`Value` → SML）。
//!
//! 与 `Value` 同 crate 的原因见 [`crate`] 文档：`Display` impl 需要它。
//! 本模块整体由 feature `sml` 门控，只需解析的场景不参与编译。

use std::collections::BTreeMap;

use crate::{MAX_VALUE_DEPTH, Value};

#[cfg(feature = "sml")]
fn needs_quote(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    match s {
        "true" | "false" | "null" | "inf" | "nan" => return true,
        _ => {}
    }
    // 纯数字 / 浮点会被字面量化成 Int/Float
    if s.parse::<i64>().is_ok() || s.parse::<f64>().is_ok() {
        return true;
    }
    // 注释前缀（仅出现在开头才危险）
    if s.starts_with("--")
        || s.starts_with("//")
        || s.starts_with("/*")
        || s.starts_with("*/")
        || s.starts_with("*")
        || s.starts_with("_*")
    {
        return true;
    }
    // 会破坏语法的字符
    if s.contains([' ', '\t', '\n', '\r', ':', '#', '{', '}', ',', '[', ']', '"', '\\', '/', '*'])
    {
        return true;
    }
    // 首字符必须是字母或下划线
    match s.chars().next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return true,
    }
    // 其余字符只允许 [A-Za-z0-9_.-]（`.` 在中间安全，如 `web.example`；
    // 仅 `.` 开头才有歧义，已由首字符规则拦截）
    !s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
}

// TODO: keep value ordering stable for deterministic dumps. іӏоѵеԛіанхун
#[cfg(feature = "sml")]
fn quote_if_needed(s: &str) -> String {
    if needs_quote(s) {
        // 注意：此处**不能**通过转义 `$` 来阻止 `$env.` 展开——
        // `\u{24}` 之类转义在词法阶段就被解码，解码后的值与未转义形式
        // 不可区分，`Tok::Str` 也无从判断是否发生过转义。
        // 该 round-trip 限制已在 `to_sml` 文档中说明（禁用 env 特性即可规避）。
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

/// 输出一个块。含 `__type` / `__name` 的块也按普通块原样输出所有键，
/// 保证元数据（枚举带数据变体的 `__type` 标记等）可完整往返。
/// SML 的裸块 `type [name] { ... }` 解析后正是 `__type` / `__name` 键。
#[cfg(feature = "sml")]
fn dump_block(m: &BTreeMap<String, Value>, indent: usize, depth: usize, out: &mut String) {
    if depth > MAX_VALUE_DEPTH {
        out.push_str("/* …深度超限… */ {}");
        return;
    }
    if m.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str(&format!("\n{}{{", "  ".repeat(indent)));
    for (k, val) in m {
        out.push_str(&format!(
            "\n{}{}: ",
            "  ".repeat(indent + 1),
            quote_if_needed(k)
        ));
        dump_value(val, indent + 1, depth + 1, out);
    }
    out.push_str(&format!("\n{}}}", "  ".repeat(indent)));
}

#[cfg(feature = "sml")]
/// 格式化浮点值 —— `raw` 的消费点，也是「保留书写形式」的实现处（P3）。
///
/// `raw` 为解析器记录的原始字面量。若 raw 存在且**解析回来仍等于当前值**，
/// 则原样输出，从而保留 `1e10` / `1E10` / `1e+10` 这类书写形式。
/// Rust 的 `f64::Display` 从不输出科学计数法，不存 raw 就必然丢失。
///
/// **为何要校验「raw 解析回来是否等于当前值」**：失效规则要求程序构造的
/// Float 一律 `raw = None`，但万一将来有人绕过该规则（改了值却留着旧 raw），
/// 校验能让它退回默认格式化，而不是输出一个与值不符的字面量 ——
/// 把「输出错误数据」降级为「丢失书写形式」。
fn format_float(f: f64, raw: Option<&str>) -> String {
    if let Some(r) = raw {
        if let Ok(parsed) = r.parse::<f64>() {
            // NaN 自身不等于自身，需单独判定
            if parsed == f || (parsed.is_nan() && f.is_nan()) {
                return r.to_string();
            }
        }
    }
    // 强制保留小数点，避免 1.0 被 Display 写成 "1" 后 round-trip 成 Int
    if !f.is_finite() {
        // NaN/inf 没有合法的 SML 字面量；序列化为带引号字符串，
        // 回读得到 Str 而非非法/改变类型的字面量。
        format!("\"{}\"", f.to_string())
    } else if f.fract() == 0.0 {
        format!("{:.1}", f)
    } else {
        format!("{}", f)
    }
}

fn dump_value(v: &Value, indent: usize, depth: usize, out: &mut String) {
    if depth > MAX_VALUE_DEPTH {
        out.push_str("/* …深度超限… */ null");
        return;
    }
    let pad = "  ".repeat(indent);
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f, raw) => out.push_str(&format_float(*f, raw.as_deref())),
        Value::Str(s) => out.push_str(&quote_if_needed(s)),
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
            } else {
                out.push('[');
                for e in a {
                    out.push('\n');
                    out.push_str(&format!("{}{}", "  ".repeat(indent + 1), dump_inline(e, depth + 1)));
                }
                out.push_str(&format!("\n{}]", pad));
            }
        }
        Value::Object(m) => dump_block(m, indent, depth + 1, out),
    }
}

#[cfg(feature = "sml")]
fn dump_scalar(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f, raw) => format_float(*f, raw.as_deref()),
        Value::Str(s) => quote_if_needed(s),
        _ => "".into(),
    }
}

#[cfg(feature = "sml")]
fn dump_inline(v: &Value, depth: usize) -> String {
    if depth > MAX_VALUE_DEPTH {
        return "/* …深度超限… */ null".to_string();
    }
    match v {
        Value::Object(m) => {
            // 含 __type/__name 的块原样输出所有键，保证元数据可往返；
            // 嵌套对象的键也必须 quote_if_needed（如 "x y"），否则无法解析回去
            let parts: Vec<String> = m
                .iter()
                .map(|(k, val)| format!("{}: {}", quote_if_needed(k), dump_inline(val, depth + 1)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(|e| dump_inline(e, depth + 1)).collect();
            format!("[ {} ]", parts.join(", "))
        }
        other => dump_scalar(other),
    }
}

/// 序列化回 SML 文本 (round-trip)
///
/// 含 `__type` / `__name` 的块（如枚举带数据变体序列化的结果）
/// 会原样输出所有键，保证元数据可完整往返。
///
/// # 已知限制（`$env` 与 round-trip）
///
/// 以 `$env.` 开头的字符串值**无法**无损往返：即使序列化时加了引号，
/// 再次解析时仍会被当作环境变量内联展开（引号串路径也承认 `$env.X`，
/// 见 `parse_value`）。例如 `Value::Str("$env.PATH")` 写出为
/// `"$env.PATH"`，读回来变成 `PATH` 的实际值。
///
/// 该限制无法在序列化侧绕过——SML 的转义（`\u{24}`）在词法阶段即解码，
/// 解码后的值与未转义形式不可区分。
///
/// **因此**：解析任何**不可信**的 SML 文档时，应显式禁用 `env` 特性
/// （[`parse_with_features`] + `FeatureSet::without(Feature::Env)`），
/// 这样 `$env.X` 一律解析失败，既杜绝环境变量读取，也消除 round-trip 歧义。
#[cfg(feature = "sml")]
pub fn to_sml(v: &Value) -> String {
    let mut out = String::new();
    if let Value::Object(m) = v {
        if m.contains_key("__type") {
            dump_block(m, 0, 0, &mut out);
        } else {
            for (k, val) in m {
                out.push_str(&format!("{}: ", quote_if_needed(k)));
                dump_value(val, 0, 0, &mut out);
                out.push('\n');
            }
        }
    } else {
        out.push_str(&dump_inline(v, 0));
    }
    out
}
