//! TOML ⇄ `Value`（迁移用）。
//!
//! **支持**
//! - 表 `[a.b]`、表数组 `[[a]]`、点号键 `a.b = 1`
//! - 内联表 `{ k = v }`、数组（可跨行）
//! - 字符串：基本 `".."`（含 `\n` `\t` `\"` `\\` `\uXXXX` `\UXXXXXXXX`）、
//!   字面量 `'..'`、多行 `""".."""` / `'''..'''`
//! - 整数（可带 `_`、正负号、`0x` / `0o` / `0b`）、浮点（含 `inf` / `nan`）、布尔
//! - 日期时间按**字符串**保留（SML 侧裸词日期本来也是字符串，往返一致）
//!
//! **不支持**（报错而不猜）：跨行的 inline table 内嵌套换行、非标准扩展。
//!
//! 序列化的形态选择：标量键写在表头之前（TOML 要求），子表用 `[path]`，
//! 对象数组用 `[[path]]` —— 这是 TOML 表达嵌套的唯一正解。

use std::collections::BTreeMap;

use sml::Value;

/// 解析 TOML 文本。
pub fn parse(text: &str) -> Result<Value, String> {
    let mut root: BTreeMap<String, Value> = BTreeMap::new();
    let mut cur: Vec<String> = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0usize;

    while i < lines.len() {
        let no = i + 1;
        let line = strip_comment(lines[i]).trim().to_string();
        i += 1;
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("[[") {
            let inner = rest.trim_end_matches("]]").trim_end_matches(']').trim();
            let path = parse_key_path(inner)?;
            push_array_table(&mut root, &path, no)?;
            cur = path;
        } else if let Some(rest) = line.strip_prefix('[') {
            let inner = rest.trim_end_matches(']').trim();
            let path = parse_key_path(inner)?;
            ensure_table(&mut root, &path, no)?;
            cur = path;
        } else {
            let (kpart, vpart) = line
                .split_once('=')
                .ok_or_else(|| format!("第 {no} 行：不是 `键 = 值` 形式：`{line}`"))?;
            let keys = parse_key_path(kpart.trim())?;
            // 值可能跨行：数组/内联表按括号平衡判断，多行字符串按 `"""` 是否闭合判断。
            // 续行用 `\n` 连接（不能是空格）—— 多行字符串的换行是内容的一部分。
            let mut vtext = vpart.trim().to_string();
            while !value_complete(&vtext) && i < lines.len() {
                vtext.push('\n');
                vtext.push_str(strip_comment(lines[i]).trim_end());
                i += 1;
            }
            let val = parse_value(&vtext, no)?;
            insert_keys(&mut root, &cur, &keys, val, no)?;
        }
    }
    Ok(Value::Object(root))
}

/// 把 `Value` 序列化为 TOML 文本。顶层须为对象。
pub fn to_toml(v: &Value) -> String {
    let Value::Object(m) = v else {
        // TOML 顶层只能是表；非对象时给出空文档而不是编造语法
        return String::new();
    };
    let mut out = String::new();
    write_table(&mut out, m, &[]);
    // 去掉末尾多余空行
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}

fn write_table(out: &mut String, m: &BTreeMap<String, Value>, path: &[String]) {
    // TOML 要求：先写标量键，再写子表/表数组
    let mut scalars: Vec<(&String, &Value)> = Vec::new();
    let mut tables: Vec<(&String, &BTreeMap<String, Value>)> = Vec::new();
    let mut arrays: Vec<(&String, &Vec<Value>)> = Vec::new();
    for (k, v) in m {
        match v {
            Value::Object(sub) => tables.push((k, sub)),
            Value::Array(a) if a.iter().all(|x| matches!(x, Value::Object(_))) && !a.is_empty() => {
                arrays.push((k, a))
            }
            _ => scalars.push((k, v)),
        }
    }
    for (k, v) in &scalars {
        out.push_str(&format!("{} = {}\n", key_repr(k), value_repr(v)));
    }
    for (k, sub) in &tables {
        let mut p = path.to_vec();
        p.push((*k).clone());
        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(&format!("[{}]\n", path_repr(&p)));
        write_table(out, sub, &p);
    }
    for (k, a) in &arrays {
        let mut p = path.to_vec();
        p.push((*k).clone());
        for item in a.iter() {
            if let Value::Object(sub) = item {
                if !out.is_empty() && !out.ends_with("\n\n") {
                    out.push('\n');
                }
                out.push_str(&format!("[[{}]]\n", path_repr(&p)));
                write_table(out, sub, &p);
            }
        }
    }
}

fn path_repr(p: &[String]) -> String {
    p.iter().map(|s| key_repr(s)).collect::<Vec<_>>().join(".")
}

/// 键名：只含 `A-Za-z0-9_-` 时用裸键，否则加引号。
fn key_repr(k: &str) -> String {
    if !k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        k.to_string()
    } else {
        format!("\"{}\"", escape_basic(k))
    }
}

fn value_repr(v: &Value) -> String {
    match v {
        Value::Null => "\"\"".to_string(), // TOML 无 null，退化为空串
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f, _) => {
            if f.is_nan() {
                "nan".to_string()
            } else if f.is_infinite() {
                if *f > 0.0 { "inf".into() } else { "-inf".into() }
            } else if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{f:.1}") // 保持浮点形态（1.0 而不是 1）
            } else {
                format!("{f}")
            }
        }
        Value::Str(s) => format!("\"{}\"", escape_basic(s)),
        Value::Array(a) => {
            let items: Vec<String> = a.iter().map(value_repr).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(m) => {
            let items: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{} = {}", key_repr(k), value_repr(v)))
                .collect();
            format!("{{ {} }}", items.join(", "))
        }
    }
}

fn escape_basic(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// 去注释：`#` 不在字符串内才算注释。
fn strip_comment(s: &str) -> &str {
    let b = s.as_bytes();
    let (mut in_s, mut in_d) = (false, false);
    for i in 0..b.len() {
        match b[i] {
            b'"' if !in_s => in_d = !in_d,
            b'\'' if !in_d => in_s = !in_s,
            b'#' if !in_s && !in_d => return &s[..i],
            _ => {}
        }
    }
    s
}

/// 值是否已完整（无需继续读下一行）。
///
/// 多行字符串 `"""` / `'''` 的闭合不能用括号平衡判断，必须单独看结尾引号；
/// 否则 `b = """` 会被当成一个空字符串，其后的正文行会被误认为新的键值对。
fn value_complete(s: &str) -> bool {
    let t = s.trim_start();
    if let Some(rest) = t.strip_prefix("\"\"\"") {
        return rest.contains("\"\"\"");
    }
    if let Some(rest) = t.strip_prefix("'''") {
        return rest.contains("'''");
    }
    balanced(s)
}

/// 括号是否平衡（用于判断值是否跨行）。
fn balanced(s: &str) -> bool {
    let mut depth = 0i32;
    let (mut in_s, mut in_d) = (false, false);
    for c in s.chars() {
        match c {
            '"' if !in_s => in_d = !in_d,
            '\'' if !in_d => in_s = !in_s,
            '[' | '{' if !in_s && !in_d => depth += 1,
            ']' | '}' if !in_s && !in_d => depth -= 1,
            _ => {}
        }
    }
    depth <= 0
}

/// 解析键路径：`a.b`、`"a b".c`、`'x'`。
fn parse_key_path(s: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let (mut in_s, mut in_d) = (false, false);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if !in_s => {
                in_d = !in_d;
                if !in_d {
                    out.push(std::mem::take(&mut cur));
                }
            }
            '\'' if !in_d => {
                in_s = !in_s;
                if !in_s {
                    out.push(std::mem::take(&mut cur));
                }
            }
            '.' if !in_s && !in_d => {
                if !cur.trim().is_empty() {
                    out.push(cur.trim().to_string());
                }
                cur.clear();
            }
            c if !in_s && !in_d => cur.push(c),
            c => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    if out.is_empty() {
        return Err(format!("空的键名：`{s}`"));
    }
    Ok(out)
}

/// 导航到 `path` 指向的表（不存在则创建）；`[[a]]` 时落在最后一个元素上。
fn ensure_table<'a>(
    root: &'a mut BTreeMap<String, Value>,
    path: &[String],
    no: usize,
) -> Result<(), String> {
    let mut cur = root;
    for k in path.iter() {
        if !cur.contains_key(k) {
            cur.insert(k.clone(), Value::Object(BTreeMap::new()));
        }
        // 关键：`get_mut` 的结果必须**一次性移动**给 `cur`。
        // 若在同一个 match 的分支里再去 `cur.insert`，会对 `cur` 产生第二次
        // 可变借用（E0499）—— 所以插入放在 match 之前。
        cur = match cur.get_mut(k) {
            Some(Value::Object(sub)) => sub,
            Some(Value::Array(a)) => match a.last_mut() {
                Some(Value::Object(sub)) => sub,
                _ => return Err(format!("第 {no} 行：`{k}` 不是表数组")),
            },
            _ => return Err(format!("第 {no} 行：`{k}` 已存在且不是表")),
        };
    }
    Ok(())
}

/// `[[path]]`：在父表下追加一个新的表元素。
fn push_array_table(
    root: &mut BTreeMap<String, Value>,
    path: &[String],
    no: usize,
) -> Result<(), String> {
    let (last, parents) = path.split_last().ok_or_else(|| format!("第 {no} 行：空表名"))?;
    // 先确保父路径存在
    let mut cur = root;
    for k in parents {
        if !cur.contains_key(k) {
            cur.insert(k.clone(), Value::Object(BTreeMap::new()));
        }
        match cur.get_mut(k) {
            Some(Value::Object(sub)) => cur = sub,
            Some(Value::Array(a)) => {
                let Some(Value::Object(sub)) = a.last_mut() else {
                    return Err(format!("第 {no} 行：`{k}` 不是表数组"));
                };
                cur = sub;
            }
            _ => return Err(format!("第 {no} 行：`{k}` 不是表")),
        }
    }
    match cur.get_mut(last) {
        Some(Value::Array(a)) => a.push(Value::Object(BTreeMap::new())),
        None => {
            cur.insert(last.clone(), Value::Array(vec![Value::Object(BTreeMap::new())]));
        }
        Some(_) => return Err(format!("第 {no} 行：`{last}` 已存在且不是表数组")),
    }
    Ok(())
}

/// 在当前表下插入（支持点号键逐级建表）。
fn insert_keys(
    root: &mut BTreeMap<String, Value>,
    cur: &[String],
    keys: &[String],
    val: Value,
    no: usize,
) -> Result<(), String> {
    // 先定位到 cur 指向的表
    let mut tmp_root = std::mem::take(root);
    ensure_table(&mut tmp_root, cur, no)?;
    // 再按 keys 逐级深入
    let mut node: &mut BTreeMap<String, Value> = &mut tmp_root;
    if !cur.is_empty() {
        node = descend(&mut tmp_root, cur);
    }
    let (last, parents) = keys.split_last().ok_or_else(|| format!("第 {no} 行：空键"))?;
    for k in parents {
        if !node.contains_key(k) {
            node.insert(k.clone(), Value::Object(BTreeMap::new()));
        }
        match node.get_mut(k) {
            Some(Value::Object(sub)) => node = sub,
            _ => {
                *root = tmp_root;
                return Err(format!("第 {no} 行：`{k}` 不是表，无法作为前缀"));
            }
        }
    }
    node.insert(last.clone(), val);
    *root = tmp_root;
    Ok(())
}

/// 取 `path` 指向的可变表。
///
/// 前置条件：调用方已用 [`ensure_table`] 建好路径，故这里「取不到」不可能发生。
/// 用 `unreachable!` 而不是 `break`：提前 `return cur` 会与循环内的可变借用
/// 冲突（E0499，返回值要求借用持续到 `'a`）。
fn descend<'a>(
    root: &'a mut BTreeMap<String, Value>,
    path: &[String],
) -> &'a mut BTreeMap<String, Value> {
    let mut cur = root;
    for k in path {
        cur = match cur.get_mut(k) {
            Some(Value::Object(sub)) => sub,
            Some(Value::Array(a)) => match a.last_mut() {
                Some(Value::Object(sub)) => sub,
                _ => unreachable!("ensure_table 已保证路径上是表"),
            },
            _ => unreachable!("ensure_table 已保证路径上是表"),
        };
    }
    cur
}

/// 解析一个 TOML 值。
fn parse_value(s: &str, no: usize) -> Result<Value, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err(format!("第 {no} 行：缺少值"));
    }
    // 多行字符串
    if let Some(rest) = t.strip_prefix("\"\"\"") {
        let body = rest.trim_end_matches("\"\"\"").trim_start_matches('\n');
        return Ok(Value::Str(body.replace("\\\n", "")));
    }
    if let Some(rest) = t.strip_prefix("'''") {
        let body = rest.trim_end_matches("'''").trim_start_matches('\n');
        return Ok(Value::Str(body.to_string()));
    }
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        return Ok(Value::Str(unescape_toml(&t[1..t.len() - 1])));
    }
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        return Ok(Value::Str(t[1..t.len() - 1].to_string()));
    }
    if t.starts_with('[') {
        let inner = t.trim_start_matches('[').trim_end_matches(']');
        let mut items = Vec::new();
        for part in split_top(inner, ',') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            items.push(parse_value(p, no)?);
        }
        return Ok(Value::Array(items));
    }
    if t.starts_with('{') {
        let inner = t.trim_start_matches('{').trim_end_matches('}');
        let mut m = BTreeMap::new();
        for part in split_top(inner, ',') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            let (k, v) = p
                .split_once('=')
                .ok_or_else(|| format!("第 {no} 行：内联表里 `{p}` 缺少 `=`"))?;
            let keys = parse_key_path(k.trim())?;
            let val = parse_value(v, no)?;
            let mut node = &mut m;
            let (last, parents) = keys.split_last().unwrap();
            for kk in parents {
                if !node.contains_key(kk) {
                    node.insert(kk.clone(), Value::Object(BTreeMap::new()));
                }
                match node.get_mut(kk) {
                    Some(Value::Object(sub)) => node = sub,
                    _ => return Err(format!("第 {no} 行：`{kk}` 不是表")),
                }
            }
            node.insert(last.clone(), val);
        }
        return Ok(Value::Object(m));
    }
    match t {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        "inf" | "+inf" => return Ok(Value::float(f64::INFINITY)),
        "-inf" => return Ok(Value::float(f64::NEG_INFINITY)),
        "nan" | "+nan" | "-nan" => return Ok(Value::float(f64::NAN)),
        _ => {}
    }
    if let Some(i) = parse_int(t) {
        return Ok(Value::Int(i));
    }
    if let Some(f) = parse_float(t) {
        return Ok(Value::float(f));
    }
    // 日期时间等 → 按字符串保留（SML 侧裸词日期同样是字符串，往返一致）
    Ok(Value::Str(t.to_string()))
}

/// 顶层分隔（不进入引号与嵌套括号）。
fn split_top(s: &str, sep: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let (mut in_s, mut in_d) = (false, false);
    for c in s.chars() {
        match c {
            '"' if !in_s => {
                in_d = !in_d;
                cur.push(c);
            }
            '\'' if !in_d => {
                in_s = !in_s;
                cur.push(c);
            }
            '[' | '{' if !in_s && !in_d => {
                depth += 1;
                cur.push(c);
            }
            ']' | '}' if !in_s && !in_d => {
                depth -= 1;
                cur.push(c);
            }
            c if c == sep && depth == 0 && !in_s && !in_d => {
                out.push(std::mem::take(&mut cur));
            }
            c => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn unescape_toml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('b') => out.push('\u{8}'),
            Some('f') => out.push('\u{c}'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('u') => {
                let hex: String = it.by_ref().take(4).collect();
                if let Ok(n) = u32::from_str_radix(&hex, 16) {
                    if let Some(ch) = char::from_u32(n) {
                        out.push(ch);
                    }
                }
            }
            Some('U') => {
                let hex: String = it.by_ref().take(8).collect();
                if let Ok(n) = u32::from_str_radix(&hex, 16) {
                    if let Some(ch) = char::from_u32(n) {
                        out.push(ch);
                    }
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn parse_int(t: &str) -> Option<i64> {
    let s = t.replace('_', "");
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s.as_str()),
    };
    let body = body.strip_prefix('+').unwrap_or(body);
    let v = if let Some(h) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if let Some(o) = body.strip_prefix("0o").or_else(|| body.strip_prefix("0O")) {
        i64::from_str_radix(o, 8).ok()?
    } else if let Some(b) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        i64::from_str_radix(b, 2).ok()?
    } else {
        body.parse::<i64>().ok()?
    };
    Some(if neg { -v } else { v })
}

fn parse_float(t: &str) -> Option<f64> {
    let s = t.replace('_', "");
    if !(s.contains('.') || s.contains('e') || s.contains('E')) {
        return None;
    }
    s.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kv<'a>(v: &'a Value, k: &str) -> &'a Value {
        v.get(k).unwrap_or_else(|| panic!("缺少键 `{k}`：{v:?}"))
    }

    #[test]
    fn scalars() {
        let v = parse("a = 1\nb = \"x\"\nc = true\nd = 1.5\ne = 'lit'\n").unwrap();
        assert_eq!(kv(&v, "a"), &Value::Int(1));
        assert_eq!(kv(&v, "b"), &Value::Str("x".into()));
        assert_eq!(kv(&v, "c"), &Value::Bool(true));
        assert_eq!(kv(&v, "d"), &Value::float(1.5));
        assert_eq!(kv(&v, "e"), &Value::Str("lit".into()));
    }

    #[test]
    fn tables_and_dotted_keys() {
        let src = "[server]\nhost = \"db1\"\n[server.tls]\nenabled = true\n";
        let v = parse(src).unwrap();
        let s = v.get("server").unwrap();
        assert_eq!(kv(s, "host"), &Value::Str("db1".into()));
        assert_eq!(kv(kv(s, "tls"), "enabled"), &Value::Bool(true));
    }

    #[test]
    fn array_of_tables() {
        let src = "[[srv]]\nname = \"a\"\n[[srv]]\nname = \"b\"\n";
        let v = parse(src).unwrap();
        let Value::Array(a) = v.get("srv").unwrap() else {
            panic!("应为数组");
        };
        assert_eq!(a.len(), 2);
        assert_eq!(a[0].get("name"), Some(&Value::Str("a".into())));
        assert_eq!(a[1].get("name"), Some(&Value::Str("b".into())));
    }

    #[test]
    fn inline_table_and_array() {
        let v = parse("p = { x = 1, y = 2 }\nlist = [1, 2, 3]\n").unwrap();
        assert_eq!(kv(kv(&v, "p"), "y"), &Value::Int(2));
        let list = match v.get("list") {
            Some(Value::Array(a)) => a,
            other => panic!("应为数组：{other:?}"),
        };
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn escapes_and_multiline() {
        let v = parse("a = \"line\\nnext\"\nb = \"\"\"\nraw\n\"\"\"\n").unwrap();
        assert_eq!(kv(&v, "a"), &Value::Str("line\nnext".into()));
        assert_eq!(kv(&v, "b"), &Value::Str("raw\n".into()));
    }

    #[test]
    fn cargo_like_document() {
        let src = "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n[dependencies]\nserde = \"1\"\n";
        let v = parse(src).unwrap();
        assert_eq!(kv(kv(&v, "package"), "name"), &Value::Str("demo".into()));
        assert_eq!(kv(kv(&v, "dependencies"), "serde"), &Value::Str("1".into()));
    }

    #[test]
    fn roundtrip_via_to_toml() {
        let src = "title = \"demo\"\n[server]\nhost = \"db1\"\nport = 5432\n";
        let v = parse(src).unwrap();
        let out = to_toml(&v);
        let back = parse(&out).unwrap();
        assert_eq!(kv(&back, "title"), &Value::Str("demo".into()));
        assert_eq!(kv(kv(&back, "server"), "port"), &Value::Int(5432));
    }

    #[test]
    fn to_toml_emits_array_of_tables() {
        let mut item = BTreeMap::new();
        item.insert("name".to_string(), Value::Str("a".into()));
        let mut root = BTreeMap::new();
        root.insert("srv".to_string(), Value::Array(vec![Value::Object(item)]));
        let out = to_toml(&Value::Object(root));
        assert!(out.contains("[[srv]]"), "应输出表数组头：{out}");
    }
}
