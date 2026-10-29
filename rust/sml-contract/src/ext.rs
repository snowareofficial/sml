// SPDX-License-Identifier: MulanPSL-2.0
//! 契约的**外置扩展点**：让下游注册自己的字段类型，无需改动本 crate 源码。
//!
//! ## 动机
//!
//! 契约内置类型只有 `str / int / num / bool / any / enum / [T] / @type Pattern`
//! 这几个通用类型。但下游迟早要写 `image`（附件 id）、`link`（分享链接）、
//! `time`（时间戳）这类**领域类型**。
//!
//! 过去的做法只能是把它们逐个加进 [`crate::TypeSpec`] 枚举 —— 那样 SML 规范层
//! 就会沾上具体业务的语义（"image 是什么"不该由数据格式来回答）。
//! 本模块把这条口子外置：**类型名与校验规则都留在下游仓库**。
//!
//! ## 设计约束
//!
//! - **不注册任何扩展时，行为与既有实现完全一致**。
//! - 外置类型存为 [`TypeSpec::Ext`]，其校验器随字段规格一起传递
//!   （见 [`crate::FieldSpec::ext`]），因此**校验时不需要全局状态**。
//! - 外置类型名不得与内置类型名冲突（注册时报错），避免改写核心语义。

use std::collections::BTreeMap;
use std::sync::Arc;

use sml_codes::{E_EXT_002, SmlError};
use sml_value::Value;

use crate::FieldSpec;

/// 内置类型名。供错误消息列出，并**禁止外置扩展占用**。
pub const BUILTIN_TYPES: &[&str] = &[
    "str", "int", "num", "bool", "any", "enum",
];

/// 外置契约类型：只回答两件事 —— 叫什么、值合不合法。
///
/// 要求 `Debug`：`FieldSpec` 派生了 `Debug`，而校验器会被克隆进字段规格，
/// 二者必须一起可打印（否则调试时只能看到 `<dyn TypeCheck>`）。
pub trait TypeCheck: Send + Sync + std::fmt::Debug {
    /// 类型名，如 `"image"`。
    fn name(&self) -> &str;

    /// 校验值。不合法时返回给人看的错误信息（会被包上字段名与路径）。
    fn check(&self, v: &Value) -> Result<(), String>;
}

/// 外置**字段修饰符**：如 `items_max 20`（数组元素个数上限）。
///
/// 为什么需要它：`max` 在 SML 契约里是**数值上界**，且数组不参与 min/max 校验；
/// 下游想把 `max` 当"个数上限"用就会与核心语义撞车。扩展点让这类需求
/// **新起一个名字**，而不是改写内置修饰符的含义。
pub trait Modifier: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;

    /// **解析期**：校验修饰符取值的合法性，并可改写字段规格
    /// （`required` / `default` / `min` / `max`）。
    /// 需要把值带进校验期时，写进 [`crate::FieldSpec::ext_data`]。
    fn apply(&self, spec: &mut FieldSpec, v: &Value) -> Result<(), String>;

    /// **校验期**：内置校验（类型 / 枚举 / 区间）全部通过后才被调用。
    /// 默认放行 —— 只在解析期改写规格的修饰符不必实现它。
    fn check(&self, _spec: &FieldSpec, _v: &Value) -> Result<(), String> {
        Ok(())
    }

    /// **校验期（跨字段）**：整块的单字段校验全部通过后才被调用。
    ///
    /// 与 [`Self::check`] 的分工：`check` 只见**这一个字段的值**，够用于
    /// `items_max`（个数上限）这类约束；而**互斥 / 计数 / 依赖**必须看到
    /// 「兄弟字段有哪些出现了」—— 那是 [`GroupView`] 的活。
    ///
    /// 默认放行，故既有的修饰符实现一个字都不用改。
    /// 回调顺序：字段按**名字**（字典序）遍历，同一字段上的多个修饰符按注册顺序。
    fn check_group(&self, _view: &GroupView<'_>) -> Result<(), String> {
        Ok(())
    }
}

/// **跨字段**约束的入参（互斥 / 计数这类写在某个字段上、却要看整个块的约束）。
///
/// 语法上修饰符只能挂在某个字段上（`card: str optional one_of [ card alipay ]`），
/// 但它表达的是**整个块**的性质 —— 所以 [`Modifier::check_group`] 拿到的不是
/// 单个字段的值，而是下面这一组上下文。
#[derive(Debug, Clone, Copy)]
pub struct GroupView<'a> {
    /// 契约名（叠加时是 `A+B` 这种合并视图的名字，见 [`crate::apply_contracts`]）。
    pub contract: &'a str,
    /// 当前块的路径（顶层块为空串）。用于把错误定位到 `pay.card` 而非只报 `card`。
    pub path: &'a str,
    /// 声明本修饰符的**那个字段名**。它不要求自己是组成员之一 ——
    /// 组 who 写在哪里是排版问题，成员清单才是语义。
    pub owner: &'a str,
    /// `owner` 的字段规格：组成员清单就在 `spec.ext_data[修饰符名]` 里。
    pub spec: &'a FieldSpec,
    /// 契约声明的**全部**字段。组成员有没有被声明过，靠它判定
    /// （写错成员名必须响亮报错，而不是「组里少了一个没人要的成员」静默通过）。
    pub declared: &'a BTreeMap<String, FieldSpec>,
    /// 用户在文档里**显式写出**的键 → 值。
    ///
    /// ⚠️ **不含 `default` 回填的值**。理由是可用性：若把默认值算进互斥计数，
    /// 「组内两个成员都写了 `default`」的契约会**恒失败** —— 而那只是作者给的
    /// 兜底策略，不是「用户同时塞进两种支付方式」的意图表达。
    /// 「一个都不写算不算错」另由 `required` / [`ExactlyOne`] 表达。
    /// ⚠️ 键存在即算出现，**含显式写 `null`** —— 写了这个键就是意图表达。
    pub present: &'a BTreeMap<&'a str, &'a Value>,
}

impl<'a> GroupView<'a> {
    /// 把字段名拼成**本块**的完整路径（顶层字段没有前缀）。
    pub fn field_path(&self, name: &str) -> String {
        if self.path.is_empty() {
            name.to_string()
        } else {
            format!("{}.{}", self.path, name)
        }
    }

    /// 该字段是否被用户**显式**写出（不看 `default`）。
    pub fn is_present(&self, name: &str) -> bool {
        self.present.contains_key(name)
    }
}

/// 内置修饰符名。禁止外置扩展占用，避免改写核心语义。
pub const BUILTIN_MODIFIERS: &[&str] = &["optional", "required", "default", "min", "max"];

/// 外置类型注册表。
///
/// 用 [`Arc`] 包 trait 对象，使校验器能被克隆进每一条 [`crate::FieldSpec`]
/// （契约会被跨块复用，而 trait 对象本身不可克隆）。
#[derive(Default)]
pub struct ContractExt {
    types: BTreeMap<String, Arc<dyn TypeCheck>>,
    modifiers: BTreeMap<String, Arc<dyn Modifier>>,
}

impl Clone for ContractExt {
    fn clone(&self) -> Self {
        Self {
            types: self.types.clone(),
            modifiers: self.modifiers.clone(),
        }
    }
}

impl ContractExt {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一个外置类型。与内置类型同名、或重复注册，一律报错（不静默覆盖）。
    pub fn register_type(&mut self, t: impl TypeCheck + 'static) -> Result<(), SmlError> {
        let name = t.name().to_string();
        if BUILTIN_TYPES.contains(&name.as_str()) {
            return Err(SmlError::new(
                E_EXT_002,
                format!(
                    "sml: 不可注册与内置类型同名的扩展类型 `{name}`（内置：{}）",
                    BUILTIN_TYPES.join(" / ")
                ),
            ));
        }
        if self.types.contains_key(&name) {
            return Err(SmlError::new(
                E_EXT_002,
                format!("sml: 扩展类型 `{name}` 已注册"),
            ));
        }
        self.types.insert(name, Arc::new(t));
        Ok(())
    }

    /// 注册一个外置修饰符。与内置修饰符同名、或重复注册，一律报错。
    pub fn register_modifier(&mut self, m: impl Modifier + 'static) -> Result<(), SmlError> {
        let name = m.name().to_string();
        if BUILTIN_MODIFIERS.contains(&name.as_str()) {
            return Err(SmlError::new(
                E_EXT_002,
                format!(
                    "sml: 不可注册与内置修饰符同名的扩展修饰符 `{name}`（内置：{}）",
                    BUILTIN_MODIFIERS.join(" / ")
                ),
            ));
        }
        if self.modifiers.contains_key(&name) {
            return Err(SmlError::new(
                E_EXT_002,
                format!("sml: 扩展修饰符 `{name}` 已注册"),
            ));
        }
        self.modifiers.insert(name, Arc::new(m));
        Ok(())
    }

    /// 按名取出一份 **Arc 克隆**（供塞进 `FieldSpec`）。
    pub fn type_arc(&self, name: &str) -> Option<Arc<dyn TypeCheck + 'static>> {
        self.types.get(name).cloned()
    }

    /// 按名取出一份 **Arc 克隆**（供塞进 `FieldSpec`）。
    pub fn modifier_arc(&self, name: &str) -> Option<Arc<dyn Modifier + 'static>> {
        self.modifiers.get(name).cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty() && self.modifiers.is_empty()
    }

    pub fn len(&self) -> usize {
        self.types.len() + self.modifiers.len()
    }

    /// 已注册的类型名（按字典序）。
    pub fn type_names(&self) -> impl Iterator<Item = &str> {
        self.types.keys().map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// 数组元素**个数**下界（I-7 缺口②上半）
// ---------------------------------------------------------------------------

/// 修饰符 **`items_min <n>`**：数组元素的**个数下限**（至少有 `n` 项）。
///
/// 它与 `items_max`（个数上限，见教科书扩展点的示例）成对，同样**刻意不姓 min/max**：
/// `min` / `max` 在 SML 契约里是**数值区间**语义，而数组根本不参与数值校验，
/// 借过来会让同一写法在两处含义不同。
///
/// ⚠️ 判定对象是字段的**生效值**（`check` 只见这一个值），与互斥组用
/// [`GroupView::present`] 的口径**不同**：`check` 无从得知这个值是用户写的还是
/// `default` 回填的，且回填后的值就是要往下流的数据 —— 它不满足约束必须报。
/// 契约作者给出的兜底值自己违反 `items_min` 属于契约自相矛盾，早失败好过把坏数据放过去。
///
/// > 注：当前契约语法里 `default` 只接受裸词 / 引号串，数组字段**写不出**
/// > `default [ ... ]`，故上述交互暂不可触发；口径先钉在这份文档里，
/// > 将来支持数组默认值时照此办理。
#[derive(Debug)]
pub struct ItemsMin;

impl Modifier for ItemsMin {
    fn name(&self) -> &str {
        "items_min"
    }
    fn apply(&self, _spec: &mut FieldSpec, v: &Value) -> Result<(), String> {
        match v {
            Value::Int(n) if *n >= 0 => Ok(()),
            _ => Err("须为非负整数（元素个数下限）".into()),
        }
    }
    fn check(&self, spec: &FieldSpec, v: &Value) -> Result<(), String> {
        let limit = match spec.ext_data.get("items_min") {
            Some(Value::Int(n)) => *n,
            // 值没带过来（多半是外部直接构造的 FieldSpec）⇒ 放行而不是假警报
            _ => return Ok(()),
        };
        match v {
            Value::Array(items) if (items.len() as i64) < limit => {
                Err(format!("至少 {limit} 项，实得 {} 项", items.len()))
            }
            // 非数组由前面 `check_type` 的类型校验负责；这里不重复报错
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// 互斥（I-7 缺口①）：一组字段「至多一个」/「恰好一个」出现
//
// 为什么是 Modifier 而不是内置语法：互斥没有专属 token，靠外置修饰符只需动本
// crate 一处；五端不必同日跟进（I-7 Design 的定调：「先做成 Modifier，再考虑
// 升为内置语法」）。等它被写进多家契约、确证明值得一套专用语法时，再升级不迟。
// ---------------------------------------------------------------------------

/// 取出并校验**组成员清单**：须是字段名的数组（`[ a b ]`），至少两个且互不相同。
///
/// 三重检查都是「早失败」：继承自 fugue 项目一贯的取向 —— 写坏的约束应当**立刻**
/// 报错，而不是让用户拿着一份语义不明的契约去猜为什么数据通不过。
fn group_members(name: &str, v: &Value) -> Result<Vec<String>, String> {
    let Value::Array(items) = v else {
        return Err(format!(
            "`{name}` 的取值须是字段名数组（如 `[ a b ]`），实为 {}",
            crate::value_kind(v)
        ));
    };
    let mut out = Vec::with_capacity(items.len());
    for it in items {
        match it {
            Value::Str(s) => {
                if s.is_empty() {
                    return Err(format!("`{name}` 的组成员不可为空字段名的占位"));
                }
                if out.iter().any(|x| x == s) {
                    return Err(format!(
                        "`{name}` 的组成员 `{s}` 重复；重复多半是笔误，且会让计数含义含混"
                    ));
                }
                out.push(s.clone());
            }
            other => {
                return Err(format!(
                    "`{name}` 的组成员须是字段名（裸词或引号串），实为 {}",
                    crate::value_kind(other)
                ))
            }
        }
    }
    if out.len() < 2 {
        return Err(format!(
            "`{name}` 的组至少要有两个成员，实得 {} 个",
            out.len()
        ));
    }
    Ok(out)
}

/// 取出**组成员清单**（已校验成员都在本契约里声明过），供三个计数类约束共用。
fn resolve_members(name: &str, view: &GroupView<'_>) -> Result<Vec<String>, String> {
    let v = view
        .spec
        .ext_data
        .get(name)
        .ok_or_else(|| format!("字段 `{}` 上登记的 `{name}` 取值丢失", view.owner))?;
    let members = group_members(name, v)?;
    // 成员必须在**同一份契约**里声明过：写错成员名就等同配了一条永远命中不了的
    // 约束（数据安全错觉），必须响亮报错。
    for m in &members {
        if !view.declared.contains_key(m) {
            return Err(format!(
                "组成员 `{}` 未在契约 `{}` 里声明（是否拼错了字段名？）",
                view.field_path(m),
                view.contract
            ));
        }
    }
    Ok(members)
}

/// 组里被用户**显式写出**的成员（完整路径形式）。
fn present_members(view: &GroupView<'_>, members: &[String]) -> Vec<String> {
    members
        .iter()
        .filter(|m| view.is_present(m))
        .map(|m| view.field_path(m))
        .collect()
}

/// 互斥组的判定本体：`one_of` / `exactly_one` 共用。
///
/// `require_one`：true = 恰好一个（缺了也报错），false = 至多一个（缺了放行）。
fn check_mutual_exclusion(name: &str, view: &GroupView<'_>, require_one: bool) -> Result<(), String> {
    let members = resolve_members(name, view)?;
    let hit = present_members(view, &members);
    let list = members.join(" ");
    let rule = if require_one { "须恰好一个" } else { "至多一个" };
    match hit.len() {
        // 0 个：`one_of`（至多一个）放行，`exactly_one` 报错
        0 if !require_one => Ok(()),
        0 => Err(format!(
            "互斥组 [ {list} ] 里一个字段都没出现（组内{rule}）"
        )),
        1 => Ok(()),
        n => Err(format!(
            "互斥组 [ {list} ] 里同时出现了 {n} 个字段：{}（组内{rule}）",
            hit.join("、")
        )),
    }
}

/// 互斥修饰符 **`one_of [ a b c ]`**：组内**至多一个**字段可出现。
///
/// - 出现 ≥2 个 ⇒ 报错（码 `E-CONTRACT-011`，与其余外置修饰符同一码）；
/// - 恰好 1 个 ⇒ 通过；
/// - **一个都不出现 ⇒ 通过** —— 「必须填一个」请另用 `required` 或 [`ExactlyOne`]。
///   理由：缺失与否已经由 `required` / `optional` / `default` 三态负责，
///   再让 `one_of` 兼管「必须有一个」会让两个语义互相打架
///   （组里写两个 `required` 就是必然冲突）。各管一段，判据才好背。
#[derive(Debug)]
pub struct OneOf;

impl Modifier for OneOf {
    fn name(&self) -> &str {
        "one_of"
    }
    fn apply(&self, _spec: &mut FieldSpec, v: &Value) -> Result<(), String> {
        group_members(self.name(), v).map(|_| ())
    }
    fn check_group(&self, view: &GroupView<'_>) -> Result<(), String> {
        check_mutual_exclusion(self.name(), view, false)
    }
}

/// 互斥修饰符 **`exactly_one [ a b c ]`**：组内**恰好一个**字段须出现
/// （0 个或 ≥2 个都报错）。它是 [`OneOf`] 的「必填版」。
///
/// ⚠️ 陷阱：因为「出现」= **用户显式写出**、`default` 回填不算，
/// 给组内每个成员都写 `default` 会让本修饰符**恒失败**。想要「不写也给个兜底值」，
/// 请只给其中一个成员写 `default`，其余留作可选。
#[derive(Debug)]
pub struct ExactlyOne;

impl Modifier for ExactlyOne {
    fn name(&self) -> &str {
        "exactly_one"
    }
    fn apply(&self, _spec: &mut FieldSpec, v: &Value) -> Result<(), String> {
        group_members(self.name(), v).map(|_| ())
    }
    fn check_group(&self, view: &GroupView<'_>) -> Result<(), String> {
        check_mutual_exclusion(self.name(), view, true)
    }
}

/// 计数修饰符 **`any_of [ a b c ]`**：组内**至少一个**字段须出现（同时出现多个 ⇒ 放行）。
///
/// 它与 [`OneOf`]（至多一个）、[`ExactlyOne`]（恰好一个）凑成完整三件套 ——
/// **至多一 / 恰好一 / 至少一**，三条各自独立、不互相包含：
///
/// | 修饰符 | 0 个 | 1 个 | ≥2 个 |
/// |---|---|---|---|
/// | `one_of` | 通过 | 通过 | 报错 |
/// | `exactly_one` | 报错 | 通过 | 报错 |
/// | `any_of` | 报错 | 通过 | 通过 |
///
/// ⚠️ 与 `default` 的交互：判定看用户**显式写出**的键，`default` 回填不算
/// （理由见 [`GroupView::present`]）⇒ 组内成员**全都只靠 `default`** 时，
/// 本修饰符仍算「一个都没有」并报错。这是**刻意**的 —— 作者给的兜底策略不是
/// 「用户确实填了一种联系方式」的意图表达；若只为了给通过后的数据兜底，
/// 照旧写 `default` 即可（它不影响本约束的判定结果）。
#[derive(Debug)]
pub struct AnyOf;

impl Modifier for AnyOf {
    fn name(&self) -> &str {
        "any_of"
    }
    fn apply(&self, _spec: &mut FieldSpec, v: &Value) -> Result<(), String> {
        group_members(self.name(), v).map(|_| ())
    }
    fn check_group(&self, view: &GroupView<'_>) -> Result<(), String> {
        check_at_least_one(self.name(), view)
    }
}

/// 必选组的判定本体：组内一个都没被显式写出就算失败。
fn check_at_least_one(name: &str, view: &GroupView<'_>) -> Result<(), String> {
    let members = resolve_members(name, view)?;
    let hit = present_members(view, &members);
    if hit.is_empty() {
        return Err(format!(
            "必选组 [ {} ] 里至少要出现一个字段，实际一个都没写（注意：`default` 回填不算用户写出）",
            members.join(" ")
        ));
    }
    Ok(())
}

/// 依赖修饰符 **`depends_on b`**（或 `depends_on [ b c ]`）：**本字段出现 ⇒ 所列字段必须出现**。
///
/// 写法上挂在**依赖方**字段上，语义表（`a` = 挂本修饰符的字段，`b` = 依赖目标）：
///
/// | `a` | `b` | 结果 |
/// |---|---|---|
/// | 未出现 | 任意 | **通过** —— 本条不管「依赖目标怎么单独出现」 |
/// | 出现 | 出现 | 通过 |
/// | 出现 | 未出现 | **报错** |
///
/// ⚠️ **与块级指令 `@requires` 毫无关系**，两者刻意不共用名字：
/// `@requires` 是**特性门控**（应用契约前必须已开启某个 `@feature`，契约体里的声明），
/// 而 `depends_on` 是**字段与字段之间的陪伴要求**（契约**字段**的修饰符）。
/// 用同一个词会让读者分不清 `X` 到底是特性名还是字段名，故另起一名。
///
/// ⚠️ 依赖目标必须是**同一份契约**里声明过的字段名，且不可是字段自身
/// （自依赖恒成立，多半是把字段名抄到了自己身上）—— 两种情形都响亮报错。
#[derive(Debug)]
pub struct DependsOn;

impl Modifier for DependsOn {
    fn name(&self) -> &str {
        "depends_on"
    }
    fn apply(&self, _spec: &mut FieldSpec, v: &Value) -> Result<(), String> {
        dependency_targets(self.name(), v).map(|_| ())
    }
    fn check_group(&self, view: &GroupView<'_>) -> Result<(), String> {
        check_dependency(self.name(), view)
    }
}

/// 取出并校验**依赖目标清单**：单个字段名（`[b]` 的便捷写法就写作 `b`）或字段名数组。
///
/// 与 [`group_members`] 同族的三重检查秉承同一条取向 —— 写坏的约束应当**立刻**报错。
fn dependency_targets(name: &str, v: &Value) -> Result<Vec<String>, String> {
    let raw: Vec<&Value> = match v {
        Value::Str(_) => vec![v],
        Value::Array(items) if !items.is_empty() => items.iter().collect(),
        Value::Array(_) => return Err(format!("`{name}` 的依赖目标至少要有 1 个，实得 0 个")),
        other => {
            return Err(format!(
                "`{name}` 的依赖目标须是字段名（裸词或引号串）或其数组（如 `[ b c ]`），实为 {}",
                crate::value_kind(other)
            ))
        }
    };
    let mut out = Vec::with_capacity(raw.len());
    for it in raw {
        match it {
            Value::Str(s) => {
                if s.is_empty() {
                    return Err(format!("`{name}` 的依赖目标不可为空字段名的占位"));
                }
                if out.iter().any(|x| x == s) {
                    return Err(format!(
                        "`{name}` 的依赖目标 `{s}` 重复；重复多半是笔误"
                    ));
                }
                out.push(s.clone());
            }
            other => {
                return Err(format!(
                    "`{name}` 的依赖目标须是字段名（裸词或引号串），实为 {}",
                    crate::value_kind(other)
                ))
            }
        }
    }
    Ok(out)
}

/// 跨字段依赖的判定本体。
fn check_dependency(name: &str, view: &GroupView<'_>) -> Result<(), String> {
    let v = view
        .spec
        .ext_data
        .get(name)
        .ok_or_else(|| format!("字段 `{}` 上登记的 `{name}` 取值丢失", view.owner))?;
    let targets = dependency_targets(name, v)?;
    // 自依赖恒成立 —— 写了多半是笔误，留着它只会让人误以为约束生效了。
    if targets.iter().any(|t| t.as_str() == view.owner) {
        return Err(format!(
            "`{name}` 的依赖目标不可是本字段自身 `{}`",
            view.field_path(view.owner)
        ));
    }
    for t in &targets {
        if !view.declared.contains_key(t) {
            return Err(format!(
                "依赖目标 `{}` 未在契约 `{}` 里声明（是否拼错了字段名？）",
                view.field_path(t),
                view.contract
            ));
        }
    }
    // 依赖方（挂本修饰符的字段）没出现 ⇒ 本条不管：它缺席与否由
    // `required` / `optional` / `default` 三态负责，两个语义不互相代劳。
    if !view.is_present(view.owner) {
        return Ok(());
    }
    let missing: Vec<String> = targets
        .iter()
        .filter(|t| !view.is_present(t.as_str()))
        .map(|t| view.field_path(t))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "字段 `{}` 出现了，但它依赖的 {} 未出现",
            view.field_path(view.owner),
            missing.join("、")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 只接受字符串的外置类型，用于验证注册与查表。
    #[derive(Debug)]
    struct StrOnly;

    impl TypeCheck for StrOnly {
        fn name(&self) -> &str {
            "image"
        }
        fn check(&self, v: &Value) -> Result<(), String> {
            match v {
                Value::Str(_) => Ok(()),
                other => Err(format!("应为字符串，得 {other:?}")),
            }
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut ext = ContractExt::new();
        ext.register_type(StrOnly).unwrap();
        assert_eq!(ext.len(), 1);
        let t = ext.type_arc("image").expect("应能查到");
        t.check(&Value::Str("x".into())).unwrap();
        assert!(t.check(&Value::Int(1)).is_err());
        assert!(ext.type_arc("nope").is_none());
    }

    #[test]
    fn builtin_type_names_rejected() {
        for n in ["str", "int", "num", "bool", "any", "enum"] {
            let mut ext = ContractExt::new();
            // 借用名字构造一个同名类型：内置名一律拒绝
            #[derive(Debug)]
            struct Fake(&'static str);
            impl TypeCheck for Fake {
                fn name(&self) -> &str {
                    self.0
                }
                fn check(&self, _v: &Value) -> Result<(), String> {
                    Ok(())
                }
            }
            assert!(
                ext.register_type(Fake(n)).is_err(),
                "`{n}` 是内置类型，注册应失败"
            );
        }
    }

    #[test]
    fn duplicate_registration_errors() {
        let mut ext = ContractExt::new();
        ext.register_type(StrOnly).unwrap();
        assert!(ext.register_type(StrOnly).is_err());
    }
}
