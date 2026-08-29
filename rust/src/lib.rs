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
                // 注释到行尾
                for c2 in chars.by_ref() {
                    if c2 == '\n' {
                        break;
                    }
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

fn coerce_word(w: &str, fragments: &BTreeMap<String, Value>) -> Value {
    match w {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        "null" => return Value::Null,
        _ => {}
    }
    // $env.VAR 内联
    if let Some(ev) = w.strip_prefix("$env.") {
        return Value::Str(std::env::var(ev).unwrap_or_default());
    }
    // 片段引用 &name
    if let Some(name) = w.strip_prefix('&') {
        if let Some(v) = fragments.get(name) {
            return v.clone();
        }
        return Value::Str(w.to_string());
    }
    // 数字: int / float / 科学计数
    if let Ok(i) = w.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = w.parse::<f64>() {
        return Value::Float(f);
    }
    Value::Str(w.to_string())
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
    fragments: BTreeMap<String, Value>,
    /// 契约表：名 -> 契约。由 `@contract Name { ... }` 填充
    contracts: BTreeMap<String, Contract>,
}

impl Parser {
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
                            Some(Tok::Word(w2)) => coerce_word(&w2, &self.fragments),
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
                        self.contracts.insert(
                            cname.clone(),
                            Contract { name: cname, fields, allow_extra },
                        );
                        continue;
                    }
                    // —— 契约应用：`@is Name`（在当前块内）——
                    if fname == "is" {
                        let cname = match self.next() {
                            Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                            other => {
                                return Err(format!("sml: @is 后须契约名, 得 {:?}", other))
                            }
                        };
                        applied_contract = Some(cname);
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
                        self.fragments.insert(fname, Value::Object(sub));
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
                            args.push(coerce_word(&w, &self.fragments));
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
                    let mut sub = self.parse_block(Some(Tok::RBrace))?;
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
                    Tok::Word(w) => coerce_word(&w, &self.fragments),
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
                    Ok(coerce_word(key, &self.fragments))
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
                    arr.push(coerce_word(&w, &self.fragments));
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    /// v1：初始公开版本
    V1,
}

impl Version {
    /// 当前实现支持的最新版本
    pub const CURRENT: Version = Version::V1;

    /// 解析版本字面量（`v1` / `1`）
    fn from_word(w: &str) -> Option<Version> {
        match w {
            "v1" | "1" => Some(Version::V1),
            _ => None,
        }
    }

    /// 版本名（用于错误信息与序列化回显）
    pub fn name(self) -> &'static str {
        match self {
            Version::V1 => "v1",
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
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

/// 解析 SML 文本，并返回其声明的语法版本。
///
/// 未声明版本时按 `Version::CURRENT`（v1）处理，**既有文档不受影响**。
pub fn parse_versioned(text: &str) -> Result<(Value, Version), String> {
    let (rest, declared) = strip_version(text)?;
    Ok((parse_impl(&rest)?, declared.unwrap_or(Version::CURRENT)))
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
    let expanded = resolve_includes(&text, &base)?;
    parse_versioned(&expanded)
}

/// 解析 SML 文本
///
/// 会自动识别并剥离 `@version` 声明（需要版本信息时用 [`parse_versioned`]）。
pub fn parse(text: &str) -> Result<Value, String> {
    let (rest, _) = strip_version(text)?;
    parse_impl(&rest)
}

/// 不含版本处理的底层解析
fn parse_impl(text: &str) -> Result<Value, String> {
    let toks = tokenize(text)?;
    let mut p = Parser {
        toks,
        i: 0,
        fragments: BTreeMap::new(),
        contracts: BTreeMap::new(),
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

/// 若该行是 include 指令，返回目标路径；否则返回 None
fn include_target(line: &str) -> Option<String> {
    let content = strip_line_comment(line).trim();
    let content = content.strip_prefix('@').unwrap_or(content).trim_start();
    // 复用词法器处理路径，使含空格的路径（引号串）能被正确识别
    let toks = tokenize(content).ok()?;
    match toks.as_slice() {
        [Tok::Word(w), Tok::Str(p)] if w == "include" => Some(p.clone()),
        [Tok::Word(w), Tok::Word(p)] if w == "include" => Some(p.clone()),
        _ => None,
    }
}

/// 把 text 中的 include 指令递归展开为不含指令的纯 SML 文本。
///
/// `base` 为相对路径的解析基准目录（通常是当前文件所在目录）。
/// 循环引用与缺失文件都会返回错误，不会静默跳过。
pub fn resolve_includes(text: &str, base: &Path) -> Result<String, String> {
    let mut out = String::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    expand_includes(text, base, &mut out, &mut stack)?;
    Ok(out)
}

fn expand_includes(
    text: &str,
    base: &Path,
    out: &mut String,
    stack: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if stack.len() >= MAX_INCLUDE_DEPTH {
        return Err(format!("include 嵌套超过 {MAX_INCLUDE_DEPTH} 层"));
    }
    for line in text.lines() {
        match include_target(line) {
            Some(rel) => {
                let path = base.join(&rel);
                let canon = path
                    .canonicalize()
                    .map_err(|e| format!("include 无法定位 {}: {e}", path.display()))?;
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
                stack.push(canon);
                expand_includes(&content, &child_base, out, stack)?;
                stack.pop();
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    Ok(())
}

/// 解析 SML 文件，并展开其中的 include 指令。
///
/// 相对路径以**该文件所在目录**为基准。
pub fn parse_file(path: impl AsRef<Path>) -> Result<Value, String> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读取失败 {}: {e}", path.display()))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let expanded = resolve_includes(&text, &base)?;
    parse(&expanded)
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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

/// sml_free(p): 释放由 sml_parse / sml_dump 返回的字符串
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sml_free(p: *mut c_char) {
    if !p.is_null() {
        drop(unsafe { std::ffi::CString::from_raw(p) });
    }
}

/// sml_version() -> 版本字符串 (调用方 sml_free)
#[unsafe(no_mangle)]
pub extern "C" fn sml_version() -> *mut c_char {
    cstr(concat!("sml ", env!("CARGO_PKG_VERSION")))
}

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
    fn version_defaults_to_current_when_absent() {
        // 既有文档没有版本声明，必须仍能解析且默认为当前版本
        let (v, ver) = parse_versioned("a: 1\n").unwrap();
        assert_eq!(ver, Version::CURRENT);
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
        std::fs::write(d.join("main.sml"), "host: local\ninclude \"part.sml\"\n").unwrap();

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
        std::fs::write(d.join("sub/leaf.sml"), "leaf: yes\n").unwrap();
        // mid 在根，include sub/mid2；mid2 在 sub 内，include leaf.sml（相对 sub）
        std::fs::write(d.join("sub/mid2.sml"), "include \"leaf.sml\"\n").unwrap();
        std::fs::write(d.join("main.sml"), "include \"sub/mid2.sml\"\n").unwrap();

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
        std::fs::write(d.join("fields.sml"), "region: cn-north-1\nzone: a\n").unwrap();
        std::fs::write(d.join("main.sml"), "server web {\ninclude \"fields.sml\"\nport: 8080\n}\n").unwrap();

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
    fn include_line_is_not_confused_with_key_named_include() {
        // `key: include` 是指令吗？不是——前面有 key 与冒号
        assert_eq!(include_target("key: include"), None);
        assert_eq!(include_target("include \"a.sml\""), Some("a.sml".into()));
        assert_eq!(include_target("@include \"a.sml\""), Some("a.sml".into()));
        assert_eq!(include_target("# include \"a.sml\""), None, "注释行不生效");
    }

    // ---------------- 邮箱 / 裸词中的 @ ----------------

    #[test]
    fn email_in_bare_word_survives() {
        // 回归：裸词中的 `@` 曾被切成 At token，导致邮箱被截断为 `a`
        let v = parse("to: a@b.c\nfrom: \"sal <sal@mail.swebase.cn>\"\n").unwrap();
        assert_eq!(v.get("to").unwrap().as_str(), Some("a@b.c"), "got: {v:?}");
        assert_eq!(
            v.get("from").unwrap().as_str(),
            Some("sal <sal@mail.swebase.cn>"),
            "got: {v:?}"
        );
    }

    #[test]
    fn email_roundtrips_through_to_sml() {
        let v = Value::Object(BTreeMap::from([(
            "to".to_string(),
            Value::Str("SALflake@qq.com".into()),
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
        // Rust 2024 edition 下 set_var 为 unsafe（1.85+）
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
