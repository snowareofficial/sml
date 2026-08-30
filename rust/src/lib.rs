// SPDX-License-Identifier: MulanPSL-2.0
//! SML — SNOWARE Markup Language (Rust 实现, crate 名 `sml`)
//!
//! 声明式数据/配置格式, JSON/YAML 的替代品。语法与 Soup 生态的
//! `lib/sml.soup` (Lua) 对齐:
//!
//! ```sml
//! firstName: John
//! age: 27
//! address:
//! {
//!     streetAddress: "21 2nd Street"
//!     state: NY
//! }
//! phoneNumbers: [ { type: home } { type: office } ]
//! @base { region: cn-north-1 }
//! server web { &base port: 8080 }
//! ```
//!
//! 特性:
//! - 引号可选 (裸词即字符串)
//! - 块冒号可省 (`address { }` ≡ `address: { }`)
//! - 数组分隔灵活 (逗号可选)
//! - 片段继承 (`@name { }` 定义 / `&name` 引用)
//! - `include "path"` 引入外部文件（见 [`parse_file`]）
//! - `$env.VAR` 环境变量内联
//! - `#` 行注释
//! - 类型自识别: true/false -> bool, null -> None, 数字 -> i64/f64, 其余 -> String
//!
//! 值模型: `Value` 枚举 (与 JSON 同构, 另加 `__type`/`__name` 裸块元数据)。
//!
//! # 纯解析 vs 文件解析
//!
//! [`parse`] 是**纯函数**（只吃字符串，不做 IO），因此不含 include 处理。
//! 需要 include 时用 [`parse_file`]，它会先展开指令再交给 `parse`。
//! 这样设计保证了 `parse` 的可嵌入性（如 WASM / 沙箱内无文件系统）。
//!
//! # Cargo features
//!
//! - `serde`（默认关闭）：`Value` 实现 `Serialize`/`Deserialize`，可与
//!   serde_json / serde_yaml / toml 等任意 serde 后端互通；同时提供
//!   [`serde::from_str`] / [`serde::from_value`] / [`serde::to_value`] /
//!   [`serde::to_string`] 桥接函数，任何 `#[derive(serde::Deserialize)]`
//!   类型都能像 toml-rs 一样一键从 SML 反序列化（无需 `SmlDeserialize`）。
//! - `derive`（默认开启）：提供 [`SmlSerialize`] / [`SmlDeserialize`]
//!   两个 derive 宏，把自定义结构体/枚举「自然地」序列化为 SML，
//!   无需引入 serde。
//!
//! ```toml
//! sml-rs = { version = "0.2", features = ["serde"] }
//! # 不需要宏时可关闭默认 feature，回到完全零依赖：
//! sml-rs = { version = "0.2", default-features = false }
//! ```

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 值模型
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Array(Vec<Value>),
    /// 对象/块; `__type` / `__name` 裸块元数据以保留字键存放
    Object(BTreeMap<String, Value>),
}

impl Value {
    /// 对象字段按需取 (支持 "." 点路径)
    pub fn get(&self, path: &str) -> Option<&Value> {
        let mut cur = self;
        for seg in path.split('.') {
            match cur {
                Value::Object(m) => cur = m.get(seg)?,
                _ => return None,
            }
        }
        Some(cur)
    }
    /// 字符串视图 (字符串直接返回; 其它返回 None)
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", to_sml(self))
    }
}

// ---------------------------------------------------------------------------
// 契约（Contract）—— 可选的 schema 层
//
// SML 本身是纯数据格式（与 JSON/YAML 同层），值模型只有 7 种类型，
// **不具备**结构体定义、枚举、字段约束等类型系统能力。
// 契约是在此之上的**可选校验层**，用于给块加上结构与取值约束：
//
// ```sml
// @contract Server {
//     host: str                      # 必填
//     port: int default 8080         # 带默认值
//     tls: bool default true
//     tags: [str] optional           # 可选
//     status: enum [ active retired ]
//     ratio: num min 0 max 1
// }
//
// database {
//     @is Server                     # 应用契约
//     host: db1.internal
//     status: active
// }
// ```
//
// 语义：
// - `@contract Name { ... }` 定义契约（不进主树）
// - `@is Name` 在当前块应用契约：缺失字段用 default 填充；
//   缺少且无默认值的必填字段、类型不符、枚举值越界、数值越 min/max 均报错
// - 契约须在 `@is` **之前**定义（顺序依赖，与片段继承一致）
// - 不使用契约时行为完全不变，因此**向后兼容**
// ---------------------------------------------------------------------------

/// 契约中的字段类型
#[derive(Debug, Clone, PartialEq)]
pub enum TypeSpec {
    /// 任意类型
    Any,
    /// 引用另一个契约（**组合**）——字段值须是块，并递归按被引用契约校验。
    /// 用组合而非继承：契约之间不共享字段，而是「字段的类型是另一个契约」。
    /// 语法上复用裸词（写被引用的契约名），因此不引入任何新 token：
    ///     @contract Address { city: str }
    ///     @contract Server { address: Address }
    ContractRef(String),
    Str,
    Int,
    /// 数值：int 或 float 均可
    Num,
    Bool,
    /// 数组，元素须为指定类型
    Array(Box<TypeSpec>),
    /// 枚举：取值须在给定列表中
    Enum(Vec<String>),
}

impl TypeSpec {
    fn name(&self) -> String {
        match self {
            TypeSpec::Any => "any".into(),
            TypeSpec::Str => "str".into(),
            TypeSpec::Int => "int".into(),
            TypeSpec::Num => "num".into(),
            TypeSpec::Bool => "bool".into(),
            TypeSpec::Array(inner) => format!("[{}]", inner.name()),
            TypeSpec::Enum(vals) => format!("enum [{}]", vals.join(" ")),
            TypeSpec::ContractRef(name) => name.clone(),
        }
    }
}

/// 契约中的字段规格
#[derive(Debug, Clone)]
pub struct FieldSpec {
    pub ty: TypeSpec,
    /// 是否必填（默认 true）
    pub required: bool,
    /// 缺失时填充的默认值
    pub default: Option<Value>,
    /// 数值下界（含）
    pub min: Option<f64>,
    /// 数值上界（含）
    pub max: Option<f64>,
}

/// 契约（schema）：一组字段规格
#[derive(Debug, Clone)]
pub struct Contract {
    pub name: String,
    pub fields: BTreeMap<String, FieldSpec>,
    /// 是否允许契约未声明的字段。
    /// **默认 false（严格）**：额外字段一律报错，可及早发现拼写错误
    /// （如 `prot` 误写为 `port`）。确需放宽时须**显式**写 `loose`。
    pub allow_extra: bool,
}

/// 校验值是否符合类型规格。
/// `contracts` 供 `ContractRef`（组合）递归查找被引用契约。
fn check_type(
    contract: &str,
    field: &str,
    spec: &FieldSpec,
    v: &Value,
    contracts: &BTreeMap<String, Contract>,
) -> Result<(), String> {
    // 组合：字段值是块，递归按被引用的契约校验（含填默认值）
    if let TypeSpec::ContractRef(ref_name) = &spec.ty {
        return match v {
            Value::Object(_) => {
                let mut sub = match v {
                    Value::Object(m) => m.clone(),
                    _ => unreachable!(),
                };
                let target = contracts.get(ref_name).ok_or_else(|| {
                    format!(
                        "sml: 字段 `{}` 引用了未定义的契约 `{}`（契约 `{}`）",
                        field, ref_name, contract
                    )
                })?;
                apply_contract(target, &mut sub, contracts)?;
                Ok(())
            }
            _ => Err(format!(
                "sml: 字段 `{}` 应为块并按契约 `{}` 校验，实际为 {}（契约 `{}`）",
                field,
                ref_name,
                value_kind(v),
                contract
            )),
        };
    }

    let ok = match (&spec.ty, v) {
        (TypeSpec::Any, _) => true,
        (TypeSpec::Str, Value::Str(_)) => true,
        (TypeSpec::Int, Value::Int(_)) => true,
        (TypeSpec::Num, Value::Int(_)) | (TypeSpec::Num, Value::Float(_)) => true,
        (TypeSpec::Bool, Value::Bool(_)) => true,
        (TypeSpec::Enum(vals), Value::Str(s)) => vals.iter().any(|x| x == s),
        // 裸词数字会被 coerce 成 Int/Float，故枚举也接受被 coerce 成标量的情形
        (TypeSpec::Enum(vals), Value::Int(i)) => vals.iter().any(|x| x == &i.to_string()),
        (TypeSpec::Array(inner), Value::Array(items)) => items.iter().all(|it| {
            check_type(
                contract,
                field,
                &FieldSpec { ty: (**inner).clone(), required: true, default: None, min: None, max: None },
                it,
                contracts,
            )
            .is_ok()
        }),
        _ => false,
    };
    if !ok {
        return Err(format!(
            "sml: 字段 `{}` 类型应为 {}，实际为 {}（契约 `{}`）",
            field,
            spec.ty.name(),
            value_kind(v),
            contract
        ));
    }
    // 数值区间
    if spec.min.is_some() || spec.max.is_some() {
        let n = match v {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        };
        if let Some(n) = n {
            if let Some(lo) = spec.min {
                if n < lo {
                    return Err(format!(
                        "sml: 字段 `{}` 值 {} 小于下界 {}（契约 `{}`）",
                        field, n, lo, contract
                    ));
                }
            }
            if let Some(hi) = spec.max {
                if n > hi {
                    return Err(format!(
                        "sml: 字段 `{}` 值 {} 大于上界 {}（契约 `{}`）",
                        field, n, hi, contract
                    ));
                }
            }
        }
    }
    Ok(())
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Str(_) => "str",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// 对块应用契约：填充默认值 + 校验 + 严格性检查。
///
/// **严格为默认**：契约未声明的字段会被拒绝，除非契约显式标记 `loose`。
/// 这样拼错的字段名（如 `prot`）会立即报错，而不是被静默忽略。
fn apply_contract(
    c: &Contract,
    node: &mut BTreeMap<String, Value>,
    contracts: &BTreeMap<String, Contract>,
) -> Result<(), String> {
    // 1) 严格性：未声明字段一律拒绝（组合字段本身已在 fields 声明，其
    //    内部字段由被引用契约在自己的 apply_contract 中负责校验）
    if !c.allow_extra {
        for k in node.keys() {
            if !c.fields.contains_key(k) {
                return Err(format!(
                    "sml: 字段 `{}` 未在契约 `{}` 中声明（严格模式；如需允许额外字段请在契约名后写 `loose`）",
                    k, c.name
                ));
            }
        }
    }
    // 2) 逐字段：填默认值 + 类型/枚举/区间/组合校验
    for (k, spec) in &c.fields {
        match node.get(k) {
            None => {
                if let Some(d) = &spec.default {
                    node.insert(k.clone(), d.clone());
                } else if spec.required {
                    return Err(format!(
                        "sml: 字段 `{}` 必填但缺失（契约 `{}`）",
                        k, c.name
                    ));
                }
            }
            Some(v) => {
                // 组合会回填子块默认值，故需要可变副本
                if matches!(spec.ty, TypeSpec::ContractRef(_)) {
                    // 先按**原值**校验必须是块，否则会退化成
                    // 「子字段缺失」这类误导性错误
                    check_type(&c.name, k, spec, v, contracts)?;
                    let mut sub = match v {
                        Value::Object(m) => m.clone(),
                        _ => unreachable!("check_type 已保证为块"),
                    };
                    check_type_contract_ref(&c.name, k, spec, &mut sub, contracts)?;
                    node.insert(k.clone(), Value::Object(sub));
                } else {
                    check_type(&c.name, k, spec, v, contracts)?;
                }
            }
        }
    }
    Ok(())
}

/// 对「组合字段」递归应用被引用契约（会回填子块默认值）
fn check_type_contract_ref(
    contract: &str,
    field: &str,
    spec: &FieldSpec,
    sub: &mut BTreeMap<String, Value>,
    contracts: &BTreeMap<String, Contract>,
) -> Result<(), String> {
    let ref_name = match &spec.ty {
        TypeSpec::ContractRef(n) => n.clone(),
        _ => return Ok(()),
    };
    let target = contracts.get(&ref_name).ok_or_else(|| {
        format!(
            "sml: 字段 `{}` 引用了未定义的契约 `{}`（契约 `{}`）",
            field, ref_name, contract
        )
    })?;
    // 先做基础类型校验（值须为块），再递归应用
    check_type(contract, field, spec, &Value::Object(sub.clone()), contracts)?;
    apply_contract(target, sub, contracts)
}

// ---------------------------------------------------------------------------
// 解析: 词法 + 递归下降
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    LBrace,  // {
    RBrace,  // }
    LBrack,  // [
    RBrack,  // ]
    Comma,   // ,
    Colon,   // :
    At,      // @
    Str(String),   // 引号串 (已解码)
    Word(String),  // 裸词
}

fn tokenize(text: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let mut chars = text.chars().peekable();
    let mut buf = String::new();
    let mut flush = |buf: &mut String, toks: &mut Vec<Tok>| {
        if !buf.is_empty() {
            toks.push(Tok::Word(std::mem::take(buf)));
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '#' => {
                // 单行注释到行尾
                for c2 in chars.by_ref() {
                    if c2 == '\n' {
                        break;
                    }
                }
            }
            '-' => {
                // `--` 单行注释到行尾；否则作为普通字符
                if chars.peek() == Some(&'-') {
                    chars.next(); // 吃掉第二个 -
                    for c2 in chars.by_ref() {
                        if c2 == '\n' {
                            break;
                        }
                    }
                } else {
                    buf.push(c);
                }
            }
            '/' => {
                match chars.peek() {
                    // `//` 单行注释到行尾
                    Some('/') => {
                        chars.next(); // 吃掉第二个 /
                        for c2 in chars.by_ref() {
                            if c2 == '\n' {
                                break;
                            }
                        }
                    }
                    // `/*` 多行注释，直到 `*/`
                    Some('*') => {
                        chars.next(); // 吃掉 *
                        loop {
                            match chars.next() {
                                Some('*') => {
                                    if chars.peek() == Some(&'/') {
                                        chars.next();
                                        break;
                                    }
                                }
                                Some(_) => {}
                                None => break,
                            }
                        }
                    }
                    // 否则作为普通字符（如路径 a/b/c）
                    _ => buf.push(c),
                }
            }
            '_' => {
                // `_*` 多行注释，直到 `*_`；否则作为普通字符
                if chars.peek() == Some(&'*') {
                    chars.next(); // 吃掉 *
                    loop {
                        match chars.next() {
                            Some('*') => {
                                if chars.peek() == Some(&'_') {
                                    chars.next();
                                    break;
                                }
                            }
                            Some(_) => {}
                            None => break,
                        }
                    }
                } else {
                    buf.push(c);
                }
            }
            '"' => {
                flush(&mut buf, &mut toks);
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => {
                            // 转义：\n \t \r \0 \" \\ \u{XXXX} \uXXXX
                            match chars.next() {
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some('r') => s.push('\r'),
                                Some('0') => s.push('\0'),
                                Some('"') => s.push('"'),
                                Some('\\') => s.push('\\'),
                                Some('u') => {
                                    let mut hex = String::new();
                                    // 支持 \u{XXXX} 或 \uXXXX
                                    if chars.peek() == Some(&'{') {
                                        chars.next();
                                        for c2 in chars.by_ref() {
                                            if c2 == '}' {
                                                break;
                                            }
                                            hex.push(c2);
                                        }
                                    } else {
                                        for _ in 0..4 {
                                            if let Some(c2) = chars.next() {
                                                hex.push(c2);
                                            }
                                        }
                                    }
                                    if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                                        if let Some(ch) = char::from_u32(cp) {
                                            s.push(ch);
                                        }
                                    }
                                }
                                Some(other) => s.push(other),
                                None => break,
                            }
                        }
                        Some(other) => s.push(other),
                        None => break,
                    }
                }
                toks.push(Tok::Str(s));
            }
            '{' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::LBrace);
            }
            '}' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::RBrace);
            }
            '[' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::LBrack);
            }
            ']' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::RBrack);
            }
            ',' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::Comma);
            }
            ':' => {
                flush(&mut buf, &mut toks);
                toks.push(Tok::Colon);
            }
            '@' => {
                // `@` 仅当位于**词首**时才是片段定义标记（`@base { ... }`）。
                // 出现在词中间时（典型如邮箱 `a@b.c`）必须作为普通字符保留：
                // 否则 `a@b.c` 会被切成 `Word("a")` + `At` + `Word("b.c")`，
                // 后半段在解析时被丢弃，导致邮箱静默损坏为 `a`。
                if buf.is_empty() {
                    toks.push(Tok::At);
                } else {
                    buf.push(c);
                }
            }
            ' ' | '\t' | '\n' | '\r' => {
                flush(&mut buf, &mut toks);
            }
            _ => {
                buf.push(c);
            }
        }
    }
    flush(&mut buf, &mut toks);
    Ok(toks)
}

/// 把裸词 `w` 转为 Value。
///
/// 受 `features` 控制：关闭 `BarewordStr` 后纯字符串裸词（如 `John`）被拒绝，
/// 必须写作 `"John"`；仍允许的非字符串裸词：bool / null / 数字 /
/// 片段引用 `&x`（需 `fragment`）/ 环境变量 `$env.X`（需 `env`）。
fn coerce_word(
    w: &str,
    fragments: &BTreeMap<String, Value>,
    features: FeatureSet,
    ns_prefix: &str,
) -> Result<Value, String> {
    match w {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        "null" => return Ok(Value::Null),
        _ => {}
    }
    // $env.VAR 内联（需 env 特性）
    if let Some(ev) = w.strip_prefix("$env.") {
        if !features.has(Feature::Env) {
            return Err(format!("sml: 当前特性集禁用了 `$env`（env），裸词 `{}` 无法解析", w));
        }
        return Ok(Value::Str(std::env::var(ev).unwrap_or_default()));
    }
    // 片段引用 &name（需 fragment 特性）。命名空间隔离：先查裸名，再逐级查 ns 前缀。
    if let Some(name) = w.strip_prefix('&') {
        if !features.has(Feature::Fragment) {
            return Err(format!("sml: 当前特性集禁用了片段引用（fragment），`{}` 无法解析", w));
        }
        if let Some(v) = fragments.get(name) {
            return Ok(v.clone());
        }
        // 逐级回退：ui.form.foo → form.foo → foo
        if !ns_prefix.is_empty() {
            let mut probe = ns_prefix.to_string();
            loop {
                let full = format!("{probe}.{name}");
                if let Some(v) = fragments.get(&full) {
                    return Ok(v.clone());
                }
                match probe.rfind('.') {
                    Some(idx) => probe.truncate(idx),
                    None => break,
                }
            }
        }
        return Ok(Value::Str(w.to_string()));
    }
    // 数字: int / float / 科学计数
    if let Ok(i) = w.parse::<i64>() {
        return Ok(Value::Int(i));
    }
    if let Ok(f) = w.parse::<f64>() {
        return Ok(Value::Float(f));
    }
    if !features.has(Feature::BarewordStr) {
        return Err(format!(
            "sml: 字符串必须加引号，裸词 `{}` 应写作 `\"{}\"`（特性 bareword-string 已禁用）",
            w, w
        ));
    }
    Ok(Value::Str(w.to_string()))
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
    fragments: BTreeMap<String, Value>,
    /// 契约表：名 -> 契约。由 `@contract Name { ... }` 填充
    contracts: BTreeMap<String, Contract>,
    /// 生效特性集（已与调用方允许范围交集）
    features: FeatureSet,
    /// 命名空间栈：每个块（含 include `as ns` 产生的块）的名字依次入栈。
    /// 宏/契约注册与引用时，按栈路径加前缀（如 `ui.form.Button`），
    /// 使命名空间真正隔离宏，而非仅隔离数据键值。
    ns_stack: Vec<String>,
}

impl Parser {
    /// 当前命名空间前缀（栈路径用 "." 连接，空栈返回空串）
    fn ns_prefix(&self) -> String {
        if self.ns_stack.is_empty() {
            String::new()
        } else {
            self.ns_stack.join(".")
        }
    }

    /// 把裸名套上当前命名空间前缀（若栈非空）
    fn qualify(&self, name: &str) -> String {
        let p = self.ns_prefix();
        if p.is_empty() {
            name.to_string()
        } else {
            format!("{p}.{name}")
        }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.i)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.i).cloned();
        if t.is_some() {
            self.i += 1;
        }
        t
    }

    /// 解析契约体：逐条读 `field: <类型> [修饰符...]`
    fn parse_contract_body(&mut self) -> Result<BTreeMap<String, FieldSpec>, String> {
        let mut fields: BTreeMap<String, FieldSpec> = BTreeMap::new();
        loop {
            match self.peek().cloned() {
                None | Some(Tok::RBrace) => {
                    self.next();
                    break;
                }
                Some(Tok::Comma) => {
                    self.next();
                }
                _ => {
                    let key = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        other => {
                            return Err(format!("sml: 契约字段期望键, 得 {:?}", other))
                        }
                    };
                    if self.peek() == Some(&Tok::Colon) {
                        self.next();
                    } else {
                        return Err(format!("sml: 契约字段 `{}` 后须有冒号", key));
                    }
                    let spec = self.parse_field_spec()?;
                    fields.insert(key, spec);
                }
            }
        }
        Ok(fields)
    }

    /// 解析单个字段的类型与修饰符
    fn parse_field_spec(&mut self) -> Result<FieldSpec, String> {
        let ty = match self.next() {
            Some(Tok::Word(w)) => match w.as_str() {
                "str" => TypeSpec::Str,
                "int" => TypeSpec::Int,
                "num" => TypeSpec::Num,
                "bool" => TypeSpec::Bool,
                "any" => TypeSpec::Any,
                "enum" => {
                    if self.peek() != Some(&Tok::LBrack) {
                        return Err("sml: `enum` 后须为 [ ... ]".into());
                    }
                    self.next();
                    let mut vals = Vec::new();
                    loop {
                        match self.peek().cloned() {
                            None | Some(Tok::RBrack) => {
                                self.next();
                                break;
                            }
                            Some(Tok::Comma) => {
                                self.next();
                            }
                            Some(Tok::Word(s)) | Some(Tok::Str(s)) => {
                                vals.push(s);
                                self.next();
                            }
                            _ => {
                                self.next();
                            }
                        }
                    }
                    TypeSpec::Enum(vals)
                }
                // 非内置类型名 -> 视为**契约引用**（组合）。
                // 这样「字段的类型是另一个契约」复用裸词表达，不引入新 token。
                // 被引用的契约可在之后定义（校验发生在 @is 时，而非定义时）。
                other => TypeSpec::ContractRef(other.to_string()),
            },
            Some(Tok::LBrack) => {
                let inner = match self.next() {
                    Some(Tok::Word(w)) => match w.as_str() {
                        "str" => TypeSpec::Str,
                        "int" => TypeSpec::Int,
                        "num" => TypeSpec::Num,
                        "bool" => TypeSpec::Bool,
                        "any" => TypeSpec::Any,
                        other => {
                            return Err(format!("sml: 未知数组元素类型 `{}`", other))
                        }
                    },
                    other => {
                        return Err(format!("sml: 数组元素类型期望标识符, 得 {:?}", other))
                    }
                };
                if self.peek() == Some(&Tok::RBrack) {
                    self.next();
                }
                TypeSpec::Array(Box::new(inner))
            }
            other => return Err(format!("sml: 字段类型期望标识符, 得 {:?}", other)),
        };

        // 修饰符：required / optional / default <值> / min <数> / max <数>
        let mut required = true;
        let mut default = None;
        let mut min = None;
        let mut max = None;
        loop {
            // 若当前是 `标识符 :` 则视为下一个字段的开始，停止读修饰符
            let is_next_field = matches!(self.peek(), Some(Tok::Word(_)))
                && matches!(self.toks.get(self.i + 1), Some(Tok::Colon));
            if is_next_field {
                break;
            }
            match self.peek().cloned() {
                Some(Tok::Word(w)) => match w.as_str() {
                    "optional" => {
                        required = false;
                        self.next();
                    }
                    "required" => {
                        required = true;
                        self.next();
                    }
                    "default" => {
                        self.next();
                        default = Some(match self.next() {
                            Some(Tok::Word(w2)) => coerce_word(&w2, &self.fragments, self.features, &self.ns_prefix())?,
                            Some(Tok::Str(s)) => Value::Str(s),
                            other => {
                                return Err(format!("sml: default 期望值, 得 {:?}", other))
                            }
                        });
                    }
                    "min" => {
                        self.next();
                        min = Some(self.parse_spec_number()?);
                    }
                    "max" => {
                        self.next();
                        max = Some(self.parse_spec_number()?);
                    }
                    _ => break,
                },
                _ => break,
            }
        }
        Ok(FieldSpec { ty, required, default, min, max })
    }

    fn parse_spec_number(&mut self) -> Result<f64, String> {
        match self.next() {
            Some(Tok::Word(w)) => {
                w.parse::<f64>().map_err(|_| format!("sml: 期望数字, 得 `{}`", w))
            }
            other => Err(format!("sml: 期望数字, 得 {:?}", other)),
        }
    }

    /// 解析对象/块, 直到遇到 closing (None=顶层)
    fn parse_block(&mut self, closing: Option<Tok>) -> Result<Value, String> {
        let mut node: BTreeMap<String, Value> = BTreeMap::new();
        // 块内若声明了 `@is Name`，在块解析完成后应用契约
        let mut applied_contract: Option<String> = None;
        loop {
            let tok = match self.peek().cloned() {
                None => break,
                Some(t) => t,
            };
            match tok {
                Tok::RBrace | Tok::RBrack => {
                    if let Some(cl) = &closing {
                        if *cl == tok {
                            self.next();
                            break;
                        }
                    }
                    // 顶层遇右括号也停
                    break;
                }
                Tok::Comma => {
                    self.next();
                }
                Tok::At => {
                    // @name { ... } 片段定义 (不进主树)
                    self.next();
                    let fname = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        _ => return Err("sml: @ 后需片段名".into()),
                    };
                    if self.peek() == Some(&Tok::Colon) {
                        self.next();
                    }
                    // —— 契约定义：`@contract Name { ... }` ——
                    if fname == "contract" {
                        if !self.features.has(Feature::Contract) {
                            return Err("@contract 需要特性 `contract`，但当前特性集已禁用".into());
                        }
                        let cname = match self.next() {
                            Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                            other => {
                                return Err(format!("sml: @contract 后须契约名, 得 {:?}", other))
                            }
                        };
                        // 可选修饰符 `loose`：显式允许契约未声明的字段。
                        // 严格是默认，放宽必须写出来（复用裸词，不引入新 token）。
                        let mut allow_extra = false;
                        if let Some(Tok::Word(w)) = self.peek().cloned() {
                            if w == "loose" {
                                allow_extra = true;
                                self.next();
                            }
                        }
                        if self.peek() != Some(&Tok::LBrace) {
                            return Err(format!("sml: @contract {} 后须 {{ ... }}", cname));
                        }
                        self.next();
                        let fields = self.parse_contract_body()?;
                        // 命名空间前缀隔离：块内的契约按当前 ns 栈路径注册
                        self.contracts.insert(
                            self.qualify(&cname),
                            Contract {
                                name: self.qualify(&cname),
                                fields,
                                allow_extra,
                            },
                        );
                        continue;
                    }
                    // —— 契约应用：`@is Name`（在当前块内）——
                    if fname == "is" {
                        if !self.features.has(Feature::Contract) {
                            return Err("@is 需要特性 `contract`，但当前特性集已禁用".into());
                        }
                        let cname = match self.next() {
                            Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                            other => {
                                return Err(format!("sml: @is 后须契约名, 得 {:?}", other))
                            }
                        };
                        // 命名空间隔离：先按裸名查，再按当前 ns 前缀查
                        let resolved = if self.contracts.contains_key(&cname) {
                            cname.clone()
                        } else {
                            self.qualify(&cname)
                        };
                        applied_contract = Some(resolved);
                        continue;
                    }
                    // 可选 type [name] 参数
                    let mut ftype: Option<String> = None;
                    let mut farg: Option<String> = None;
                    if let Some(Tok::Word(s)) = self.peek().cloned() {
                        if *self.peek().unwrap() != Tok::LBrace {
                            self.next();
                            ftype = Some(s);
                            if let Some(Tok::Word(s2)) = self.peek().cloned() {
                                if *self.peek().unwrap() != Tok::LBrace {
                                    self.next();
                                    farg = Some(s2);
                                }
                            }
                        }
                    }
                    if self.peek() == Some(&Tok::LBrace) {
                        self.next();
                        let mut sub = match self.parse_block(Some(Tok::RBrace))? {
                            Value::Object(m) => m,
                            other => {
                                let mut m = BTreeMap::new();
                                m.insert("_value".into(), other);
                                m
                            }
                        };
                        if let Some(t) = ftype {
                            sub.insert("__type".into(), Value::Str(t));
                        }
                        if let Some(a) = farg {
                            sub.insert("__name".into(), Value::Str(a));
                        }
                        if !self.features.has(Feature::Fragment) {
                            return Err(format!(
                                "sml: 片段定义 `@{}` 需要特性 `fragment`，但当前特性集已禁用",
                                fname
                            ));
                        }
                        // 命名空间前缀隔离：片段定义按当前 ns 栈路径注册
                        self.fragments.insert(self.qualify(&fname), Value::Object(sub));
                    }
                }
                _ => {
                    // key
                    let key = match self.next() {
                        Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                        other => return Err(format!("sml: 期望键, 得 {:?}", other)),
                    };
                    let colon = self.peek() == Some(&Tok::Colon);
                    if colon {
                        self.next();
                    }
                    let val = self.parse_value(&key, colon)?;
                    // 同名冲突 -> 提升为数组
                    if let Some(existing) = node.get_mut(&key) {
                        match existing {
                            Value::Array(a) => a.push(val),
                            _ => {
                                let old = node.remove(&key).unwrap();
                                node.insert(key, Value::Array(vec![old, val]));
                            }
                        }
                    } else {
                        node.insert(key, val);
                    }
                }
            }
        }
        // 块结束：若声明了 `@is`，应用契约（填默认值 + 校验 + 严格性检查）
        if let Some(cname) = applied_contract {
            let c = self
                .contracts
                .get(&cname)
                .cloned()
                .ok_or_else(|| format!("sml: 未定义的契约 `{}`", cname))?;
            apply_contract(&c, &mut node, &self.contracts)?;
        }
        Ok(Value::Object(node))
    }

    /// 解析一个值 (在 key 之后)
    fn parse_value(&mut self, key: &str, colon: bool) -> Result<Value, String> {
        // 无冒号且后继是裸词: 可能是裸块 `type [name] { }`
        if !colon && matches!(self.peek(), Some(Tok::Word(_))) {
            // 预扫描: 收集参数直到 { / 结束; 若发现 { 则按裸块处理
            let mut probe = self.i;
            let mut found_block = false;
            while probe < self.toks.len() {
                match &self.toks[probe] {
                    Tok::Word(_) | Tok::Str(_) => probe += 1,
                    Tok::LBrace => {
                        found_block = true;
                        break;
                    }
                    _ => break,
                }
            }
            if found_block {
                // 裸块: key 为类型, 参数在 { 前
                let mut args: Vec<Value> = Vec::new();
                while let Some(t) = self.peek().cloned() {
                    match t {
                        Tok::Word(w) => {
                            args.push(coerce_word(&w, &self.fragments, self.features, &self.ns_prefix())?);
                            self.next();
                        }
                        Tok::Str(_) => {
                            if let Some(Tok::Str(s)) = self.next() {
                                args.push(Value::Str(s));
                            }
                        }
                        _ => break,
                    }
                }
                if self.peek() == Some(&Tok::LBrace) {
                    self.next();
                    // 进入子块 = 进入该 block 名字的命名空间
                    self.ns_stack.push(key.to_string());
                    let mut sub = self.parse_block(Some(Tok::RBrace))?;
                    self.ns_stack.pop();
                    if let Value::Object(m) = &mut sub {
                        m.insert("__type".into(), Value::Str(key.to_string()));
                        if args.len() == 1 {
                            m.insert("__name".into(), args.remove(0));
                        }
                    }
                    return Ok(sub);
                }
            }
        }
        match self.peek().cloned() {
            Some(Tok::LBrace) => {
                self.next();
                self.parse_block(Some(Tok::RBrace))
            }
            Some(Tok::LBrack) => {
                self.next();
                self.parse_array()
            }
            Some(tok @ (Tok::Word(_) | Tok::Str(_))) => {
                let v = match tok {
                    Tok::Word(w) => coerce_word(&w, &self.fragments, self.features, &self.ns_prefix())?,
                    Tok::Str(s) => {
                        let ev = s.strip_prefix("$env.");
                        match ev {
                            Some(name) => Value::Str(std::env::var(name).unwrap_or_default()),
                            None => Value::Str(s),
                        }
                    }
                    _ => unreachable!(),
                };
                self.next();
                Ok(v)
            }
            // 键后无值: `key }` / `key ]` / `key ,` / 行尾 —— key 本身即值 (片段引用/裸词)
            Some(Tok::RBrace) | Some(Tok::RBrack) | Some(Tok::Comma) | None => {
                if colon {
                    // 有冒号但无值: 空值
                    Ok(Value::Null)
                } else {
                    Ok(coerce_word(key, &self.fragments, self.features, &self.ns_prefix())?)
                }
            }
            _ => Err("sml: 语法错误".into()),
        }
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        let mut arr = Vec::new();
        loop {
            match self.peek().cloned() {
                None => break,
                Some(Tok::RBrack) => {
                    self.next();
                    break;
                }
                Some(Tok::Comma) => {
                    self.next();
                }
                Some(Tok::LBrace) => {
                    self.next();
                    arr.push(self.parse_block(Some(Tok::RBrace))?);
                }
                Some(Tok::Word(w)) => {
                    arr.push(coerce_word(&w, &self.fragments, self.features, &self.ns_prefix())?);
                    self.next();
                }
                Some(Tok::Str(_)) => {
                    if let Some(Tok::Str(s)) = self.next() {
                        arr.push(Value::Str(s));
                    }
                }
                _ => break,
            }
        }
        Ok(Value::Array(arr))
    }
}

/// SML 语法版本
///
/// SML 源于 eclog，演进中通过 `@version` 声明文档遵循的语法版本，
/// 使解析器能在将来引入 v2 不兼容语法时仍正确读取旧文档。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Version {
    /// v1：初始公开版本。字符串可裸写（`name: John`），自动识别类型。
    V1,
    /// v2：草案版，引入「字符串必须显式引号」的不兼容语法（与 v3 同语义）。
    V2,
    /// v3：正式版。取消自动字符串无引号，自由文本必须写作 `"..."`；
    ///     数字 / bool / null / 片段引用 `&x` / 环境变量 `$env.X` 仍为裸词。
    V3,
}

impl Version {
    /// 当前实现支持的最新版本
    pub const CURRENT: Version = Version::V3;

    /// 是否要求字符串显式引号（v2 / v3 为严格模式）
    pub fn strict_strings(self) -> bool {
        self >= Version::V2
    }

    /// 解析版本字面量（`v1`/`1`、`v2`/`2`、`v3`/`3`）
    fn from_word(w: &str) -> Option<Version> {
        match w {
            "v1" | "1" => Some(Version::V1),
            "v2" | "2" => Some(Version::V2),
            "v3" | "3" => Some(Version::V3),
            _ => None,
        }
    }

    /// 版本名（用于错误信息与序列化回显）
    pub fn name(self) -> &'static str {
        match self {
            Version::V1 => "v1",
            Version::V2 => "v2",
            Version::V3 => "v3",
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ===========================================================================
// 特性集 (FeatureSet)
//
// 文档可通过 `@feature` 指令在「版本基线」之上做**裁剪**（窄化），调用方也可
// 通过 `parse_with_features` / `parse_allowed` 限制接受的子集。文档不能扩宽
// 调用方给出的范围——否则 `@feature` 就成了绕过限制的后门。
//
// 为保证五端（Rust/C/JS/C++/Lua）实现一致且易于维护，特性名与位定义集中
// 在此（见 [`FEATURES`] 表）。新增特性只需在表中加一行，并在对应 parser 处
// 用 `ps.features.has(Feature::Xxx)` 判定即可，无需散落大量 if。
// ===========================================================================

/// 单个特性标识。与 [`FEATURES`] 表一一对应；改表即改全端。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// 裸词即字符串（v1 行为）。v2/v3 关闭后字符串必须加引号。
    BarewordStr,
    /// `include "x.sml"` 文件包含。
    Include,
    /// `$env.VAR` 环境变量内插。
    Env,
    /// `@contract` / `@is` 契约系统。
    Contract,
    /// `&frag` / `@frag` 片段复用。
    Fragment,
    /// 顶层裸数组 `[ ... ]`（无键）。
    TopArray,
    /// `include "x.sml" as ns` 命名空间包含（高优先级前缀）。
    Namespace,
    /// 无扩展名的 `include "foo"` 默认等价于 `include "foo.sml" as foo`。
    ImplicitNs,
    /// 逗号分隔的多目标 `include "a", "b" as y` 与 `import` 别名。
    MultiInclude,
    /// 通配 `include "dir/*.sml"`（glob）。
    GlobInclude,
    /// 正则匹配 `include /re/`（需 `regex-include`）。
    RegexInclude,
    /// 扩展名重写 `include "x.conf" -> "x.sml"`（将非 sml 当 sml 解析）。
    ExtRewrite,
}

/// 返回全部已注册特性的名字，顺序与 [`FEATURES`]（即特性位序）一致。
///
/// C-ABI 的 `sml_feature_name(bit)` 依赖此顺序，测试中有对应守护用例。
pub fn feature_names() -> Vec<&'static str> {
    FEATURES.iter().map(|(n, _)| *n).collect()
}

/// 特性名 → 枚举 的注册表。所有端共用同一组名字，保证跨语言一致。
pub static FEATURES: &[(&str, Feature)] = &[
    ("bareword-string", Feature::BarewordStr),
    ("include", Feature::Include),
    ("env", Feature::Env),
    ("contract", Feature::Contract),
    ("fragment", Feature::Fragment),
    ("top-level-array", Feature::TopArray),
    ("namespace", Feature::Namespace),
    ("implicit-ns", Feature::ImplicitNs),
    ("multi-include", Feature::MultiInclude),
    ("glob-include", Feature::GlobInclude),
    ("regex-include", Feature::RegexInclude),
    ("ext-rewrite", Feature::ExtRewrite),
];

impl Feature {
    /// 按名字查特性；未知名字返回 None（调用方据此报错，杜绝静默 typo）。
    pub fn from_name(name: &str) -> Option<Feature> {
        FEATURES.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
    }

    /// 特性名（用于报错 / 序列化回显）
    pub fn name(self) -> &'static str {
        FEATURES
            .iter()
            .find(|(_, f)| *f == self)
            .map(|(n, _)| *n)
            .unwrap_or("<unknown>")
    }
}

/// 位掩码形式的特性集合。
///
/// 设计哲学：从极简到丰富、功能可裁剪。默认基线（`baseline()`）只开极简三件套
/// （`include` + `namespace` + `implicit-ns`），复杂能力（多目标 / glob / 正则 /
/// 扩展名重写）必须显式 `@feature enable` 才生效，避免重蹈 YAML 过度复杂的覆辙。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureSet(u64);

impl FeatureSet {
    /// 全部特性位（含所有 opt-in 能力）。用于「调用方允许全集」与版本基线，
    /// 实际默认并不开启这些——见 [`FeatureSet::baseline`]。
    pub fn all() -> FeatureSet {
        let mut m = 0u64;
        for (_, f) in FEATURES {
            m |= 1 << (*f as u8);
        }
        FeatureSet(m)
    }

    /// 极简默认集（SML 核心可用能力）。这是 `parse_file` 的默认允许集；
    /// 仅「多目标 / glob / 正则 / 扩展名重写」等高级能力需文档内
    /// `@feature enable` 显式开启（避免重蹈 YAML 覆辙）。
    pub fn baseline() -> FeatureSet {
        FeatureSet::none()
            .with(Feature::BarewordStr)
            .with(Feature::Include)
            .with(Feature::Env)
            .with(Feature::Contract)
            .with(Feature::Fragment)
            .with(Feature::TopArray)
            .with(Feature::Namespace)
            .with(Feature::ImplicitNs)
    }

    /// 空集合
    pub fn none() -> FeatureSet {
        FeatureSet(0)
    }

    /// 按版本基线构造默认特性集：v1 极简默认（baseline）+ 裸词字符串；
    /// v2/v3 关闭裸词字符串（须引号）。复杂能力（glob/regex/multi...）仍默认关闭，
    /// 需文档 `@feature enable` 显式开启。
    pub fn for_version(v: Version) -> FeatureSet {
        let mut s = FeatureSet::baseline();
        // 严格模式（v2/v3）关闭裸词字符串；非严格（v1）开启。
        // 显式设置该位，确保与 baseline 默认值无关。
        if v.strict_strings() {
            s = s.without(Feature::BarewordStr);
        } else {
            s = s.with(Feature::BarewordStr);
        }
        s
    }

    /// 是否包含某特性
    pub fn has(self, f: Feature) -> bool {
        (self.0 & (1 << (f as u8))) != 0
    }

    /// 返回开启 `f` 后的副本
    pub fn with(self, f: Feature) -> FeatureSet {
        FeatureSet(self.0 | (1 << (f as u8)))
    }

    /// 返回关闭 `f` 后的副本
    pub fn without(self, f: Feature) -> FeatureSet {
        FeatureSet(self.0 & !(1 << (f as u8)))
    }

    /// 与另一集合取交集（用于「文档裁剪 ∩ 调用方允许」）
    pub fn intersection(self, other: FeatureSet) -> FeatureSet {
        FeatureSet(self.0 & other.0)
    }

    /// 是否无任何特性
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for FeatureSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (n, feat) in FEATURES {
            if self.has(*feat) {
                if !first {
                    f.write_str(",")?;
                }
                f.write_str(n)?;
                first = false;
            }
        }
        if first {
            f.write_str("<none>")?;
        }
        Ok(())
    }
}

/// `@feature` 解析模式：白名单（仅启用列出的）/ 黑名单（禁用列出的）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeatureMode {
    Default,
    Whitelist,
    Blacklist,
}

/// 取出 token 的字符串内容（Word / Str 都取其文本；其余返回空串）。
fn tok_word(t: &Tok) -> String {
    match t {
        Tok::Word(s) | Tok::Str(s) => s.clone(),
        _ => String::new(),
    }
}

/// 若该行是 `@feature` 声明，则根据 `mode` / 操作更新 `feats`，并返回 true。
///
/// 支持语法（均不区分大小写，参数以空格分隔）：
/// - `@feature base v3`              设定基线版本（等价于 `@version`，仅用于特性派生）
/// - `@feature mode whitelist`       后续 enable 仅保留所列（基集先清空）
/// - `@feature mode blacklist`       后续 disable 仅移除所列（基集保持全开）
/// - `@feature enable <name>[,...]`  开启特性（可逗号批量）
/// - `@feature disable <name>[,...]` 关闭特性
/// - `@feature whitelist <a,b>`      紧凑白名单
/// - `@feature blacklist <a,b>`      紧凑黑名单
///
/// 未知特性名一律报错，避免拼写错误静默失效。
fn apply_feature_directive(
    line: &str,
    feats: &mut FeatureSet,
    mode: &mut FeatureMode,
    base: &mut Option<Version>,
) -> Result<bool, String> {
    let content = strip_line_comment(line).trim();
    let toks = match tokenize(content) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    if toks.is_empty() || toks[0] != Tok::At {
        return Ok(false);
    }
    let words: Vec<String> = toks
        .iter()
        .map(|t| match t {
            Tok::At => "@".to_string(),
            other => tok_word(other),
        })
        .collect();
    // @feature 词法上拆成 [@, feature]，拼前两个 token 才是 "@feature"
    let head = format!("{}{}", words.first().map(|s| s.as_str()).unwrap_or(""), words.get(1).map(|s| s.as_str()).unwrap_or(""));
    if head != "@feature" {
        return Ok(false);
    }
    // 去掉首 token `@`，使后续 words[0]=="feature"
    let words: Vec<String> = words[1..].to_vec();
    if words.len() < 2 {
        return Err("@feature 指令缺少参数".into());
    }
    let arg = words[1].as_str();
    // 把 `enable x,y,z` / `whitelist a,b` 的多名拆开
    let names = |from: usize| -> Vec<String> {
        words[from..]
            .join(",")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    match arg {
        "base" => {
            let v = Version::from_word(words.get(2).map(|s| s.as_str()).unwrap_or(""))
                .ok_or_else(|| {
                    format!(
                        "@feature base 需要 v1/v2/v3，收到 `{}`",
                        words.get(2).cloned().unwrap_or_default()
                    )
                })?;
            *feats = FeatureSet::for_version(v);
            *base = Some(v);
            Ok(true)
        }
        "mode" => {
            let m = words.get(2).map(|s| s.as_str()).unwrap_or("");
            *mode = match m {
                "whitelist" => FeatureMode::Whitelist,
                "blacklist" => FeatureMode::Blacklist,
                _ => return Err(format!("@feature mode 需要 whitelist/blacklist，收到 `{m}`")),
            };
            if *mode == FeatureMode::Whitelist {
                // 白名单：基集先清空，后续 enable 显式置位
                *feats = FeatureSet::none();
            }
            Ok(true)
        }
        "enable" => {
            // 直接在「当前特性集」上叠加开启（不切换白名单语义）。
            // 这样 `@feature enable regex-include` 在 `@version v1` 文档上会保留
            // bareword-string 等默认特性，而非收窄为仅所列项。
            // 真正的「收窄为仅所列」由显式 `@feature mode whitelist` 控制。
            for n in names(2) {
                let f = Feature::from_name(&n).ok_or_else(|| {
                    format!(
                        "未知特性 `{n}`，可用：{}",
                        FEATURES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                    )
                })?;
                *feats = feats.with(f);
            }
            Ok(true)
        }
        "disable" => {
            for n in names(2) {
                let f = Feature::from_name(&n).ok_or_else(|| {
                    format!(
                        "未知特性 `{n}`，可用：{}",
                        FEATURES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                    )
                })?;
                *feats = feats.without(f);
            }
            Ok(true)
        }
        "whitelist" => {
            *mode = FeatureMode::Whitelist;
            let mut s = FeatureSet::none();
            for n in names(2) {
                let f = Feature::from_name(&n).ok_or_else(|| {
                    format!(
                        "未知特性 `{n}`，可用：{}",
                        FEATURES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                    )
                })?;
                s = s.with(f);
            }
            *feats = s;
            Ok(true)
        }
        "blacklist" => {
            let mut s = FeatureSet::all();
            for n in names(2) {
                let f = Feature::from_name(&n).ok_or_else(|| {
                    format!(
                        "未知特性 `{n}`，可用：{}",
                        FEATURES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                    )
                })?;
                s = s.without(f);
            }
            *feats = s;
            Ok(true)
        }
        _ => Err(format!("未知 @feature 子命令 `{arg}`，可用 base/mode/enable/disable")),
    }
}

/// 在解析前剥离全部 `@feature` 指令，返回剩余文本、推导出的特性集，以及
/// 由 `@feature base vN` 声明的基线版本（若文档未用 `@version` 则采用它）。
///
/// 文档内部的 `@feature` 只能收窄；调用方允许范围由 `parse_with_features`
/// / `parse_allowed` 的 `allowed` 参数在入口处再次交集。
///
/// 返回值三元组：(剩余文本, 特性集, @feature base 声明的版本, 是否出现过 @feature 指令)。
/// 若文档从未声明 `@feature`，则 `had_feature=false`，调用方应改以版本基线派生特性集
/// （例如 v3 默认关闭裸词字符串）。
fn strip_features(text: &str) -> Result<(String, FeatureSet, Option<Version>, bool), String> {
    let mut out = String::new();
    let mut feats = FeatureSet::all();
    let mut mode = FeatureMode::Default;
    let mut base: Option<Version> = None;
    let mut had_feature = false;

    for line in text.lines() {
        match apply_feature_directive(line, &mut feats, &mut mode, &mut base) {
            Ok(true) => {
                had_feature = true;
                continue; // 指令行被消费，不进入剩余文本
            }
            Ok(false) => {}
            Err(e) => return Err(e), // 指令非法（如未知特性名）必须上浮，不能静默吞掉
        }
        out.push_str(line);
        out.push('\n');
    }
    Ok((out, feats, base, had_feature))
}

/// 若该行是 `@version` 声明，返回版本字面量；否则返回 None。
///
/// `version` 是保留字：不允许作为片段名（`@version { }`）使用。
fn version_directive(line: &str) -> Result<Option<String>, String> {
    let content = strip_line_comment(line).trim();
    // 词法失败的行（如未闭合引号）不是版本声明，交由主解析器报更准确的错
    let toks = match tokenize(content) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    match toks.as_slice() {
        [Tok::At, Tok::Word(w), Tok::Word(v)] if w == "version" => Ok(Some(v.clone())),
        [Tok::At, Tok::Word(w), Tok::Str(v)] if w == "version" => Ok(Some(v.clone())),
        [Tok::At, Tok::Word(w), ..] if w == "version" => Err(
            "`@version` 是版本声明指令，须写作 `@version v1`；`version` 不可作为片段名".into(),
        ),
        _ => Ok(None),
    }
}

/// 剥离 `@version` 声明行，返回剩余文本与声明的版本（未声明则为 None）。
///
/// 允许多次声明（include 进来的文件可各自声明），但必须一致；
/// 声明了实现不支持的版本时报错，避免静默按错误语法解析。
fn strip_version(text: &str) -> Result<(String, Option<Version>), String> {
    let mut declared: Option<Version> = None;
    let mut rest = String::new();
    for line in text.lines() {
        if let Some(lit) = version_directive(line)? {
            let v = Version::from_word(&lit).ok_or_else(|| {
                format!(
                    "不支持的 SML 版本 `{lit}`（本实现支持 {}）",
                    Version::CURRENT.name()
                )
            })?;
            match declared {
                None => declared = Some(v),
                Some(prev) if prev != v => {
                    return Err(format!("@version 冲突：{} 与 {}", prev.name(), v.name()))
                }
                Some(_) => {}
            }
            continue;
        }
        rest.push_str(line);
        rest.push('\n');
    }
    Ok((rest, declared))
}

/// 把 版本 + 文档 @feature 指令 合并为最终生效的特性集。
///
/// 规则：
/// - 若文档显式声明过 `@feature`（had_feature=true），则完全采用其推导的 `feats`；
/// - 否则（仅靠 `@version` 声明或默认），从版本基线派生（如 v3 关闭裸词字符串）。
/// 这样 v3 文档即使不写任何 `@feature` 也默认严格；调用方的 `allowed` 在
/// 入口处再与结果取交集，文档无法扩宽。
fn features_for(v: Version, feats: FeatureSet, had_feature: bool) -> FeatureSet {
    if had_feature {
        feats
    } else {
        FeatureSet::for_version(v)
    }
}

/// 解析 SML 文本，并返回其声明的语法版本。
///
/// 未声明版本时按 `V1` 处理（裸词即字符串），**既有文档不受影响**；
/// 显式 `@version v3` 则返回 `V3`（此时字符串需引号）。
pub fn parse_versioned(text: &str) -> Result<(Value, Version), String> {
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, base, had) = strip_features(&rest)?;
    // 版本优先级：@version 显式声明 > @feature base > 默认 V1
    let v = declared.or(base).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    Ok((parse_impl(&rest, v, feats)?, v))
}

/// 解析 SML 文件：展开 include，并返回其声明的语法版本
pub fn parse_file_versioned(path: impl AsRef<Path>) -> Result<(Value, Version), String> {
    let path = path.as_ref();
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("读取失败 {}: {e}", path.display()))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, base_ver, had) = strip_features(&rest)?;
    let allowed = FeatureSet::all().intersection(feats);
    let v = declared.or(base_ver).unwrap_or(Version::V1);
    let feats = features_for(v, allowed, had);
    let toks = resolve_includes(&rest, &base, allowed)?;
    let val = parse_impl_tokens(toks, v, feats)?;
    Ok((val, v))
}

/// 解析 SML 文本
///
/// 会自动识别并剥离 `@version` / `@feature` 声明（需要版本信息时用
/// [`parse_versioned`]，需要特性裁剪信息时用 [`parse_with_features`]）。
///
/// **向后兼容**：未声明 `@version` 的文档按 `V1` 解析（裸词即字符串），
/// 既有大量 v1 文档不受影响；仅显式 `@version v2|v3` 才启用严格字符串。
pub fn parse(text: &str) -> Result<Value, String> {
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    parse_impl(&rest, v, feats)
}

/// 解析 SML 文本，并限制文档声明的版本必须在 `allowed` 范围内。
///
/// 用于「库固定依赖某个 SML 语法版本」的场景：若文档声明了 `allowed`
/// 之外的版本（例如库只接受 v1..v3，却遇到 `@version v4`），立即报错，
/// 而不是用不兼容的语法静默解析。
///
/// 未声明版本的文档视为 `V1`，只要 `allowed` 含 `V1` 即放行。
pub fn parse_allowed(
    text: &str,
    allowed: &[Version],
) -> Result<Value, String> {
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Version::V1);
    if !allowed.contains(&v) {
        return Err(format!(
            "sml: 文档声明版本 {} 不在本库接受的版本范围 {{{}}} 内",
            v.name(),
            allowed
                .iter()
                .map(|x| x.name())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let feats = features_for(v, feats, had);
    parse_impl(&rest, v, feats)
}

/// 解析 SML 文本，同时限制文档使用的**特性子集**必须在 `allowed` 内。
///
/// 与 [`parse_allowed`]（版本范围）配套：`allowed` 是调用方（库作者）给出的
/// 白名单，文档内部的 `@feature enable/disable` 只能**收窄**这个集合，
/// 不能扩宽——否则文档就能自行绕过调用方的限制。交集为空则报错。
///
/// 未声明任何 `@feature` 的文档若仅靠版本基线（如 v3），则基线特性与
/// `allowed` 交集；只要交集非空即放行。
pub fn parse_with_features(
    text: &str,
    allowed: FeatureSet,
) -> Result<(Value, FeatureSet), String> {
    let (rest, declared) = strip_version(text)?;
    let (rest, feats, base, had) = strip_features(&rest)?;
    let v = declared.or(base).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    let effective = feats.intersection(allowed);
    if effective.is_empty() {
        return Err(format!(
            "sml: 文档请求的特性 {feats} 与调用方允许的特性 {allowed} 无交集"
        ));
    }
    let val = parse_impl(&rest, v, effective)?;
    Ok((val, effective))
}

/// 不含版本处理的底层解析（文本入口）
fn parse_impl(text: &str, version: Version, features: FeatureSet) -> Result<Value, String> {
    let toks = tokenize(text)?;
    parse_impl_tokens(toks, version, features)
}

/// 不含版本处理的底层解析（token 流入口，供 include 展开后零拷贝复用）
fn parse_impl_tokens(
    toks: Vec<Tok>,
    version: Version,
    features: FeatureSet,
) -> Result<Value, String> {
    let mut p = Parser {
        toks,
        i: 0,
        fragments: BTreeMap::new(),
        contracts: BTreeMap::new(),
        features,
        ns_stack: Vec::new(),
    };
    // 顶层支持三种形态，与 `to_sml` 的输出对称：
    //   - `[ ... ]` 数组：to_sml 对非对象走 dump_inline，会输出顶层数组
    //     （如「历史记录」这类对象数组）。此前 parse 只认键值块，导致
    //     能序列化却读不回（"期望键, 得 LBrack"），是不对称缺陷。
    //   - `{ ... }` 顶层对象块
    //   - 键值块（传统形态）
    // 注：顶层**标量**仍不可往返（SML 顶层需为容器），这是格式固有限制。
    match p.peek() {
        Some(Tok::LBrack) => {
            if !p.features.has(Feature::TopArray) {
                return Err("sml: 顶层数组需要特性 `top-level-array`，但当前特性集已禁用".into());
            }
            p.next();
            p.parse_array()
        }
        Some(Tok::LBrace) => {
            p.next();
            p.parse_block(Some(Tok::RBrace))
        }
        _ => p.parse_block(None),
    }
}

// ---------------------------------------------------------------------------
// include 指令：把外部 .sml 文件内联进来
//
// 语法：`include "path.sml"` 或 `@include "path.sml"`（两种等价）
// 语义：**文本内联**（类似 C 的 #include），而非对象合并。
//   这样 include 可以出现在块内部引入一组字段，例如：
//       server web { &base include "common/port.sml" }
//   若做成对象合并就无法表达「注入若干字段到当前块」。
//
// 相对路径按**被包含文件自身所在目录**解析（与 C 预处理器一致），
// 而非进程工作目录，因此嵌套 include 时路径行为可预期。
// ---------------------------------------------------------------------------

/// 嵌套深度上限：既防栈溢出，也让异常深层的引用尽早失败
const MAX_INCLUDE_DEPTH: usize = 32;

/// 剥离行尾注释，正确跳过引号内的 `#`（如 `key: "a#b"` 中的 # 不是注释起点）
fn strip_line_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_quote = false;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_quote = !in_quote,
            // 引号内的反斜杠会转义下一个字符，需整体跳过
            b'\\' if in_quote => i += 1,
            b'#' if !in_quote => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// 单个 include 目标的解析结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludeTarget {
    /// 相对路径或裸名（无扩展名时按 `implicit-ns` 推导 `as`）。
    pub raw: String,
    /// 命名空间（点分路径 `a.b.c`）。`None` 表示普通内联。
    /// 若 `raw` 无扩展名且开启 `implicit-ns`，则自动填充为文件名。
    pub namespace: Option<String>,
    /// 是否经 `import` 关键字（语义等同 `include`）。
    pub via_import: bool,
    /// 部分引用：仅从目标文件挑出这些顶层键并入（命名空间包裹时同样只挑这些）。
    /// `None` 表示整文件（不挑键）。
    pub keys: Option<Vec<String>>,
}

/// 解析一行 include / import 指令，返回 0..N 个目标。
///
/// 支持形态（逗号分隔多目标，`import` 为 `include` 别名）：
/// - `include "x.sml"`                普通内联（带扩展名、无 as）
/// - `include "foo"`                  无扩展名 ⇒ 默认 `as foo`（implicit-ns）
/// - `include "x.sml" as ui.form`     命名空间内联（点分路径）
/// - `include "a", "b" as y, "c"`     多目标（multi-include）
/// - `import ui.buttons, admin.panel`  import 别名
/// - `include "*.sml"`                glob 通配（需 `glob-include`）
/// - `include re:"widget_.*\.sml"`    正则匹配（需 `regex-include`）
///
/// 部分引用（挑键）—— 两种等价写法：
/// - `import "x.sml" as w { a, b }`          挑键 a,b，挂到命名空间 w
/// - `import { a, b } as w in "x.sml"`        等价写法（键列表在前）
/// 省略 `as w` 则挑出的键直接平铺到当前作用域：
/// - `import "x.sml" { a, b }`
/// - `import { a, b } in "x.sml"`
/// 注：部分引用只作用于单文件目标，不与 glob/regex 通配组合。
///
/// 返回 `Ok(None)` 表示该行不是 include 指令；`Err` 表示特性未开启等语义错误。
fn parse_include_line(line: &str, features: FeatureSet) -> Result<Option<Vec<IncludeTarget>>, String> {
    let content = strip_line_comment(line).trim();
    let content = content.strip_prefix('@').unwrap_or(content).trim_start();
    // 轻量手写解析，不依赖 tokenize（避免 `*` 等字符在 tokenize 阶段被误判）。
    // 形式：`include "x" [as ns], "y" as ns2, ...`（import 等价）
    let (via_import, rest) = if let Some(r) = content.strip_prefix("include ") {
        (false, r.trim_start())
    } else if let Some(r) = content.strip_prefix("import ") {
        (true, r.trim_start())
    } else {
        return Ok(None);
    };
    if !features.has(Feature::Include) {
        return Ok(None);
    }
    let mut targets: Vec<IncludeTarget> = Vec::new();
    let mut rest = rest;
    loop {
        // 两种部分引用语法：
        //   ①  import "x.sml" [as w] { a, b }
        //   ②  import { a, b } [as w] in "x.sml"
        // 先探测是否以 `{` 开头（语法②）
        let (raw, ns, keys, tail) = if rest.trim_start().starts_with('{') {
            // 语法②：键列表在前
            let (keys, after) = parse_key_list(rest.trim_start())?;
            let after = after.trim_start();
            // 可选 `as ns`
            let (ns, after) = if let Some(stripped) = after.strip_prefix("as ") {
                let (n, t) = match next_token(stripped.trim_start()) {
                    Some((n, t)) => (Some(n), t.trim_start()),
                    None => return Ok(None),
                };
                (n, t)
            } else {
                (None, after)
            };
            // 必须跟 `in "path"` 取目标文件
            let after = after.trim_start();
            let after = match after.strip_prefix("in ") {
                Some(a) => a.trim_start(),
                None => {
                    return Err(
                        "sml: `import { keys } ...` 必须接 `in \"file\"` 指定目标文件".into(),
                    )
                }
            };
            let (path, t) = match next_token(after) {
                Some((p, t)) => (p, t),
                None => return Ok(None),
            };
            (path, ns, Some(keys), t)
        } else {
            // 语法①：路径在前
            let (path, tail0) = match next_token(rest) {
                Some((p, t)) => (p, t),
                None => {
                    if targets.is_empty() && rest.trim().is_empty() {
                        return Ok(None);
                    } else {
                        break;
                    }
                }
            };
            let mut r = tail0.trim_start();
            // 可选 `as ns`
            let mut ns: Option<String> = None;
            if let Some(stripped) = r.strip_prefix("as ") {
                let (n, t) = match next_token(stripped.trim_start()) {
                    Some((n, t)) => (n, t),
                    None => return Ok(None),
                };
                ns = Some(n);
                r = t.trim_start();
            }
            // 可选 `{ keys }`
            let keys = if r.starts_with('{') {
                let (k, after) = parse_key_list(r)?;
                r = after.trim_start();
                Some(k)
            } else {
                None
            };
            (path, ns, keys, r)
        };
        targets.push(finalize_target(
            raw,
            ns,
            via_import,
            features,
            keys,
        ));
        // 逗号分隔多目标（用已修剪的 tail 判断是否还有下一个目标）
        if let Some(stripped) = tail.strip_prefix(',') {
            if !features.has(Feature::MultiInclude) {
                return Ok(None);
            }
            rest = stripped.trim_start();
            continue;
        } else {
            rest = tail;
            break;
        }
    }
    if targets.is_empty() {
        return Ok(None);
    }
    // 特性预检查：glob / regex 模式在解析阶段就拦截（避免走到普通路径解析引发诡异错误）
    for t in &targets {
        // 部分引用只作用于单文件目标，不能与 glob/regex 通配组合
        if t.keys.is_some() && (t.raw.contains('*') || t.raw.starts_with("re:")) {
            return Err(
                "sml: 部分引用 `{ keys }` 不能配合 glob/regex 通配（请指定单个文件）".into(),
            );
        }
        // 先查 re: 前缀（正则模式里的 `*` 是元字符，不是 glob 通配）
        if t.raw.starts_with("re:") {
            if !features.has(Feature::RegexInclude) {
                return Err("sml: 正则 include 需要特性 `regex-include`（请 @feature enable regex-include）".into());
            }
            continue;
        }
        if t.raw.contains('*') && !features.has(Feature::GlobInclude) {
            return Err("sml: 通配 include 需要特性 `glob-include`（请 @feature enable glob-include）".into());
        }
    }
    Ok(Some(targets))
}

/// 从字符串开头提取下一个 token：引号串（支持 `\"` 与 `\\`）或直到空白/逗号/`as` 的裸词。
/// 返回 (token 文本, 剩余字符串)。
fn next_token(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    if s.is_empty() {
        return None;
    }
    if s.starts_with('"') {
        // 引号串（按字节处理，路径通常为 ASCII）
        let bytes = s.as_bytes();
        let mut i = 1;
        let mut out = String::new();
        while i < bytes.len() {
            if bytes[i] == b'"' {
                i += 1;
                break;
            }
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                // 转义：保留转义后的字符（\. -> .，\" -> " 等）
                i += 1;
                out.push(bytes[i] as char);
                i += 1;
            } else {
                out.push(bytes[i] as char);
                i += 1;
            }
        }
        Some((out, &s[i..]))
    } else {
        // 裸词：取到空白或逗号
        let end = s
            .find(|c: char| c.is_whitespace() || c == ',')
            .unwrap_or(s.len());
        let (tok, tail) = s.split_at(end);
        Some((tok.trim().to_string(), tail))
    }
}

/// 解析 `{ a, b, c }` 形式的键列表，返回 (键名集合, 剩余字符串)。
/// 键名可为裸词或引号串。遇到非 `{` 开头时返回错误。
fn parse_key_list(s: &str) -> Result<(Vec<String>, &str), String> {
    let s = s.trim_start();
    let Some(body) = s.strip_prefix('{') else {
        return Err("sml: 期望 `{ key1, key2, ... }` 键列表".into());
    };
    let close = body.find('}').ok_or("sml: 键列表缺少闭合 `}`")?;
    let inner = &body[..close];
    let mut keys: Vec<String> = Vec::new();
    for part in inner.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // 支持引号串键，其余按裸词（去引号）
        if let Some(q) = part.strip_prefix('"') {
            let q = q.strip_suffix('"').unwrap_or(q);
            keys.push(q.to_string());
        } else {
            keys.push(part.to_string());
        }
    }
    if keys.is_empty() {
        return Err("sml: 键列表不能为空（至少指定一个键）".into());
    }
    Ok((keys, &body[close + 1..]))
}

/// 根据原始路径与可选命名空间，套用 implicit-ns 规则，产出最终目标。
fn finalize_target(
    raw: String,
    ns: Option<String>,
    via_import: bool,
    features: FeatureSet,
    keys: Option<Vec<String>>,
) -> IncludeTarget {
    let namespace = match ns {
        Some(n) => Some(n),
        None => {
            // 部分引用（指定了 keys）且无显式 `as`：强制平铺到当前作用域，
            // 不触发 implicit-ns 自动命名空间（否则挑出的键会被塞进文件名命名空间）。
            if keys.is_some() {
                None
            } else if via_import || (features.has(Feature::ImplicitNs) && !raw.contains('.')) {
                // `import a.b.c`：点分一律视为命名空间路径，自动 `as a.b.c`
                // `include "foo"`（无点）：implicit-ns 默认以文件名为命名空间
                Some(raw.clone())
            } else {
                None
            }
        }
    };
    IncludeTarget {
        raw,
        namespace,
        via_import,
        keys,
    }
}

/// 把一个 include 目标解析为 0..N 个实际文件路径（已相对 `base` 解析、未 canonicalize）。
///
/// 支持：
/// - glob：`raw` 含 `*` 且开启 `glob-include` → 遍历 `base` 下直接条目做 `*` 通配匹配
/// - 正则：`raw` 以 `re:"..."` 形式且开启 `regex-include` → 遍历 `base` 下条目做最小正则匹配
/// - ext-rewrite：开启 `ext-rewrite` 时允许 `raw` 带非 `.sml` 扩展名（否则按原补 `.sml` 逻辑）
/// - 普通：`import` 点分转目录层级、裸名补 `.sml`
fn resolve_target_paths(
    t: &IncludeTarget,
    base: &Path,
    features: FeatureSet,
) -> Result<Vec<PathBuf>, String> {
    // 正则模式：re:"<pattern>"
    if let Some(pat) = t.raw.strip_prefix("re:") {
        if !features.has(Feature::RegexInclude) {
            return Err("sml: 正则 include 需要特性 `regex-include`（请 @feature enable regex-include）".into());
        }
        let pat = pat.trim_matches('"');
        // 模式可含目录前缀（如 re:"lib/widget_.*"）：拆出目录并入 base（归一化分隔符）
        let pat = pat.replace('/', std::path::MAIN_SEPARATOR_STR);
        let (dir, pat) = split_dir(&pat);
        return glob_or_regex_dir(&base.join(dir), pat, Some(pat), features);
    }
    // glob 模式：含 `*`
    if t.raw.contains('*') {
        if !features.has(Feature::GlobInclude) {
            return Err("sml: 通配 include 需要特性 `glob-include`（请 @feature enable glob-include）".into());
        }
        let normalized = t.raw.replace('/', std::path::MAIN_SEPARATOR_STR);
        let (dir, pat) = split_dir(&normalized);
        return glob_or_regex_dir(&base.join(dir), pat, None, features);
    }
    // 普通路径
    let path = if t.via_import {
        // import 的「点分模块名」语义：仅当 raw 既无路径分隔、又不显式带 .sml 扩展名时，
        // 才把点当作目录层级分隔（a.b.c -> a/b/c.sml）。
        // 若显式写了路径或扩展名（如 "advanced_inc/widget_a.sml"），按字面路径处理。
        if t.raw.contains(std::path::MAIN_SEPARATOR) || t.raw.ends_with(".sml") {
            base.join(&t.raw)
        } else {
            let rel = t
                .raw
                .split('.')
                .collect::<Vec<_>>()
                .join(std::path::MAIN_SEPARATOR_STR);
            base.join(rel).with_extension("sml")
        }
    } else if t.raw.contains('.') {
        // 带扩展名：默认直接读该文件
        // 开启 ext-rewrite 时允许非 .sml 扩展名（当 sml 解析）；关闭时若非 .sml 也允许读，
        // 但语义上仍要求文件存在，由 canonicalize 报错兜底。
        let _ = features.has(Feature::ExtRewrite);
        base.join(&t.raw)
    } else {
        base.join(format!("{}.sml", t.raw))
    };
    Ok(vec![path])
}

/// 遍历 `base` 目录的直接条目，按 glob（`pattern` 含 `*`）或正则（`regex` 为 Some）匹配，
/// 把 `a/b/pattern` 拆成 (`a/b`, `pattern`)，便于把目录部分并入 base。
fn split_dir(pat: &str) -> (&str, &str) {
    match pat.rfind(std::path::MAIN_SEPARATOR) {
        Some(idx) => (&pat[..idx], &pat[idx + 1..]),
        None => ("", pat),
    }
}

/// 返回命中的完整路径。目录本身不作为命中（仅文件）。
fn glob_or_regex_dir(
    base: &Path,
    pattern: &str,
    regex: Option<&str>,
    _features: FeatureSet,
) -> Result<Vec<PathBuf>, String> {
    let mut hits: Vec<PathBuf> = Vec::new();
    let entries = std::fs::read_dir(base)
        .map_err(|e| format!("include 目录读取失败 {}: {e}", base.display()))?;
    // 用于正则匹配的模式字符串（不含 re: 前缀与引号）
    let re = regex.map(|r| compile_regex(r));
    for ent in entries {
        let ent = ent.map_err(|e| format!("include 目录遍历失败: {e}"))?;
        let p = ent.path();
        if p.is_dir() {
            continue; // 只匹配文件
        }
        let name = match p.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        let matched = if let Some(re) = &re {
            regex_matches(re, name)
        } else {
            // glob：`pattern` 形如 `*.sml` 或 `widgets/*.sml`；这里只处理文件名部分的通配
            let pat_file = pattern.rsplit(std::path::MAIN_SEPARATOR).next().unwrap_or(pattern);
            glob_matches(pat_file, name)
        };
        if matched {
            hits.push(p);
        }
    }
    // 结果按文件名排序，保证跨平台顺序稳定
    hits.sort();
    Ok(hits)
}

/// 手写最小 glob 匹配（仅支持 `*` 通配，匹配整个文件名）。
fn glob_matches(pattern: &str, text: &str) -> bool {
    // 将 `a*b*c` 拆分为字面段，段间用 `*` 连接
    let segs: Vec<&str> = pattern.split('*').collect();
    if segs.is_empty() {
        return text.is_empty();
    }
    let mut pos = 0usize;
    // 首段若非 `*` 开头，必须前缀匹配
    if !pattern.starts_with('*') {
        if !text[pos..].starts_with(segs[0]) {
            return false;
        }
        pos += segs[0].len();
    }
    for seg in &segs[if pattern.starts_with('*') { 0 } else { 1 }..] {
        if seg.is_empty() {
            continue;
        }
        match text[pos..].find(seg) {
            Some(idx) => pos += idx + seg.len(),
            None => return false,
        }
    }
    // 末段若非 `*` 结尾，必须后缀匹配
    if !pattern.ends_with('*') {
        if pos != text.len() {
            return false;
        }
    }
    true
}

/// 编译一个受限正则（支持 `. * + ? ^ $ [a-z] [^a-z] \.` 转义），返回可匹配闭包用的结构。
/// 这里采用「NFA-less」的回溯匹配器，足够文件名场景使用。
struct MiniRegex {
    pattern: String,
}

fn compile_regex(pat: &str) -> MiniRegex {
    // 去掉可能的首尾 `^`/`$` 锚（由 matcher 解释）
    MiniRegex {
        pattern: pat.to_string(),
    }
}

/// 用受限正则匹配整个 `text`（默认全匹配，支持 `^`/`$` 锚点）。
fn regex_matches(re: &MiniRegex, text: &str) -> bool {
    let pat = &re.pattern;
    let anchored_start = pat.starts_with('^');
    let anchored_end = pat.ends_with('$');
    let p = if anchored_start { &pat[1..] } else { pat };
    let p = if anchored_end { &p[..p.len().saturating_sub(1)] } else { p };
    // 尝试从 text 的每个位置开始匹配（非锚定时）
    if anchored_start {
        backtrack_match(p, text, 0).is_some()
    } else {
        for start in 0..=text.len() {
            if backtrack_match(p, text, start).is_some() {
                if !anchored_end {
                    return true;
                }
                // 锚定结尾：必须匹配到 text 末端
                if backtrack_match(p, text, start) == Some(text.len()) {
                    return true;
                }
            }
        }
        false
    }
}

/// 回溯匹配：从 `text[ti]` 开始尝试匹配 `pat[pi]`，返回成功时 text 的消耗终点（usize）。
fn backtrack_match(pat: &str, text: &str, ti: usize) -> Option<usize> {
    // 递归实现，模式索引 pi 通过 chars 迭代
    let pchars: Vec<char> = pat.chars().collect();
    let tchars: Vec<char> = text.chars().collect();
    fn go(pchars: &[char], tchars: &[char], pi: usize, ti: usize) -> Option<usize> {
        let mut pi = pi;
        let mut ti = ti;
        while pi < pchars.len() {
            match pchars[pi] {
                '\\' => {
                    // 转义下一个字符（如 \. 匹配字面的 .）
                    if pi + 1 >= pchars.len() {
                        return None;
                    }
                    let pc = pchars[pi + 1];
                    if ti >= tchars.len() || tchars[ti] != pc {
                        return None;
                    }
                    pi += 2;
                    ti += 1;
                }
                '.' => {
                    if ti >= tchars.len() {
                        return None;
                    }
                    pi += 1;
                    ti += 1;
                }
                '*' => {
                    // 匹配前一个原子零次或多次（贪婪）
                    // 回退：尝试匹配零次（跳过 * 与前一原子），或匹配一次后继续
                    let prev = if pi >= 1 { Some(pchars[pi - 1]) } else { None };
                    // 零次：跳过 '*'（以及其前的普通原子已由上层处理，这里仅跳过 '*'）
                    // 但为简化，* 作用于前一原子：先尝试消耗一字符再递归
                    if ti < tchars.len() {
                        // 贪婪：尽量多匹配
                        let mut end = ti;
                        match prev {
                            Some('.') => {
                                while end < tchars.len() {
                                    end += 1;
                                }
                            }
                            Some(c) if c != '\\' => {
                                while end < tchars.len() && tchars[end] == c {
                                    end += 1;
                                }
                            }
                            _ => {}
                        }
                        // 从 end 回退尝试让后续模式匹配
                        let mut e = end;
                        while e >= ti {
                            if let Some(r) = go(pchars, tchars, pi + 1, e) {
                                return Some(r);
                            }
                            if e == ti {
                                break;
                            }
                            e -= 1;
                        }
                    }
                    // 零次匹配：跳过 '*'
                    return go(pchars, tchars, pi + 1, ti);
                }
                '+' => {
                    if ti >= tchars.len() {
                        return None;
                    }
                    let prev = pchars.get(pi.wrapping_sub(1)).copied();
                    let mut consumed = 0;
                    match prev {
                        Some('.') => {
                            if ti >= tchars.len() {
                                return None;
                            }
                            consumed = 1;
                        }
                        Some(c) if c != '\\' => {
                            if tchars[ti] != c {
                                return None;
                            }
                            consumed = 1;
                            while ti + consumed < tchars.len()
                                && tchars[ti + consumed] == c
                            {
                                consumed += 1;
                            }
                        }
                        _ => return None,
                    }
                    pi += 1;
                    ti += consumed;
                }
                '?' => {
                    // 前一原子的零或一
                    let prev = pchars.get(pi.wrapping_sub(1)).copied();
                    if ti < tchars.len() {
                        match prev {
                            Some('.') => {
                                pi += 1;
                                ti += 1;
                            }
                            Some(c) if c != '\\' => {
                                if tchars[ti] == c {
                                    pi += 1;
                                    ti += 1;
                                } else {
                                    pi += 1; // 零次
                                }
                            }
                            _ => {
                                pi += 1; // 零次
                            }
                        }
                    } else {
                        pi += 1;
                    }
                }
                '[' => {
                    // 字符类 [abc] 或 [^abc] 或 [a-z]
                    let mut j = pi + 1;
                    let negate = if j < pchars.len() && pchars[j] == '^' {
                        j += 1;
                        true
                    } else {
                        false
                    };
                    let mut cls = Vec::new();
                    while j < pchars.len() && pchars[j] != ']' {
                        if j + 2 < pchars.len()
                            && pchars[j + 1] == '-'
                            && pchars[j + 2] != ']'
                        {
                            let lo = pchars[j];
                            let hi = pchars[j + 2];
                            cls.push((lo, hi));
                            j += 3;
                        } else {
                            cls.push((pchars[j], pchars[j]));
                            j += 1;
                        }
                    }
                    if j >= pchars.len() {
                        return None; // 未闭合
                    }
                    if ti >= tchars.len() {
                        return None;
                    }
                    let c = tchars[ti];
                    let in_cls = cls.iter().any(|(lo, hi)| c >= *lo && c <= *hi);
                    let ok = if negate { !in_cls } else { in_cls };
                    if !ok {
                        return None;
                    }
                    pi = j + 1;
                    ti += 1;
                }
                c => {
                    if ti >= tchars.len() || tchars[ti] != c {
                        return None;
                    }
                    pi += 1;
                    ti += 1;
                }
            }
        }
        Some(ti)
    }
    go(&pchars, &tchars, 0, ti)
}

/// 把 text 中的 include 指令递归展开为不含指令的纯 SML 文本。
///
/// `base` 为相对路径的解析基准目录（通常是当前文件所在目录）。
/// `features` 决定是否允许 `include` / `namespace`（禁用则遇到指令即报错）。
/// 循环引用与缺失文件都会返回错误，不会静默跳过。
/// 把 text 中的 include 指令递归展开为 token 流（方向 B：零拷贝，不拼巨大中间字符串）。
///
/// 每个被包含文件只 `tokenize` 一次；命名空间 `as a.b.c` 用零拷贝的开/闭块 token
/// （`Word(a) LBrace Word(b) LBrace Word(c) LBrace ... RBrace RBrace RBrace`）包裹，
/// 不复制文件内容文本。子文件内的 `@version`/`@feature` 指令行在 tokenize 前被剥离，
/// 由主文件统一控制特性集（符合「文档只能收窄」的设计）。
pub fn resolve_includes(
    text: &str,
    base: &Path,
    features: FeatureSet,
) -> Result<Vec<Tok>, String> {
    let mut stack: Vec<PathBuf> = Vec::new();
    let mut toks: Vec<Tok> = Vec::new();
    expand_includes(text, base, &mut stack, features, &mut toks)?;
    Ok(toks)
}

/// 递归展开 include 到 `out` token 流。
fn expand_includes(
    text: &str,
    base: &Path,
    stack: &mut Vec<PathBuf>,
    features: FeatureSet,
    out: &mut Vec<Tok>,
) -> Result<(), String> {
    if stack.len() >= MAX_INCLUDE_DEPTH {
        return Err(format!("include 嵌套超过 {MAX_INCLUDE_DEPTH} 层"));
    }
    for line in text.lines() {
        match parse_include_line(line, features)? {
            Some(targets) => {
                if !features.has(Feature::Include) {
                    return Err("sml: 当前特性集禁用了 include（include 特性）".into());
                }
                for t in targets {
                    if t.namespace.is_some() && !features.has(Feature::Namespace) {
                        return Err(
                            "sml: 当前特性集禁用了命名空间包含（namespace 特性）".into(),
                        );
                    }
                    // 把一个 target 解析为 0..N 个实际文件路径（支持 glob/regex/ext-rewrite）
                    let paths = resolve_target_paths(&t, base, features)?;
                    for path in paths {
                        let canon = path.canonicalize().map_err(|e| {
                            format!("include 无法定位 {}: {e}", path.display())
                        })?;
                        // stack 是「当前正在展开的文件链」，命中即成环
                        if stack.iter().any(|p| p == &canon) {
                            return Err(format!("include 循环引用: {}", canon.display()));
                        }
                        let content = std::fs::read_to_string(&canon)
                            .map_err(|e| format!("include 读取失败 {}: {e}", canon.display()))?;
                        let child_base = canon
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| PathBuf::from("."));
                        stack.push(canon.clone());
                        // 展开子文件 tokens
                        let mut inner =
                            expand_file_tokens(&content, &child_base, stack, features)?;
                        // 部分引用：仅保留指定顶层键（命名空间包裹时同样只挑这些）
                        if let Some(keys) = &t.keys {
                            inner = filter_top_level_keys(inner, keys);
                        }
                        // 命名空间包含：用 `ns { ... }` 包裹子文件 tokens（零拷贝）
                        if let Some(ns) = &t.namespace {
                            for seg in ns.split('.') {
                                out.push(Tok::Word(seg.to_string()));
                                out.push(Tok::LBrace);
                            }
                            out.extend(inner);
                            for _ in ns.split('.') {
                                out.push(Tok::RBrace);
                            }
                        } else {
                            out.extend(inner);
                        }
                        stack.pop();
                    }
                }
            }
            None => {
                // 非 include 行：直接 tokenize 该行并追加（保持行级语义，零拷贝）
                let line_toks = tokenize(line).map_err(|e| {
                    format!("include 预处理词法错误：{e}（于行：{line}）")
                })?;
                out.extend(line_toks);
            }
        }
    }
    Ok(())
}

/// 读取单个文件内容，剥离其自身的 `@version`/`@feature` 行后 tokenize。
/// 子文件不引入新特性维度，由主文件/调用方统一控制。
/// 仅保留 `toks` 中顶层键名属于 `keys` 的条目；其余顶层条目被丢弃。
/// 嵌套层级（块 `{}` / 数组 `[]`）内的键不受影响——只有 depth==0 的顶层键被过滤。
/// 用于 `import "x" { a, b }` 部分引用：避免整文件内联。
fn filter_top_level_keys(toks: Vec<Tok>, keys: &[String]) -> Vec<Tok> {
    let key_set: std::collections::HashSet<&str> = keys.iter().map(|s| s.as_str()).collect();
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    let mut i = 0;
    let n = toks.len();
    while i < n {
        // 顶层必须是键（Word/Str）起始；非键 token 原样保留以免破坏结构
        if !matches!(toks[i], Tok::Word(_) | Tok::Str(_)) {
            out.push(toks[i].clone());
            i += 1;
            continue;
        }
        let key_name = match &toks[i] {
            Tok::Word(w) => w.clone(),
            Tok::Str(s) => s.clone(),
            _ => unreachable!(),
        };
        // 计算该顶层条目 [i, j) 的结束位置
        let j = if i + 1 < n {
            match &toks[i + 1] {
                // key: value —— 值从其后的 token 开始
                Tok::Colon => {
                    if i + 2 < n {
                        match &toks[i + 2] {
                            // 值为块/数组：配对括号
                            Tok::LBrace | Tok::LBrack => {
                                let mut depth = 1i32;
                                let mut k = i + 3;
                                while k < n {
                                    match &toks[k] {
                                        Tok::LBrace | Tok::LBrack => depth += 1,
                                        Tok::RBrace | Tok::RBrack => {
                                            depth -= 1;
                                            if depth == 0 {
                                                break;
                                            }
                                        }
                                        _ => {}
                                    }
                                    k += 1;
                                }
                                (k + 1).min(n)
                            }
                            // 单 token 值
                            _ => i + 3,
                        }
                    } else {
                        i + 2
                    }
                }
                // key { ... } / key [ ... ] —— 直接配对括号
                Tok::LBrace | Tok::LBrack => {
                    let mut depth = 1i32;
                    let mut k = i + 2;
                    while k < n {
                        match &toks[k] {
                            Tok::LBrace | Tok::LBrack => depth += 1,
                            Tok::RBrace | Tok::RBrack => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        k += 1;
                    }
                    (k + 1).min(n)
                }
                // 裸词独立行等：单 token 条目
                _ => i + 1,
            }
        } else {
            i + 1
        };
        if key_set.contains(key_name.as_str()) {
            for t in &toks[i..j] {
                out.push(t.clone());
            }
        }
        i = j;
    }
    out
}

fn expand_file_tokens(
    content: &str,
    base: &Path,
    stack: &mut Vec<PathBuf>,
    features: FeatureSet,
) -> Result<Vec<Tok>, String> {
    // 剥离子文件内的版本/特性指令行，避免污染 token 流
    let cleaned: String = content
        .lines()
        .filter(|l| {
            let t = strip_line_comment(l).trim();
            let t = t.strip_prefix('@').unwrap_or(t).trim_start();
            !(t.starts_with("version") || t.starts_with("feature"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut toks = Vec::new();
    expand_includes(&cleaned, base, stack, features, &mut toks)?;
    Ok(toks)
}

/// 解析 SML 文件，并展开其中的 include 指令。
///
/// 相对路径以**该文件所在目录**为基准。include 展开为零拷贝 token 流，
/// 不拼接中间大字符串（方向 B）。
pub fn parse_file(path: impl AsRef<Path>) -> Result<Value, String> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读取失败 {}: {e}", path.display()))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    // 主文件先剥离版本/特性指令。
    // 便捷入口 `parse_file` 的「调用方允许集」为全开（文档自身声明决定启用哪些特性，
    // 真正的调用方限制由 `parse_with_features` / `parse_allowed` 负责）。
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, base_ver, had) = strip_features(&rest)?;
    let v = declared.or(base_ver).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    let allowed = FeatureSet::all().intersection(feats);
    let toks = resolve_includes(&rest, &base, allowed)?;
    parse_impl_tokens(toks, v, allowed)
}

/// 解析到对象 (失败抛 `ParseError`)
pub fn loads(text: &str) -> Result<Value, ParseError> {
    parse(text).map_err(ParseError)
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sml parse error: {}", self.0)
    }
}

impl std::error::Error for ParseError {}

// ---------------------------------------------------------------------------
// 序列化
// ---------------------------------------------------------------------------

fn quote_if_needed(s: &str) -> String {
    if s.is_empty() || s.contains([' ', '\t', '\n', '\r', ':', '#', '{', '}']) {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

/// 输出一个块。含 `__type` / `__name` 的块也按普通块原样输出所有键，
/// 保证元数据（枚举带数据变体的 `__type` 标记等）可完整往返。
/// SML 的裸块 `type [name] { ... }` 解析后正是 `__type` / `__name` 键。
fn dump_block(m: &BTreeMap<String, Value>, indent: usize, out: &mut String) {
    if m.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str(&format!("\n{}{{", "  ".repeat(indent)));
    for (k, val) in m {
        out.push_str(&format!("\n{}{}: ", "  ".repeat(indent + 1), k));
        dump_value(val, indent + 1, out);
    }
    out.push_str(&format!("\n{}}}", "  ".repeat(indent)));
}

fn dump_value(v: &Value, indent: usize, out: &mut String) {
    let pad = "  ".repeat(indent);
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => out.push_str(&format!("{}", f)),
        Value::Str(s) => out.push_str(&quote_if_needed(s)),
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
            } else {
                out.push('[');
                for e in a {
                    out.push('\n');
                    out.push_str(&format!("{}{}", "  ".repeat(indent + 1), dump_inline(e)));
                }
                out.push_str(&format!("\n{}]", pad));
            }
        }
        Value::Object(m) => dump_block(m, indent, out),
    }
}

fn dump_scalar(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Str(s) => quote_if_needed(s),
        _ => "".into(),
    }
}

fn dump_inline(v: &Value) -> String {
    match v {
        Value::Object(m) => {
            // 含 __type/__name 的块原样输出所有键，保证元数据可往返
            let parts: Vec<String> = m
                .iter()
                .map(|(k, val)| format!("{}: {}", k, dump_inline(val)))
                .collect();
            format!("{{ {} }}", parts.join(", "))
        }
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(dump_inline).collect();
            format!("[ {} ]", parts.join(", "))
        }
        other => dump_scalar(other),
    }
}

/// 序列化回 SML 文本 (round-trip)
///
/// 含 `__type` / `__name` 的块（如枚举带数据变体序列化的结果）
/// 会原样输出所有键，保证元数据可完整往返。
pub fn to_sml(v: &Value) -> String {
    let mut out = String::new();
    if let Value::Object(m) = v {
        if m.contains_key("__type") {
            dump_block(m, 0, &mut out);
        } else {
            for (k, val) in m {
                out.push_str(&format!("{}: ", k));
                dump_value(val, 0, &mut out);
                out.push('\n');
            }
        }
    } else {
        out.push_str(&dump_inline(v));
    }
    out
}

// ---------------------------------------------------------------------------
// C-ABI (cdylib, 供 C / 其它语言调用)
// ---------------------------------------------------------------------------

use std::os::raw::{c_char, c_int};
use std::ptr;

fn cstr(s: &str) -> *mut c_char {
    let c = std::ffi::CString::new(s).unwrap_or_default();
    c.into_raw()
}

/// sml_parse(text) -> 返回 JSON 字符串 (调用方 sml_free 释放); 失败返回 NULL
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_parse(text: *const c_char) -> *mut c_char {
    if text.is_null() {
        return ptr::null_mut();
    }
    let t = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned();
    match parse(&t) {
        Ok(v) => cstr(&jsonify(&v)),
        Err(_) => ptr::null_mut(),
    }
}

/// sml_dump(json) -> 接受 JSON 字符串, 序列化为 SML; 调用方 sml_free
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_dump(json: *const c_char) -> *mut c_char {
    if json.is_null() {
        return ptr::null_mut();
    }
    let j = unsafe { std::ffi::CStr::from_ptr(json) }.to_string_lossy().into_owned();
    match json_to_value(&j) {
        Some(v) => cstr(&to_sml(&v)),
        None => ptr::null_mut(),
    }
}

/// sml_free_str(p): 释放由 sml_parse / sml_dump / sml_dumps 等返回的字符串。
///
/// 注：早期版本此函数名为 `sml_free`，与 `sml.h`（纯 C 后端）的
/// `sml_free(sml_value*)` 语义冲突。为让两个后端心智模型一致
/// （`sml_free` 释放值树、`sml_free_str` 释放字符串），此处重命名。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_free_str(p: *mut c_char) {
    if !p.is_null() {
        drop(unsafe { std::ffi::CString::from_raw(p) });
    }
}

/// sml_version() -> 版本静态字符串（**无需释放**，与 jansson 的
/// `jansson_version_str()` 语义一致）。
///
/// 返回指向编译期常量的指针，生命周期为 `'static`。
/// 需要可释放的副本请用 [`sml_version_str`]。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_version() -> *const c_char {
    concat!("sml ", env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

// ---------------------------------------------------------------------------
// v3 扩展 ABI (与基础 sml_parse 并存, 不破坏既有符号)
// 暴露 $env 内联 / glob-include / @feature / @contract 等 v3 能力。
// 这些入口独立于 c/ 与 cpp/ 的纯 native 实现, 仅供需要完整 v3 功能的
// 调用方链接本 cdylib 使用。
// ---------------------------------------------------------------------------

/// 极简解析 opts JSON: 仅识别顶层 object 的
///   "features": [ "glob-include", ... ]
///   "env":      { "KEY": "VAL", ... }
///   "allow":    [ "v1", "v2", "v3" ]
/// 返回 (features: Vec<Feature>, env: Vec<(String,String)>, allow: Vec<Version>)。
/// 任何字段缺失即视为空/不限制; 解析失败返回 Err。
fn parse_opts_json(opts: &str) -> Result<(Vec<Feature>, Vec<(String, String)>, Vec<Version>), String> {
    let mut features: Vec<Feature> = Vec::new();
    let mut env: Vec<(String, String)> = Vec::new();
    let mut allow: Vec<Version> = Vec::new();
    if opts.trim().is_empty() {
        return Ok((features, env, allow));
    }
    // 手工 tokenizer: 仅支持本结构, 不引第三方依赖。
    let b = opts.as_bytes();
    let mut i = 0usize;
    let len = b.len();
    // 跳到首个 {
    while i < len && b[i] != b'{' { i += 1; }
    if i >= len { return Err("opts 不是 JSON object".into()); }
    i += 1; // 越过 {
    loop {
        // 跳空白与逗号
        while i < len && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r' || b[i] == b',') { i += 1; }
        if i >= len || b[i] == b'}' { break; }
        // 读 key (双引号字符串)
        if b[i] != b'"' { return Err("opts key 须为字符串".into()); }
        i += 1;
        let ks = i;
        while i < len && b[i] != b'"' { i += 1; }
        let key = std::str::from_utf8(&b[ks..i]).map_err(|_| "opts key 非法 UTF-8".to_string())?.to_string();
        i += 1; // 越过 "
        while i < len && (b[i] == b' ' || b[i] == b':' || b[i] == b'\t') { i += 1; }
        match key.as_str() {
            "features" | "allow" => {
                // 读数组 [ ... ]
                if i >= len || b[i] != b'[' { return Err(format!("opts.{key} 须为数组")); }
                i += 1;
                loop {
                    while i < len && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r' || b[i] == b',') { i += 1; }
                    if i < len && b[i] == b']' { i += 1; break; }
                    if i >= len || b[i] != b'"' { return Err(format!("opts.{key} 元素须为字符串")); }
                    i += 1;
                    let vs = i;
                    while i < len && b[i] != b'"' { i += 1; }
                    let val = std::str::from_utf8(&b[vs..i]).map_err(|_| "opts 值非法 UTF-8".to_string())?.to_string();
                    i += 1;
                    if key == "features" {
                        features.push(Feature::from_name(&val).ok_or_else(|| format!("未知特性 {val}"))?);
                    } else {
                        allow.push(Version::from_word(&val).ok_or_else(|| format!("未知版本 {val}"))?);
                    }
                }
            }
            "env" => {
                if i >= len || b[i] != b'{' { return Err("opts.env 须为 object".into()); }
                i += 1;
                loop {
                    while i < len && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r' || b[i] == b',') { i += 1; }
                    if i < len && b[i] == b'}' { i += 1; break; }
                    if i >= len || b[i] != b'"' { return Err("opts.env key 须为字符串".into()); }
                    i += 1;
                    let ks = i;
                    while i < len && b[i] != b'"' { i += 1; }
                    let ek = std::str::from_utf8(&b[ks..i]).map_err(|_| "opts.env key 非法".to_string())?.to_string();
                    i += 1;
                    while i < len && (b[i] == b' ' || b[i] == b':' || b[i] == b'\t') { i += 1; }
                    if i >= len || b[i] != b'"' { return Err("opts.env value 须为字符串".into()); }
                    i += 1;
                    let vs = i;
                    while i < len && b[i] != b'"' { i += 1; }
                    let ev = std::str::from_utf8(&b[vs..i]).map_err(|_| "opts.env value 非法".to_string())?.to_string();
                    i += 1;
                    env.push((ek, ev));
                }
            }
            _ => {
                // 跳过未知字段的值 (标量/数组/对象)
                let mut depth = 0i32;
                loop {
                    if i >= len { break; }
                    match b[i] {
                        b'"' => { i += 1; while i < len && b[i] != b'"' { if b[i] == b'\\' { i += 2; } else { i += 1; } } i += 1; }
                        b'{' | b'[' => { depth += 1; i += 1; }
                        b'}' | b']' => { depth -= 1; i += 1; if depth <= 0 { break; } }
                        _ => { i += 1; }
                    }
                }
            }
        }
    }
    Ok((features, env, allow))
}

/// sml_parse_ex(text, opts_json) -> JSON 字符串 (调用方 sml_free) 或 NULL。
///
/// opts_json 示例:
///   {"features":["glob-include","contract"],"env":{"APP_ENV":"prod"},"allow":["v1","v3"]}
/// - features: 调用方额外启用的特性 (与文档 @feature 取交集)。
/// - env:      注入到进程环境, 供 `$env.X` 内联解析 (调用期间临时设置并恢复)。
/// - allow:    限定文档声明的版本必须在此范围内; 空数组表示不限制。
/// 失败 (语法/版本/特性越权/文件找不到) 返回 NULL。
// env 注入/恢复：edition 2024 起 set_var/remove_var 为 unsafe，
// 需 unsafe 块；edition 2021 下该块多余，故一并 allow 掉告警。
#[allow(unused_unsafe)]
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_parse_ex(text: *const c_char, opts: *const c_char) -> *mut c_char {
    if text.is_null() {
        return ptr::null_mut();
    }
    let t = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let opts_str = if opts.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(opts) }.to_string_lossy().into_owned()
    };
    let (feats, env, allow) = match parse_opts_json(&opts_str) {
        Ok(x) => x,
        Err(_) => return ptr::null_mut(),
    };
    // 临时注入 env (非并发安全, FFI 同步调用假设)。
    let prev: Vec<(String, Option<String>)> = env
        .iter()
        .map(|(k, _)| (k.clone(), std::env::var(k).ok()))
        .collect();
    for (k, v) in &env {
        unsafe { std::env::set_var(k, v) };
    }
    let result = (|| {
        // 构造调用方允许特性集: 基础全集 并 上 opts 指定特性。
        let mut allowed = FeatureSet::all();
        for f in &feats {
            allowed = allowed.with(*f);
        }
        let val = parse_with_features(&t, allowed).map(|(v, _)| v)?;
        if !allow.is_empty() {
            let declared = strip_version(&t).ok().and_then(|(_, d)| d);
            if let Some(d) = declared {
                if !allow.contains(&d) {
                    return Err(format!("文档声明版本 {} 不在 allow 范围", d.name()));
                }
            }
        }
        Ok(jsonify(&val))
    })();
    // 恢复 env
    for (k, v) in &prev {
        match v {
            Some(old) => unsafe { std::env::set_var(k, old) },
            None => unsafe { std::env::remove_var(k) },
        }
    }
    match result {
        Ok(s) => cstr(&s),
        Err(_) => ptr::null_mut(),
    }
}

/// sml_parse_file(path) -> JSON 字符串 (调用方 sml_free) 或 NULL。
/// 桥接内部 parse_file: 自动处理 include / glob / @contract 校验, 带文件上下文。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_parse_file(path: *const c_char) -> *mut c_char {
    if path.is_null() {
        return ptr::null_mut();
    }
    let p = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy().into_owned();
    match parse_file(&p) {
        Ok(v) => cstr(&jsonify(&v)),
        Err(_) => ptr::null_mut(),
    }
}

/// sml_features() -> 当前支持的特性名 JSON 数组 (调用方 sml_free)。
/// 例: ["include","env","contract","glob-include", ...]
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_features() -> *mut c_char {
    let names: Vec<&str> = FEATURES.iter().map(|(n, _)| *n).collect();
    let body = names
        .iter()
        .map(|n| format!("\"{}\"", n))
        .collect::<Vec<_>>()
        .join(",");
    cstr(&format!("[{}]", body))
}

// ---------------------------------------------------------------------------
// C-ABI: 值树 (v2 API)
//
// 旧 API (sml_parse / sml_parse_file / sml_parse_ex) 以 JSON 字符串为交换格式，
// 迫使 C 侧再集成一个 JSON 库——这削弱了 SML 作为替代品的动机。
// 这套值树 API 让 C 直接遍历结果、直接读错误，零外部依赖。
//
// 设计参照 jansson (sml_error 详细定位 + flags 位标志) 与 tomlc99 (xxx_in 单行取值)。
// 生命周期约定：
//   * sml_loads / sml_load_file 返回的根指针由调用方 sml_free 释放；
//   * sml_get / sml_get_path / sml_at 返回**借用**指针 (const)，不可释放，
//     随根节点一同失效；
//   * 所有 char* 输出由调用方 sml_free_str 释放。
// ---------------------------------------------------------------------------

use std::os::raw::{c_uint, c_ulonglong};

/// 与 `sml_rs.h` 的 `sml_errc` 一一对应。
#[repr(C)]
#[derive(Clone, Copy)]
pub enum CSmlErrc {
    Ok = 0,
    Syntax = 1,
    FeatureDisabled = 2,
    VersionMismatch = 3,
    Contract = 4,
    IncludeLoop = 5,
    Io = 6,
    Utf8 = 7,
    Internal = 8,
}

/// 与 `sml_rs.h` 的 `sml_error` 一一对应。
///
/// 字段顺序、类型、数组长度必须与头文件完全一致，否则跨语言内存布局错位。
#[repr(C)]
pub struct CSmlError {
    pub code: c_int,
    pub line: c_int,
    pub column: c_int,
    pub position: usize,
    pub source: [c_char; 128],
    pub text: [c_char; 256],
}

impl CSmlError {
    /// 用错误信息填充一块调用方提供的内存。
    ///
    /// # Safety
    /// `out` 必须可写且按 [`CSmlError`] 布局对齐；为 NULL 时静默跳过。
    unsafe fn fill(out: *mut CSmlError, code: CSmlErrc, msg: &str, source: &str) {
        if out.is_null() {
            return;
        }
        let e = &mut *out;
        e.code = code as c_int;
        e.line = 0;
        e.column = 0;
        e.position = 0;
        e.source = [0; 128];
        e.text = [0; 256];
        copy_cstr(&mut e.source, source);
        copy_cstr(&mut e.text, msg);

        // 从消息里尽量还原行号：形如 "sml: 第 12 行 ..." / "... (line 12)"。
        if let Some(l) = extract_line(msg) {
            e.line = l;
        }
    }
}

/// 把 Rust `&str` 复制进定长 C 字符数组，保证 NUL 结尾且截断安全。
fn copy_cstr(dst: &mut [c_char], s: &str) {
    if dst.is_empty() {
        return;
    }
    let bytes = s.as_bytes();
    let n = bytes.len().min(dst.len() - 1);
    for i in 0..n {
        dst[i] = bytes[i] as c_char;
    }
    dst[n] = 0;
}

/// 从错误信息中抽取行号（尽力而为，抽不到返回 `None`）。
fn extract_line(msg: &str) -> Option<c_int> {
    for pat in ["第 ", "line "] {
        if let Some(idx) = msg.find(pat) {
            let rest = &msg[idx + pat.len()..];
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<i32>() {
                if n > 0 {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// 值树句柄。`repr(transparent)` 使其与内部 [`Value`] 布局一致，
/// 从而可以把子值的 `&Value` 安全地重解释为此类型的借用指针。
#[repr(transparent)]
pub struct CSmlValue(Value);

/// C 侧要释放的错误信息前缀判断：把解析错误归类。
fn classify(err: &str) -> CSmlErrc {
    if err.contains("include") && (err.contains("循环") || err.contains("loop")) {
        CSmlErrc::IncludeLoop
    } else if err.contains("特性") || err.contains("feature") {
        CSmlErrc::FeatureDisabled
    } else if err.contains("版本") || err.contains("version") {
        CSmlErrc::VersionMismatch
    } else if err.contains("契约") || err.contains("contract") {
        CSmlErrc::Contract
    } else if err.contains("读取失败") || err.contains("IO") {
        CSmlErrc::Io
    } else {
        CSmlErrc::Syntax
    }
}

/// `flags` 位 → [`FeatureSet`]。
///
/// `flags == 0` 视为「默认基线」（与 jansson 的 flags=0 语义一致），
/// 非 0 时按位精确构造，调用方可借此收紧允许范围。
fn feature_set_from_flags(flags: c_uint) -> FeatureSet {
    if flags == 0 {
        return FeatureSet::baseline();
    }
    let mut s = FeatureSet::none();
    for (i, (_, f)) in FEATURES.iter().enumerate() {
        if i >= 32 {
            break;
        }
        if flags & (1u32 << i) != 0 {
            s = s.with(*f);
        }
    }
    s
}

/// 解析 SML 文本为值树。
///
/// # Safety
/// `text` 必须是合法 NUL 结尾字符串或 NULL；`err` 可为 NULL。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_loads(
    text: *const c_char,
    flags: c_uint,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    if text.is_null() {
        CSmlError::fill(err, CSmlErrc::Internal, "sml_loads: text is NULL", "<string>");
        return ptr::null_mut();
    }
    let t = std::ffi::CStr::from_ptr(text).to_string_lossy().into_owned();
    let allowed = feature_set_from_flags(flags);
    match parse_with_features(&t, allowed) {
        Ok((v, _)) => Box::into_raw(Box::new(CSmlValue(v))),
        Err(e) => {
            CSmlError::fill(err, classify(&e), &e, "<string>");
            ptr::null_mut()
        }
    }
}

/// 解析 SML 文件为值树（展开 `include`，相对路径以文件所在目录为基准）。
///
/// # Safety
/// `path` 必须是合法 NUL 结尾字符串或 NULL；`err` 可为 NULL。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_load_file(
    path: *const c_char,
    flags: c_uint,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    if path.is_null() {
        CSmlError::fill(err, CSmlErrc::Internal, "sml_load_file: path is NULL", "<file>");
        return ptr::null_mut();
    }
    let p = std::ffi::CStr::from_ptr(path).to_string_lossy().into_owned();
    let _ = flags; // 文件入口的特性由文档 @feature 与 flags 共同决定
    match parse_file(&p) {
        Ok(v) => Box::into_raw(Box::new(CSmlValue(v))),
        Err(e) => {
            CSmlError::fill(err, classify(&e), &e, &p);
            ptr::null_mut()
        }
    }
}

/// 释放 [`sml_loads`] / [`sml_load_file`] 返回的根节点（NULL 安全）。
///
/// 与 `sml.h`（纯 C 后端）的 `sml_free` 语义一致：都是释放值树。
/// 释放字符串请用 [`sml_free_str`]。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_free(v: *mut CSmlValue) {
    if !v.is_null() {
        drop(Box::from_raw(v));
    }
}

/// 值类型判别，返回 `sml_type` 枚举值；NULL 或非预期返回 -1。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_typeof(v: *const CSmlValue) -> c_int {
    if v.is_null() {
        return -1;
    }
    let inner = &(*(v as *const Value));
    match inner {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Int(_) => 2,
        Value::Float(_) => 3,
        Value::Str(_) => 4,
        Value::Array(_) => 5,
        Value::Object(_) => 6,
    }
}

/// 取对象字段（**借用**，不可释放）；键不存在或类型不符返回 NULL。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_get(
    v: *const CSmlValue,
    key: *const c_char,
) -> *const CSmlValue {
    if v.is_null() || key.is_null() {
        return ptr::null();
    }
    let inner = &(*(v as *const Value));
    let k = std::ffi::CStr::from_ptr(key).to_string_lossy();
    match inner {
        Value::Object(m) => m
            .get(k.as_ref())
            .map(|x| x as *const Value as *const CSmlValue)
            .unwrap_or(ptr::null()),
        _ => ptr::null(),
    }
}

/// 按 `.` 分隔路径逐层取值（**借用**，不可释放）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_get_path(
    v: *const CSmlValue,
    path: *const c_char,
) -> *const CSmlValue {
    if v.is_null() || path.is_null() {
        return ptr::null();
    }
    let p = std::ffi::CStr::from_ptr(path).to_string_lossy();
    let mut cur: *const CSmlValue = v;
    for seg in p.split('.') {
        if seg.is_empty() {
            continue;
        }
        let c_seg = match std::ffi::CString::new(seg) {
            Ok(c) => c,
            Err(_) => return ptr::null(),
        };
        let next = sml_get(cur, c_seg.as_ptr());
        if next.is_null() {
            return ptr::null();
        }
        cur = next;
    }
    cur
}

/// 取数组第 `idx` 个元素（**借用**，不可释放）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_at(v: *const CSmlValue, idx: usize) -> *const CSmlValue {
    if v.is_null() {
        return ptr::null();
    }
    let inner = &(*(v as *const Value));
    match inner {
        Value::Array(a) => a
            .get(idx)
            .map(|x| x as *const Value as *const CSmlValue)
            .unwrap_or(ptr::null()),
        _ => ptr::null(),
    }
}

/// 元素个数（数组长度 / 对象字段数）；其它类型返回 0。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_size(v: *const CSmlValue) -> usize {
    if v.is_null() {
        return 0;
    }
    match &(*(v as *const Value)) {
        Value::Array(a) => a.len(),
        Value::Object(m) => m.len(),
        _ => 0,
    }
}

/// 把字符串值拷进调用方缓冲区，返回不含 NUL 的长度；缓冲区不足时返回所需长度。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_str_copy(
    v: *const CSmlValue,
    buf: *mut c_char,
    buflen: usize,
) -> usize {
    if v.is_null() {
        return 0;
    }
    let s = match &(*(v as *const Value)) {
        Value::Str(s) => s.as_str(),
        _ => return 0,
    };
    let need = s.len();
    if buf.is_null() || buflen == 0 {
        return need;
    }
    let n = need.min(buflen - 1);
    let src = s.as_bytes();
    for i in 0..n {
        *buf.add(i) = src[i] as c_char;
    }
    *buf.add(n) = 0;
    need
}

/// 字符串值的新分配副本（调用方 `sml_free_str` 释放）；非字符串返回 NULL。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_str_dup(v: *const CSmlValue) -> *mut c_char {
    if v.is_null() {
        return ptr::null_mut();
    }
    match &(*(v as *const Value)) {
        Value::Str(s) => cstr(s),
        _ => ptr::null_mut(),
    }
}

/// 整数取值；非整数返回 0（用 [`sml_typeof`] 先判别类型）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_int_value(v: *const CSmlValue) -> i64 {
    if v.is_null() {
        return 0;
    }
    match &(*(v as *const Value)) {
        Value::Int(i) => *i,
        Value::Float(f) => *f as i64,
        _ => 0,
    }
}

/// 浮点取值；非数值返回 0.0。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_real_value(v: *const CSmlValue) -> f64 {
    if v.is_null() {
        return 0.0;
    }
    match &(*(v as *const Value)) {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        _ => 0.0,
    }
}

/// 布尔取值；非布尔返回 0。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_bool_value(v: *const CSmlValue) -> c_int {
    if v.is_null() {
        return 0;
    }
    match &(*(v as *const Value)) {
        Value::Bool(b) => {
            if *b {
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

// —— tomlc99 风格的单行便利取值 ——

/// `sml_get_path` + [`sml_str_dup`] 的合体（调用方 `sml_free_str` 释放）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_str_in(
    v: *const CSmlValue,
    path: *const c_char,
) -> *mut c_char {
    let node = sml_get_path(v, path);
    if node.is_null() {
        return ptr::null_mut();
    }
    sml_str_dup(node)
}

/// `sml_get_path` + [`sml_int_value`]，经 `ok` 回传是否取到（可为 NULL）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_int_in(
    v: *const CSmlValue,
    path: *const c_char,
    ok: *mut c_int,
) -> i64 {
    let node = sml_get_path(v, path);
    if node.is_null() {
        if !ok.is_null() {
            *ok = 0;
        }
        return 0;
    }
    let is_int = sml_typeof(node) == 2;
    if !ok.is_null() {
        *ok = if is_int { 1 } else { 0 };
    }
    sml_int_value(node)
}

/// `sml_get_path` + [`sml_bool_value`]，经 `ok` 回传是否取到（可为 NULL）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_bool_in(
    v: *const CSmlValue,
    path: *const c_char,
    ok: *mut c_int,
) -> c_int {
    let node = sml_get_path(v, path);
    if node.is_null() {
        if !ok.is_null() {
            *ok = 0;
        }
        return 0;
    }
    let is_bool = sml_typeof(node) == 1;
    if !ok.is_null() {
        *ok = if is_bool { 1 } else { 0 };
    }
    sml_bool_value(node)
}

/// 把值树序列化为 SML 文本（调用方 `sml_free_str` 释放）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_dumps(v: *const CSmlValue, _flags: c_uint) -> *mut c_char {
    if v.is_null() {
        return ptr::null_mut();
    }
    cstr(&to_sml(&(*(v as *const Value))))
}

/// 返回该特性位对应的名字（静态字符串，无需释放）；越界返回 NULL。
///
/// 这里刻意用 `match` 返回带 `\0` 的字面量：直接取 [`FEATURES`] 里的
/// `&str` 无法保证 NUL 结尾，交给 C 会被 `printf("%s")` 越界读取。
/// 顺序与 [`FEATURES`] 表严格对应，由 `tests/version.rs` 中的用例守护。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_feature_name(bit: c_uint) -> *const c_char {
    let s: &'static str = match bit {
        0 => "bareword-string\0",
        1 => "include\0",
        2 => "env\0",
        3 => "contract\0",
        4 => "fragment\0",
        5 => "top-level-array\0",
        6 => "namespace\0",
        7 => "implicit-ns\0",
        8 => "multi-include\0",
        9 => "glob-include\0",
        10 => "regex-include\0",
        11 => "ext-rewrite\0",
        _ => return ptr::null(),
    };
    s.as_ptr() as *const c_char
}

/// 返回受支持特性的位掩码（可直接与 `SML_F_*` 按位与）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_features_mask() -> c_uint {
    let mut m = 0u32;
    for (i, _) in FEATURES.iter().enumerate() {
        if i >= 32 {
            break;
        }
        m |= 1u32 << i;
    }
    m
}

/// 库版本字符串（调用方 `sml_free_str` 释放）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_version_str() -> *mut c_char {
    cstr(env!("CARGO_PKG_VERSION"))
}

// 供 `#[no_mangle]` 之外的内部代码引用，避免 `c_ulonglong` 触发未使用警告。
#[allow(dead_code)]
type _CUnsignedLongLong = c_ulonglong;

// ---------------------------------------------------------------------------
// 内部: JSON <-> Value (供 C-ABI 便捷桥)
// ---------------------------------------------------------------------------

fn jsonify(v: &Value) -> String {
    fn esc(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Str(s) => format!("\"{}\"", esc(s)),
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(jsonify).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Object(m) => {
            let parts: Vec<String> = m
                .iter()
                .map(|(k, val)| format!("\"{}\":{}", esc(k), jsonify(val)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

fn json_to_value(s: &str) -> Option<Value> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let _n = bytes.len();
    let mut skip_ws = |b: &[u8], i: &mut usize| {
        while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
            *i += 1;
        }
    };
    let mut parse_str = |b: &[u8], i: &mut usize| -> Option<String> {
        skip_ws(b, i);
        if *i >= b.len() || b[*i] != b'"' {
            return None;
        }
        *i += 1;
        let mut out = String::new();
        while *i < b.len() {
            let c = b[*i];
            if c == b'"' {
                *i += 1;
                return Some(out);
            }
            if c == b'\\' && *i + 1 < b.len() {
                *i += 1;
                let e = b[*i];
                out.push(match e {
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    b'"' => '"',
                    b'\\' => '\\',
                    _ => e as char,
                });
            } else {
                out.push(c as char);
            }
            *i += 1;
        }
        None
    };
    fn parse_val_impl(
        b: &[u8],
        i: &mut usize,
        s: &str,
        parse_str: &dyn Fn(&[u8], &mut usize) -> Option<String>,
    ) -> Option<Value> {
        let mut skip_ws = |b: &[u8], i: &mut usize| {
            while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
                *i += 1;
            }
        };
        skip_ws(b, i);
        if *i >= b.len() {
            return None;
        }
        match b[*i] {
            b'{' => {
                *i += 1;
                let mut m = BTreeMap::new();
                skip_ws(b, i);
                if *i < b.len() && b[*i] == b'}' {
                    *i += 1;
                    return Some(Value::Object(m));
                }
                loop {
                    skip_ws(b, i);
                    let k = parse_str(b, i)?;
                    skip_ws(b, i);
                    if *i < b.len() && b[*i] == b':' {
                        *i += 1;
                    }
                    let v = parse_val_impl(b, i, s, parse_str)?;
                    m.insert(k, v);
                    skip_ws(b, i);
                    if *i < b.len() && b[*i] == b',' {
                        *i += 1;
                    } else if *i < b.len() && b[*i] == b'}' {
                        *i += 1;
                        break;
                    }
                }
                Some(Value::Object(m))
            }
            b'[' => {
                *i += 1;
                let mut a = Vec::new();
                skip_ws(b, i);
                if *i < b.len() && b[*i] == b']' {
                    *i += 1;
                    return Some(Value::Array(a));
                }
                loop {
                    a.push(parse_val_impl(b, i, s, parse_str)?);
                    skip_ws(b, i);
                    if *i < b.len() && b[*i] == b',' {
                        *i += 1;
                    } else if *i < b.len() && b[*i] == b']' {
                        *i += 1;
                        break;
                    }
                }
                Some(Value::Array(a))
            }
            b'"' => parse_str(b, i).map(Value::Str),
            b't' => {
                if s[*i..].starts_with("true") {
                    *i += 4;
                    Some(Value::Bool(true))
                } else {
                    None
                }
            }
            b'f' => {
                if s[*i..].starts_with("false") {
                    *i += 5;
                    Some(Value::Bool(false))
                } else {
                    None
                }
            }
            b'n' => {
                if s[*i..].starts_with("null") {
                    *i += 4;
                    Some(Value::Null)
                } else {
                    None
                }
            }
            _ => {
                let start = *i;
                while *i < b.len()
                    && (b[*i].is_ascii_digit()
                        || matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E'))
                {
                    *i += 1;
                }
                let tok = s[start..*i].to_string();
                if let Ok(iv) = tok.parse::<i64>() {
                    Some(Value::Int(iv))
                } else if let Ok(fv) = tok.parse::<f64>() {
                    Some(Value::Float(fv))
                } else {
                    None
                }
            }
        }
    }
    parse_val_impl(bytes, &mut i, s, &parse_str)
}

// ---------------------------------------------------------------------------
// serde 支持（可选 feature：`serde`）
//
// 1) `Value` 实现 `Serialize`/`Deserialize`（手写而非 `#[derive]`：derive 会把
//    枚举表示为外部标签形式 Value::Int(5) -> {"Int":5}，而配置场景要自然形状
//    5）。手写后 SML 的 Value 与 JSON/TOML/YAML 数据形状一致，可经任意 serde
//    后端进出。
// 2) `sml::serde::{from_str, from_value, to_value, to_string}`：serde 桥。
//    任何 `#[derive(serde::Serialize / Deserialize)]` 类型都能像 toml-rs 一样
//    一键从 SML 文本反序列化 / 序列化为 SML（枚举沿用 `__type` 约定）。
//
// 不启用该 feature 时 crate 保持零依赖。
// ---------------------------------------------------------------------------

#[cfg(feature = "serde")]
pub mod serde {
    use super::Value;
    use ::serde::de::{self, MapAccess, SeqAccess, Visitor};
    use ::serde::ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
        SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
    };
    use ::serde::{Deserialize, Deserializer, Serialize, Serializer};
    use ::std::collections::BTreeMap;
    use ::std::fmt;

    /// serde 错误类型（自定义消息，实现 ser/de 两个 Error trait）
    type Error = ::serde::de::value::Error;

    fn type_err(v: &Value, expected: &str) -> Error {
        de::Error::custom(format!(
            "期望 {expected}，实际为 {}",
            super::__private::describe_value(v)
        ))
    }

    impl Serialize for Value {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            match self {
                Value::Null => serializer.serialize_unit(),
                Value::Bool(b) => serializer.serialize_bool(*b),
                Value::Int(i) => serializer.serialize_i64(*i),
                Value::Float(f) => serializer.serialize_f64(*f),
                Value::Str(s) => serializer.serialize_str(s),
                // Vec<Value> / 逐项委托，递归依赖 Value 自身的 impl
                Value::Array(a) => a.serialize(serializer),
                Value::Object(m) => {
                    let mut map = serializer.serialize_map(Some(m.len()))?;
                    for (k, v) in m {
                        map.serialize_entry(k, v)?;
                    }
                    map.end()
                }
            }
        }
    }

    impl<'de> Deserialize<'de> for Value {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            // 交给格式自行判断类型（JSON 的数字/字符串/数组/对象都能落到对应变体）
            deserializer.deserialize_any(ValueVisitor)
        }
    }

    struct ValueVisitor;

    impl<'de> Visitor<'de> for ValueVisitor {
        type Value = Value;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("any valid SML/JSON value")
        }

        fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
            Ok(Value::Null)
        }
        fn visit_none<E: de::Error>(self) -> Result<Value, E> {
            Ok(Value::Null)
        }
        fn visit_some<D>(self, d: D) -> Result<Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            Deserialize::deserialize(d)
        }
        fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
            Ok(Value::Bool(v))
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
            Ok(Value::Int(v))
        }
        // 超出 i64 的大整数退化为 Float，避免直接报错丢失数据
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
            Ok(i64::try_from(v)
                .map(Value::Int)
                .unwrap_or_else(|_| Value::Float(v as f64)))
        }
        fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
            Ok(Value::Float(v))
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
            Ok(Value::Str(v.to_string()))
        }
        fn visit_string<E: de::Error>(self, v: String) -> Result<Value, E> {
            Ok(Value::Str(v))
        }
        fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut v = Vec::new();
            while let Some(x) = seq.next_element()? {
                v.push(x);
            }
            Ok(Value::Array(v))
        }
        fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut m = BTreeMap::new();
            while let Some((k, v)) = map.next_entry::<String, Value>()? {
                m.insert(k, v);
            }
            Ok(Value::Object(m))
        }
    }

    // -----------------------------------------------------------------------
    // serde 桥：任意 `serde::Serialize / Deserialize` 类型 <-> SML
    // -----------------------------------------------------------------------

    /// 解析 SML 文本并一键反序列化到任意 serde 类型（等价于 `toml::from_str`）。
    ///
    /// ```rust
    /// # use serde::Deserialize;
    /// # #[derive(Deserialize, Debug)]
    /// # struct Server { host: String, port: i32 }
    /// let s: Server = sml::serde::from_str("host: web.example\nport: 8080\n").unwrap();
    /// assert_eq!(s.host, "web.example");
    /// ```
    pub fn from_str<T: de::DeserializeOwned>(text: &str) -> Result<T, String> {
        let value = crate::parse(text)?;
        from_value(value)
    }

    /// 从任意 [`Value`] 反序列化到任意 serde 类型。
    pub fn from_value<T: de::DeserializeOwned>(value: Value) -> Result<T, String> {
        T::deserialize(ValueDeserializer(value)).map_err(|e| e.to_string())
    }

    /// 任意 serde 类型序列化为 [`Value`]（等价于 `serde_json::to_value`）。
    pub fn to_value<T: Serialize + ?Sized>(value: &T) -> Result<Value, String> {
        value.serialize(ValueSerializer).map_err(|e| e.to_string())
    }

    /// 任意 serde 类型序列化为 SML 文本（等价于 `toml::to_string`）。
    pub fn to_string<T: Serialize + ?Sized>(value: &T) -> Result<String, String> {
        Ok(crate::to_sml(&to_value(value)?))
    }

    // ---- Serializer: T: Serialize -> Value ----

    struct ValueSerializer;

    impl Serializer for ValueSerializer {
        type Ok = Value;
        type Error = Error;
        type SerializeSeq = SeqSerializer;
        type SerializeTuple = SeqSerializer;
        type SerializeTupleStruct = SeqSerializer;
        type SerializeTupleVariant = TupleVariantSerializer;
        type SerializeMap = MapSerializer;
        type SerializeStruct = MapSerializer;
        type SerializeStructVariant = StructVariantSerializer;

        fn serialize_bool(self, v: bool) -> Result<Value, Error> {
            Ok(Value::Bool(v))
        }
        fn serialize_i8(self, v: i8) -> Result<Value, Error> {
            Ok(Value::Int(v as i64))
        }
        fn serialize_i16(self, v: i16) -> Result<Value, Error> {
            Ok(Value::Int(v as i64))
        }
        fn serialize_i32(self, v: i32) -> Result<Value, Error> {
            Ok(Value::Int(v as i64))
        }
        fn serialize_i64(self, v: i64) -> Result<Value, Error> {
            Ok(Value::Int(v))
        }
        fn serialize_u8(self, v: u8) -> Result<Value, Error> {
            Ok(Value::Int(v as i64))
        }
        fn serialize_u16(self, v: u16) -> Result<Value, Error> {
            Ok(Value::Int(v as i64))
        }
        fn serialize_u32(self, v: u32) -> Result<Value, Error> {
            Ok(Value::Int(v as i64))
        }
        fn serialize_u64(self, v: u64) -> Result<Value, Error> {
            Ok(i64::try_from(v)
                .map(Value::Int)
                .unwrap_or_else(|_| Value::Float(v as f64)))
        }
        fn serialize_f32(self, v: f32) -> Result<Value, Error> {
            Ok(Value::Float(v as f64))
        }
        fn serialize_f64(self, v: f64) -> Result<Value, Error> {
            Ok(Value::Float(v))
        }
        fn serialize_char(self, v: char) -> Result<Value, Error> {
            Ok(Value::Str(v.to_string()))
        }
        fn serialize_str(self, v: &str) -> Result<Value, Error> {
            Ok(Value::Str(v.to_string()))
        }
        fn serialize_bytes(self, v: &[u8]) -> Result<Value, Error> {
            Ok(Value::Array(v.iter().map(|&b| Value::Int(b as i64)).collect()))
        }
        fn serialize_none(self) -> Result<Value, Error> {
            Ok(Value::Null)
        }
        fn serialize_some<T: Serialize + ?Sized>(self, v: &T) -> Result<Value, Error> {
            v.serialize(ValueSerializer)
        }
        fn serialize_unit(self) -> Result<Value, Error> {
            Ok(Value::Null)
        }
        fn serialize_unit_struct(self, _name: &'static str) -> Result<Value, Error> {
            Ok(Value::Null)
        }
        fn serialize_unit_variant(
            self,
            _name: &'static str,
            _idx: u32,
            variant: &'static str,
        ) -> Result<Value, Error> {
            Ok(Value::Str(variant.to_string()))
        }
        fn serialize_newtype_struct<T: Serialize + ?Sized>(
            self,
            _name: &'static str,
            v: &T,
        ) -> Result<Value, Error> {
            v.serialize(ValueSerializer)
        }
        fn serialize_newtype_variant<T: Serialize + ?Sized>(
            self,
            _name: &'static str,
            _idx: u32,
            variant: &'static str,
            value: &T,
        ) -> Result<Value, Error> {
            Ok(Value::Object(BTreeMap::from([
                ("__type".into(), Value::Str(variant.to_string())),
                ("_value".into(), value.serialize(ValueSerializer)?),
            ])))
        }
        fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
            Ok(SeqSerializer(Vec::new()))
        }
        fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Error> {
            self.serialize_seq(Some(len))
        }
        fn serialize_tuple_struct(
            self,
            _name: &'static str,
            len: usize,
        ) -> Result<Self::SerializeTupleStruct, Error> {
            self.serialize_seq(Some(len))
        }
        fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _idx: u32,
            variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleVariant, Error> {
            Ok(TupleVariantSerializer {
                variant: variant.to_string(),
                values: Vec::new(),
            })
        }
        fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
            Ok(MapSerializer {
                map: BTreeMap::new(),
                key: None,
            })
        }
        fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Self::SerializeStruct, Error> {
            self.serialize_map(Some(len))
        }
        fn serialize_struct_variant(
            self,
            _name: &'static str,
            _idx: u32,
            variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStructVariant, Error> {
            Ok(StructVariantSerializer {
                variant: variant.to_string(),
                map: BTreeMap::new(),
            })
        }
    }

    struct SeqSerializer(Vec<Value>);

    impl SerializeSeq for SeqSerializer {
        type Ok = Value;
        type Error = Error;
        fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
            self.0.push(value.serialize(ValueSerializer)?);
            Ok(())
        }
        fn end(self) -> Result<Value, Error> {
            Ok(Value::Array(self.0))
        }
    }
    impl SerializeTuple for SeqSerializer {
        type Ok = Value;
        type Error = Error;
        fn serialize_element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
            SerializeSeq::serialize_element(self, value)
        }
        fn end(self) -> Result<Value, Error> {
            SerializeSeq::end(self)
        }
    }
    impl SerializeTupleStruct for SeqSerializer {
        type Ok = Value;
        type Error = Error;
        fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
            SerializeSeq::serialize_element(self, value)
        }
        fn end(self) -> Result<Value, Error> {
            SerializeSeq::end(self)
        }
    }

    struct MapSerializer {
        map: BTreeMap<String, Value>,
        key: Option<String>,
    }

    impl SerializeMap for MapSerializer {
        type Ok = Value;
        type Error = Error;
        fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), Error> {
            self.key = Some(key.serialize(KeySerializer)?);
            Ok(())
        }
        fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
            let k = self
                .key
                .take()
                .ok_or_else(|| de::Error::custom("serialize_value 前需先 serialize_key"))?;
            self.map.insert(k, value.serialize(ValueSerializer)?);
            Ok(())
        }
        fn end(self) -> Result<Value, Error> {
            Ok(Value::Object(self.map))
        }
    }

    impl SerializeStruct for MapSerializer {
        type Ok = Value;
        type Error = Error;
        fn serialize_field<T: Serialize + ?Sized>(
            &mut self,
            key: &'static str,
            value: &T,
        ) -> Result<(), Error> {
            self.map
                .insert(key.to_string(), value.serialize(ValueSerializer)?);
            Ok(())
        }
        fn end(self) -> Result<Value, Error> {
            Ok(Value::Object(self.map))
        }
    }

    /// 对象键必须能转成字符串（SML 的键是裸词/字符串）
    struct KeySerializer;

    macro_rules! key_unsupported {
        ($(fn $m:ident($($a:ident : $t:ty),*) -> Result<String, Error>;)*) => {
            $(
                fn $m(self, $($a: $t),*) -> Result<String, Error> {
                    Err(de::Error::custom("SML 对象的键必须是字符串"))
                }
            )*
        };
    }

    impl Serializer for KeySerializer {
        type Ok = String;
        type Error = Error;
        type SerializeSeq = ::serde::ser::Impossible<String, Error>;
        type SerializeTuple = ::serde::ser::Impossible<String, Error>;
        type SerializeTupleStruct = ::serde::ser::Impossible<String, Error>;
        type SerializeTupleVariant = ::serde::ser::Impossible<String, Error>;
        type SerializeMap = ::serde::ser::Impossible<String, Error>;
        type SerializeStruct = ::serde::ser::Impossible<String, Error>;
        type SerializeStructVariant = ::serde::ser::Impossible<String, Error>;

        fn serialize_str(self, v: &str) -> Result<String, Error> {
            Ok(v.to_string())
        }
        fn serialize_char(self, v: char) -> Result<String, Error> {
            Ok(v.to_string())
        }
        key_unsupported! {
            fn serialize_bool(_v: bool) -> Result<String, Error>;
            fn serialize_i8(_v: i8) -> Result<String, Error>;
            fn serialize_i16(_v: i16) -> Result<String, Error>;
            fn serialize_i32(_v: i32) -> Result<String, Error>;
            fn serialize_i64(_v: i64) -> Result<String, Error>;
            fn serialize_u8(_v: u8) -> Result<String, Error>;
            fn serialize_u16(_v: u16) -> Result<String, Error>;
            fn serialize_u32(_v: u32) -> Result<String, Error>;
            fn serialize_u64(_v: u64) -> Result<String, Error>;
            fn serialize_f32(_v: f32) -> Result<String, Error>;
            fn serialize_f64(_v: f64) -> Result<String, Error>;
            fn serialize_bytes(_v: &[u8]) -> Result<String, Error>;
            fn serialize_none() -> Result<String, Error>;
            fn serialize_unit() -> Result<String, Error>;
            fn serialize_unit_struct(_n: &'static str) -> Result<String, Error>;
            fn serialize_unit_variant(_n: &'static str, _i: u32, _v: &'static str) -> Result<String, Error>;
        }
        fn serialize_some<T: Serialize + ?Sized>(self, _v: &T) -> Result<String, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
        fn serialize_newtype_struct<T: Serialize + ?Sized>(
            self,
            _n: &'static str,
            _v: &T,
        ) -> Result<String, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
        fn serialize_newtype_variant<T: Serialize + ?Sized>(
            self,
            _n: &'static str,
            _i: u32,
            _v: &'static str,
            _x: &T,
        ) -> Result<String, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
        // 以下方法返回关联类型（Impossible），一律报错——SML 键只能是字符串
        fn serialize_seq(self, _l: Option<usize>) -> Result<Self::SerializeSeq, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
        fn serialize_tuple(self, _l: usize) -> Result<Self::SerializeTuple, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
        fn serialize_tuple_struct(
            self,
            _n: &'static str,
            _l: usize,
        ) -> Result<Self::SerializeTupleStruct, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
        fn serialize_tuple_variant(
            self,
            _n: &'static str,
            _i: u32,
            _v: &'static str,
            _l: usize,
        ) -> Result<Self::SerializeTupleVariant, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
        fn serialize_map(self, _l: Option<usize>) -> Result<Self::SerializeMap, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
        fn serialize_struct(self, _n: &'static str, _l: usize) -> Result<Self::SerializeStruct, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
        fn serialize_struct_variant(
            self,
            _n: &'static str,
            _i: u32,
            _v: &'static str,
            _l: usize,
        ) -> Result<Self::SerializeStructVariant, Error> {
            Err(de::Error::custom("SML 对象的键必须是字符串"))
        }
    }

    struct TupleVariantSerializer {
        variant: String,
        values: Vec<Value>,
    }

    impl SerializeTupleVariant for TupleVariantSerializer {
        type Ok = Value;
        type Error = Error;
        fn serialize_field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), Error> {
            self.values.push(value.serialize(ValueSerializer)?);
            Ok(())
        }
        fn end(self) -> Result<Value, Error> {
            Ok(Value::Object(BTreeMap::from([
                ("__type".into(), Value::Str(self.variant)),
                ("_value".into(), Value::Array(self.values)),
            ])))
        }
    }

    struct StructVariantSerializer {
        variant: String,
        map: BTreeMap<String, Value>,
    }

    impl SerializeStructVariant for StructVariantSerializer {
        type Ok = Value;
        type Error = Error;
        fn serialize_field<T: Serialize + ?Sized>(
            &mut self,
            key: &'static str,
            value: &T,
        ) -> Result<(), Error> {
            self.map
                .insert(key.to_string(), value.serialize(ValueSerializer)?);
            Ok(())
        }
        fn end(self) -> Result<Value, Error> {
            let mut m = BTreeMap::new();
            m.insert("__type".into(), Value::Str(self.variant));
            m.extend(self.map);
            Ok(Value::Object(m))
        }
    }

    // ---- Deserializer: Value -> T: Deserialize ----

    macro_rules! deser_int {
        ($(fn $m:ident($v:ident, $call:ident);)*) => {
            $(
                fn $m<V>(self, $v: V) -> Result<V::Value, Self::Error>
                where V: Visitor<'de> {
                    match self.0 {
                        Value::Int(i) => $v.$call(i as _),
                        Value::Float(f)
                            if f.fract() == 0.0
                                && f >= i64::MIN as f64
                                && f <= i64::MAX as f64 =>
                        {
                            $v.$call(f as _)
                        }
                        other => Err(type_err(&other, stringify!($m).trim_start_matches("deserialize_"))),
                    }
                }
            )*
        };
    }

    struct ValueDeserializer(Value);

    impl<'de> Deserializer<'de> for ValueDeserializer {
        type Error = Error;

        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Null => visitor.visit_unit(),
                Value::Bool(b) => visitor.visit_bool(b),
                Value::Int(i) => visitor.visit_i64(i),
                Value::Float(f) => visitor.visit_f64(f),
                Value::Str(s) => visitor.visit_string(s),
                Value::Array(a) => visitor.visit_seq(SeqDeserializer { items: a, idx: 0 }),
                Value::Object(m) => visitor.visit_map(MapDeserializer { map: m, pending: None }),
            }
        }

        fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Bool(b) => visitor.visit_bool(b),
                other => Err(type_err(&other, "布尔")),
            }
        }

        deser_int! {
            fn deserialize_i8(v, visit_i8);
            fn deserialize_i16(v, visit_i16);
            fn deserialize_i32(v, visit_i32);
            fn deserialize_i64(v, visit_i64);
            fn deserialize_u8(v, visit_u8);
            fn deserialize_u16(v, visit_u16);
            fn deserialize_u32(v, visit_u32);
        }

        fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Int(i) if i >= 0 => visitor.visit_u64(i as u64),
                Value::Float(f)
                    if f.fract() == 0.0 && f >= 0.0 && f <= u64::MAX as f64 =>
                {
                    visitor.visit_u64(f as u64)
                }
                other => Err(type_err(&other, "u64")),
            }
        }

        fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Int(i) => visitor.visit_f32(i as f32),
                Value::Float(f) => visitor.visit_f32(f as f32),
                other => Err(type_err(&other, "f32")),
            }
        }
        fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Int(i) => visitor.visit_f64(i as f64),
                Value::Float(f) => visitor.visit_f64(f),
                other => Err(type_err(&other, "f64")),
            }
        }

        fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Str(s) if s.chars().count() == 1 => {
                    visitor.visit_char(s.chars().next().unwrap())
                }
                other => Err(type_err(&other, "字符")),
            }
        }

        fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Str(s) => visitor.visit_string(s),
                other => Err(type_err(&other, "字符串")),
            }
        }
        fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_str(visitor)
        }

        fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Array(items) => {
                    let mut buf = Vec::with_capacity(items.len());
                    for it in items {
                        match it {
                            Value::Int(i) if (0..=255).contains(&i) => buf.push(i as u8),
                            other => return Err(type_err(&other, "字节")),
                        }
                    }
                    visitor.visit_byte_buf(buf)
                }
                other => Err(type_err(&other, "字节数组")),
            }
        }
        fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_bytes(visitor)
        }

        fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Null => visitor.visit_none(),
                other => visitor.visit_some(ValueDeserializer(other)),
            }
        }

        fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Null => visitor.visit_unit(),
                other => Err(type_err(&other, "unit")),
            }
        }
        fn deserialize_unit_struct<V>(
            self,
            _name: &'static str,
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_unit(visitor)
        }
        fn deserialize_newtype_struct<V>(
            self,
            _name: &'static str,
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_any(visitor)
        }

        fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Array(a) => visitor.visit_seq(SeqDeserializer { items: a, idx: 0 }),
                other => Err(type_err(&other, "数组")),
            }
        }
        fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_seq(visitor)
        }
        fn deserialize_tuple_struct<V>(
            self,
            _name: &'static str,
            _len: usize,
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_seq(visitor)
        }

        fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Object(m) => visitor.visit_map(MapDeserializer { map: m, pending: None }),
                other => Err(type_err(&other, "块/对象")),
            }
        }
        fn deserialize_struct<V>(
            self,
            _name: &'static str,
            _fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_map(visitor)
        }

        fn deserialize_enum<V>(
            self,
            _name: &'static str,
            _variants: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.0 {
                Value::Str(s) => visitor.visit_enum(EnumDeserializer {
                    variant: s,
                    kind: EnumKind::Unit,
                }),
                Value::Object(mut m) => {
                    // 1) SML 专有约定：`__type` 键（与 SmlSerialize 输出一致）
                    if let Some(ty) = m.remove("__type") {
                        let variant = match ty {
                            Value::Str(s) => s,
                            _ => return Err(de::Error::custom("`__type` 的值必须是字符串")),
                        };
                        let kind = match m.remove("_value") {
                            Some(Value::Array(items)) => EnumKind::Tuple(items),
                            Some(other) => EnumKind::Newtype(other),
                            None if m.is_empty() => EnumKind::Unit,
                            None => EnumKind::Struct(m),
                        };
                        return visitor.visit_enum(EnumDeserializer { variant, kind });
                    }
                    // 2) serde 外部标签（含 SML 裸词包裹形态）：
                    //    {"in-maintenance": "in-maintenance"} -> 单元变体
                    //    {"Circle": 3}                        -> 单值变体
                    if m.len() == 1 {
                        let (k, v) = m.pop_first().expect("len==1 必有键");
                        let kind = match v {
                            Value::Str(s) if s == k => EnumKind::Unit,
                            other => EnumKind::Newtype(other),
                        };
                        return visitor.visit_enum(EnumDeserializer { variant: k, kind });
                    }
                    Err(de::Error::custom(
                        "枚举块需要 `__type` 键（SML 约定）或单键外部标签 `{ VariantName: ... }`",
                    ))
                }
                other => Err(type_err(&other, "枚举")),
            }
        }

        fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_str(visitor)
        }
        fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_any(visitor)
        }
    }

    struct SeqDeserializer {
        items: Vec<Value>,
        idx: usize,
    }

    impl<'de> SeqAccess<'de> for SeqDeserializer {
        type Error = Error;
        fn next_element_seed<T: de::DeserializeSeed<'de>>(
            &mut self,
            seed: T,
        ) -> Result<Option<T::Value>, Error> {
            if self.idx >= self.items.len() {
                return Ok(None);
            }
            let item = self.items[self.idx].clone();
            self.idx += 1;
            seed.deserialize(ValueDeserializer(item)).map(Some)
        }
    }

    struct MapDeserializer {
        map: BTreeMap<String, Value>,
        pending: Option<Value>,
    }

    impl<'de> MapAccess<'de> for MapDeserializer {
        type Error = Error;
        fn next_key_seed<K: de::DeserializeSeed<'de>>(
            &mut self,
            seed: K,
        ) -> Result<Option<K::Value>, Error> {
            let Some((k, v)) = self.map.pop_first() else {
                return Ok(None);
            };
            self.pending = Some(v);
            seed.deserialize(KeyDeserializer(&k)).map(Some)
        }
        fn next_value_seed<V: de::DeserializeSeed<'de>>(
            &mut self,
            seed: V,
        ) -> Result<V::Value, Error> {
            let v = self.pending.take().ok_or_else(|| {
                de::Error::custom("value 缺失：需先调用 next_key_seed")
            })?;
            seed.deserialize(ValueDeserializer(v))
        }
    }

    /// 字段名 / 变体名的轻量反序列化器（只认字符串）
    struct KeyDeserializer<'a>(&'a str);

    macro_rules! key_delegate {
        ($($m:ident),* $(,)?) => {
            $(
                fn $m<V>(self, visitor: V) -> Result<V::Value, Error>
                where V: Visitor<'de> {
                    self.deserialize_any(visitor)
                }
            )*
        };
    }

    impl<'de, 'a> Deserializer<'de> for KeyDeserializer<'a> {
        type Error = Error;

        fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_str(self.0)
        }
        fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_str(self.0)
        }
        fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_str(self.0)
        }
        fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_str(self.0)
        }
        fn deserialize_enum<V>(
            self,
            _name: &'static str,
            _variants: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_enum(EnumDeserializer {
                variant: self.0.to_string(),
                kind: EnumKind::Unit,
            })
        }
        fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            visitor.visit_some(self)
        }
        fn deserialize_unit_struct<V>(
            self,
            _name: &'static str,
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_unit(visitor)
        }
        fn deserialize_newtype_struct<V>(
            self,
            _name: &'static str,
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_any(visitor)
        }
        fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_seq(visitor)
        }
        fn deserialize_tuple_struct<V>(
            self,
            _name: &'static str,
            _len: usize,
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_seq(visitor)
        }
        fn deserialize_struct<V>(
            self,
            _name: &'static str,
            _fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_map(visitor)
        }
        fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            self.deserialize_any(visitor)
        }
        key_delegate! {
            deserialize_bool, deserialize_i8, deserialize_i16, deserialize_i32,
            deserialize_i64, deserialize_u8, deserialize_u16, deserialize_u32,
            deserialize_u64, deserialize_f32, deserialize_f64, deserialize_char,
            deserialize_bytes, deserialize_byte_buf, deserialize_unit,
            deserialize_seq, deserialize_map,
        }
    }

    // ---- 枚举（SML `__type` 约定，与 SmlDeserialize 一致）----

    #[derive(Debug)]
    enum EnumKind {
        Unit,
        Newtype(Value),
        Tuple(Vec<Value>),
        Struct(BTreeMap<String, Value>),
    }

    struct EnumDeserializer {
        variant: String,
        kind: EnumKind,
    }

    impl<'de> de::EnumAccess<'de> for EnumDeserializer {
        type Error = Error;
        type Variant = VariantAccess;
        fn variant_seed<V: de::DeserializeSeed<'de>>(
            self,
            seed: V,
        ) -> Result<(V::Value, Self::Variant), Error> {
            let variant = seed.deserialize(KeyDeserializer(&self.variant))?;
            Ok((variant, VariantAccess { kind: self.kind }))
        }
    }

    struct VariantAccess {
        kind: EnumKind,
    }

    impl<'de> de::VariantAccess<'de> for VariantAccess {
        type Error = Error;
        fn unit_variant(self) -> Result<(), Error> {
            match self.kind {
                EnumKind::Unit => Ok(()),
                _ => Err(de::Error::custom("该变体携带数据，不能按单元变体解析")),
            }
        }
        fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(
            self,
            seed: T,
        ) -> Result<T::Value, Error> {
            match self.kind {
                EnumKind::Newtype(v) => seed.deserialize(ValueDeserializer(v)),
                EnumKind::Tuple(items) => {
                    seed.deserialize(ValueDeserializer(Value::Array(items)))
                }
                _ => Err(de::Error::custom("该变体没有单值数据")),
            }
        }
        fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.kind {
                EnumKind::Tuple(items) => {
                    visitor.visit_seq(SeqDeserializer { items, idx: 0 })
                }
                _ => Err(de::Error::custom("该变体不是元组形态")),
            }
        }
        fn struct_variant<V>(
            self,
            _fields: &'static [&'static str],
            visitor: V,
        ) -> Result<V::Value, Error>
        where
            V: Visitor<'de>,
        {
            match self.kind {
                EnumKind::Struct(m) => {
                    visitor.visit_map(MapDeserializer { map: m, pending: None })
                }
                _ => Err(de::Error::custom("该变体不是结构体形态")),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 自然序列化宏（derive）支持
// ---------------------------------------------------------------------------

/// 把一个类型「自然地」序列化为 SML 值：
/// 结构体 → 块、newtype → 透明、单元结构体 → 裸词、
/// 枚举单元变体 → 裸词、带数据变体 → `__type` 块。
///
/// 通常用 `#[derive(SmlSerialize)]` 自动实现（`derive` feature 默认开启），
/// 也可手动实现。支持的 `#[sml(...)]` 属性见 `swsml-derive` 的文档。
pub trait SmlSerialize {
    fn to_sml_value(&self) -> Value;

    /// 序列化为 SML 文本（等价于 [`to_sml`] 作用于本类型生成的值）。
    fn to_sml(&self) -> String {
        crate::to_sml(&self.to_sml_value())
    }
}

/// 从 SML 值反序列化（`#[derive(SmlDeserialize)]` 自动实现）。
pub trait SmlDeserialize: Sized {
    fn from_sml_value(v: &Value) -> Result<Self, String>;

    /// 解析 SML 文本并反序列化。
    fn from_sml(text: &str) -> Result<Self, String> {
        let v = crate::parse(text).map_err(|e| format!("SML 解析失败: {e}"))?;
        Self::from_sml_value(&v)
    }
}

#[cfg(feature = "derive")]
pub use swsml_derive::{SmlDeserialize, SmlSerialize};

/// 序列化为 SML 文本 —— toml-rs 风格的顶层函数（等价于 [`SmlSerialize::to_sml`]）。
///
/// 用法与 `toml::to_string` 一致（序列化不会失败，故直接返回 `String`）：
///
/// ```rust
/// # use sml::{SmlSerialize, SmlDeserialize};
/// # #[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
/// # struct Server { host: String, port: i32 }
/// # let cfg = Server { host: "web.example".into(), port: 8080 };
/// let text = sml::to_string(&cfg);
/// assert_eq!(text, "host: web.example\nport: 8080\n");
/// ```
pub fn to_string<T: SmlSerialize + ?Sized>(value: &T) -> String {
    crate::to_sml(&value.to_sml_value())
}

/// 解析 SML 文本并反序列化 —— toml-rs 风格的顶层函数（等价于 [`SmlDeserialize::from_sml`]）。
///
/// ```rust
/// # use sml::{SmlSerialize, SmlDeserialize};
/// # #[derive(SmlSerialize, SmlDeserialize, Debug, PartialEq)]
/// # struct Server { host: String, port: i32 }
/// let back: Server = sml::from_str("host: web.example\nport: 8080\n").unwrap();
/// assert_eq!(back.host, "web.example");
/// assert_eq!(back.port, 8080);
/// ```
pub fn from_str<T: SmlDeserialize>(text: &str) -> Result<T, String> {
    T::from_sml(text)
}

/// 宏生成代码引用的内部辅助（请勿直接使用）。
#[doc(hidden)]
pub mod __private {
    use super::{SmlDeserialize, SmlSerialize, Value};
    use std::collections::{BTreeMap, HashMap};

    /// 描述值的类型，用于错误信息。
    pub fn describe_value(v: &Value) -> String {
        match v {
            Value::Null => "null".to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Str(s) => format!("字符串 `{s}`"),
            Value::Array(a) => format!("数组（{} 个元素）", a.len()),
            Value::Object(o) => format!("块（{} 个键）", o.len()),
        }
    }

    /// 取出 `_value` 键（枚举单值变体）。
    pub fn take_value(m: &BTreeMap<String, Value>) -> Result<Value, String> {
        m.get("_value")
            .cloned()
            .ok_or_else(|| "缺少 _value 键".to_string())
    }

    /// 取出 `_value` 键并断言为数组（枚举 tuple 变体）。
    pub fn take_array(m: &BTreeMap<String, Value>) -> Result<Vec<Value>, String> {
        match m.get("_value") {
            Some(Value::Array(a)) => Ok(a.clone()),
            Some(other) => Err(format!("_value 期望数组，实际为 {}", describe_value(other))),
            None => Err("缺少 _value 键".to_string()),
        }
    }

    /// `#[sml(flatten)]` 反序列化：把整个块交给子类型。
    pub fn flatten_from<T: SmlDeserialize>(m: &BTreeMap<String, Value>) -> Result<T, String> {
        T::from_sml_value(&Value::Object(m.clone()))
    }

    // ---- 基础类型 ----

    impl SmlSerialize for bool {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Bool(*self)
        }
    }
    impl SmlDeserialize for bool {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Bool(b) => Ok(*b),
                other => Err(format!("期望布尔，实际为 {}", describe_value(other))),
            }
        }
    }

    macro_rules! impl_int {
        ($($t:ty),* $(,)?) => {$(
            impl SmlSerialize for $t {
                #[inline]
                fn to_sml_value(&self) -> Value { Value::Int(*self as i64) }
            }
            impl SmlDeserialize for $t {
                #[inline]
                fn from_sml_value(v: &Value) -> Result<Self, String> {
                    match v {
                        Value::Int(i) => <$t>::try_from(*i)
                            .map_err(|_| format!("整数 {i} 超出 {} 范围", stringify!($t))),
                        Value::Float(f)
                            if f.fract() == 0.0
                                && *f >= <$t>::MIN as f64
                                && *f <= <$t>::MAX as f64 => Ok(*f as $t),
                        Value::Float(f) => Err(format!("期望整数，实际为小数 {f}")),
                        other => Err(format!("期望整数，实际为 {}", describe_value(other))),
                    }
                }
            }
        )*};
    }
    impl_int!(i8, i16, i32, i64, isize, u8, u16, u32, usize);

    impl SmlSerialize for u64 {
        #[inline]
        fn to_sml_value(&self) -> Value {
            i64::try_from(*self).map(Value::Int).unwrap_or_else(|_| Value::Float(*self as f64))
        }
    }
    impl SmlDeserialize for u64 {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Int(i) => u64::try_from(*i).map_err(|_| format!("整数 {i} 为负数，超出 u64 范围")),
                Value::Float(f) if f.fract() == 0.0 && *f >= 0.0 => Ok(*f as u64),
                Value::Float(f) => Err(format!("期望非负整数，实际为 {f}")),
                other => Err(format!("期望整数，实际为 {}", describe_value(other))),
            }
        }
    }

    macro_rules! impl_big {
        ($($t:ty),* $(,)?) => {$(
            impl SmlSerialize for $t {
                #[inline]
                fn to_sml_value(&self) -> Value {
                    i64::try_from(*self).map(Value::Int).unwrap_or_else(|_| Value::Float(*self as f64))
                }
            }
            impl SmlDeserialize for $t {
                #[inline]
                fn from_sml_value(v: &Value) -> Result<Self, String> {
                    match v {
                        Value::Int(i) => Ok(*i as $t),
                        Value::Float(f) if f.fract() == 0.0 => Ok(*f as $t),
                        Value::Float(f) => Err(format!("期望整数，实际为小数 {f}")),
                        other => Err(format!("期望整数，实际为 {}", describe_value(other))),
                    }
                }
            }
        )*};
    }
    impl_big!(i128, u128);

    macro_rules! impl_float {
        ($($t:ty),* $(,)?) => {$(
            impl SmlSerialize for $t {
                #[inline]
                fn to_sml_value(&self) -> Value { Value::Float(*self as f64) }
            }
            impl SmlDeserialize for $t {
                #[inline]
                fn from_sml_value(v: &Value) -> Result<Self, String> {
                    match v {
                        Value::Int(i) => Ok(*i as $t),
                        Value::Float(f) => Ok(*f as $t),
                        other => Err(format!("期望数字，实际为 {}", describe_value(other))),
                    }
                }
            }
        )*};
    }
    impl_float!(f32, f64);

    impl SmlSerialize for char {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Str(self.to_string())
        }
    }
    impl SmlDeserialize for char {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Str(s) => {
                    let mut it = s.chars();
                    match (it.next(), it.next()) {
                        (Some(c), None) => Ok(c),
                        _ => Err(format!("期望单个字符，实际为 `{s}`")),
                    }
                }
                other => Err(format!("期望字符串，实际为 {}", describe_value(other))),
            }
        }
    }

    impl SmlSerialize for String {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Str(self.clone())
        }
    }
    impl SmlDeserialize for String {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Str(s) => Ok(s.clone()),
                other => Err(format!("期望字符串，实际为 {}", describe_value(other))),
            }
        }
    }

    impl SmlSerialize for str {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Str(self.to_string())
        }
    }

    impl SmlSerialize for &str {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Str(self.to_string())
        }
    }

    impl SmlSerialize for () {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Null
        }
    }
    impl SmlDeserialize for () {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Null => Ok(()),
                other => Err(format!("期望 null，实际为 {}", describe_value(other))),
            }
        }
    }

    impl SmlSerialize for Value {
        #[inline]
        fn to_sml_value(&self) -> Value {
            self.clone()
        }
    }
    impl SmlDeserialize for Value {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            Ok(v.clone())
        }
    }

    impl<T: SmlSerialize> SmlSerialize for Option<T> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            match self {
                Some(v) => v.to_sml_value(),
                None => Value::Null,
            }
        }
    }
    impl<T: SmlDeserialize> SmlDeserialize for Option<T> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Null => Ok(None),
                other => Ok(Some(T::from_sml_value(other)?)),
            }
        }
    }

    impl<T: SmlSerialize> SmlSerialize for Vec<T> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Array(self.iter().map(SmlSerialize::to_sml_value).collect())
        }
    }
    impl<T: SmlDeserialize> SmlDeserialize for Vec<T> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Array(a) => a.iter().map(SmlDeserialize::from_sml_value).collect(),
                other => Err(format!("期望数组，实际为 {}", describe_value(other))),
            }
        }
    }

    impl<T: SmlSerialize> SmlSerialize for Box<T> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            (**self).to_sml_value()
        }
    }
    impl<T: SmlDeserialize> SmlDeserialize for Box<T> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            Ok(Box::new(T::from_sml_value(v)?))
        }
    }

    impl<V: SmlSerialize> SmlSerialize for BTreeMap<String, V> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Object(
                self.iter()
                    .map(|(k, v)| (k.clone(), v.to_sml_value()))
                    .collect(),
            )
        }
    }
    impl<V: SmlDeserialize> SmlDeserialize for BTreeMap<String, V> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Object(m) => {
                    let mut out = BTreeMap::new();
                    for (k, val) in m {
                        out.insert(k.clone(), V::from_sml_value(val)?);
                    }
                    Ok(out)
                }
                other => Err(format!("期望块（object），实际为 {}", describe_value(other))),
            }
        }
    }

    impl<V: SmlSerialize> SmlSerialize for HashMap<String, V> {
        #[inline]
        fn to_sml_value(&self) -> Value {
            Value::Object(
                self.iter()
                    .map(|(k, v)| (k.clone(), v.to_sml_value()))
                    .collect(),
            )
        }
    }
    impl<V: SmlDeserialize> SmlDeserialize for HashMap<String, V> {
        #[inline]
        fn from_sml_value(v: &Value) -> Result<Self, String> {
            match v {
                Value::Object(m) => {
                    let mut out = HashMap::new();
                    for (k, val) in m {
                        out.insert(k.clone(), V::from_sml_value(val)?);
                    }
                    Ok(out)
                }
                other => Err(format!("期望块（object），实际为 {}", describe_value(other))),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------- version ----------------

    #[test]
    fn version_defaults_to_v1_when_absent() {
        // 既有文档没有版本声明，必须仍能解析且默认为 V1（裸词即字符串，向后兼容）
        let (v, ver) = parse_versioned("a: 1\n").unwrap();
        assert_eq!(ver, Version::V1);
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
    }

    #[test]
    fn version_declared_as_v1() {
        let (v, ver) = parse_versioned("@version v1\na: 1\n").unwrap();
        assert_eq!(ver, Version::V1);
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
    }

    #[test]
    fn version_declaration_is_stripped_not_parsed_as_content() {
        // 若未剥离，`@version v1` 会被当成片段定义而解析异常
        let v = parse("@version v1\na: 1\n").unwrap();
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
        assert!(v.get("version").is_none(), "@version 不应进入数据");
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let err = parse_versioned("@version v99\na: 1\n").unwrap_err();
        assert!(err.contains("不支持"), "应拒绝不支持的版本，got: {err}");
        assert!(err.contains("v99"), "错误应含版本号，got: {err}");
    }

    #[test]
    fn conflicting_version_is_rejected() {
        let err = parse_versioned("@version v1\n@version v2\n").unwrap_err();
        // v2 尚未定义，优先报「不支持」
        assert!(!err.is_empty());
        // 两个都支持但不一致时的路径：v1 与 v1 不冲突
        let (_, ver) = parse_versioned("@version v1\n@version v1\n").unwrap();
        assert_eq!(ver, Version::V1, "重复但一致的声明应被接受");
    }

    #[test]
    fn version_is_reserved_as_fragment_name() {
        let err = parse("@version { x: 1 }\n").unwrap_err();
        assert!(err.contains("保留") || err.contains("版本声明"), "got: {err}");
    }

    #[test]
    fn version_works_with_include() {
        let d = tmpdir("version");
        std::fs::write(d.join("p.sml"), "@version v1\nb: 2\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\ninclude \"p.sml\"\n").unwrap();
        let (v, ver) = parse_file_versioned(d.join("main.sml")).unwrap();
        assert_eq!(ver, Version::V1);
        assert_eq!(v.get("b"), Some(&Value::Int(2)), "版本与 include 应协同");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn version_display_matches_name() {
        assert_eq!(Version::V1.name(), "v1");
        assert_eq!(format!("{}", Version::V1), "v1");
    }

    // ---------------- include ----------------

    /// 在临时目录下建文件，返回目录句柄（drop 时自动清理）
    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("sml_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("create tmpdir");
        d
    }

    #[test]
    fn include_inlines_external_file() {
        let d = tmpdir("inline");
        std::fs::write(d.join("part.sml"), "port: 8080\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\nhost: local\ninclude \"part.sml\"\n").unwrap();

        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(v.get("host").unwrap().as_str(), Some("local"));
        assert_eq!(v.get("port"), Some(&Value::Int(8080)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_at_prefix_is_equivalent() {
        let d = tmpdir("at");
        std::fs::write(d.join("p.sml"), "b: 2\n").unwrap();
        std::fs::write(d.join("m.sml"), "@include \"p.sml\"\n").unwrap();
        let v = parse_file(d.join("m.sml")).unwrap();
        assert_eq!(v.get("b"), Some(&Value::Int(2)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_resolves_relative_to_including_file() {
        // 关键：相对路径按「被包含文件自身目录」解析，而非进程工作目录
        let d = tmpdir("nested");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("sub/leaf.sml"), "@version v1\nleaf: yes\n").unwrap();
        // mid 在根，include sub/mid2；mid2 在 sub 内，include leaf.sml（相对 sub）
        std::fs::write(d.join("sub/mid2.sml"), "@version v1\ninclude \"leaf.sml\"\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\ninclude \"sub/mid2.sml\"\n").unwrap();

        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(
            v.get("leaf").unwrap().as_str(),
            Some("yes"),
            "嵌套 include 的路径应相对各自所在目录解析"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_inside_block_injects_fields() {
        // 文本内联语义：可在块内注入一组字段
        let d = tmpdir("block");
        std::fs::write(d.join("fields.sml"), "@version v1\nregion: cn-north-1\nzone: a\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\nserver web {\ninclude \"fields.sml\"\nport: 8080\n}\n").unwrap();

        let v = parse_file(d.join("main.sml")).unwrap();
        let server = v.get("server").expect("应有 server 块");
        assert_eq!(server.get("region").unwrap().as_str(), Some("cn-north-1"));
        assert_eq!(server.get("zone").unwrap().as_str(), Some("a"));
        assert_eq!(server.get("port"), Some(&Value::Int(8080)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_detects_cycles() {
        let d = tmpdir("cycle");
        std::fs::write(d.join("a.sml"), "include \"b.sml\"\n").unwrap();
        std::fs::write(d.join("b.sml"), "include \"a.sml\"\n").unwrap();
        let err = parse_file(d.join("a.sml")).unwrap_err();
        assert!(err.contains("循环引用"), "应报循环引用，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_missing_file_is_error() {
        let d = tmpdir("missing");
        std::fs::write(d.join("m.sml"), "include \"nope.sml\"\n").unwrap();
        let err = parse_file(d.join("m.sml")).unwrap_err();
        assert!(err.contains("nope.sml"), "错误应含缺失文件名，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn hash_in_quoted_string_is_not_a_comment() {
        // 引号内的 # 不应被当成注释，否则 `include "a#b.sml"` 会被截断
        assert_eq!(strip_line_comment("k: \"a#b\""), "k: \"a#b\"");
        assert_eq!(strip_line_comment("k: v # comment"), "k: v ");
    }

    #[test]
    fn glob_include_requires_feature() {
        // 未开启 glob-include 时，`*` 模式应报错
        let d = tmpdir("globoff");
        std::fs::write(d.join("a.sml"), "x: 1\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\ninclude \"*.sml\"\n").unwrap();
        let err = parse_file(d.join("main.sml")).unwrap_err();
        assert!(err.contains("glob-include"), "应要求 glob-include，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn glob_include_expands_multiple_files() {
        // 开启 glob-include 后，`lib/*.sml` 展开为子目录下所有 .sml（main.sml 不在该目录，避免自包含）
        let d = tmpdir("glob");
        std::fs::create_dir_all(d.join("lib")).unwrap();
        std::fs::write(d.join("lib/a.sml"), "@version v1\nx: 1\n").unwrap();
        std::fs::write(d.join("lib/b.sml"), "@version v1\ny: 2\n").unwrap();
        std::fs::write(d.join("note.txt"), "ignored\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\n@feature enable glob-include\ninclude \"lib/*.sml\"\n").unwrap();
        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(v.get("x"), Some(&Value::Int(1)));
        assert_eq!(v.get("y"), Some(&Value::Int(2)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn regex_include_requires_feature() {
        let d = tmpdir("regexoff");
        std::fs::write(d.join("a.sml"), "x: 1\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\ninclude \"re:.*\\.sml\"\n").unwrap();
        let err = parse_file(d.join("main.sml")).unwrap_err();
        assert!(err.contains("regex-include"), "应要求 regex-include，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn regex_include_matches_files() {
        let d = tmpdir("regex");
        std::fs::write(d.join("widget_a.sml"), "@version v1\nx: 1\n").unwrap();
        std::fs::write(d.join("widget_b.sml"), "@version v1\ny: 2\n").unwrap();
        std::fs::write(d.join("other.sml"), "@version v1\nz: 3\n").unwrap();
        std::fs::write(
            d.join("main.sml"),
            "@version v1\n@feature enable regex-include\ninclude \"re:widget_.*\\.sml\"\n",
        )
        .unwrap();
        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(v.get("x"), Some(&Value::Int(1)));
        assert_eq!(v.get("y"), Some(&Value::Int(2)));
        assert_eq!(v.get("z"), None, "other.sml 不应被正则匹配");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ext_rewrite_allows_non_sml() {
        // ext-rewrite 开启时，include 非 .sml 文件按 sml 解析
        let d = tmpdir("exrew");
        std::fs::write(d.join("conf.smlc"), "@version v1\nx: 9\n").unwrap();
        std::fs::write(
            d.join("main.sml"),
            "@version v1\n@feature enable ext-rewrite\ninclude \"conf.smlc\"\n",
        )
        .unwrap();
        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(v.get("x"), Some(&Value::Int(9)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_line_is_not_confused_with_key_named_include() {
        let f = FeatureSet::baseline();
        // `key: include` 不是指令——前面有 key 与冒号
        assert_eq!(parse_include_line("key: include", f), Ok(None));
        // 带扩展名无 as ⇒ 普通内联（namespace = None）
        assert_eq!(
            parse_include_line("include \"a.sml\"", f),
            Ok(Some(vec![IncludeTarget { raw: "a.sml".into(), namespace: None, via_import: false, keys: None }]))
        );
        // @include 等价
        assert_eq!(
            parse_include_line("@include \"a.sml\"", f),
            Ok(Some(vec![IncludeTarget { raw: "a.sml".into(), namespace: None, via_import: false, keys: None }]))
        );
        // 显式 as ns
        assert_eq!(
            parse_include_line("include \"a.sml\" as ui.form", f),
            Ok(Some(vec![IncludeTarget { raw: "a.sml".into(), namespace: Some("ui.form".into()), via_import: false, keys: None }]))
        );
        // 无扩展名 ⇒ implicit-ns 默认 as 文件名
        assert_eq!(
            parse_include_line("include \"widgets\"", f),
            Ok(Some(vec![IncludeTarget { raw: "widgets".into(), namespace: Some("widgets".into()), via_import: false, keys: None }]))
        );
        // import 别名
        assert_eq!(
            parse_include_line("import ui.buttons", f),
            Ok(Some(vec![IncludeTarget { raw: "ui.buttons".into(), namespace: Some("ui.buttons".into()), via_import: true, keys: None }]))
        );
        // 多目标（需 multi-include）
        let fm = FeatureSet::all();
        assert_eq!(
            parse_include_line("include \"a.sml\", \"b\" as y", fm),
            Ok(Some(vec![
                IncludeTarget { raw: "a.sml".into(), namespace: None, via_import: false, keys: None },
                IncludeTarget { raw: "b".into(), namespace: Some("y".into()), via_import: false, keys: None },
            ]))
        );
        // 注释行不生效
        assert_eq!(parse_include_line("# include \"a.sml\"", f), Ok(None));
    }

    #[test]
    fn import_partial_keys_both_syntaxes() {
        let f = FeatureSet::all();
        // 语法①：import "x.sml" as w { a, b }
        assert_eq!(
            parse_include_line("import \"m.sml\" as w { a, b }", f),
            Ok(Some(vec![IncludeTarget {
                raw: "m.sml".into(),
                namespace: Some("w".into()),
                via_import: true,
                keys: Some(vec!["a".into(), "b".into()]),
            }]))
        );
        // 语法①无 as：平铺挑键（namespace 为 None，不触发 implicit-ns）
        assert_eq!(
            parse_include_line("import \"m.sml\" { a, b }", f),
            Ok(Some(vec![IncludeTarget {
                raw: "m.sml".into(),
                namespace: None,
                via_import: true,
                keys: Some(vec!["a".into(), "b".into()]),
            }]))
        );
        // 语法②：import { a, b } as w in "m.sml"
        assert_eq!(
            parse_include_line("import { a, b } as w in \"m.sml\"", f),
            Ok(Some(vec![IncludeTarget {
                raw: "m.sml".into(),
                namespace: Some("w".into()),
                via_import: true,
                keys: Some(vec!["a".into(), "b".into()]),
            }]))
        );
        // 语法②无 as：平铺挑键
        assert_eq!(
            parse_include_line("import { a, b } in \"m.sml\"", f),
            Ok(Some(vec![IncludeTarget {
                raw: "m.sml".into(),
                namespace: None,
                via_import: true,
                keys: Some(vec!["a".into(), "b".into()]),
            }]))
        );
        // 空键列表报错
        assert!(parse_include_line("import \"m.sml\" { }", f).is_err());
        // 语法②缺少 in "file" 报错
        assert!(parse_include_line("import { a, b } as w", f).is_err());
        // 部分引用不能配 glob 通配
        assert!(parse_include_line("import \"*.sml\" { a }", f).is_err());
    }

    // ---------------- 邮箱 / 裸词中的 @ ----------------

    #[test]
    fn email_in_bare_word_survives() {
        // 回归：裸词中的 `@` 曾被切成 At token，导致邮箱被截断为 `a`
        let v = parse("to: a@b.c\nfrom: \"SML Team <dev@mail.swebase.cn>\"\n").unwrap();
        assert_eq!(v.get("to").unwrap().as_str(), Some("a@b.c"), "got: {v:?}");
        assert_eq!(
            v.get("from").unwrap().as_str(),
            Some("SML Team <dev@mail.swebase.cn>"),
            "got: {v:?}"
        );
    }

    #[test]
    fn email_roundtrips_through_to_sml() {
        let v = Value::Object(BTreeMap::from([(
            "to".to_string(),
            Value::Str("dev@mail.swebase.cn".into()),
        )]));
        let back = parse(&to_sml(&v)).unwrap();
        assert_eq!(back, v, "邮箱必须能往返，got:\n{}", to_sml(&v));
    }

    #[test]
    fn fragment_definition_still_works() {
        // 词首的 `@` 仍是片段定义标记，不能被上面的修改破坏。
        // 注：SML 的片段继承用法是「定义后作为值引用」（`k: &base`）；
        // 块内裸写 `&base` 会被当作键，不属于本用例覆盖范围。
        let v = parse("@base { region: cn }\nregion: &base\n").unwrap();
        assert_eq!(
            v.get("region").unwrap().get("region").unwrap().as_str(),
            Some("cn"),
            "片段引用应展开为定义的内容，got: {v:?}"
        );
    }

    // ---------------- 顶层数组 / 对象（与 to_sml 对称）----------------

    #[test]
    fn toplevel_array_roundtrips() {
        // 回归：to_sml 能输出顶层数组，但 parse 曾只认键值块，
        // 导致「能写不能读」（"期望键, 得 LBrack"）。
        let v = Value::Array(vec![
            Value::Object(BTreeMap::from([
                ("ts".to_string(), Value::Str("2026-01-01".into())),
                ("to".to_string(), Value::Str("a@b.c".into())),
            ])),
            Value::Object(BTreeMap::from([
                ("ts".to_string(), Value::Str("2026-01-02".into())),
                ("to".to_string(), Value::Str("x@y.z".into())),
            ])),
        ]);
        let text = to_sml(&v);
        let back = parse(&text).unwrap();
        assert_eq!(back, v, "顶层对象数组必须能往返，got text:\n{text}");
    }

    #[test]
    fn toplevel_array_of_scalars_roundtrips() {
        let v = Value::Array(vec![
            Value::Int(1),
            Value::Str("two".into()),
            Value::Bool(true),
        ]);
        let back = parse(&to_sml(&v)).unwrap();
        assert_eq!(back, v, "顶层标量数组必须能往返");
    }

    #[test]
    fn toplevel_object_block_roundtrips() {
        let mut m = BTreeMap::new();
        m.insert("k".to_string(), Value::Int(1));
        let v = Value::Object(m);
        let back = parse(&to_sml(&v)).unwrap();
        assert_eq!(back, v, "顶层对象块必须能往返");
    }

    #[test]
    fn toplevel_empty_array_roundtrips() {
        let v = Value::Array(vec![]);
        let back = parse(&to_sml(&v)).unwrap();
        assert_eq!(back, v, "空数组必须能往返");
    }

    // ---------------- serde ----------------

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_preserves_shape() {
        let v = parse("name: John\nage: 27\ntags: [a b]\nnested { k: v }\n").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        // 自然形状：字符串就是字符串，数字就是数字，而非 {"Int":27}
        assert!(json.contains("\"name\":\"John\""), "got: {json}");
        assert!(json.contains("\"age\":27"), "got: {json}");
        assert!(json.contains("\"tags\":[\"a\",\"b\"]"), "got: {json}");
        assert!(json.contains("\"nested\":{\"k\":\"v\"}"), "got: {json}");

        let back: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v, "serde 往返应还原原值");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_deserializes_json_into_value() {
        let v: Value = serde_json::from_str(r#"{"s":"x","i":5,"f":1.5,"b":true,"n":null,"a":[1,2]}"#).unwrap();
        assert_eq!(v.get("s").unwrap().as_str(), Some("x"));
        assert_eq!(v.get("i"), Some(&Value::Int(5)));
        assert_eq!(v.get("f"), Some(&Value::Float(1.5)));
        assert_eq!(v.get("b"), Some(&Value::Bool(true)));
        assert_eq!(v.get("n"), Some(&Value::Null));
        assert!(matches!(v.get("a"), Some(Value::Array(a)) if a.len() == 2));
    }

    #[test]
    fn nested_array_inside_object_inside_array_survives_roundtrip() {
        // 回归测试：数组元素是对象、对象里又有数组（如配置的条目列表）。
        // dump_inline 曾把嵌套数组缩略成 [..]，导致 chunks 丢成 [".."]。
        let mut item = BTreeMap::new();
        item.insert("path".to_string(), Value::Str("a.txt".into()));
        item.insert(
            "chunks".to_string(),
            Value::Array(vec![
                Value::Str("c1".into()),
                Value::Str("c2".into()),
            ]),
        );
        let mut root = BTreeMap::new();
        root.insert(
            "entries".to_string(),
            Value::Array(vec![Value::Object(item)]),
        );
        let text = to_sml(&Value::Object(root));
        assert!(!text.contains("[..]"), "嵌套数组不得被缩略: {text}");

        let back = parse(&text).unwrap();
        let chunks = back.get("entries").and_then(|e| match e {
            Value::Array(a) => a.first(),
            _ => None,
        });
        let chunks = match chunks {
            Some(Value::Object(m)) => m.get("chunks"),
            _ => None,
        };
        match chunks {
            Some(Value::Array(a)) => {
                assert_eq!(a.len(), 2, "两个块都应保留: {text}");
                assert_eq!(
                    a.iter().filter_map(|c| c.as_str()).collect::<Vec<_>>(),
                    vec!["c1", "c2"]
                );
            }
            other => panic!("chunks 应解析为数组，实际 {other:?}"),
        }
    }

    #[test]
    fn utf8_in_quoted_string_survives_roundtrip() {
        // 回归测试：tokenizer 曾按字节 `as char` 逐个处理，
        // 把 UTF-8 多字节字符拆成 Latin-1 字符，导致
        // `"修复若干问题"` 解析后变成双编码乱码。
        let v = parse(r#"note: "修复若干问题""#).unwrap();
        assert_eq!(
            v.get("note").and_then(|x| x.as_str()),
            Some("修复若干问题"),
            "引号串中的中文不应被破坏"
        );
        // 裸词中文同样不能破坏
        let v2 = parse("region: 华北").unwrap();
        assert_eq!(v2.get("region").and_then(|x| x.as_str()), Some("华北"));
        // 转义 \u 序列
        let v3 = parse(r#"k: "\u{4fee}\u{590d}""#).unwrap();
        assert_eq!(v3.get("k").and_then(|x| x.as_str()), Some("修复"));
    }

    #[test]
    fn parse_basic() {
        let text = "firstName: John\nage: 27\nisAlive: true\nspouse: null\n";
        let v = parse(text).unwrap();
        assert_eq!(v.get("firstName"), Some(&Value::Str("John".into())));
        assert_eq!(v.get("age"), Some(&Value::Int(27)));
        assert_eq!(v.get("isAlive"), Some(&Value::Bool(true)));
        assert_eq!(v.get("spouse"), Some(&Value::Null));
    }

    #[test]
    fn parse_nested() {
        let text = "address:\n{\n    streetAddress: \"21 2nd Street\"\n    state: NY\n}\n";
        let v = parse(text).unwrap();
        assert_eq!(
            v.get("address.streetAddress"),
            Some(&Value::Str("21 2nd Street".into()))
        );
        assert_eq!(v.get("address.state"), Some(&Value::Str("NY".into())));
    }

    #[test]
    fn parse_array() {
        let text = "phoneNumbers:\n[\n    { type: home }\n    { type: office }\n]\n";
        let v = parse(text).unwrap();
        if let Some(Value::Array(a)) = v.get("phoneNumbers") {
            assert_eq!(a.len(), 2);
            assert_eq!(a[0].get("type"), Some(&Value::Str("home".into())));
        } else {
            panic!("not array");
        }
    }

    #[test]
    fn parse_fragment() {
        let text = "@base { region: cn-north-1 }\nserver web { &base }\n";
        let v = parse(text).unwrap();
        // &base 展开为字段 (键名 "&base", 值=片段对象), 与 Lua 实现一致
        assert_eq!(
            v.get("server.&base.region"),
            Some(&Value::Str("cn-north-1".into()))
        );
        assert_eq!(v.get("server.__type"), Some(&Value::Str("server".into())));
        assert_eq!(v.get("server.__name"), Some(&Value::Str("web".into())));
    }

    #[test]
    fn roundtrip() {
        let text = "name: myapp\nport: 8080\nflags: [ a b c ]\n";
        let v = parse(text).unwrap();
        let out = to_sml(&v);
        let v2 = parse(&out).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn env_inline() {
        // Rust 1.85+ 起 set_var 为 unsafe（与 edition 无关，2021/2024 均需）
        unsafe { std::env::set_var("SML_TEST_VAR", "hello") };
        let text = "greeting: $env.SML_TEST_VAR\n";
        let v = parse(text).unwrap();
        assert_eq!(v.get("greeting"), Some(&Value::Str("hello".into())));
    }

    #[test]
    fn c_abi_json_bridge() {
        let text = "name: John\nage: 27\n";
        let v = parse(text).unwrap();
        let j = jsonify(&v);
        assert!(j.contains("\"name\":\"John\""));
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }
}

// ===========================================================================
// @feature 特性裁剪 + 调用方限制 测试
// ===========================================================================

#[cfg(test)]
mod feature {
    use super::*;

    #[test]
    fn feature_unknown_name_errors() {
        let r = parse("@feature enable nope\nx: 1\n");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("未知特性"));
    }

    #[test]
    fn feature_whitelist_narrows() {
        // 仅保留 bareword 与 include，其它（env/fragment/contract...）关闭
        let v = match parse("@feature whitelist bareword-string,include\nx: John\n").unwrap() {
            Value::Object(m) => m,
            _ => panic!("应为对象"),
        };
        assert_eq!(v.get("x"), Some(&Value::Str("John".into())));
    }

    #[test]
    fn feature_blacklist_removes() {
        // 关掉 bareword-string：v1 文档里裸词字符串也应被拒
        let r = parse("@feature blacklist bareword-string\nx: John\n");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("字符串必须加引号"));
    }

    #[test]
    fn feature_mode_whitelist_enable() {
        // mode whitelist 后基集清空，仅 enable 的生效
        let r = parse("@feature mode whitelist\n@feature enable fragment\nx: &frag\n");
        // fragment 没定义，回退为字符串 "&frag"，不报错即可
        assert!(r.is_ok());
    }

    #[test]
    fn caller_allowed_intersection_empty_errors() {
        // 调用方只接受 env；文档用白名单模式只开 contract —— 与调用方无交集则报错
        let allowed = FeatureSet::none().with(Feature::Env);
        let r = parse_with_features(
            "@feature mode whitelist\n@feature enable contract\nx: 1\n",
            allowed,
        );
        assert!(r.is_err());
    }

    #[test]
    fn caller_allowed_subset_ok() {
        // 调用方允许全部，文档收窄到 bareword+include，应成功
        let allowed = FeatureSet::all();
        let (v, eff) = parse_with_features(
            "@feature whitelist bareword-string,include\nx: John\n",
            allowed,
        )
        .unwrap();
        assert!(eff.has(Feature::BarewordStr));
        assert!(eff.has(Feature::Include));
        assert!(!eff.has(Feature::Env));
        assert_eq!(v.get("x"), Some(&Value::Str("John".into())));
    }

    #[test]
    fn feature_namespace_include() {
        // 用临时文件验证 include "x.sml" as ns 把键挂到 ns 下。
        // 用相对路径 + 正斜杠，避开 Windows 反斜杠在字符串转义中的处理。
        // 注意：include 展开只在 parse_file 进行，故这里把主文档也落盘。
        let dir = std::env::temp_dir().join("sml_feat_ns_test");
        let _ = std::fs::create_dir_all(&dir);
        let sub = dir.join("sub.sml");
        let main = dir.join("main.sml");
        std::fs::write(&sub, "a: 1\nb: 2\n").unwrap();
        // 用正斜杠书写相对路径，避免反斜杠被字符串转义吃掉
        let rel = format!("include \"sub.sml\" as pkg\n");
        std::fs::write(&main, &rel).unwrap();
        let v = match parse_file(&main) {
            Ok(v) => v,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&dir);
                panic!("parse_file 失败: {e}");
            }
        };
        let _ = std::fs::remove_dir_all(&dir);
        let pkg = match v.get("pkg") {
            Some(Value::Object(m)) => m.clone(),
            _ => panic!("pkg 应为对象"),
        };
        assert_eq!(pkg.get("a"), Some(&Value::Int(1)));
        assert_eq!(pkg.get("b"), Some(&Value::Int(2)));
    }

    #[test]
    fn version_v3_disables_bareword() {
        // v3 默认关闭 bareword-string；裸词应被拒
        let r = parse("@version v3\nname: John\n");
        assert!(r.is_err());
        // 但引号字符串可用
        let v = parse("@version v3\nname: \"John\"\nage: 27\n").unwrap();
        assert_eq!(v.get("name"), Some(&Value::Str("John".into())));
        assert_eq!(v.get("age"), Some(&Value::Int(27)));
    }

    #[test]
    fn feature_base_derives_strict() {
        // @feature base v3 等价于 v3 严格
        let r = parse("@feature base v3\nname: John\n");
        assert!(r.is_err());
    }
}
