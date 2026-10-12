//! 契约推断（任务 E，TASK-agent-w24 §7）。
//!
//! 从一组「数据实例」（对象）反推一份契约。**严格不猜**：
//!
//! - 不推断 `default` / `min` / `max` / `enum` / 自定义 `@type`；
//! - `optional` 仅在「多实例合并」时判定（某字段在部分实例缺失）；单实例一律
//!   `required`，并提示「单实例无法判定 optional」；
//! - 生成的契约默认 `loose`（允许额外字段），避免用户下次新增字段被严格模式拒绝；
//! - 输出确定性：字段按名排序（`BTreeMap`）、无时间戳、同输入逐字节相同（golden）。
//!
//! 类型映射（§7）：
//! `Int→int` / `Float→num` / `Bool→bool` / `Str→str` / `Null→any` /
//! `Object→子契约（组合，按字段名大驼峰命名）` / `Array→[T]`（元素合一）。

use std::collections::{BTreeMap, BTreeSet};

use sml_value::Value;

use crate::{Contract, FieldSpec, TypeSpec};

/// 推断结果：一组命名契约（含根与若干子契约）+ 告警。
pub struct InferResult {
    /// 根契约名（调用方传入，默认 `Inferred`）。
    pub root_name: String,
    /// 名称 → 契约。根契约名为 [`InferResult::root_name`]，其余为按字段名大驼峰派生的子契约。
    pub contracts: BTreeMap<String, Contract>,
    /// 推断过程中的告警（混合类型退化成 `any`、单实例提示等），输出时打到 stderr。
    pub warnings: Vec<String>,
}

/// 从一份解析后的 `Value` 推断契约。
///
/// - 根为对象数组：每个元素是一条实例；
/// - 根为单个对象：按单实例处理（字段全 `required` 并告警）；
/// - 其它根类型 / 空数组：返回错误。
pub fn infer_from_value(root_name: &str, value: &Value) -> Result<InferResult, String> {
    let instances: Vec<&BTreeMap<String, Value>> = match value {
        Value::Array(items) => {
            let mut v = Vec::with_capacity(items.len());
            for it in items {
                match it {
                    Value::Object(m) => v.push(m),
                    _ => {
                        return Err(
                            "infer-contract: 数组元素必须是对象（每条实例），不能出现标量或嵌套数组元素"
                                .to_string(),
                        )
                    }
                }
            }
            v
        }
        Value::Object(m) => vec![m],
        _ => {
            return Err(
                "infer-contract: 根必须是对象或对象数组（每条实例是一个对象）".to_string(),
            )
        }
    };
    if instances.is_empty() {
        return Err("infer-contract: 没有可用的实例（空数组或非对象根）".to_string());
    }
    let single = instances.len() == 1;
    let mut warnings = Vec::new();
    if single {
        warnings.push("单实例无法判定 optional：所有字段按 required 推断".to_string());
    }
    let mut contracts: BTreeMap<String, Contract> = BTreeMap::new();
    build_object_contract(root_name, &instances, &mut contracts, &mut warnings, single);
    Ok(InferResult {
        root_name: root_name.to_string(),
        contracts,
        warnings,
    })
}

/// 由一个字段名推导子契约名（大驼峰）：`address` → `Address`，`server_name` → `ServerName`。
fn sub_name(field: &str) -> String {
    let mut out = String::new();
    for seg in field.split('_') {
        let mut chars = seg.chars();
        if let Some(first) = chars.next() {
            for u in first.to_uppercase() {
                out.push(u);
            }
            for c in chars {
                out.push(c);
            }
        }
    }
    if out.is_empty() {
        out.push_str("Sub");
    }
    out
}

/// 收集一组值的标量种类（`int`/`float`/`bool`/`str`/`null`）。
fn scalar_kinds(vals: &[&Value]) -> BTreeSet<&'static str> {
    let mut set = BTreeSet::new();
    for v in vals {
        match v {
            Value::Int(_) => {
                set.insert("int");
            }
            Value::Float(_, _) => {
                set.insert("float");
            }
            Value::Bool(_) => {
                set.insert("bool");
            }
            Value::Str(_) => {
                set.insert("str");
            }
            Value::Null => {
                set.insert("null");
            }
            Value::Object(_) | Value::Array(_) => {}
        }
    }
    set
}

/// 把「纯标量」取值合一为一个类型；冲突（如 int 与 str）退化为 `any` 并告警。
/// `int` + `float` 视为相容 → `num`。
fn scalar_to_ty(field: &str, kinds: &BTreeSet<&str>, warnings: &mut Vec<String>) -> TypeSpec {
    let non_null: Vec<&str> = ["int", "float", "bool", "str"]
        .iter()
        .cloned()
        .filter(|k| kinds.contains(k))
        .collect();
    match non_null.len() {
        0 => TypeSpec::Any,
        1 => match non_null[0] {
            "int" => TypeSpec::Int,
            "float" => TypeSpec::Num,
            "bool" => TypeSpec::Bool,
            "str" => TypeSpec::Str,
            _ => TypeSpec::Any,
        },
        _ => {
            if kinds.contains("int") && kinds.contains("float") && non_null.len() == 2 {
                TypeSpec::Num
            } else {
                warnings.push(format!(
                    "字段 `{}` 的取值类型冲突（如 int 与 str 混用），退化为 any",
                    field
                ));
                TypeSpec::Any
            }
        }
    }
}

/// 构建（或合并）一个对象契约，写入 `contracts[name]`。
///
/// 同一 `name` 可能被多次调用（同一字段名出现在多处 → 复用同一子契约），
/// 故走「字段并集 + 取最宽松」的合并语义（与契约叠加的「顺序无关」同思路）。
fn build_object_contract(
    name: &str,
    instances: &[&BTreeMap<String, Value>],
    contracts: &mut BTreeMap<String, Contract>,
    warnings: &mut Vec<String>,
    single: bool,
) {
    // 1) 收集字段 → 各实例中的取值（不持 map 的可变借用）
    let mut field_vals: BTreeMap<String, Vec<&Value>> = BTreeMap::new();
    for inst in instances {
        for (k, v) in *inst {
            field_vals.entry(k.clone()).or_default().push(v);
        }
    }
    // 2) 逐字段合一类型（可能向 contracts 写入子契约；此时不持任何借用，安全）
    let mut field_ty: BTreeMap<String, TypeSpec> = BTreeMap::new();
    let mut field_required: BTreeMap<String, bool> = BTreeMap::new();
    for (k, vals) in &field_vals {
        let required = single || vals.len() == instances.len();
        let ty = unify(k, vals, contracts, warnings, single);
        field_ty.insert(k.clone(), ty);
        field_required.insert(k.clone(), required);
    }
    // 3) 写入/合并到本契约（此时再索取 entry 的可变借用）
    let contract = contracts.entry(name.to_string()).or_insert_with(|| Contract {
        name: name.to_string(),
        fields: BTreeMap::new(),
        allow_extra: true,
    });
    for (k, _vals) in &field_vals {
        let ty = field_ty.get(k).unwrap().clone();
        let required = *field_required.get(k).unwrap();
        let spec = FieldSpec {
            ty,
            required,
            default: None,
            min: None,
            max: None,
            ext: None,
            ext_data: BTreeMap::new(),
            mods: Vec::new(),
        };
        // 合并：已存在则取「最宽松」（required 取与；类型不同退化 any）
        if let Some(existing) = contract.fields.get(k) {
            let merged_required = existing.required && required;
            let merged_ty = if existing.ty == spec.ty {
                spec.ty
            } else {
                warnings.push(format!(
                    "字段 `{}` 在不同实例中类型不一致（{} vs {}），退化为 any",
                    k,
                    existing.ty.name(),
                    spec.ty.name()
                ));
                TypeSpec::Any
            };
            contract.fields.insert(
                k.clone(),
                FieldSpec {
                    ty: merged_ty,
                    required: merged_required,
                    ..spec
                },
            );
        } else {
            contract.fields.insert(k.clone(), spec);
        }
    }
}

/// 把一组「同字段的值」合一为一个类型。
fn unify(
    field: &str,
    vals: &[&Value],
    contracts: &mut BTreeMap<String, Contract>,
    warnings: &mut Vec<String>,
    single: bool,
) -> TypeSpec {
    let n = vals.len();
    let n_obj = vals.iter().filter(|v| matches!(v, Value::Object(_))).count();
    let n_arr = vals.iter().filter(|v| matches!(v, Value::Array(_))).count();

    // 情况 1：全是对象 → 子契约（组合）
    if n_obj == n {
        let objs: Vec<&BTreeMap<String, Value>> = vals
            .iter()
            .map(|v| match *v {
                Value::Object(m) => m,
                _ => unreachable!(),
            })
            .collect();
        let name = sub_name(field);
        build_object_contract(&name, &objs, contracts, warnings, single);
        return TypeSpec::ContractRef(name);
    }
    // 情况 2：全是数组 → 元素合一
    if n_arr == n {
        let mut elems: Vec<&Value> = Vec::new();
        for v in vals {
            if let Value::Array(a) = v {
                for e in a {
                    elems.push(e);
                }
            }
        }
        let elem_ty = if elems.is_empty() {
            TypeSpec::Any
        } else {
            unify_array_elems(&elems, field, contracts, warnings, single)
        };
        return TypeSpec::Array(Box::new(elem_ty));
    }
    // 情况 3：全是标量（含 null）→ 标量合一
    if n_obj == 0 && n_arr == 0 {
        return scalar_to_ty(field, &scalar_kinds(vals), warnings);
    }
    // 情况 4：混合（对象/数组/标量跨类混用）→ any + 告警
    warnings.push(format!(
        "字段 `{}` 的取值类型混合（对象/数组/标量混用），退化为 any",
        field
    ));
    TypeSpec::Any
}

/// 数组元素的类型合一（与字段级 `unify` 类似，但对「对象元素」额外要求「同形」）。
fn unify_array_elems(
    elems: &[&Value],
    field: &str,
    contracts: &mut BTreeMap<String, Contract>,
    warnings: &mut Vec<String>,
    single: bool,
) -> TypeSpec {
    let n = elems.len();
    let n_obj = elems.iter().filter(|v| matches!(v, Value::Object(_))).count();
    let n_arr = elems.iter().filter(|v| matches!(v, Value::Array(_))).count();

    // 全是对象：要求「同形」才生成 `[Sub]`，否则退化为 `[any]`。
    if n_obj == n {
        let shape0: BTreeSet<String> = match elems[0] {
            Value::Object(m) => m.keys().cloned().collect(),
            _ => unreachable!(),
        };
        let same_shape = elems.iter().all(|v| match v {
            Value::Object(m) => m.keys().cloned().collect::<BTreeSet<String>>() == shape0,
            _ => false,
        });
        if same_shape {
            let objs: Vec<&BTreeMap<String, Value>> = elems
                .iter()
                .map(|v| match *v {
                    Value::Object(m) => m,
                    _ => unreachable!(),
                })
                .collect();
            let name = sub_name(field);
            build_object_contract(&name, &objs, contracts, warnings, single);
            return TypeSpec::ContractRef(name);
        }
        warnings.push(format!(
            "数组 `{}` 的元素对象形状不一致，退化为 [any]",
            field
        ));
        return TypeSpec::Any;
    }
    // 全是数组：递归合一（嵌套数组）
    if n_arr == n {
        let mut nested: Vec<&Value> = Vec::new();
        for v in elems {
            if let Value::Array(a) = v {
                for e in a {
                    nested.push(e);
                }
            }
        }
        let inner = if nested.is_empty() {
            TypeSpec::Any
        } else {
            unify_array_elems(&nested, field, contracts, warnings, single)
        };
        return TypeSpec::Array(Box::new(inner));
    }
    // 全是标量
    if n_obj == 0 && n_arr == 0 {
        return scalar_to_ty(field, &scalar_kinds(elems), warnings);
    }
    warnings.push(format!("数组 `{}` 的元素类型混合，退化为 [any]", field));
    TypeSpec::Any
}

impl InferResult {
    /// 渲染为 SML `@contract` 文本（确定性：根优先，其余按名排序；字段按名排序；无时间戳）。
    /// 契约之间以空行分隔，整体以单个换行结尾（便于作为文件/管道内容逐字节稳定）。
    pub fn to_sml(&self) -> String {
        let mut names: Vec<&String> = self.contracts.keys().collect();
        names.sort();
        if let Some(pos) = names.iter().position(|n| *n == &self.root_name) {
            names.remove(pos);
            names.insert(0, &self.root_name);
        }
        let mut out = String::new();
        let mut first = true;
        for name in names {
            if !first {
                out.push('\n');
            }
            out.push_str(&contract_to_sml(name, &self.contracts[name]));
            first = false;
        }
        out
    }

    /// 渲染为 JSON 字段类型表（确定性，同 [`to_sml`](Self::to_sml)）。
    pub fn to_json(&self) -> String {
        let mut names: Vec<&String> = self.contracts.keys().collect();
        names.sort();
        if let Some(pos) = names.iter().position(|n| *n == &self.root_name) {
            names.remove(pos);
            names.insert(0, &self.root_name);
        }
        let mut out = String::new();
        out.push_str("{\n");
        out.push_str("  \"contracts\": {\n");
        for (i, name) in names.iter().enumerate() {
            let c = &self.contracts[*name];
            out.push_str(&format!("    {}: {{\n", json_str(name)));
            out.push_str(&format!("      \"loose\": {},\n", c.allow_extra));
            out.push_str("      \"fields\": {\n");
            let fcount = c.fields.len();
            for (j, (fname, spec)) in c.fields.iter().enumerate() {
                out.push_str(&format!(
                    "        {}: {{\"type\": {}, \"optional\": {}}}",
                    json_str(fname),
                    json_str(&spec.ty.name()),
                    !spec.required
                ));
                if j + 1 < fcount {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("      }\n");
            out.push_str("    }");
            if i + 1 < names.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  },\n");
        out.push_str("  \"warnings\": [\n");
        for (i, w) in self.warnings.iter().enumerate() {
            out.push_str(&format!("    {}", json_str(w)));
            if i + 1 < self.warnings.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("  ]\n");
        out.push_str("}\n");
        out
    }
}

/// 单个契约 → SML `@contract Name loose { ... }` 片段。
fn contract_to_sml(name: &str, c: &Contract) -> String {
    let mut s = format!("@contract {} ", name);
    if c.allow_extra {
        s.push_str("loose ");
    }
    s.push_str("{\n");
    for (fname, spec) in &c.fields {
        s.push_str("    ");
        s.push_str(fname);
        s.push_str(": ");
        s.push_str(&spec.ty.name());
        if !spec.required {
            s.push_str(" optional");
        }
        s.push('\n');
    }
    s.push_str("}\n");
    s
}

/// JSON 字符串转义（手搓，不引第三方依赖）。
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(pairs: &[(&str, Value)]) -> Value {
        let mut m = BTreeMap::new();
        for (k, v) in pairs {
            m.insert(k.to_string(), v.clone());
        }
        Value::Object(m)
    }

    #[test]
    fn single_instance_all_required_with_warning() {
        let v = obj(&[
            ("name", Value::Str("web".into())),
            ("port", Value::Int(80)),
        ]);
        let r = infer_from_value("Inferred", &v).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("单实例")));
        assert_eq!(
            r.to_sml(),
            "@contract Inferred loose {\n    name: str\n    port: int\n}\n"
        );
    }

    #[test]
    fn multi_instance_optional_when_missing() {
        let a = obj(&[
            ("name", Value::Str("web".into())),
            ("port", Value::Int(80)),
        ]);
        let b = obj(&[("name", Value::Str("db".into()))]); // port 缺失
        let v = Value::Array(vec![a, b]);
        let r = infer_from_value("Inferred", &v).unwrap();
        assert!(!r.warnings.iter().any(|w| w.contains("单实例")));
        assert_eq!(
            r.to_sml(),
            "@contract Inferred loose {\n    name: str\n    port: int optional\n}\n"
        );
    }

    #[test]
    fn nested_object_becomes_sub_contract() {
        let mut addr = BTreeMap::new();
        addr.insert("city".into(), Value::Str("bj".into()));
        let mut m = BTreeMap::new();
        m.insert("name".into(), Value::Str("web".into()));
        m.insert("address".into(), Value::Object(addr));
        let r = infer_from_value("Inferred", &Value::Object(m)).unwrap();
        assert_eq!(
            r.to_sml(),
            "@contract Inferred loose {\n    address: Address\n    name: str\n}\n\n@contract Address loose {\n    city: str\n}\n"
        );
    }

    #[test]
    fn array_of_ints_unifies_to_int() {
        let mut m = BTreeMap::new();
        m.insert(
            "tags".into(),
            Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
        );
        let r = infer_from_value("Inferred", &Value::Object(m)).unwrap();
        assert_eq!(
            r.to_sml(),
            "@contract Inferred loose {\n    tags: [int]\n}\n"
        );
    }

    #[test]
    fn array_of_mixed_degrades_to_any_with_warning() {
        let mut m = BTreeMap::new();
        m.insert(
            "tags".into(),
            Value::Array(vec![Value::Int(1), Value::Str("x".into())]),
        );
        let r = infer_from_value("Inferred", &Value::Object(m)).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("tags") && w.contains("any")));
        assert_eq!(
            r.to_sml(),
            "@contract Inferred loose {\n    tags: [any]\n}\n"
        );
    }

    #[test]
    fn array_of_objects_same_shape_becomes_sub() {
        let item1 = obj(&[("id", Value::Int(1))]);
        let item2 = obj(&[("id", Value::Int(2))]);
        let mut m = BTreeMap::new();
        m.insert("items".into(), Value::Array(vec![item1, item2]));
        let r = infer_from_value("Inferred", &Value::Object(m)).unwrap();
        assert_eq!(
            r.to_sml(),
            "@contract Inferred loose {\n    items: [Items]\n}\n\n@contract Items loose {\n    id: int\n}\n"
        );
    }

    #[test]
    fn array_of_objects_diff_shape_degrades_to_any() {
        let item1 = obj(&[("id", Value::Int(1))]);
        let item2 = obj(&[("name", Value::Str("x".into()))]);
        let mut m = BTreeMap::new();
        m.insert("items".into(), Value::Array(vec![item1, item2]));
        let r = infer_from_value("Inferred", &Value::Object(m)).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("items")));
        assert_eq!(
            r.to_sml(),
            "@contract Inferred loose {\n    items: [any]\n}\n"
        );
    }

    #[test]
    fn json_output_is_deterministic() {
        let v = obj(&[
            ("name", Value::Str("web".into())),
            ("port", Value::Int(80)),
        ]);
        let r = infer_from_value("Inferred", &v).unwrap();
        assert_eq!(
            r.to_json(),
            "{\n  \"contracts\": {\n    \"Inferred\": {\n      \"loose\": true,\n      \"fields\": {\n        \"name\": {\"type\": \"str\", \"optional\": false},\n        \"port\": {\"type\": \"int\", \"optional\": false}\n      }\n    }\n  },\n  \"warnings\": [\n    \"单实例无法判定 optional：所有字段按 required 推断\"\n  ]\n}\n"
        );
    }

    #[test]
    fn float_unifies_to_num() {
        let mut m = BTreeMap::new();
        m.insert("ratio".into(), Value::float(0.5));
        let r = infer_from_value("Inferred", &Value::Object(m)).unwrap();
        assert_eq!(
            r.to_sml(),
            "@contract Inferred loose {\n    ratio: num\n}\n"
        );
    }
}
