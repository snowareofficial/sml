//! SML 模式语言（Loom 融入 SML 的产物）：L1 层的无回溯匹配引擎。
//!
//! # 为什么不是正则
//!
//! 规则就是**普通 SML 数据**（片段），不引入新 token，因此能被 SML 自身的
//! include / 契约 / 中文关键字复用。匹配侧用 Thompson NFA 并行推进，
//! **结构上不存在回溯**，故免疫 ReDoS（见 [`nfa`] 模块说明）。
//!
//! # 关键字与语言无关
//!
//! 书写形式（中文 / 英文 / 任意语言）由 [`KeywordTable`] trait 决定，
//! 编译逻辑只认 [`Concept`]。默认 [`BILINGUAL`]（中英等价）。
//!
//! # 规则写法（中英等价，可混用）
//!
//! ```sml
//! @日期ISO {
//!     序列: [
//!         { 名: 年, 类: 数字, 次: 4 }
//!         { 字面: "-" }
//!         { 名: 月, 类: 数字, 次: 2 }
//!     ]
//! }
//! ```

mod i18n;
mod nfa;
pub mod regex;

use std::collections::BTreeMap;

pub use i18n::{BILINGUAL, CHINESE, Concept, ENGLISH, KeywordTable, StaticTable};
use i18n::Concept::*;

use sml_value::Value;

/// 字符类。默认 Unicode 语义：`Alpha` 匹配汉字，`Digit` 匹配全角数字。
/// （regex 的 `\d` 在 JS 是 ASCII、Python 是 Unicode，这类跨语言不一致是 bug 温床，
/// 这里明确选定 Unicode 一侧。）
#[derive(Debug, Clone, PartialEq)]
pub enum Class {
    Digit,
    Alpha,
    Space,
    Word,
    Any,
    /// 单个字符（由字面量编译而来）
    One(char),
    /// 字符范围 `[a-z]`
    Range(char, char),
    /// 字符类的并集 `[abc]` / `[a-z_]`
    Set(Vec<Class>),
    /// 补集 `[^...]` / `\D` `\W` `\S`
    Not(Box<Class>),
}

impl Class {
    pub fn matches(&self, ch: char) -> bool {
        match self {
            Class::Digit => ch.is_numeric(),
            Class::Alpha => ch.is_alphabetic(),
            Class::Space => ch.is_whitespace(),
            Class::Word => ch.is_alphanumeric() || ch == '_',
            Class::Any => true,
            Class::One(c) => ch == *c,
            Class::Range(a, b) => (*a..=*b).contains(&ch),
            Class::Set(cs) => cs.iter().any(|c| c.matches(ch)),
            Class::Not(c) => !c.matches(ch),
        }
    }
}

/// 模式 IR（L1：无递归、无守卫）
// PartialEq 供 sml-contract 的 TypeSpec 派生比较（契约类型需要可比较）
#[derive(Debug, Clone, PartialEq)]
pub enum Pat {
    Class(Class),
    Lit(String),
    Seq(Vec<Pat>),
    Alt(Vec<Pat>),
    /// 量词。`max: None` 表示无上界（`+` / `*`）
    Repeat {
        pat: Box<Pat>,
        min: usize,
        max: Option<usize>,
    },
    /// 命名捕获：与规则定义同形，只是多一个名字
    Named {
        name: String,
        pat: Box<Pat>,
    },
    /// 反复推进直到 `pat` 成立（等价 regex 的 `.*?pat`，但无贪婪性概念）
    Until {
        pat: Box<Pat>,
    },
}

// ---------------------------------------------------------------------------
// 从 SML 数据编译
// ---------------------------------------------------------------------------

/// 默认步数预算：无回溯已保证多项式时间，此上限仅用于拦住极端病态输入。
const DEFAULT_BUDGET: u64 = 10_000_000;

/// 把一条规则编译为模式 IR，关键字表取默认 [`BILINGUAL`]。
pub fn compile_rule(rule: &Value, rules: &BTreeMap<String, Value>) -> Result<Pat, String> {
    compile_rule_with(rule, rules, &BILINGUAL)
}

/// 用指定的关键字表编译（供非中英语言或项目自有方言使用）。
pub fn compile_rule_with(
    rule: &Value,
    rules: &BTreeMap<String, Value>,
    table: &dyn KeywordTable,
) -> Result<Pat, String> {
    let mut stack = Vec::new();
    compile_value(rule, rules, &mut stack, table)
}

fn compile_value(
    v: &Value,
    rules: &BTreeMap<String, Value>,
    stack: &mut Vec<String>,
    table: &dyn KeywordTable,
) -> Result<Pat, String> {
    match v {
        // 数组：视为序列（保序，这是规则体的常见形态）
        Value::Array(items) => {
            let mut seq = Vec::new();
            for it in items {
                seq.push(compile_value(it, rules, stack, table)?);
            }
            Ok(Pat::Seq(seq))
        }
        // 裸词 / 字符串：字面量
        Value::Str(s) => Ok(Pat::Lit(s.clone())),
        Value::Object(map) => compile_elem(map, rules, stack, table),
        other => Err(format!("sml: 模式元素类型不支持: {}", type_name(other))),
    }
}

fn compile_elem(
    map: &BTreeMap<String, Value>,
    rules: &BTreeMap<String, Value>,
    stack: &mut Vec<String>,
    table: &dyn KeywordTable,
) -> Result<Pat, String> {
    // 1) 按键的**语义**归类。遍历而非硬编码查某个键名，
    //    这样换一张关键字表就能换语言，编译逻辑无需改动。
    let mut lit: Option<&Value> = None;
    let mut cls: Option<&Value> = None;
    let mut alt: Option<&Value> = None;
    let mut grp: Option<&Value> = None;
    let mut seq: Option<&Value> = None;
    let mut use_ref: Option<&Value> = None;
    let mut until: Option<&Value> = None;
    let mut times: Option<&Value> = None;
    let mut optional: Option<&Value> = None;
    let mut name: Option<&Value> = None;
    let mut regex_src: Option<&Value> = None;
    // 平铺量词（机翻等价直觉写法）：最小 / 最大 直接写在元素上，
    // 与 `次: { 最小, 最大 }` 同义。此前归入 `_ => {}` 被静默忽略，现已生效。
    let mut flat_min_raw: Option<&Value> = None;
    let mut flat_max_raw: Option<&Value> = None;

    for (k, v) in map.iter() {
        match table.lookup(k) {
            Some(Concept::Lit) => lit = Some(v),
            Some(Concept::Class) => cls = Some(v),
            Some(Concept::Alt) => alt = Some(v),
            Some(Concept::Group) => grp = Some(v),
            Some(Concept::Seq) => seq = Some(v),
            Some(Concept::Use) => use_ref = Some(v),
            Some(Concept::Until) => until = Some(v),
            Some(Concept::Times) => times = Some(v),
            Some(Concept::Optional) => optional = Some(v),
            Some(Concept::Min) => flat_min_raw = Some(v),
            Some(Concept::Max) => flat_max_raw = Some(v),
            Some(Concept::Name) => name = Some(v),
            Some(Concept::Regex) => regex_src = Some(v),
            // 未知键：容忍，便于在规则里附加说明性字段（如 说明: "手机号"）
            _ => {}
        }
    }

    // 2) 基底
    let base = if let Some(v) = lit {
        Pat::Lit(as_str(v, "字面/lit")?)
    } else if let Some(v) = cls {
        Pat::Class(parse_class(&as_str(v, "类/class")?, table)?)
    } else if let Some(v) = alt {
        let items = as_array(v, "任一/alt")?;
        let mut alts = Vec::new();
        for it in items {
            alts.push(compile_value(it, rules, stack, table)?);
        }
        Pat::Alt(alts)
    } else if let Some(v) = grp {
        compile_value(v, rules, stack, table)?
    } else if let Some(v) = seq {
        compile_value(v, rules, stack, table)?
    } else if let Some(v) = use_ref {
        let ref_name = as_str(v, "用/use")?;
        if stack.contains(&ref_name) {
            return Err(format!(
                "sml: 规则 `{ref_name}` 存在循环引用（L1 不支持递归；如需嵌套结构请先升级到 L2）"
            ));
        }
        let target = rules
            .get(&ref_name)
            .ok_or_else(|| format!("sml: 未定义的规则 `{ref_name}`"))?;
        stack.push(ref_name.clone());
        let p = compile_value(target, rules, stack, table)?;
        stack.pop();
        p
    } else if let Some(v) = until {
        let u = as_str(v, "直到/until")?;
        match table.lookup(&u) {
            // 「直到行尾」= 吃掉剩余全部字符
            Some(Concept::Eol) => Pat::Repeat {
                pat: Box::new(Pat::Class(Class::Any)),
                min: 0,
                max: None,
            },
            _ => Pat::Until {
                pat: Box::new(Pat::Lit(u)),
            },
        }
    } else if let Some(v) = regex_src {
        // regex 逃生舱：解析后编译进同一个 IR，故仍是无回溯、仍免疫 ReDoS
        let src = as_str(v, "正则/regex")?;
        crate::regex::parse_regex(&src)
            .map_err(|e| format!("sml: regex 编译失败：{e}"))?
    } else {
        return Err(format!(
            "sml: 无法识别的模式元素（需 字面/类/任一/组/序列/用/直到/正则 之一），实际键: {:?}",
            map.keys().collect::<Vec<_>>()
        ));
    };

    // 3) 量词：优先平铺 最小/最大（机翻等价直觉写法），否则用 次 / 可选。
    //    任一来源只产生一个 Repeat，避免重复包裹。无效输入一律报错，绝不静默忽略。
    let flat_min = match flat_min_raw {
        Some(v) => Some(as_int(v).ok_or_else(|| format!("sml: 量词最小必须为整数，得 {v:?}"))?),
        None => None,
    };
    let flat_max = match flat_max_raw {
        Some(v) => Some(as_int(v).ok_or_else(|| format!("sml: 量词最大必须为整数，得 {v:?}"))?),
        None => None,
    };

    let base = if flat_min.is_some() || flat_max.is_some() {
        let mn = flat_min.unwrap_or(0);
        let mx = flat_max;
        if mn < 0 {
            return Err(format!("sml: 量词最小不能为负（得 {mn}）"));
        }
        if let Some(mxv) = mx {
            if mxv < mn {
                return Err(format!("sml: 量词最大({mxv})不能小于最小({mn})"));
            }
        }
        Pat::Repeat {
            pat: Box::new(base),
            min: mn as usize,
            max: mx.map(|x| x as usize),
        }
    } else if let Some(t) = times {
        let (min, max) = parse_times(t, table)?;
        Pat::Repeat {
            pat: Box::new(base),
            min,
            max,
        }
    } else if matches!(optional, Some(Value::Bool(true))) {
        Pat::Repeat {
            pat: Box::new(base),
            min: 0,
            max: Some(1),
        }
    } else {
        base
    };

    // 5) 命名
    match name {
        None => Ok(base),
        Some(n) => Ok(Pat::Named {
            name: as_str(n, "名/name")?,
            pat: Box::new(base),
        }),
    }
}

fn parse_class(s: &str, table: &dyn KeywordTable) -> Result<Class, String> {
    match table.lookup(s) {
        Some(Concept::Digit) => Ok(Class::Digit),
        Some(Concept::Alpha) => Ok(Class::Alpha),
        Some(Concept::Space) => Ok(Class::Space),
        Some(Concept::Word) => Ok(Class::Word),
        Some(Concept::Any) => Ok(Class::Any),
        // 非内置类：单字符按字面处理（如 类: "-"）
        _ => {
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Ok(Class::One(c)),
                _ => Err(format!("sml: 未知字符类 `{s}`")),
            }
        }
    }
}

fn parse_times(v: &Value, table: &dyn KeywordTable) -> Result<(usize, Option<usize>), String> {
    let (min, max): (i64, Option<i64>) = match v {
        Value::Int(n) if *n >= 0 => (*n, Some(*n)),
        Value::Str(s) => match s.as_str() {
            "+" => (1, None),
            "*" => (0, None),
            "?" => (0, Some(1)),
            other => {
                if let Some((a, b)) = other.split_once('-') {
                    let a = a
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| format!("sml: 量词 `{other}` 非法"))?;
                    let b = b
                        .trim()
                        .parse::<i64>()
                        .map_err(|_| format!("sml: 量词 `{other}` 非法"))?;
                    (a, Some(b))
                } else {
                    return Err(format!(
                        "sml: 未知量词 `{other}`（可用 数字 / + / * / ? / a-b）"
                    ));
                }
            }
        },
        Value::Object(m) => {
            // 量词对象形式：{ 最小: 2, 最大: 5 } —— 键名同样走关键字表
            let mut min = 0i64;
            let mut max: Option<i64> = None;
            for (k, v) in m.iter() {
                match table.lookup(k) {
                    Some(Concept::Min) => {
                        min = as_int(v).ok_or_else(|| format!("sml: 量词最小必须为整数，得 {v:?}"))?
                    }
                    Some(Concept::Max) => {
                        max = Some(
                            as_int(v)
                                .ok_or_else(|| format!("sml: 量词最大必须为整数，得 {v:?}"))?,
                        )
                    }
                    _ => {}
                }
            }
            (min, max)
        }
        other => return Err(format!("sml: 量词类型不支持: {}", type_name(other))),
    };
    // 统一校验：下界非负、上界不小于下界（非法输入一律报错，绝不静默）
    if min < 0 {
        return Err(format!("sml: 量词最小不能为负（得 {min}）"));
    }
    if let Some(mx) = max {
        if mx < min {
            return Err(format!("sml: 量词最大({mx})不能小于最小({min})"));
        }
    }
    Ok((min as usize, max.map(|x| x as usize)))
}

// ---------------------------------------------------------------------------
// 公开匹配 API
// ---------------------------------------------------------------------------

/// 用已编译的模式匹配整段文本（全文匹配，非搜索）。
pub fn is_match(pat: &Pat, text: &str) -> Result<bool, String> {
    let prog = nfa::compile(pat);
    nfa::run(&prog, text, DEFAULT_BUDGET)
}

/// 便捷入口：直接用「规则表 + 规则名」匹配文本（默认双语关键字）。
pub fn matches(rules: &BTreeMap<String, Value>, name: &str, text: &str) -> Result<bool, String> {
    matches_with(rules, name, text, &BILINGUAL)
}

/// 指定关键字表的匹配入口。
pub fn matches_with(
    rules: &BTreeMap<String, Value>,
    name: &str,
    text: &str,
    table: &dyn KeywordTable,
) -> Result<bool, String> {
    let rule = rules
        .get(name)
        .ok_or_else(|| format!("sml: 未定义的规则 `{name}`"))?;
    let pat = compile_rule_with(rule, rules, table)?;
    is_match(&pat, text)
}

// ---------------------------------------------------------------------------
// 取值辅助（Value API 的最小适配）
// ---------------------------------------------------------------------------

fn as_str(v: &Value, field: &str) -> Result<String, String> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        other => Err(format!(
            "sml: `{field}` 须为字符串，实际 {}",
            type_name(other)
        )),
    }
}

fn as_array<'a>(v: &'a Value, field: &str) -> Result<&'a Vec<Value>, String> {
    match v {
        Value::Array(a) => Ok(a),
        other => Err(format!("sml: `{field}` 须为数组，实际 {}", type_name(other))),
    }
}

fn as_int(v: &Value) -> Option<i64> {
    match v {
        Value::Int(n) => Some(*n),
        _ => None,
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Str(_) => "字符串",
        Value::Int(_) => "整数",
        Value::Float(..) => "浮点数",
        Value::Bool(_) => "布尔",
        Value::Null => "null",
        Value::Array(_) => "数组",
        Value::Object(_) => "对象",
    }
}
