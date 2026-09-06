// SPDX-License-Identifier: MulanPSL-2.0
//! SML 契约（Contract）—— 可选的 schema 层，只依赖值模型。
//!
//! SML 本身是纯数据格式（与 JSON/YAML 同层），值模型只有 7 种类型，
//! **不具备**结构体定义、枚举、字段约束等类型系统能力。
//! 契约是在此之上的**可选校验层**，用于给块加上结构与取值约束。

use std::collections::BTreeMap;

use sml_value::Value;

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
    /// 自定义模式类型：由 `@type name: X { 模式 }` 声明。
    /// 值须为**字符串**且匹配该模式（手机号 / 身份证号等格式约束）。
    /// 模式是无回溯的 L1 匹配器，故不存在灾难性回溯。
    Pattern {
        name: String,
        pat: sml_pattern::Pat,
    },
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
        TypeSpec::Pattern { name, .. } => name.clone(),
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

    // 自定义模式类型：值须为字符串并匹配模式。
    //
    // 非字符串必须**显式报错并提示加引号**：像 `证件号: 221099 1988 0987 1211`
    // 这种裸写会被词法切成数字，最终只剩末段 `1211` 而不报任何错——
    // 这正是「静默数据损坏」，是本项目明确要消灭的一类行为。
    if let TypeSpec::Pattern { name, pat } = &spec.ty {
        return match v {
            Value::Str(s) => match sml_pattern::is_match(pat, s) {
                Ok(true) => Ok(()),
                Ok(false) => Err(format!(
                    "sml: 字段 `{}` 的值 `{}` 不符合类型 `{}` 的格式要求（契约 `{}`）",
                    path, s, name, contract
                )),
                Err(e) => Err(format!("sml: 类型 `{}` 匹配时出错：{}", name, e)),
            },
            other => Err(format!(
                "sml: 字段 `{}` 类型 `{}` 要求字符串（号码 / 编号 / 身份证请用引号包裹），实际为 {}（契约 `{}`）",
                path, name, value_kind(other), contract
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
        (TypeSpec::Num, Value::Int(_)) | (TypeSpec::Num, Value::Float(_, _)) => true,
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
            Value::Float(f, _) => Some(*f),
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
        Value::Float(_, _) => "float",
        Value::Str(_) => "str",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// 对块应用契约：填充默认值 + 校验 + 严格性检查。
///
/// **严格为默认**：契约未声明的字段会被拒绝，除非契约显式标记 `loose`。
/// 这样拼错的字段名（如 `prot`）会立即报错，而不是被静默忽略。
pub fn apply_contract(
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
