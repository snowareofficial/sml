// SPDX-License-Identifier: MulanPSL-2.0
//! SML contracts — an optional schema layer that depends only on the value model. ｜ SML 契约（Contract）—— 可选的 schema 层，只依赖值模型。
//!
//! SML itself is a plain data format (same layer as JSON/YAML) and its value model has only 7 types, so it has no struct definitions, enums or field constraints. ｜ SML 本身是纯数据格式（与 JSON/YAML 同层），值模型只有 7 种类型，**不具备**结构体定义、枚举、字段约束等类型系统能力。
//! Contracts are the optional validation layer on top of it, adding shape and value constraints to blocks. ｜ 契约是在此之上的**可选校验层**，用于给块加上结构与取值约束。

use std::collections::{BTreeMap, BTreeSet};

// `Map` = `Value::Object` 的载体（默认 `BTreeMap`；`preserve_order` 档是保序 `Object`）。
// 本 crate 里凡是**来自/进入 `Value::Object`** 的表都必须用它，
// 其余（`contracts` 契约表、`fields` 字段表、`ext_data`）是普通表，仍是 `BTreeMap`。
use sml_codes::{
    E_CONTRACT_001, E_CONTRACT_002, E_CONTRACT_003, E_CONTRACT_004, E_CONTRACT_005, E_CONTRACT_006,
    E_CONTRACT_007, E_CONTRACT_008, E_CONTRACT_009, E_CONTRACT_010, E_CONTRACT_011, E_CONTRACT_016,
    E_CONTRACT_024, SmlError,
};
use sml_value::{Map, Value};

/// 外置扩展点：下游注册自己的字段类型，无需改动本 crate 源码。
pub mod ext;

// 契约推断（任务 E）：从数据实例反推契约。落地仅此模块 + smltools 子命令。
pub mod infer;

pub use ext::{
    AnyOf, ContractExt, DependsOn, ExactlyOne, GroupView, ItemsMin, Modifier, OneOf, TypeCheck,
    XOnly,
};

/// Finds the registered contract name closest to `bad`, used to append a "did you mean X?" hint to the undefined-contract error (I-9). ｜ 在已注册契约名里找一个与 `bad` 最相近的名字，用于「未定义的契约」报错时追加「你是不是想写 X」提示（I-9）。
///
/// 相似度门槛：**编辑距离 ≤ 2**（覆盖少/多/错一个字符、大小写差异这类典型笔误）。
/// 没有候选（全部相距 > 2）时返回 `None`，调用方据此**不追加**任何提示、保持原文案，
/// 因此不会误报。空契约表直接返回 `None`（不 panic）。
///
/// 命中多个同距候选时，取名字较短者（更少多余字符更可能是笔误目标），再取字典序最小者，
/// 保证结果确定、可测试。
pub fn suggest_contract_name(bad: &str, contracts: &BTreeMap<String, Contract>) -> Option<String> {
    if contracts.is_empty() {
        return None;
    }
    let mut best: Option<(usize, &str)> = None; // (距离, 名字)
    for name in contracts.keys() {
        if name == bad {
            continue;
        }
        let d = levenshtein(bad, name);
        match best {
            None => best = Some((d, name)),
            Some((bd, _)) if d < bd => best = Some((d, name)),
            // 同距取较短（更少多余字符更可能是笔误目标）
            Some((bd, bn)) if d == bd && name.len() < bn.len() => best = Some((d, name)),
            _ => {}
        }
    }
    match best {
        Some((d, name)) if d <= 2 => Some(name.to_string()),
        _ => None,
    }
}

/// 经典 Levenshtein 编辑距离（按 `char` 计，非字节），用于近似契约名判定。
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
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

/// A field type inside a contract ｜ 契约中的字段类型。
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
    /// **外置类型**：由下游通过 [`crate::ext::TypeCheck`] 注册。
    /// 名字与校验规则都留在下游仓库，规范层不认识它 —— 这正是扩展点的意义：
    /// 「`image` / `link` 是什么」不该由数据格式来回答。
    Ext(String),
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
        TypeSpec::Ext(name) => name.clone(),
        }
    }
}

/// A field spec inside a contract ｜ 契约中的字段规格。
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
    /// **外置类型**的校验器（见 [`crate::ext::TypeCheck`]）。
    ///
    /// 只在 `ty` 为 [`TypeSpec::Ext`] 时有值。随字段规格一起传递，
    /// 因此校验时**不需要全局注册表**（契约可跨块复用、也可安全克隆）。
    pub ext: Option<std::sync::Arc<dyn crate::ext::TypeCheck>>,
    /// **外置修饰符**解析出的值（字段名 → 值）。
    /// `FieldSpec` 本身没有对应字段时（如 `items_max`）存放在这里。
    pub ext_data: BTreeMap<String, Value>,
    /// 本字段上生效的**外置修饰符**，校验期依次回调 [`crate::ext::Modifier::check`]。
    pub mods: Vec<std::sync::Arc<dyn crate::ext::Modifier>>,
    /// **`from-featag`**：该键只能由 featag 落点写入，文档里直接赋值即 `E-CONTRACT-024`（§6.2）。
    ///
    /// 只有两档：默认（谁都能写）与 `from-featag`。**刻意没有 `none` / `readonly` 第三档** ——
    /// 「谁都写不了」用「必填但无人提供」或 `optional` 表达（用户 2026-11-03 定的方向）。
    pub from_featag: bool,
}

/// How strict a contract is — three levels ｜ 契约严格性（三态）。
///
/// 只管「未声明字段」这一类**形状宽容违约**；类型不符 / 必填缺失 / 数值越界 /
/// 枚举越界 / 互斥组违约等**恒为 `Err`**，即使处在 `Warn` 态也不放行 ——
/// 错类型的数据放行出去是灾难，形状宽容与数据正确性不是一回事。
///
/// 优先级（D4）：`Strict > Warn > Loose`。变体按此顺序声明，故 [`Ord`] 取
/// `min` 即得「最严一档」，多契约叠加（`[`apply_contracts`]`）据此实现
/// strict 优先且顺序无关。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Strictness {
    /// 未声明字段 ⇒ `Err(E-CONTRACT-004)`（保持现状，契约级默认档）。
    Strict,
    /// 未声明字段 ⇒ **放行**并产出一条 `DiagnosticKind::Warning`（由 sml-parse 转译）。
    Warn,
    /// 未声明字段 ⇒ 静默放行，无诊断（即旧 `loose` 档）。
    Loose,
}

/// Non-fatal warning produced when a `warn`-mode contract lets an undeclared field pass ｜ 契约 warn 态放行「未声明字段」时产出的非致命告警。
///
/// 定义在 `sml-contract` 而非 `sml-parse`，以避免 `sml-contract → sml-parse`
/// 的反向依赖（`sml-parse` 已依赖 `sml-contract`）。调用方（sml-parse）把它
/// 转译为 `Diagnostic { kind: DiagnosticKind::Warning, .. }`，只经 `*_with_warnings`
/// 入口交出；`parse()` 恒不返回（见 `sml-parse` 的 `api`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractWarning {
    /// 告警文案（含字段路径与契约名）。
    pub message: String,
}

/// 契约里声明的一个 featag（S4）｜ one `!featag` declaration inside a contract.
///
/// 声明写在契约体内、记号与使用处一致（`docs/swsml/spec/featag.md` §2.1）：
///
/// ```sml
/// @contract wea {
///   a: int
///   !critical              # 无参：纯标签
///   !obey: [int]           # 带参：参数类型走契约字段类型语法
///   !tls: [str] -> tls     # 带落点：参数落到 `tls`
///   !cert: str -> tls.cert # 落点可以是点路径（**不穿数组**）
/// }
/// ```
///
/// ⚠️ **作用域是契约私有的**（§5）：`wea.critical` 与 `other.critical` 互不干扰，
/// 故所有诊断必须写全 `<契约>.<featag>`。
///
/// ⚠️ 本结构**只承载声明**：参数校验（未声明即拒 / 类型是否相符）在 S5，
/// 落点填充在 S6。S4 阶段它唯一的用途是「收集 + 查重」。
#[derive(Debug, Clone)]
pub struct FeatagDecl {
    /// 名字（不含 `!`）。
    pub name: String,
    /// 参数规格；`None` = 无参 featag（纯标签）。复用 [`FieldSpec`] 是为了让 S5 的
    /// 类型校验能直接用现成那一套（含 `default` / `min` / `max` / 外置类型）。
    pub spec: Option<FieldSpec>,
    /// 落点：**点路径**，只在「有参 + 写了 `->`」时存在；`None` = 不写任何键。
    ///
    /// 用 [`sml_value::Path`] 表达，因为 S6 的填充要按段走（`tls.cert` 是两段键，
    /// 不是一次字符串查找）。数组下标在**声明时**就拒掉（落点不穿数组，§10 Q12）。
    pub target: Option<sml_value::Path>,
}

/// A contract (schema): a set of field specs ｜ 契约（schema）：一组字段规格。
#[derive(Debug, Clone)]
pub struct Contract {
    pub name: String,
    pub fields: BTreeMap<String, FieldSpec>,
    /// 严格性（三态，仅管「未声明字段」这一类形状宽容违约；类型不符 / 必填缺失 /
    /// 数值越界 / 枚举越界 / 互斥组违约等**恒为 Err**，warn 态也不放行）。
    ///
    /// - `Strict`（默认）：未声明字段 ⇒ `Err(E-CONTRACT-004)`；
    /// - `Warn`：未声明字段 ⇒ **放行** + 一条 `DiagnosticKind::Warning`；
    /// - `Loose`：未声明字段 ⇒ 静默放行，无诊断（旧 `loose` 档）。
    ///
    /// 优先级：`strict > warn > loose`；多契约叠加取最严一档（见
    /// [`apply_contracts`]）。块级 `@is X warn|loose`
    /// 可把本块显式降到 `Warn`/`Loose`（D4①）。
    pub strictness: Strictness,
    /// 应用本契约前必须已开启的特性（由契约体里的 `@requires` 声明）。
    ///
    /// 仅用于文档化与跨端拒绝：Rust 在**契约定义时**已逐条校验这些特性都已开启
    /// （见 `sml-parse` 的契约解析分支），未开启即 `E-CONTRACT-018`；应用期不再重复校验。
    /// 跨端（JS / C / C++ / Lua）暂未实现 `@requires` 的解析，见到即按 `E-CONTRACT-018`
    /// 响亮拒绝（参照 `E-CONTRACT-017` 契约叠加未实现的先例），绝不静默忽略或静默给树。
    pub requires: Vec<String>,
    /// 契约级开关 **`!any`**（S8，§10 Q7）：允许本契约使用 `any` 类型。
    ///
    /// `any` 等于放弃类型校验，默认**禁用**；契约里显式写 `!any` 才放行（含 `any` 里的 `null`）。
    /// **不占特性位**：它是契约级开关，不是文档级特性 —— 一个文档里可以只有个别契约放开 `any`。
    pub allow_any: bool,
    /// 契约级开关 **`!strict-paths`**（featag 落点规则 §6.1 规则 3）。
    ///
    /// `false`（默认）：落点路径上的父对象不存在 ⇒ **自动创建**（那是「插入」，不是「覆盖」）。
    /// `true`：父对象必须已存在，缺了就报 `E-CONTRACT-023` —— 适合「落点必须落在作者写好的
    /// 结构里」的场景（防止 featag 顺手造出一整棵没人预期的树）。
    pub strict_paths: bool,
    /// 契约里声明的 featag（**保序**，声明顺序即诊断里的列举顺序）。
    ///
    /// 只在 Rust 侧由 `@contract` 解析填充（S4）；其它四端没有 featag，
    /// 所以它们见不到这个字段（跨端以「本端不支持」拒绝，见 S9）。
    pub featags: Vec<FeatagDecl>,
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

/// 按单条字段规格校验一个值 —— **对外**入口，供 featag 参数复用（S5）。
///
/// 与契约内部的 [`check_type`] 完全同一条链：数组类型、契约引用、枚举、区间、外置类型、
/// 修饰符回调都会生效。`contract` / `path` 只用于**错误文案**定位
/// （featag 传 `契约名.featag名`，如 `wea.obey`）。
///
/// `contracts` 供 `ContractRef`（组合）递归查找被引用契约 —— 与字段校验同一张表。
pub fn check_value(
    contract: &str,
    path: &str,
    spec: &FieldSpec,
    v: &Value,
    contracts: &BTreeMap<String, Contract>,
) -> Result<(), SmlError> {
    let mut warnings: Vec<ContractWarning> = Vec::new();
    check_type(contract, path, spec, v, contracts, &mut warnings)
}

/// 检查「`from-featag` 的键没有被文档手写」（featag S7，§6.2）。
///
/// ⚠️ **必须在 featag 落点填充之前调用**：填充之后，featag 自己写进去的键与文档手写的键
/// 长得一模一样，就分不出「来源合法」还是「伪造」了 —— 而来源正是这条规则的**全部意义**。
///
/// 递归：遇到 `ContractRef` 字段且值确实是块时，按被引用契约继续查（嵌套的 `from-featag`）。
/// 数组里的契约引用**暂不检查**（`[T]` 的逐项归属需要数组下标信息，等有真实需求再加）。
pub fn check_from_featag(
    contract: &Contract,
    map: &Map,
    contracts: &BTreeMap<String, Contract>,
    path: &str,
) -> Result<(), SmlError> {
    for (fname, spec) in &contract.fields {
        let full = join_path(path, fname);
        if spec.from_featag && map.contains_key(fname) {
            return Err(SmlError::from_code_title(E_CONTRACT_024).with_detail(format!(
                "字段 `{full}` 声明为 `from-featag`：只能由 featag 落点写入，文档里不能直接赋值"
            )));
        }
        if let TypeSpec::ContractRef(ref_name) = &spec.ty {
            if let Some(Value::Object(child)) = map.get(fname) {
                if let Some(inner) = contracts.get(ref_name) {
                    check_from_featag(inner, child, contracts, &full)?;
                }
            }
        }
    }
    Ok(())
}

/// 校验值是否符合类型规格。
/// `contracts` 供 `ContractRef`（组合）递归查找被引用契约。
///
/// `path` 是该字段的**完整路径**（如 `api.port`、`items[2].name`），
/// 由调用方在递归时用 [`join_path`] 拼接后传入，仅用于错误信息定位。
///
/// `warnings` 透传至 [`apply_contract`]（组合契约自身的 warn 态告警也要收集）。
fn check_type(
    contract: &str,
    path: &str,
    spec: &FieldSpec,
    v: &Value,
    contracts: &BTreeMap<String, Contract>,
    warnings: &mut Vec<ContractWarning>,
) -> Result<(), SmlError> {
    // 组合：字段值是块，递归按被引用的契约校验（含填默认值）
    if let TypeSpec::ContractRef(ref_name) = &spec.ty {
        return match v {
            Value::Object(_) => {
                let mut sub = match v {
                    Value::Object(m) => m.clone(),
                    _ => unreachable!(),
                };
                let hint = suggest_contract_name(ref_name, contracts)
                    .map(|c| format!("（你是不是想写 `{c}`？）"))
                    .unwrap_or_default();
                let target = contracts.get(ref_name).ok_or_else(|| {
                    SmlError::from_code_title(E_CONTRACT_001).with_detail(format!(
                        "字段 `{path}` 引用了未定义的契约 `{ref_name}`（契约 `{contract}`）{hint}"
                    ))
                })?;
                apply_contract(target, &mut sub, contracts, path, warnings)?;
                Ok(())
            }
            _ => Err(SmlError::from_code_title(E_CONTRACT_008).with_detail(format!(
                "字段 `{path}` 应为块并按契约 `{ref_name}` 校验，实际为 {}（契约 `{contract}`）",
                value_kind(v)
            ))),
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
                Ok(false) => Err(SmlError::from_code_title(E_CONTRACT_009).with_detail(
                    format!("字段 `{path}` 的值 `{s}` 不符合类型 `{name}` 的格式要求（契约 `{contract}`）"),
                )),
                Err(e) => Err(SmlError::from_code_title(E_CONTRACT_009)
                    .with_detail(format!("类型 `{name}` 匹配时出错：{e}"))),
            },
            other => Err(SmlError::from_code_title(E_CONTRACT_009).with_detail(format!(
                "字段 `{path}` 类型 `{name}` 要求字符串（号码 / 编号 / 身份证请用引号包裹），实际为 {}（契约 `{contract}`）",
                value_kind(other)
            ))),
        };
    }

    // 外置类型：校验器随字段规格带来（见 `FieldSpec::ext`），故此处无需全局查表。
    // 「这个类型是什么」由下游回答，规范层只负责把错误包装上字段名与路径。
    if let TypeSpec::Ext(name) = &spec.ty {
        return match &spec.ext {
            Some(t) => t.check(v).map_err(|e| {
                SmlError::from_code_title(E_CONTRACT_007).with_detail(format!(
                    "字段 `{path}` 不符合扩展类型 `{name}`：{e}（契约 `{contract}`）"
                ))
            }),
            // 有类型名但没有校验器：契约多半是从别处直接构造的（如序列化产物），
            // 此时**放行**而不是误报 —— 没有校验器就无从判定，报错才是假警报。
            None => Ok(()),
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
            // 外置元素类型（如 `[image]`）沿用同一个校验器
            ext: spec.ext.clone(),
            // 修饰符作用于**字段整体**，不逐元素重复；故元素规格不带这两项
            ext_data: BTreeMap::new(),
            mods: Vec::new(),
            // 来源规则同样作用于字段整体，不逐元素重复
            from_featag: false,
        };
        for (i, it) in items.iter().enumerate() {
            check_type(contract, &format!("{}[{}]", path, i), &elem, it, contracts, warnings)?;
        }
        // 元素全部通过。数组本身不参与 min/max 区间校验（那是数值语义）。
        // 但**外置修饰符仍要跑**（`items_max` 这类约束正是作用在数组整体上），
        // 否则数组字段的扩展修饰符会被静默跳过。
        check_ext_mods(contract, path, spec, v)?;
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
        // 「取值不在枚举列表内」是**独立条件**，不能与「类型不符」共用一个码 ——
        // 否则同一份文档在 Rust 与 JS 上会拿到不同的码，「同因同码」当场破功。
        if let TypeSpec::Enum(vals) = &spec.ty {
            let got = match v {
                Value::Str(s) => s.clone(),
                Value::Int(i) => i.to_string(),
                Value::Float(f, _) => f.to_string(),
                other => value_kind(other).to_string(),
            };
            return Err(SmlError::from_code_title(E_CONTRACT_006).with_detail(format!(
                "字段 `{path}` 类型应为 enum [{}]，实际取值 `{got}` 不在列表内（契约 `{contract}`）",
                vals.join(" ")
            )));
        }
        return Err(SmlError::from_code_title(E_CONTRACT_002).with_detail(format!(
            "字段 `{path}` 类型应为 {}，实际为 {}（契约 `{contract}`）",
            spec.ty.name(),
            value_kind(v)
        )));
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
                return Err(SmlError::from_code_title(E_CONTRACT_010).with_detail(format!(
                    "字段 `{path}` 的值为非有限数（NaN/inf），不可作为数值约束的取值（契约 `{contract}`）"
                )));
            }
            if let Some(lo) = spec.min {
                if n < lo {
                    return Err(SmlError::from_code_title(E_CONTRACT_005).with_detail(
                        format!("字段 `{path}` 值 {n} 小于下界 {lo}（契约 `{contract}`）"),
                    ));
                }
            }
            if let Some(hi) = spec.max {
                if n > hi {
                    return Err(SmlError::from_code_title(E_CONTRACT_005).with_detail(
                        format!("字段 `{path}` 值 {n} 大于上界 {hi}（契约 `{contract}`）"),
                    ));
                }
            }
        }
    }
    // 外置修饰符的校验期钩子：内置校验（类型 / 枚举 / 区间）**全部通过后**才交给下游。
    // 顺序不能反 —— 先满足规范层语义再看领域规则，否则错误信息会失去层次。
    check_ext_mods(contract, path, spec, v)
}

/// 依次回调字段上的外置修饰符（[`crate::ext::Modifier::check`]）。
fn check_ext_mods(
    contract: &str,
    path: &str,
    spec: &FieldSpec,
    v: &Value,
) -> Result<(), SmlError> {
    for m in &spec.mods {
        m.check(spec, v).map_err(|e| {
            SmlError::from_code_title(E_CONTRACT_011).with_detail(format!(
                "字段 `{path}` 不符合修饰符 `{}`：{e}（契约 `{contract}`）",
                m.name()
            ))
        })?;
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

/// Contract stacking: one block applies **several** contracts, merged into a single view ｜ 契约叠加：一个块应用**多份**契约。
///
/// 写法：`@is a, b` / `@is a b` / 块级 `conta contb item { }`（TASK-agent-w24 任务 C）。
///
/// 语义（D3，用户裁决）：
/// - **strict 优先**：任一契约严格 ⇒ 整体按严格校验；
/// - **顺序无关**：字段按名取并集（`BTreeMap` 天然有序）；
/// - **冲突即报错**：同字段名但类型 / 默认值 / 区间不一致 ⇒ `E-CONTRACT-016`
///   （不允许「后者覆盖」—— 那会让结果依赖书写顺序）。
///
/// 实现走**合并视图**：构造一个临时 `Contract`，复用既有 [`apply_contract`] 的
/// 默认值回填 / 类型校验 / 组合递归 —— 不另写一条校验路径，行为与单契约同源。
pub fn apply_contracts(
    entries: &[(String, Option<Strictness>)],
    node: &mut Map,
    contracts: &BTreeMap<String, Contract>,
    path: &str,
    warnings: &mut Vec<ContractWarning>,
) -> Result<(), SmlError> {
    // 每条契约的**有效**严格度 = 块级显式覆盖（D4①）.unwrap_or(契约级声明)；
    // 多契约叠加按 D4②「strict 优先、顺序无关」⇒ 取最严一档
    // （`Strict < Warn < Loose`，`Ord` 取 `min` 即得）。
    let mut list: Vec<&Contract> = Vec::with_capacity(entries.len());
    let mut effs: Vec<Strictness> = Vec::with_capacity(entries.len());
    for (n, ov) in entries {
        let hint = suggest_contract_name(n, contracts)
            .map(|c| format!("（你是不是想写 `{c}`？）"))
            .unwrap_or_default();
        let c = contracts.get(n).ok_or_else(|| {
            SmlError::from_code_title(E_CONTRACT_001).with_detail(format!("`{n}`{hint}"))
        })?;
        list.push(c);
        effs.push(ov.unwrap_or(c.strictness));
    }
    // 两两冲突预检：同字段名但类型 / 默认值 / 区间不同即冲突。
    // ⚠️ 按**字段名**遍历（BTreeMap 有序），与书写顺序无关。
    // 类型比较用 `TypeSpec::name()` 的规范串（ext 类型即注册名，同名即同校验器）。
    for i in 0..list.len() {
        for j in (i + 1)..list.len() {
            for (fname, fa) in &list[i].fields {
                if let Some(fb) = list[j].fields.get(fname) {
                    let clash = fa.ty.name() != fb.ty.name()
                        || fa.default != fb.default
                        || fa.min != fb.min
                        || fa.max != fb.max;
                    if clash {
                        return Err(SmlError::from_code_title(E_CONTRACT_016).with_detail(
                            format!(
                                "字段 `{}` 在 `{}` 与 `{}` 里类型 / 默认值 / 区间不一致",
                                join_path(path, fname),
                                list[i].name,
                                list[j].name
                            ),
                        ));
                    }
                }
            }
        }
    }
    // 合并视图：字段并集。同字段的 ty/default/min/max 预检已保证一致，取任一即可；
    // 但 `required` **不在**预检内，必须显式合并 —— 取**或**（任一致约要求即必填）。
    // 这才满足 D3「顺序无关 / strict 优先」：`@is A B` 与 `@is B A` 结果相同。
    // （早先此处用 `insert` 后者覆盖，required 于是随书写顺序变，是 D3 违返，已修。）
    let mut fields: BTreeMap<String, FieldSpec> = BTreeMap::new();
    for c in &list {
        for (k, v) in &c.fields {
            fields
                .entry(k.clone())
                .and_modify(|e| e.required = e.required || v.required)
                .or_insert_with(|| v.clone());
        }
    }
    // 多契约叠加的严格度：取各契约**有效**严格度的最严一档（D4②）。空表不可能
    // 走到这里（调用方至少传一个契约），`unwrap_or(Strict)` 仅为让类型闭合。
    let strictness = effs.into_iter().min().unwrap_or(Strictness::Strict);
    let mut merged_requires: Vec<String> = Vec::new();
    for c in &list {
        for r in &c.requires {
            if !merged_requires.contains(r) {
                merged_requires.push(r.clone());
            }
        }
    }
    // featag 声明也是并集：叠加后仍按**各自的声明顺序**拼接（同一名字在不同契约里
    // 本就是不同 featag —— 作用域契约私有，§5；真正的「重名」在**单个契约内**由 S4 拒掉）。
    let mut merged_featags: Vec<FeatagDecl> = Vec::new();
    for c in &list {
        merged_featags.extend(c.featags.iter().cloned());
    }
    let merged = Contract {
        name: list
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join("+"),
        fields,
        strictness,
        requires: merged_requires,
        // 叠加时取**或**：任一份契约要求「父对象必须已存在」，叠加后就得守这条
        // （与 `required` 的合并口径一致：约束取并，不取某个契约的宽松度）。
        strict_paths: list.iter().any(|c| c.strict_paths),
        // `!any` 同理取**或**：叠加后只要有一份契约放开，`any` 就可用
        // （它是「允许」不是「约束」，与 `strict_paths` 的方向相反，但都按「任一」合并 —— 因为
        //  校验时看的是**合并后**这一份契约，不能因为叠加而把已放开的又收回）。
        allow_any: list.iter().any(|c| c.allow_any),
        featags: merged_featags,
    };
    apply_contract(&merged, node, contracts, path, warnings)
}

/// Apply one contract to a block: fill defaults, validate, enforce strictness ｜ 对块应用契约：填充默认值 + 校验 + 严格性检查。
///
/// **严格（`Strict`）为默认**：契约未声明的字段会被拒绝，除非契约显式标记
/// `loose` / `warn`。这样拼错的字段名（如 `prot`）会立即报错，而不是被静默忽略。
///
/// `warnings` 收集 **warn 态**放行的形状宽容违约（未声明字段）；调用方（sml-parse）
/// 转译为 `DiagnosticKind::Warning`，只经 `*_with_warnings` 可见。`parse()` 恒不返回。
pub fn apply_contract(
    c: &Contract,
    node: &mut Map,
    contracts: &BTreeMap<String, Contract>,
    path: &str,
    warnings: &mut Vec<ContractWarning>,
) -> Result<(), SmlError> {
    // 0) `x_only` 与 `strict` **互斥**：strict 下额外键先被 `E-CONTRACT-004` 拦下，
    //    修饰符的 `check_group` 永不执行 ⇒ 写了也不起作用。这里显式报错，
    //    避免「静默无效」（与 `loose` / `warn` 才搭配）。
    if c.strictness == Strictness::Strict
        && c.fields
            .values()
            .any(|f| f.mods.iter().any(|m| m.name() == "x_only"))
    {
        return Err(SmlError::from_code_title(E_CONTRACT_011).with_detail(
            "`x_only` 需要契约或块级 `loose`/`warn`（与 `strict` 互斥）",
        ));
    }
    // 1) 严格性（三态，仅管「未声明字段」这一类形状宽容违约；类型不符 / 必填缺失 /
    //    数值越界 / 枚举越界 / 互斥组违约等**恒为 Err**，warn 态也不放行 ——
    //    错类型的数据放行出去是灾难，形状宽容与数据正确性不是一回事）。
    //    优先级：strict > warn > loose。多契约叠加时由 `apply_contracts` 取最严一档。
    //    ⚠️ 违约放行时值树与 strict 下的**成功**分支逐字一致（不改数据形状）。
    match c.strictness {
        Strictness::Strict => {
            for k in node.keys() {
                if !c.fields.contains_key(k) {
                    return Err(SmlError::from_code_title(E_CONTRACT_004).with_detail(
                        format!(
                            "字段 `{}` 未在契约 `{}` 中声明（严格模式；如需允许额外字段请在契约名后写 `loose`/`warn`）",
                            join_path(path, k), c.name
                        ),
                    ));
                }
            }
        }
        Strictness::Warn => {
            for k in node.keys() {
                if !c.fields.contains_key(k) {
                    // 放行：值树与 strict 成功分支逐字一致，仅追加一条告警。
                    warnings.push(ContractWarning {
                        message: format!(
                            "sml: 字段 `{}` 未在契约 `{}` 中声明，但在 warn 态下被放行（形状宽容违约；如需严格请移除 `warn`）",
                            join_path(path, k), c.name
                        ),
                    });
                }
            }
        }
        Strictness::Loose => { /* 静默放行，无诊断（旧 loose 档） */ }
    }
    // 2) `x_only "<prefix>"`：在 `loose` / `warn`（已放行）之上再**收窄**——
    //    只有以该前缀开头的额外键允许，其余仍报错。
    //    放在这里而非 `Modifier::check_group`：后者只在该字段出现时才跑，会漏检。
    if let Some(prefix) = c
        .fields
        .values()
        .filter(|f| f.mods.iter().any(|m| m.name() == "x_only"))
        .find_map(|f| f.ext_data.get("x_only").and_then(|v| v.as_str()))
    {
        let mut bad: Vec<String> = node
            .keys()
            .map(|k| k.to_string())
            .filter(|k| !c.fields.contains_key(k.as_str()))
            .filter(|k| !k.starts_with(prefix))
            .collect();
        if !bad.is_empty() {
            bad.sort();
            return Err(SmlError::from_code_title(E_CONTRACT_011).with_detail(format!(
                "额外键 {} 不以 `{prefix}` 开头（`x_only` 只放行该前缀）",
                bad.join(", ")
            )));
        }
    }
    // 快照「用户显式写出的键」：跨字段约束（互斥组等）以它为准，`default` 回填的值
    // **不参与计数**（理由见 `ext::GroupView::present`）。
    //
    // 只存**键名**：下一步要给缺字段回填 `default`，那会 `insert` 到 `node` 里；
    // 若这里借了 `node` 的元素，就会和那次可变借用打架（E0502）。
    // 真正需要「键 → 值」时，等第 2 步改完 `node` 之后再按这份键名筛出来。
    //
    // 只有真的挂了外置修饰符才做这次快照 —— 零扩展时连这一次遍历都省掉。
    let authored_keys: Option<BTreeSet<String>> =
        if c.fields.values().any(|f| !f.mods.is_empty()) {
            // `.map(to_string)` 而非 `.cloned()`：保序档的 `keys()` 产出 `&str`，
            // `str: Clone` 不成立 —— 收成 `String` 两档同形。
            Some(node.keys().map(|k| k.to_string()).collect())
        } else {
            None
        };
    // 2) 逐字段：填默认值 + 类型/枚举/区间/组合校验
    for (k, spec) in &c.fields {
        match node.get(k) {
            None => {
                if let Some(d) = &spec.default {
                    node.insert(k.clone(), d.clone());
                } else if spec.required {
                    return Err(SmlError::from_code_title(E_CONTRACT_003).with_detail(
                        format!("字段 `{}` 必填但缺失（契约 `{}`）", join_path(path, k), c.name),
                    ));
                }
            }
            Some(v) => {
                let field_path = join_path(path, k);
                // 组合会回填子块默认值，故需要可变副本
                if matches!(spec.ty, TypeSpec::ContractRef(_)) {
                    // 先按**原值**校验必须是块，否则会退化成
                    // 「子字段缺失」这类误导性错误
                    check_type(&c.name, &field_path, spec, v, contracts, warnings)?;
                    let mut sub = match v {
                        Value::Object(m) => m.clone(),
                        _ => unreachable!("check_type 已保证为块"),
                    };
                    check_type_contract_ref(&c.name, &field_path, spec, &mut sub, contracts, warnings)?;
                    node.insert(k.clone(), Value::Object(sub));
                } else {
                    check_type(&c.name, &field_path, spec, v, contracts, warnings)?;
                }
            }
        }
    }
    // 3) 跨字段约束（互斥组等）：前置的严格性与单字段校验全部通过后才轮到它 ——
    //    报错要有层次，先说「类型不对 / 必填缺失」，再说「这几个字段不许同时出现」。
    if let Some(keys) = &authored_keys {
        // 现在才借 `node` 的值（第 2 步的 `insert` 已结束，借用不再冲突）。
        // 只留用户**显式写出**的键 —— 第 2 步回填的 `default` 值不在 `keys` 里。
        let present: BTreeMap<String, &Value> = node
            .iter()
            .map(|(k, v)| (k.to_string(), v))
            .filter(|(k, _)| keys.contains(k))
            .collect();
        check_ext_groups(c, path, &present)?;
    }
    Ok(())
}

/// 依次回调字段上的**跨字段**钩子（[`crate::ext::Modifier::check_group`]）。
///
/// `authored` 是用户在文档里显式写出的键（不含 `default` 回填），语义见
/// [`crate::ext::GroupView::present`]。
///
/// 遍历顺序：字段按**名字**（`BTreeMap` 字典序），同一字段上的多个修饰符按注册顺序
/// —— 结果与书写顺序无关，同一输入恒报同一个错。
fn check_ext_groups(
    c: &Contract,
    path: &str,
    authored: &BTreeMap<String, &Value>,
) -> Result<(), SmlError> {
    for (fname, spec) in &c.fields {
        for m in &spec.mods {
            let view = crate::ext::GroupView {
                contract: &c.name,
                path,
                owner: fname,
                spec,
                declared: &c.fields,
                present: authored,
            };
            m.check_group(&view).map_err(|e| {
                let detail = if path.is_empty() {
                    format!("契约 `{}` 的跨字段约束 `{}` 未满足：{}", c.name, m.name(), e)
                } else {
                    format!("块 `{path}` 的跨字段约束 `{}` 未满足：{e}（契约 `{}`）", m.name(), c.name)
                };
                SmlError::from_code_title(E_CONTRACT_011).with_detail(detail)
            })?;
        }
    }
    Ok(())
}

/// 对「组合字段」递归应用被引用契约（会回填子块默认值）
fn check_type_contract_ref(
    contract: &str,
    path: &str,
    spec: &FieldSpec,
    sub: &mut Map,
    contracts: &BTreeMap<String, Contract>,
    warnings: &mut Vec<ContractWarning>,
) -> Result<(), SmlError> {
    let ref_name = match &spec.ty {
        TypeSpec::ContractRef(n) => n.clone(),
        _ => return Ok(()),
    };
    let hint = suggest_contract_name(&ref_name, contracts)
        .map(|c| format!("（你是不是想写 `{c}`？）"))
        .unwrap_or_default();
    let target = contracts.get(&ref_name).ok_or_else(|| {
        SmlError::from_code_title(E_CONTRACT_001).with_detail(format!(
            "字段 `{path}` 引用了未定义的契约 `{ref_name}`（契约 `{contract}`）{hint}"
        ))
    })?;
    // 先做基础类型校验（值须为块），再递归应用
    check_type(contract, path, spec, &Value::Object(sub.clone()), contracts, warnings)?;
    // 把完整路径传下去，子块的错误才能定位到 `parent.child` 而非只报字段名
    apply_contract(target, sub, contracts, path, warnings)
}
