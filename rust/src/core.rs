use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use crate::value::Value;
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

/// 拼接字段路径：`("api", "port")` -> `"api.port"`。
///
/// 用于让错误信息定位到**嵌套字段**而非只报最内层字段名：深层配置里
/// 多个契约都可能有 `port`，只报 `字段 port` 无法判断出错位置。
/// 父路径为空时直接返回子名（顶层字段没有前缀）。
fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_string()
    } else {
        format!("{}.{}", parent, child)
    }
}

/// 校验值是否符合类型规格。
/// `contracts` 供 `ContractRef`（组合）递归查找被引用契约。
///
/// `path` 是该字段的**完整路径**（如 `api.port`、`items[2].name`），
/// 由调用方在递归时用 [`join_path`] 拼接后传入，仅用于错误信息定位。
fn check_type(
    contract: &str,
    path: &str,
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
                        path, ref_name, contract
                    )
                })?;
                apply_contract(target, &mut sub, contracts, path)?;
                Ok(())
            }
            _ => Err(format!(
                "sml: 字段 `{}` 应为块并按契约 `{}` 校验，实际为 {}（契约 `{}`）",
                path,
                ref_name,
                value_kind(v),
                contract
            )),
        };
    }

    // 数组：逐元素校验，错误**直接向上传播**（路径带下标，如 `tags[1]`）。
    //
    // 不能写成 `.all(|it| check_type(...).is_ok())` 再回落到下面的通用错误：
    // 那样会丢弃下标，只报 `tags 类型应为 [str]` 而不指出是第几个元素出错。
    if let (TypeSpec::Array(inner), Value::Array(items)) = (&spec.ty, v) {
        let elem = FieldSpec {
            ty: (**inner).clone(),
            required: true,
            default: None,
            min: None,
            max: None,
        };
        for (i, it) in items.iter().enumerate() {
            check_type(contract, &format!("{}[{}]", path, i), &elem, it, contracts)?;
        }
        // 元素全部通过。数组本身不参与 min/max 区间校验（那是数值语义），直接返回。
        return Ok(());
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
        _ => false,
    };
    if !ok {
        return Err(format!(
            "sml: 字段 `{}` 类型应为 {}，实际为 {}（契约 `{}`）",
            path,
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
            // 显式拒绝非有限值（NaN/inf）：NaN 的所有比较都为 false，会令 min/max
            // 校验被静默穿透；inf 同理不是合法数值（审计 #2）。
            if !n.is_finite() {
                return Err(format!(
                    "sml: 字段 `{}` 的值为非有限数（NaN/inf），不可作为数值约束的取值（契约 `{}`）",
                    path, contract
                ));
            }
            if let Some(lo) = spec.min {
                if n < lo {
                    return Err(format!(
                        "sml: 字段 `{}` 值 {} 小于下界 {}（契约 `{}`）",
                        path, n, lo, contract
                    ));
                }
            }
            if let Some(hi) = spec.max {
                if n > hi {
                    return Err(format!(
                        "sml: 字段 `{}` 值 {} 大于上界 {}（契约 `{}`）",
                        path, n, hi, contract
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
    path: &str,
) -> Result<(), String> {
    // 1) 严格性：未声明字段一律拒绝（组合字段本身已在 fields 声明，其
    //    内部字段由被引用契约在自己的 apply_contract 中负责校验）
    if !c.allow_extra {
        for k in node.keys() {
            if !c.fields.contains_key(k) {
                return Err(format!(
                    "sml: 字段 `{}` 未在契约 `{}` 中声明（严格模式；如需允许额外字段请在契约名后写 `loose`）",
                    join_path(path, k), c.name
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
                        join_path(path, k), c.name
                    ));
                }
            }
            Some(v) => {
                let field_path = join_path(path, k);
                // 组合会回填子块默认值，故需要可变副本
                if matches!(spec.ty, TypeSpec::ContractRef(_)) {
                    // 先按**原值**校验必须是块，否则会退化成
                    // 「子字段缺失」这类误导性错误
                    check_type(&c.name, &field_path, spec, v, contracts)?;
                    let mut sub = match v {
                        Value::Object(m) => m.clone(),
                        _ => unreachable!("check_type 已保证为块"),
                    };
                    check_type_contract_ref(&c.name, &field_path, spec, &mut sub, contracts)?;
                    node.insert(k.clone(), Value::Object(sub));
                } else {
                    check_type(&c.name, &field_path, spec, v, contracts)?;
                }
            }
        }
    }
    Ok(())
}

/// 对「组合字段」递归应用被引用契约（会回填子块默认值）
fn check_type_contract_ref(
    contract: &str,
    path: &str,
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
            path, ref_name, contract
        )
    })?;
    // 先做基础类型校验（值须为块），再递归应用
    check_type(contract, path, spec, &Value::Object(sub.clone()), contracts)?;
    // 把完整路径传下去，子块的错误才能定位到 `parent.child` 而非只报字段名
    apply_contract(target, sub, contracts, path)
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
    At,      // @（紧邻其后内容，如 `@name` / `@contract`）
    /// 孤立的 `@`：其后紧跟空白或行尾，没有片段名/指令名。
    ///
    /// 必须与 `At` 区分——若统一成 `At`，孤立 `@` 会把**其后紧跟的块**
    /// 当成片段体消费掉（`@` + `mailer { .. }` 与 `@mailer { .. }`
    /// 的 token 流完全相同），导致内容被静默丢弃且不报错。
    BareAt,
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
                    // `/*` 多行注释，直到 `*/`；EOF 未闭合则报错（与未闭合字符串一致）
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
                                None => return Err("sml: 未闭合的块注释 /* ... */（遇到文件结尾）".to_string()),
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
                            None => return Err("sml: 未闭合的块注释 _* ... *_（遇到文件结尾）".to_string()),
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
                                            } else {
                                                // B8：\uXXXX 定长读 4 字符，
                                                // 不足 4 位说明输入截断/非法，
                                                // 必须把已读的 hex 当作失败处理
                                                // 而非静默丢弃（否则会吃掉后续引号）。
                                                return Err(format!(
                                                    "sml: 字符串转义 \\u 缺少足够的十六进制数字（期望 4 位，得 {hex:?}）"
                                                ));
                                            }
                                        }
                                    }
                                    // B8：非法码点（如代理区 \uD800、空 hex、非 hex）
                                    // 必须报错，不能静默丢弃并吞掉闭合引号。
                                    if hex.is_empty() {
                                        return Err("sml: 字符串转义 \\u 后缺少十六进制数字".to_string());
                                    }
                                    let cp = u32::from_str_radix(&hex, 16).map_err(|_| {
                                        format!("sml: 字符串转义 \\u 含非十六进制数字：{hex:?}")
                                    })?;
                                    let ch = char::from_u32(cp).ok_or_else(|| {
                                        format!("sml: 字符串转义 \\u 得到非法 Unicode 码点：U+{cp:04X}")
                                    })?;
                                    s.push(ch);
                                }
                                Some(other) => {
                                    // 未知转义（非 n/t/r/0/"/\/u）：必须报错，而非静默丢弃
                                    // 反斜杠（P1-5：\U \d \z 等会让路径/正则静默损坏）。
                                    // 与 \u 系列一致的严格策略：非法转义即失败。
                                    return Err(format!(
                                        "sml: 字符串含未知转义符 \\{}（仅支持 \\n \\t \\r \\0 \\\" \\\\ \\uXXXX）",
                                        other
                                    ));
                                }
                                // B9：转义符后遇 EOF，未闭合的反斜杠报错
                                None => {
                                    return Err(
                                        "sml: 字符串中的转义符 \\ 后遇到文件结束".to_string()
                                    )
                                }
                            }
                        }
                        Some(other) => s.push(other),
                        // B9：未闭合字符串（EOF 前没有闭合引号）必须报错，
                        // 否则后续整行/整个文件会被静默吞并。
                        None => return Err("sml: 字符串未闭合（缺少结束引号 \"）".to_string()),
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
                    // 仅当 `@` 与后随内容紧邻（无空白）时才是片段/指令标记。
                    // 孤立 `@`（后接空白或行尾）单独记为 BareAt，交由解析器报错，
                    // 避免其后的块被误认作片段体而静默丢弃。
                    let adjacent = match chars.peek() {
                        None => false,
                        Some(c) => !c.is_whitespace(),
                    };
                    toks.push(if adjacent { Tok::At } else { Tok::BareAt });
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

/// 查环境变量：优先查本次解析的**覆盖表**（`env_overrides`），未命中才回落
/// 到真实进程环境。覆盖表仅作用于本次解析，不修改进程环境（避免线程不安全的
/// `std::env::set_var` 与跨解析污染）。
fn lookup_env(overrides: Option<&BTreeMap<String, String>>, name: &str) -> String {
    if let Some(m) = overrides {
        if let Some(v) = m.get(name) {
            return v.clone();
        }
    }
    std::env::var(name).unwrap_or_default()
}

/// 把裸词 `w` 转为 Value。
///
/// 受 `features` 控制：关闭 `BarewordStr` 后纯字符串裸词（如 `John`）被拒绝，
/// 必须写作 `"John"`；仍允许的非字符串裸词：bool / null / 数字 /
/// 片段引用 `&x`（需 `fragment`）/ 环境变量 `$env.X`（需 `env`）。
///
/// `env` 为本次解析的环境变量覆盖表（见 [`lookup_env`]）。
fn coerce_word(
    w: &str,
    fragments: &BTreeMap<String, Value>,
    features: FeatureSet,
    ns_prefix: &str,
    env: Option<&BTreeMap<String, String>>,
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
        return Ok(Value::Str(lookup_env(env, ev)));
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
        // 片段特性已开启但名字未定义：必须报错，不能静默降级为字符串
        // （否则拼错的片段名会得到 Str("&name")，下游 .get 取到 None，难以排查）。
        return Err(format!("sml: 未定义的片段引用 `{}`", w));
    }
    // 数字: int / float / 科学计数
    if let Ok(i) = w.parse::<i64>() {
        // 保留前导零语义：以 `0` 开头且非 0x/0b 的纯数字（如 0755、007）必须保留为
        // 字符串，否则权限位/编号会被静默改写（P1-4：mode: 0755 不应变成 755）。
        // 0xFF / 0b101 等显式进制前缀不在其列，按原逻辑（非 i64 十进制）走下方字符串。
        if w.starts_with('0') && w.len() > 1 && !w.starts_with("0x") && !w.starts_with("0b") {
            if w.chars().all(|c| c.is_ascii_digit()) {
                return Ok(Value::Str(w.to_string()));
            }
        }
        return Ok(Value::Int(i));
    }
    // B10：整数超 i64 范围时，不能静默降级为 Float（会丢精度，
    // 如 9223372036854775808 这类 uint64 上界 ID / 纳秒时间戳）。
    // 若为纯整数形态则保留为字符串（round-trip 安全、零精度损失）；
    // 带小数点/科学计数符的才走 f64。
    let looks_int = !w.contains(['.', 'e', 'E']) && w.chars().all(|c| c.is_ascii_digit() || c == '+');
    if looks_int {
        if let Ok(u) = w.parse::<u64>() {
            // 超过 i64 但属合法 uint64：保持整值语义，序列化为字符串不丢精度
            if u > i64::MAX as u64 {
                return Ok(Value::Str(w.to_string()));
            }
        }
        return Ok(Value::Str(w.to_string()));
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
    /// 当前值嵌套深度（块 / 数组的递归层数）。
    /// 由 `parse_block` / `parse_array` 的 wrapper 维护，用于防止栈溢出。
    depth: usize,
    /// 命名空间栈：每个块（含 include `as ns` 产生的块）的名字依次入栈。
    /// 宏/契约注册与引用时，按栈路径加前缀（如 `ui.form.Button`），
    /// 使命名空间真正隔离宏，而非仅隔离数据键值。
    ns_stack: Vec<String>,
    /// 本次解析的环境变量覆盖表。`$env.X` 先在此查表，未命中才读进程环境。
    /// 用于 FFI 等「不修改进程环境」的注入场景（见 [`Self::env_var`]）。
    env: BTreeMap<String, String>,
}

impl Parser {
    /// 查 `$env.X`：先查本次解析的覆盖表，再回落进程环境。
    fn env_var(&self, name: &str) -> String {
        lookup_env(Some(&self.env), name)
    }

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
    /// 前看 n 个 token（n=0 等价于 [`Self::peek`]）。
    /// 用于区分「关键字参数 `type:` `name:`」与「恰巧同名的片段名」。
    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.i + n)
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
                Some(Tok::RBrace) => {
                    self.next();
                    break;
                }
                None => {
                    // 契约体未闭合（缺 `}` 即遇到文件结尾）：必须报错，否则后续顶层
                    // key 会被错位吞进契约体，最终主文档静默清空（P0-1）。
                    return Err("sml: 契约体未闭合（缺少结束符号 }）".to_string());
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
                            Some(Tok::Word(w2)) => coerce_word(
                                &w2,
                                &self.fragments,
                                self.features,
                                &self.ns_prefix(),
                                Some(&self.env),
                            )?,
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
                let f = w
                    .parse::<f64>()
                    .map_err(|_| format!("sml: 期望数字, 得 `{}`", w))?;
                // 拒绝 NaN/inf 作为边界：Rust 的 "nan".parse::<f64>() == Ok(NaN)，
                // 而 NaN 的所有比较均为 false，会让 min/max 校验被静默绕过
                // （审计 #2）。
                if !f.is_finite() {
                    return Err(format!("sml: 数字边界必须为有限值, 得 `{}`", w));
                }
                Ok(f)
            }
            other => Err(format!("sml: 期望数字, 得 {:?}", other)),
        }
    }

    /// 解析对象/块, 直到遇到 closing (None=顶层)。
    /// 外层 wrapper：深度守卫，防止 `a{a{a{ ... }}}` 无限递归导致栈溢出。
    /// 实际实现见 [`Parser::parse_block_inner`]。
    fn parse_block(&mut self, closing: Option<Tok>) -> Result<Value, String> {
        if self.depth >= MAX_VALUE_DEPTH {
            return Err(format!(
                "sml: 嵌套过深（超过 {} 层），疑似递归或恶意输入",
                MAX_VALUE_DEPTH
            ));
        }
        self.depth += 1;
        let r = self.parse_block_inner(closing);
        self.depth -= 1;
        r
    }

    fn parse_block_inner(&mut self, closing: Option<Tok>) -> Result<Value, String> {
        let mut node: BTreeMap<String, Value> = BTreeMap::new();
        // 块内若声明了 `@is Name`，在块解析完成后应用契约
        let mut applied_contract: Option<String> = None;
        loop {
            let tok = match self.peek().cloned() {
                None => {
                    // 文件结尾：若本块期望闭合符号（嵌套块/数组），则未闭合，必须报错；
                    // 否则为顶层正常结束。
                    if closing.is_some() {
                        let want = match closing {
                            Some(Tok::RBrace) => "}",
                            Some(Tok::RBrack) => "]",
                            _ => unreachable!(),
                        };
                        return Err(format!(
                            "sml: 未闭合的块/数组（遇到文件结尾，缺少结束符号 {}）",
                            want
                        ));
                    }
                    break;
                }
                Some(t) => t,
            };
            match tok {
                Tok::RBrace | Tok::RBrack => {
                    if let Some(cl) = &closing {
                        if *cl == tok {
                            self.next();
                            break;
                        }
                        // 嵌套块内遇到类型不匹配的右符号（如 `}` 与 `]` 混用）：
                        // 必须报错，而非静默吞掉，否则后续内容会被错误吞并。
                        let want = match cl {
                            Tok::RBrace => "}",
                            Tok::RBrack => "]",
                            _ => unreachable!(),
                        };
                        let got = match tok {
                            Tok::RBrace => "}",
                            Tok::RBrack => "]",
                            _ => unreachable!(),
                        };
                        return Err(format!(
                            "sml: 块/数组未正确闭合：期望 {}，却遇到 {}",
                            want, got
                        ));
                    }
                    // 顶层遇到多余的右括号 `}` / `]`：必须报错（之前静默忽略，
                    // 会掩盖作者漏写的 `key:`、错配括号等问题）。
                    let got = match tok {
                        Tok::RBrace => "}",
                        Tok::RBrack => "]",
                        _ => unreachable!(),
                    };
                    return Err(format!("sml: 多余的结束符号 {}", got));
                }
                Tok::Comma => {
                    // 逗号在 SML 中只用于对象字段分隔（`k: v, k2: v2`）与数组元素分隔
                    // （由 `parse_array_inner` 处理，不会到此处）。
                    // 此处遇到的逗号若属于「对象字段分隔」，其后应是 `key:` 形式；
                    // 否则是裸词拆分（如 `a: x,y`）或非法孤立逗号，必须报错而非静默吞掉
                    // （P2-7：裸词逗号会凭空拆出第二个键）。
                    let looks_like_field_sep = matches!(
                        self.toks.get(self.i + 1),
                        Some(Tok::Word(_)) | Some(Tok::Str(_))
                    ) && matches!(self.toks.get(self.i + 2), Some(Tok::Colon));
                    if looks_like_field_sep {
                        self.next();
                    } else {
                        return Err(
                            "sml: 非预期的逗号（裸词中不可含逗号，请用 [..] 数组或 \"...\" 引号）".into(),
                        );
                    }
                }
                // 孤立 `@`：其后没有片段名/指令名。必须报错——否则其后紧跟的
                // 块会被当作片段体消费，导致内容被静默丢弃（不报错）。
                Tok::BareAt => {
                    return Err(
                        "sml: 孤立的 `@` 不是合法指令；片段定义须写作 `@name { ... }`（`@` 与名字之间不可有空白），或删除该 `@`"
                            .into(),
                    );
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
                    // 可选显式参数：`type: X` 与 `name: Y`（顺序不限、均可省略）。
                    //
                    // v4 起废弃 v3 的位置参数形式（`@f Server [prod] { .. }`）。
                    // 原因：位置参数使 `@nosuch Word { .. }`（拼错的指令）与
                    // 「片段定义 + type 参数」在 token 流上完全同形，解析器无法判别，
                    // 只能把块当片段体吃掉 —— 内容被静默丢弃且不报错。
                    // 改为显式关键字后，任何位置参数形式都可安全地判为错误。
                    let mut ftype: Option<String> = None;
                    let mut farg: Option<String> = None;
                    loop {
                        // 仅当 `type`/`name` 后紧邻冒号时才视为参数，
                        // 这样名为 `type`/`name` 的片段（`@type { .. }`）仍可正常定义。
                        let is_param = matches!(self.peek(), Some(Tok::Word(w)) if w == "type" || w == "name")
                            && matches!(self.peek_at(1), Some(Tok::Colon));
                        if !is_param {
                            break;
                        }
                        let kw = match self.next() {
                            Some(Tok::Word(w)) => w,
                            _ => unreachable!("is_param 已保证为 Word"),
                        };
                        self.next(); // 冒号
                        let val = match self.next() {
                            Some(Tok::Word(s)) | Some(Tok::Str(s)) => s,
                            other => {
                                return Err(format!(
                                    "sml: 片段 `@{fname}` 的参数 `{kw}:` 后须值, 得 {:?}",
                                    other
                                ))
                            }
                        };
                        if kw == "type" {
                            if ftype.is_some() {
                                return Err(format!(
                                    "sml: 片段 `@{fname}` 的 `type:` 参数重复"
                                ));
                            }
                            ftype = Some(val);
                        } else {
                            if farg.is_some() {
                                return Err(format!(
                                    "sml: 片段 `@{fname}` 的 `name:` 参数重复"
                                ));
                            }
                            farg = Some(val);
                        }
                    }
                    // 既非 `{` 也非流末尾：既可能是拼错的指令，也可能是旧的位置参数形式。
                    // 两种意图无法区分，故错误信息同时给出两条排查指引。
                    if !matches!(self.peek(), Some(Tok::LBrace) | None) {
                        return Err(format!(
                            "sml: `@{fname}` 不是合法指令且缺少片段体 {{ ... }}；\
                             若本意是「片段定义」，其参数须显式写作 `type: X` 与 `name: Y`\
                             （如 `@{fname} type: Server name: prod {{ .. }}`），\
                             位置参数形式（`@{fname} X [Y] {{ .. }}`）自 v4 起已废弃，\
                             不带参数时写作 `@{fname} {{ .. }}`；\
                             若本意是「指令」，请检查拼写（合法指令：contract / is / version / feature）"
                        ));
                    }
                    if self.peek() == Some(&Tok::LBrace) {
                        self.next();
                        let blk = self.parse_block(Some(Tok::RBrace))?;
                        let mut sub = match &blk {
                            Value::Object(m) => m.clone(),
                            other => {
                                let mut m = BTreeMap::new();
                                m.insert("_value".into(), other.clone());
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
                    } else {
                        // `@name` 既不是已知指令（contract/is）也不是片段定义（后无 `{`）：
                        // 必须报错，而非静默吞掉后续行/块（P0-2：单独 `@` 会清空整个文档）。
                        return Err(format!(
                            "sml: `@{}` 不是合法指令且缺少片段体 {{ ... }}，无法解析",
                            fname
                        ));
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
            // 用命名空间栈拼出当前块路径（如 `api`、`ns.api`），作为错误定位前缀
            apply_contract(&c, &mut node, &self.contracts, &self.ns_prefix())?;
        }
        Ok(Value::Object(node))
    }

    /// 解析一个值 (在 key 之后)
    /// 预扫描：当前位置是否形如裸块的**参数部分**（`[name...] {`），
    /// 即连续若干 `Word`/`Str` 后紧跟 `{`。不消费任何 token。
    fn bare_block_ahead(&self) -> bool {
        if !matches!(self.peek(), Some(Tok::Word(_)) | Some(Tok::Str(_))) {
            return false;
        }
        let mut probe = self.i;
        while probe < self.toks.len() {
            match &self.toks[probe] {
                Tok::Word(_) | Tok::Str(_) => probe += 1,
                Tok::LBrace => return true,
                _ => return false,
            }
        }
        false
    }

    /// 解析裸块体：调用前类型名本身已消费，当前位置处于参数处；
    /// `key` 即写入块内的 `__type`。返回解析出的块。
    fn parse_bare_block(&mut self, key: &str) -> Result<Value, String> {
        let mut args: Vec<Value> = Vec::new();
        while let Some(t) = self.peek().cloned() {
            match t {
                Tok::Word(w) => {
                    args.push(coerce_word(
                        &w,
                        &self.fragments,
                        self.features,
                        &self.ns_prefix(),
                        Some(&self.env),
                    )?);
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
        if self.peek() != Some(&Tok::LBrace) {
            return Err("sml: 语法错误".into());
        }
        self.next();
        // 进入子块 = 进入该 block 名字的命名空间
        self.ns_stack.push(key.to_string());
        let mut sub = self.parse_block(Some(Tok::RBrace))?;
        self.ns_stack.pop();
        if let Value::Object(m) = &mut sub {
            m.insert("__type".into(), Value::Str(key.to_string()));
            // 裸块参数全部保留：首个作 __name，其余放入 __args 数组，
            // 不再静默丢弃（P1-3：`server web prod {}` 的 web/prod 都应可见）。
            if !args.is_empty() {
                m.insert("__name".into(), args[0].clone());
                if args.len() > 1 {
                    m.insert("__args".into(), Value::Array(args[1..].to_vec()));
                }
            }
        }
        Ok(sub)
    }

    fn parse_value(&mut self, key: &str, colon: bool) -> Result<Value, String> {
        // 无冒号且后继是裸词: 可能是裸块 `type [name] { }`
        if !colon && self.bare_block_ahead() {
            return self.parse_bare_block(key);
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
                    Tok::Word(w) => coerce_word(
                        &w,
                        &self.fragments,
                        self.features,
                        &self.ns_prefix(),
                        Some(&self.env),
                    )?,
                    Tok::Str(s) => {
                        // 引号串同样承认 `$env.X` 内联（v3 严格模式下裸词不可用时
                        // 这是唯一写法），但必须**与裸词路径一致**地受 `Feature::Env`
                        // 约束：否则调用方禁用 env 特性后，文档仍可用引号串绕过限制
                        // 读取任意环境变量。
                        match s.strip_prefix("$env.") {
                            Some(name) => {
                                if !self.features.has(Feature::Env) {
                                    return Err(format!(
                                        "sml: 当前特性集禁用了 `$env`（env），字符串 `\"{}\"` 无法解析",
                                        s
                                    ));
                                }
                                Value::Str(self.env_var(name))
                            }
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
                    Ok(coerce_word(
                        key,
                        &self.fragments,
                        self.features,
                        &self.ns_prefix(),
                        Some(&self.env),
                    )?)
                }
            }
            _ => Err("sml: 语法错误".into()),
        }
    }

    /// 外层 wrapper：深度守卫，防止深度嵌套数组触发递归下降的栈溢出。
    /// 实际实现见 [`Parser::parse_array_inner`]。
    fn parse_array(&mut self) -> Result<Value, String> {
        if self.depth >= MAX_VALUE_DEPTH {
            return Err(format!(
                "sml: 嵌套过深（超过 {} 层），疑似递归或恶意输入",
                MAX_VALUE_DEPTH
            ));
        }
        self.depth += 1;
        let r = self.parse_array_inner();
        self.depth -= 1;
        r
    }

    fn parse_array_inner(&mut self) -> Result<Value, String> {
        let mut arr = Vec::new();
        loop {
            match self.peek().cloned() {
                None => {
                    // 顶层数组未闭合（缺少 `]`）：必须报错，而非按 EOF 静默收尾。
                    return Err("sml: 未闭合的数组（遇到文件结尾，缺少结束符号 ]）".to_string());
                }
                Some(Tok::RBrack) => {
                    self.next();
                    break;
                }
                Some(Tok::RBrace) => {
                    // 顶层数组遇到多余的 `}`：必须报错，而非静默忽略。
                    return Err("sml: 多余的结束符号 }（数组应以 ] 闭合）".to_string());
                }
                Some(Tok::Comma) => {
                    self.next();
                }
                Some(Tok::LBrace) => {
                    self.next();
                    arr.push(self.parse_block(Some(Tok::RBrace))?);
                }
                Some(Tok::LBrack) => {
                    self.next();
                    // 走 wrapper 以复用 depth 守卫（否则嵌套数组会绕过 128 层限制导致栈溢出 DoS）
                    arr.push(self.parse_array()?);
                }
                Some(Tok::Word(w)) => {
                    // 数组内的裸块 `Type [name] { ... }`：表达**有序**元素序列。
                    // 对象字段用 BTreeMap 存储（键有序、不可保书写序），因此
                    // 需要保序的子元素（UI 布局、文档章节）需写作数组。
                    if self.bare_block_ahead() {
                        self.next(); // 消费类型名
                        arr.push(self.parse_bare_block(&w)?);
                    } else {
                        arr.push(coerce_word(
                            &w,
                            &self.fragments,
                            self.features,
                            &self.ns_prefix(),
                            Some(&self.env),
                        )?);
                        self.next();
                    }
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
    /// v4：片段定义的 `type` / `name` 参数改为**显式**关键字形式：
    ///     `@f type: Server name: prod { .. }`；
    ///     废弃 v3 的位置参数形式（`@f Server prod { .. }`）。
    ///
    /// 动机：位置参数使「拼错的指令 `@nosuch Word { .. }`」与
    /// 「片段定义 + type 参数」在 token 流上完全同形，无法判别，
    /// 导致块被当作片段体消费、内容静默丢失且不报错。
    /// 语法与 v3 其余部分完全兼容（字符串引号、标量裸词等规则不变）。
    V4,
}

impl Version {
    /// 当前实现支持的最新版本
    pub const CURRENT: Version = Version::V4;

    /// 是否要求字符串显式引号（v2 / v3 为严格模式）
    pub fn strict_strings(self) -> bool {
        self >= Version::V2
    }

    /// 解析版本字面量（`v1`/`1`、`v2`/`2`、`v3`/`3`、`v4`/`4`）
    pub(crate) fn from_word(w: &str) -> Option<Version> {
        match w {
            "v1" | "1" => Some(Version::V1),
            "v2" | "2" => Some(Version::V2),
            "v3" | "3" => Some(Version::V3),
            "v4" | "4" => Some(Version::V4),
            _ => None,
        }
    }

    /// 版本名（用于错误信息与序列化回显）
    pub fn name(self) -> &'static str {
        match self {
            Version::V1 => "v1",
            Version::V2 => "v2",
            Version::V3 => "v3",
            Version::V4 => "v4",
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
                        "@feature base 需要 v1/v2/v3/v4，收到 `{}`",
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
    let spans = compute_string_spans(text);
    let mut feats = FeatureSet::all();
    let mut mode = FeatureMode::Default;
    let mut base: Option<Version> = None;
    let mut had_feature = false;

    let mut line_start = 0usize;
    for line in text.lines() {
        // 多行字符串内的行（如 `note: "..."` 跨行）里的 `@feature` 不是指令，跳过以免破坏数据
        if !line_starts_in_string(text, line_start, &spans) {
            match apply_feature_directive(line, &mut feats, &mut mode, &mut base) {
                Ok(true) => {
                    had_feature = true;
                    line_start = advance_line(line_start, line, text);
                    continue; // 指令行被消费，不进入剩余文本
                }
                Ok(false) => {}
                Err(e) => return Err(e), // 指令非法（如未知特性名）必须上浮，不能静默吞掉
            }
        }
        out.push_str(line);
        out.push('\n');
        line_start = advance_line(line_start, line, text);
    }
    Ok((out, feats, base, had_feature))
}

/// 计算文本中所有字符串字面量的字节区间（含单引号/双引号、单行与多行三引号）。
/// 用于让「按行剥离指令」在字符串内部时跳过，避免破坏多行字符串数据
/// （如 `note: "line1\n@version v1\nline2"` 中的 @version 被误当指令）。
fn compute_string_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'"' || c == b'\'' {
            let quote = c;
            // 三引号（多行）？
            let triple = i + 2 < bytes.len() && bytes[i + 1] == quote && bytes[i + 2] == quote;
            let end = if triple {
                // 找到下一个 """" / ''' 作为结束
                let mut j = i + 3;
                let mut found = None;
                while j + 2 < bytes.len() {
                    if bytes[j] == quote && bytes[j + 1] == quote && bytes[j + 2] == quote {
                        found = Some(j + 3);
                        break;
                    }
                    j += 1;
                }
                found.unwrap_or(bytes.len())
            } else {
                // 单行：遇到未转义的同类引号结束
                let mut j = i + 1;
                let mut found = None;
                while j < bytes.len() {
                    if bytes[j] == b'\\' {
                        j += 2;
                        continue;
                    }
                    if bytes[j] == quote {
                        found = Some(j + 1);
                        break;
                    }
                    j += 1;
                }
                found.unwrap_or(bytes.len())
            };
            spans.push((i, end));
            i = end;
        } else {
            i += 1;
        }
    }
    spans
}

/// 判断 `line` 起始字节 `start` 是否落在任一字符串区间内（字符串内的行不算指令）。
fn line_starts_in_string(_text: &str, start: usize, spans: &[(usize, usize)]) -> bool {
    // 指令通常位于行首（可有缩进）。检测起始位置是否在字符串内即可。
    spans.iter().any(|(s, e)| start >= *s && start < *e)
}

/// 推进到下一行的起始字节偏移（处理 \n；CRLF 也兼容）。
fn advance_line(mut start: usize, line: &str, text: &str) -> usize {
    start += line.len();
    if text[start..].starts_with('\n') {
        start += 1;
        if text[start..].starts_with('\r') {
            start += 1;
        }
    } else if text[start..].starts_with('\r') {
        start += 1;
        if text[start..].starts_with('\n') {
            start += 1;
        }
    }
    start
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
pub(crate) fn strip_version(text: &str) -> Result<(String, Option<Version>), String> {
    let spans = compute_string_spans(text);
    let mut declared: Option<Version> = None;
    let mut rest = String::new();
    let mut line_start = 0usize;
    for line in text.lines() {
        // 多行字符串内的行（如 `note: "..."` 跨行）里的 @version 不是指令，原样保留
        let is_directive = if line_starts_in_string(text, line_start, &spans) {
            false
        } else {
            matches!(version_directive(line)?, Some(_))
        };
        if is_directive {
            let lit = version_directive(line)?.unwrap();
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
            line_start = advance_line(line_start, line, text);
            continue;
        }
        rest.push_str(line);
        rest.push('\n');
        line_start = advance_line(line_start, line, text);
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
    Ok((parse_impl(&rest, v, feats, BTreeMap::new())?, v))
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
    let val = parse_impl_tokens(toks, v, feats, BTreeMap::new())?;
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
    parse_impl(&rest, v, feats, BTreeMap::new())
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
    parse_impl(&rest, v, feats, BTreeMap::new())
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
    let val = parse_impl(&rest, v, effective, BTreeMap::new())?;
    Ok((val, effective))
}

/// 与 [`parse_with_features`] 相同，但额外提供**环境变量覆盖表**。
///
/// `$env.X` 会先在此表中查找，未命中才回落到进程环境。表内的值**只作用于
/// 本次解析**，不会写入进程环境——因此该函数是线程安全的，也不会影响
/// `PATH` / `LD_PRELOAD` 等影响进程行为的变量。
///
/// 供 FFI（`sml_parse_ex` 的 `env` 选项）等需要注入变量、又不便改进程环境的
/// 场景使用。
pub fn parse_with_features_env(
    text: &str,
    allowed: FeatureSet,
    env: BTreeMap<String, String>,
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
    let val = parse_impl(&rest, v, effective, env)?;
    Ok((val, effective))
}

/// 不含版本处理的底层解析（文本入口）
fn parse_impl(
    text: &str,
    version: Version,
    features: FeatureSet,
    env: BTreeMap<String, String>,
) -> Result<Value, String> {
    let toks = tokenize(text)?;
    parse_impl_tokens(toks, version, features, env)
}

/// 不含版本处理的底层解析（token 流入口，供 include 展开后零拷贝复用）
fn parse_impl_tokens(
    toks: Vec<Tok>,
    version: Version,
    features: FeatureSet,
    env: BTreeMap<String, String>,
) -> Result<Value, String> {
    let mut p = Parser {
        toks,
        i: 0,
        fragments: BTreeMap::new(),
        contracts: BTreeMap::new(),
        features,
        depth: 0,
        ns_stack: Vec::new(),
        env,
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

/// 值嵌套深度上限：防止 `a{a{a{ ... }}}` 这类深度嵌套触发递归下降的栈溢出。
///
/// 与 [`MAX_INCLUDE_DEPTH`] 互补 —— 后者只保护 include 的文件嵌套，不保护
/// 单个文档内部块/数组的嵌套。栈溢出在 Rust 中是 abort，
/// **无法被 catch_unwind 捕获**，因此必须在递归入口主动限深，
/// 而不是依赖上层错误处理。
///
/// 128 层与 serde_json 的 RECURSION_LIMIT 对齐，远超任何真实配置所需。
const MAX_VALUE_DEPTH: usize = 128;

/// 嵌套深度上限：既防栈溢出，也让异常深层的引用尽早失败
const MAX_INCLUDE_DEPTH: usize = 32;

/// 单次 include 展开的总文件读取次数上限（防指数膨胀 DoS）。
/// 仅按「被实际展开的文件个数」计数（非深度），覆盖菱形重复包含导致的 2^N 爆炸。
const MAX_INCLUDE_EXPANSIONS: u64 = 10_000;

/// 剥离行尾注释，正确跳过引号内的 `#`（如 `key: "a#b"` 中的 # 不是注释起点）
pub(crate) fn strip_line_comment(line: &str) -> &str {
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
pub(crate) fn parse_include_line(line: &str, features: FeatureSet) -> Result<Option<Vec<IncludeTarget>>, String> {
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
        return Err(
            "sml: 当前特性集禁用了 include/import（include 特性未启用）".into(),
        );
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
                // 与其余三类（include/glob/regex）保持一致：特性未启用时返回 Err，
                // 而非静默 `Ok(None)` 把整行当普通内容解析导致数据污染
                // （此前会注入垃圾键且零报错）。
                return Err(
                    "sml: 多目标 include 需要特性 `multi-include`（请 @feature enable multi-include）"
                        .into(),
                );
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
pub struct MiniRegex {
    pattern: String,
}

/// 受限正则的模式长度上限：防止超长模式构造的灾难性回溯（ReDoS）。
const MAX_REGEX_LEN: usize = 256;
/// 单次匹配的最大回溯步数：无记忆化回溯在恶意模式（如 `a*a*a*...*b`）下会指数级
/// 爆炸，故设步数上限——超出即视为不匹配，避免挂死整个解析（审计 #3）。
const MAX_REGEX_STEPS: u64 = 2_000_000;

pub fn compile_regex(pat: &str) -> MiniRegex {
    // 去掉可能的首尾 `^`/`$` 锚（由 matcher 解释）后做长度上限检查
    let inner = pat
        .strip_prefix('^')
        .or_else(|| Some(pat))
        .map(|s| s.strip_suffix('$').unwrap_or(s))
        .unwrap_or(pat);
    if inner.chars().count() > MAX_REGEX_LEN {
        // 超出上限：用不可能匹配的模式占位（调用方会得到 false），不 panic
        return MiniRegex {
            pattern: "\u{0}".to_string(),
        };
    }
    MiniRegex {
        pattern: pat.to_string(),
    }
}

/// 用受限正则匹配整个 `text`（默认全匹配，支持 `^`/`$` 锚点）。
pub fn regex_matches(re: &MiniRegex, text: &str) -> bool {
    let pat = &re.pattern;
    let anchored_start = pat.starts_with('^');
    let anchored_end = pat.ends_with('$');
    let p = if anchored_start { &pat[1..] } else { pat };
    let p = if anchored_end { &p[..p.len().saturating_sub(1)] } else { p };
    // 步数预算必须**跨所有起始位置共享**：预算若是每次调用的局部变量，
    // 每个起点都能重新拿到 MAX_REGEX_STEPS 步，实际总开销 = 起点数 × 2M。
    // 而 glob/regex include 会对目录中每个文件名各调用一次本函数，
    // 一个恶意模式即可把整个解析挂死（安全审计 P3-1）。故在此统一持有并传递。
    let mut steps: u64 = 0;
    if anchored_start {
        backtrack_match(p, text, 0, &mut steps).is_some()
    } else {
        for start in 0..=text.len() {
            if steps > MAX_REGEX_STEPS {
                // 预算耗尽：整次匹配判定为「不匹配」，不再尝试剩余起点。
                break;
            }
            // 每个起点只算一次：既省一半开销，也避免「是否匹配到末端」
            // 被两次独立判定（原写法对同一起点调用两次，可能得出不同结论）。
            match backtrack_match(p, text, start, &mut steps) {
                Some(end) if !anchored_end || end == text.len() => return true,
                _ => {}
            }
        }
        false
    }
}

/// 回溯匹配：从 `text[ti]` 开始尝试匹配 `pat[pi]`，返回成功时 text 的消耗终点（usize）。
///
/// `steps` 由调用方持有并在**整个匹配过程**中共享累积，用于跨起点统一限流。
fn backtrack_match(pat: &str, text: &str, ti: usize, steps: &mut u64) -> Option<usize> {
    // 递归实现，模式索引 pi 通过 chars 迭代
    let pchars: Vec<char> = pat.chars().collect();
    let tchars: Vec<char> = text.chars().collect();
    fn go(pchars: &[char], tchars: &[char], pi: usize, ti: usize, steps: &mut u64) -> Option<usize> {
        // 步数预算：每次进入一个匹配决策都计一步，超出上限即中止（防 ReDoS）
        *steps += 1;
        if *steps > MAX_REGEX_STEPS {
            return None;
        }
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
                            if let Some(r) = go(pchars, tchars, pi + 1, e, steps) {
                                return Some(r);
                            }
                            if e == ti {
                                break;
                            }
                            e -= 1;
                        }
                    }
                    // 零次匹配：跳过 '*'
                    return go(pchars, tchars, pi + 1, ti, steps);
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
    go(&pchars, &tchars, 0, ti, steps)
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
    let mut expansions: u64 = 0;
    expand_includes(text, base, &mut stack, features, &mut toks, &mut expansions)?;
    Ok(toks)
}

/// 递归展开 include 到 `out` token 流。
/// `expansions` 为「全局已展开文件计数」，用于防御指数膨胀 DoS（同一文件被多处包含仍计数）。
fn expand_includes(
    text: &str,
    base: &Path,
    stack: &mut Vec<PathBuf>,
    features: FeatureSet,
    out: &mut Vec<Tok>,
    expansions: &mut u64,
) -> Result<(), String> {
    if stack.len() >= MAX_INCLUDE_DEPTH {
        return Err(format!("include 嵌套超过 {MAX_INCLUDE_DEPTH} 层"));
    }
    // 沙箱根：所有 include 命中的文件必须位于 base（规范化为绝对路径）之内，
    // 否则拒绝，防止 `../` 或 glob/regex 模式越界读取任意文件（路径遍历漏洞）。
    let base_canon = match base.canonicalize() {
        Ok(p) => p,
        Err(_) => base.to_path_buf(),
    };
    let spans = compute_string_spans(text);
    let mut line_start = 0usize;
    for line in text.lines() {
        // 多行字符串内部的行（如 `"...\ninclude \"x\"\n..."`）里的 include 不是指令，
        // 更不能被当作文件读取（防止字符串内伪造 include 触发任意文件读取）。
        let inside_string = line_starts_in_string(text, line_start, &spans);
        line_start = advance_line(line_start, line, text);
        if inside_string {
            // 当作普通行 tokenize（保持与字符串片段一致），不进入 include 解析分支
            let line_toks = tokenize(line).map_err(|e| {
                format!("include 预处理词法错误：{e}（于行：{line}）")
            })?;
            out.extend(line_toks);
            continue;
        }
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
                        // 路径遍历防护：规范化后必须仍位于沙箱根 base 之内
                        if !canon.starts_with(&base_canon) {
                            return Err(format!(
                                "include 越界拒绝：{} 不在基准目录 {} 内",
                                canon.display(),
                                base_canon.display()
                            ));
                        }
                        // stack 是「当前正在展开的文件链」，命中即成环
                        if stack.iter().any(|p| p == &canon) {
                            return Err(format!("include 循环引用: {}", canon.display()));
                        }
                        let content = std::fs::read_to_string(&canon)
                            .map_err(|e| format!("include 读取失败 {}: {e}", canon.display()))?;
                        *expansions += 1;
                        if *expansions > MAX_INCLUDE_EXPANSIONS {
                            return Err(format!(
                                "include 展开次数超过上限 {}（疑似指数膨胀 DoS）",
                                MAX_INCLUDE_EXPANSIONS
                            ));
                        }
                        let child_base = canon
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| PathBuf::from("."));
                        stack.push(canon.clone());
                        // 展开子文件 tokens（共享全局展开计数，防钻石型重复包含爆炸）
                        let mut inner = expand_file_tokens(
                            &content,
                            &child_base,
                            stack,
                            features,
                            expansions,
                        )?;
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
    expansions: &mut u64,
) -> Result<Vec<Tok>, String> {
    // 剥离子文件内的版本/特性指令行，避免污染 token 流。
    // 多行字符串内部的行不算指令，须跳过（否则会破坏字符串数据，如 "line\n@version\n..."）。
    let spans = compute_string_spans(content);
    let mut line_start = 0usize;
    let cleaned: String = content
        .lines()
        .filter(|l| {
            let inside = line_starts_in_string(content, line_start, &spans);
            line_start = advance_line(line_start, l, content);
            if inside {
                return true; // 字符串内：保留
            }
            let t = strip_line_comment(l).trim();
            let t = t.strip_prefix('@').unwrap_or(t).trim_start();
            !(t.starts_with("version") || t.starts_with("feature"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut toks = Vec::new();
    expand_includes(&cleaned, base, stack, features, &mut toks, expansions)?;
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
    parse_impl_tokens(toks, v, allowed, BTreeMap::new())
}

/// 同 [`parse_file`]，但额外接受一个「调用方允许特性集」`caller_allowed`，
/// 与文档 `@feature` 声明取交集。用于 FFI（`sml_load_file(flags)`）让调用方
/// 通过 `flags` 限制文件入口能力（如禁用 `include`/`env`）。
///
/// 行为：
/// - 最终允许集 = `FeatureSet::all() ∩ 文档声明特性 ∩ caller_allowed`。
/// - 若交集为空（调用方禁用了一切可用特性），视为「不允许任何能力」并报错，
///   避免静默以全开集回退读取文件（那会令 `flags` 形同虚设）。
pub fn parse_file_features(
    path: impl AsRef<Path>,
    caller_allowed: FeatureSet,
) -> Result<Value, String> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("读取失败 {}: {e}", path.display()))?;
    let base = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let (rest, declared) = strip_version(&text)?;
    let (rest, feats, base_ver, had) = strip_features(&rest)?;
    let v = declared.or(base_ver).unwrap_or(Version::V1);
    let feats = features_for(v, feats, had);
    let allowed = FeatureSet::all().intersection(feats).intersection(caller_allowed);
    if allowed.is_empty() {
        return Err("sml: 调用方 flags 与文档特性交集为空，不允许任何解析能力".into());
    }
    let toks = resolve_includes(&rest, &base, allowed)?;
    parse_impl_tokens(toks, v, allowed, BTreeMap::new())
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

/// 判断字符串作为裸词写出是否安全（可无损 round-trip 回来仍是 `Str`）。
///
/// 允许裸写的「安全标识符」需满足：
/// - 非空；
/// - 不等于保留字面量 `true` / `false` / `null` / `inf` / `nan`；
/// - 不是纯数字或浮点字面量（否则 `coerce_word` 会把它字面量化成 Int/Float）；
/// - 不以注释前缀开头：`--` `//` `/*` `*/` `*` `_*`；
/// - 不含任何会破坏语法的字符：空白、`:` `#` `{` `}` `,` `[` `]` `"` `\` `/` `*`；
/// - 首字符必须是字母或 `_`（避免 `-x` / `.x` / 数字开头等歧义）；
/// - 其余字符只能是 `[A-Za-z0-9_-]`（kebab-case 的 `-` 在中间是安全的，
///   只有行首的 `--` 才是注释）。
///
/// 任何不满足的情况都加引号，确保序列化结果解析回来仍是原字符串。
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
        Value::Float(f) => {
            // 强制保留小数点，避免 1.0 被 Display 写成 "1" 后 round-trip 成 Int
            if !f.is_finite() {
                // NaN/inf 无合法 SML 字面量；序列化为带引号字符串（见 dump_scalar）
                out.push_str(&format!("\"{}\"", f.to_string()));
            } else if f.fract() == 0.0 {
                out.push_str(&format!("{:.1}", f));
            } else {
                out.push_str(&format!("{}", f));
            }
        }
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
        // 强制保留小数点，避免 1.0 被 Display 写成 "1" 后 round-trip 成 Int
        Value::Float(f) => {
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
