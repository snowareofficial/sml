// SPDX-License-Identifier: MulanPSL-2.0
//! 外置扩展点：让下游注册自定义 `@指令`，**无需修改本 crate 源码**。
//!
//! ## 动机
//!
//! SML 是通用数据格式，但下游常有「给文档挂带类型的元数据块，且**不进主数据树**」
//! 的需求（表单描述 / 处理流状态机 / 文章块等）。缺少扩展点时，下游只能 fork 出
//! 自己的子集解析器 —— 于是方言诞生、两端文档互不相通（实测：某下游方言 的
//! `@form Name { }` 在本解析器里会被判为「缺少片段体」而整篇失败）。
//!
//! 本模块把这条口子补上：**方言定义留在下游仓库**，SML 核心与规范层保持通用。
//!
//! ## 设计约束
//!
//! - **未注册任何扩展时，行为与既有解析完全一致**（零扩展 = 零影响，向后兼容）。
//! - **内置指令名不可被覆盖**（注册时报错），避免方言改写核心语义。
//! - 位置参数形式（`@xxx Name { }`）自 v4 起废弃：外置指令须显式开启
//!   [`Directive::positional`]，且命中时产出一条 [`DiagnosticKind::Deprecated`]。
//!   之所以保留，是为了让既有方言文档能先跑起来再迁移。
//! - **键冲突告警**（雷 5 · A 组 4-5）：同一块内键名重复 ⇒ 提升为数组；片段展开
//!   （`&name`）覆盖父块已有的同名键 ⇒ 父块原值丢失。二者此前**完全静默**，现产出
//!   [`DiagnosticKind::DuplicateKey`] / [`DiagnosticKind::KeyOverwritten`]。
//!   ⚠️ **语义一律不变** —— 告警只说明「数据被改写 / 覆盖过」，`Value` 与旧版
//!   逐字一致。告警只经 `*_with_warnings` 入口交出（[`crate::parse_with_warnings`]），
//!   `parse()` 恒不返回；开关见 [`ParseOptions::warn_key_conflict`]。
//! - 扩展只影响**解析期**；它不改变词法，也不能引入新的 token。

use std::collections::BTreeMap;
use std::sync::Arc;

use sml_contract::ContractExt;
use sml_value::Value;

/// 内置指令名。供错误消息列出，并**禁止外置扩展占用**。
///
/// 新增内置指令时必须同步此表，否则同名外置指令会拦截掉内置语义。
pub const BUILTIN_DIRECTIVES: &[&str] = &[
    "contract", "is", "type", "version", "feature", "when", "for",
];

/// 外置指令的处理结果去向。
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// **元数据块**：不进主数据树。
    /// `@contract` / `@form` / `@policy` / `@flow` 都是这个用法 ——
    /// 文档里写了，但解析结果里不该出现。
    Discard,
    /// **展开进主数据树**：值必须是对象，其字段合并进指令所在的块。
    /// 用于「指令生成字段」的场景（如文章块展开成 `blocks: [...]`）。
    Emit(Value),
}

/// 非致命诊断。致命问题仍然走 `Err(String)`，避免把警告当错误打断解析。
///
/// ⚠️ `line` 是后加的字段：对**只读取**诊断的代码无破坏，只有在本 crate 之外
/// 用结构体字面量构造 `Diagnostic` 才会缺字段（与 `ParseOutput` 加
/// `contract_map` 时同一判据，已记入 CHANGELOG「升级注意」）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    /// 触发位置的行号（**1 起**）。拿不到时为 `None`
    /// —— 文件入口的 token 流经 include 展开、跨文件合并，**没有可映射的单一原文**
    /// （宁缺勿错，与 [`crate::api`] 的错误定位同一取舍）。
    pub line: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// 仍被接受，但已不推荐的写法（如位置参数形式）。
    Deprecated,
    /// 同一块内键名重复 ⇒ **提升为数组**（SML 既定语义：`k: 1` + `k: 2` ⇒ `k: [1, 2]`）。
    ///
    /// 值没丢，但**结构被改写**：下游按 `k` 是标量写的代码会拿到数组。
    /// 这是本仓库「静默改数据」清单 A 组第 4 项 —— 语义保留、改为告警。
    DuplicateKey,
    /// 片段展开（`&name`）覆盖了**父块已有的同名键** ⇒ 父块那个值**真的丢了**。
    ///
    /// 与 [`Self::DuplicateKey`] 的关键差别：片段合并走直接赋值，**不走**数组提升
    /// （ch03/ch08 写明的既定语义），所以这是丢值而不是改结构，危险度更高。
    KeyOverwritten,
}

/// 外置 `@` 指令。
///
/// 实现者只需回答三件事：叫什么、要不要位置参数名、拿到块之后怎么办。
/// 解析器负责「找到指令 → 取参数 → 读块 → 收集诊断」，hook 只处理语义。
pub trait Directive: Send + Sync {
    /// 指令名（**不含** `@`），如 `"form"`。
    fn name(&self) -> &str;

    /// 是否接受位置参数名（`@xxx Name { }`）。
    ///
    /// 默认 `false`：即推荐写法 `@xxx name: Name { }`。
    /// 置 `true` 会启用 v4 已废弃的位置参数形式，并产出弃用诊断 ——
    /// 这是为既有方言文档留的迁移期通道，**新方言不要开**。
    fn positional(&self) -> bool {
        false
    }

    /// 处理指令体。
    ///
    /// - `arg`：参数名（位置参数形式或 `name: X` 显式形式）；两者都没有则为 `None`
    /// - `body`：`{ ... }` 块解析出的值；指令没带块时为 [`Value::Null`]
    fn call(&self, arg: Option<&str>, body: Value) -> Result<Outcome, String>;
}

/// 外置指令注册表。
///
/// 用 [`Arc`] 包 trait 对象，使 `Parser` 的 `@for` 子解析器能低成本共享同一张表
/// （`spawn_for_child` 会克隆各字段，trait 对象本身不可克隆）。
pub struct DirectiveTable {
    items: BTreeMap<String, Arc<dyn Directive>>,
}

impl Default for DirectiveTable {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for DirectiveTable {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
        }
    }
}

impl DirectiveTable {
    pub fn new() -> Self {
        Self {
            items: BTreeMap::new(),
        }
    }

    /// 注册一条外置指令。**重名一律报错**，不静默覆盖：
    /// 与内置同名会改写核心语义，与已注册同名多半是重复注册的手误。
    pub fn register(&mut self, d: impl Directive + 'static) -> Result<(), String> {
        let name = d.name().to_string();
        if BUILTIN_DIRECTIVES.contains(&name.as_str()) {
            return Err(format!(
                "sml: 不可注册与内置指令同名的扩展指令 `@{name}`（内置：{}）",
                BUILTIN_DIRECTIVES.join(" / ")
            ));
        }
        if self.items.contains_key(&name) {
            return Err(format!("sml: 扩展指令 `@{name}` 已注册"));
        }
        self.items.insert(name, Arc::new(d));
        Ok(())
    }

    /// 按名查找（不含 `@`）。
    pub fn get(&self, name: &str) -> Option<&(dyn Directive + 'static)> {
        self.items.get(name).map(|d| d.as_ref())
    }

    /// 按名取出一份 **Arc 克隆**（不含 `@`）。
    ///
    /// 解析器需要先释放对 `self.directives` 的借用、再可变借用 `self` 去读指令体，
    /// 故这里返回拥有所有权的 `Arc` 而非引用；`Arc` 克隆是引用计数，成本可忽略。
    pub fn get_arc(&self, name: &str) -> Option<Arc<dyn Directive + 'static>> {
        self.items.get(name).cloned()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 已注册的名字（按字典序）。
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.items.keys().map(|s| s.as_str())
    }
}

/// 解析期可选项。**全部为空时与既有解析行为完全一致。**
///
/// ⚠️ [`Self::warn_key_conflict`] 默认 `true`，且它是**唯一**一个默认非假的开关：
/// 它只影响 `diagnostics` 的内容，不影响 `Value`，故不算「改变既有行为」。
#[derive(Clone)]
pub struct ParseOptions {
    /// 外置 `@` 指令表
    pub directives: DirectiveTable,
    /// 外置契约类型表（`image` / `link` / `time` 这类领域类型）。
    /// 见 [`sml_contract::ext`]。
    pub contract_ext: ContractExt,
    /// 是否记录**契约映射**（哪个实体应用了哪些契约）。
    ///
    /// ⚠️ 默认 `false`：不开启时 `ParseOutput` 与旧版**逐字一致**
    /// （`contract_map` 恒为 `None`，解析路径零额外开销）。
    /// 开启后见 [`crate::contract_map::ContractMap`]。
    pub(crate) trace_contracts: bool,
    /// 是否收集**键冲突**告警（[`DiagnosticKind::DuplicateKey`] /
    /// [`DiagnosticKind::KeyOverwritten`]）。
    ///
    /// 默认 `true`：调用方既然选了 `*_with_warnings` 入口，本意就是要看见告警。
    /// 置 `false` 可关掉（如文档刻意用重复键构造数组）。
    ///
    /// ⚠️ 本开关**只影响 `diagnostics` 的内容**，不改变 `Value`；且
    /// `parse()` / `parse_file()` 这些不带 warnings 的入口恒不返回诊断
    /// （它们在 `api.rs` 里就丢掉了）⇒ 对既有调用方零影响。
    pub warn_key_conflict: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            directives: DirectiveTable::new(),
            contract_ext: ContractExt::new(),
            trace_contracts: false,
            warn_key_conflict: true,
        }
    }
}

impl ParseOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否收集键冲突告警（链式）。默认 `true`。
    pub fn warn_key_conflict(mut self, on: bool) -> Self {
        self.warn_key_conflict = on;
        self
    }

    /// 开启契约映射记录（链式）。
    ///
    /// ```ignore
    /// let out = sml_parse::parse_with(text, ParseOptions::new().trace_contracts(true))?;
    /// out.contract_map.unwrap().names_at(&[PathSeg::Key("db".into())]);
    /// ```
    pub fn trace_contracts(mut self, on: bool) -> Self {
        self.trace_contracts = on;
        self
    }

    /// 链式注册一条外置指令（注册失败返回 Err，便于 `?` 展开）。
    pub fn directive(mut self, d: impl Directive + 'static) -> Result<Self, String> {
        self.directives.register(d)?;
        Ok(self)
    }

    /// 链式注册一个外置契约类型。
    pub fn with_type(mut self, t: impl sml_contract::TypeCheck + 'static) -> Result<Self, String> {
        self.contract_ext.register_type(t)?;
        Ok(self)
    }

    /// 链式注册一个外置字段修饰符（如 `items_max`）。
    pub fn with_modifier(
        mut self,
        m: impl sml_contract::Modifier + 'static,
    ) -> Result<Self, String> {
        self.contract_ext.register_modifier(m)?;
        Ok(self)
    }

    /// 是否携带任何扩展（用于快速跳过扩展分支，保证零扩展零开销）。
    pub fn has_ext(&self) -> bool {
        !self.directives.is_empty() || !self.contract_ext.is_empty()
    }
}

/// 带扩展的解析输出：值 + 非致命诊断。
///
/// ⚠️ 这是**公开结构体加字段**（`contract_map`）：对只读取的代码无破坏，
/// 只有结构体字面量构造会缺字段 —— 已记入 CHANGELOG「升级注意」。
#[derive(Debug, Clone)]
pub struct ParseOutput {
    pub value: Value,
    /// 弃用提示等。为空表示文档未触发任何警告。
    pub diagnostics: Vec<Diagnostic>,
    /// 契约映射（哪个实体应用了哪些契约）。
    ///
    /// **仅当 [`ParseOptions::trace_contracts`] 开启时为 `Some`**；
    /// 否则恒为 `None` —— 这是「不开启时逐字一致」的钉子
    /// （`rust/tests/contract_map.rs` 有显式断言）。
    pub contract_map: Option<crate::contract_map::ContractMap>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 元数据块指令：照单全收、不进主树。等价于 某下游方言 的 @form/@policy/@flow 用法。
    struct Meta(&'static str);

    impl Directive for Meta {
        fn name(&self) -> &str {
            self.0
        }
        fn positional(&self) -> bool {
            true // 兼容既有方言文档的位置参数写法
        }
        fn call(&self, _arg: Option<&str>, _body: Value) -> Result<Outcome, String> {
            Ok(Outcome::Discard)
        }
    }

    fn opts_for(names: &[&'static str]) -> ParseOptions {
        let mut o = ParseOptions::new();
        for n in names {
            o.directives.register(Meta(n)).unwrap();
        }
        o
    }

    /// 方言三指令（某下游方言 实际写法）应被收编：文档可解析，且不进主数据树。
    #[test]
    fn dialect_directives_are_discarded() {
        let src = concat!(
            "title: hi\n",
            "@form BugReport { title: { label: \"标题\" } }\n",
            "@policy BugReport { anon: allow }\n",
            "@flow BugReport { states: [ 已提交 已受理 ] init: 已提交 }\n",
        );
        let out = crate::parse_with(src, opts_for(&["form", "policy", "flow"])).unwrap();
        let Value::Object(m) = &out.value else {
            panic!("顶层应为对象，得 {out:?}");
        };
        assert_eq!(m.get("title").map(|v| v.as_str()), Some(Some("hi")));
        for k in ["form", "policy", "flow", "BugReport"] {
            assert!(m.get(k).is_none(), "`{k}` 不该出现在主数据树里");
        }
    }

    /// 未注册任何扩展时，`@form Name { }` 仍按既有行为报错（零扩展 = 零影响）。
    #[test]
    fn unregistered_directive_still_errors() {
        let r = crate::parse_with("@form F { a: 1 }\n", ParseOptions::new());
        assert!(r.is_err(), "未注册时不应被静默接受");
    }

    /// 位置参数形式被接受，但要产出一条弃用诊断。
    #[test]
    fn positional_form_emits_deprecated() {
        let out = crate::parse_with("@form F { a: 1 }\n", opts_for(&["form"])).unwrap();
        assert_eq!(out.diagnostics.len(), 1);
        assert_eq!(out.diagnostics[0].kind, DiagnosticKind::Deprecated);
        assert!(out.diagnostics[0].message.contains("name: F"));
    }

    /// 推荐写法 `@xxx name: X { }` 不产生诊断。
    #[test]
    fn explicit_name_arg_is_clean() {
        let out = crate::parse_with("@form name: F { a: 1 }\n", opts_for(&["form"])).unwrap();
        assert!(
            out.diagnostics.is_empty(),
            "显式参数不该报警：{:?}",
            out.diagnostics
        );
    }

    /// 内置指令名不可被外置扩展占用（否则会改写核心语义）。
    #[test]
    fn builtin_names_cannot_be_overridden() {
        for n in ["contract", "is", "version", "feature", "type"] {
            assert!(
                ParseOptions::new().directive(Meta(n)).is_err(),
                "`@{n}` 是内置指令，注册应失败"
            );
        }
    }

    /// 同名重复注册应报错，不静默覆盖。
    #[test]
    fn duplicate_registration_errors() {
        let mut o = ParseOptions::new();
        o.directives.register(Meta("form")).unwrap();
        assert!(o.directives.register(Meta("form")).is_err());
    }

    /// 外置契约类型：注册后能在契约里直接用，且校验真的生效。
    ///
    /// 这是 某下游方言 的 `images: [image]` 用例 —— 类型名与规则都留在下游，
    /// SML 规范层不需要知道「image 是什么」。
    #[derive(Debug)]
    struct Image;

    impl sml_contract::TypeCheck for Image {
        fn name(&self) -> &str {
            "image"
        }
        fn check(&self, v: &Value) -> Result<(), String> {
            match v {
                Value::Str(s) if s.starts_with("ev-") => Ok(()),
                _ => Err("须是以 ev- 开头的证据 id".into()),
            }
        }
    }

    const CONTRACT: &str = "@contract F { images: [image] }\n";

    #[test]
    fn ext_type_accepts_valid_value() {
        let src = format!("{CONTRACT}doc {{\n @is F\n images: [ \"ev-1\" \"ev-2\" ]\n}}\n");
        let out = crate::parse_with(&src, ParseOptions::new().with_type(Image).unwrap()).unwrap();
        assert!(out.diagnostics.is_empty());
    }

    #[test]
    fn ext_type_rejects_invalid_value() {
        let src = format!("{CONTRACT}doc {{\n @is F\n images: [ \"http://x\" ]\n}}\n");
        let r = crate::parse_with(&src, ParseOptions::new().with_type(Image).unwrap());
        let e = r.expect_err("非法证据 id 应被外置类型拦下");
        assert!(e.message().contains("ev-"), "错误信息应带上外置类型的理由：{e}");
        assert!(e.message().contains("images[0]"), "错误信息应带上字段路径：{e}");
    }

    /// 未注册 `image` 时，它仍是「未知数组元素类型」—— 零扩展 = 零影响。
    #[test]
    fn unregistered_ext_type_still_errors() {
        let src = format!("{CONTRACT}doc {{\n @is F\n images: [ \"ev-1\" ]\n}}\n");
        assert!(crate::parse_with(&src, ParseOptions::new()).is_err());
    }

    /// 外置修饰符 `items_max`：数组元素**个数**上限。
    ///
    /// 刻意**不复用 `max`** —— 那在 SML 契约里是数值上界语义，
    /// 而数组根本不参与 min/max 校验；复用会让同一写法在两端含义不同。
    #[derive(Debug)]
    struct ItemsMax;

    impl sml_contract::Modifier for ItemsMax {
        fn name(&self) -> &str {
            "items_max"
        }
        fn apply(&self, _spec: &mut sml_contract::FieldSpec, v: &Value) -> Result<(), String> {
            match v {
                Value::Int(n) if *n >= 0 => Ok(()),
                _ => Err("须为非负整数".into()),
            }
        }
        fn check(&self, spec: &sml_contract::FieldSpec, v: &Value) -> Result<(), String> {
            let limit = match spec.ext_data.get("items_max") {
                Some(Value::Int(n)) => *n,
                _ => return Ok(()),
            };
            match v {
                Value::Array(items) if items.len() as i64 > limit => {
                    Err(format!("最多 {limit} 项，实得 {} 项", items.len()))
                }
                _ => Ok(()),
            }
        }
    }

    const MOD_CONTRACT: &str = "@contract F { tags: [str] items_max 2 }\n";

    #[test]
    fn ext_modifier_limits_array_len() {
        let ok = format!("{MOD_CONTRACT}doc {{\n @is F\n tags: [ a b ]\n}}\n");
        crate::parse_with(&ok, ParseOptions::new().with_modifier(ItemsMax).unwrap()).unwrap();

        let too_many = format!("{MOD_CONTRACT}doc {{\n @is F\n tags: [ a b c ]\n}}\n");
        let e = crate::parse_with(
            &too_many,
            ParseOptions::new().with_modifier(ItemsMax).unwrap(),
        )
        .expect_err("超量应被外置修饰符拦下");
        assert!(e.message().contains("items_max") && e.message().contains("最多 2 项"), "{e}");
    }

    /// 未注册时 `items_max` 仍是非法写法 —— 零扩展 = 零影响。
    #[test]
    fn unregistered_modifier_still_errors() {
        let src = format!("{MOD_CONTRACT}doc {{\n @is F\n tags: [ a ]\n}}\n");
        assert!(crate::parse_with(&src, ParseOptions::new()).is_err());
    }

    /// 内置修饰符名不可被外置扩展占用。
    #[test]
    fn builtin_modifier_names_rejected() {
        let mut ext = sml_contract::ContractExt::new();
        #[derive(Debug)]
        struct Fake(&'static str);
        impl sml_contract::Modifier for Fake {
            fn name(&self) -> &str {
                self.0
            }
            fn apply(
                &self,
                _s: &mut sml_contract::FieldSpec,
                _v: &Value,
            ) -> Result<(), String> {
                Ok(())
            }
        }
        for n in ["optional", "required", "default", "min", "max"] {
            assert!(
                ext.register_modifier(Fake(n)).is_err(),
                "`{n}` 是内置修饰符，注册应失败"
            );
        }
    }

    // ---- I-7 缺口①：互斥字段组 `one_of` / `exactly_one` ----

    const PAY_CONTRACT: &str = "@contract Pay {\n  card: str optional one_of [ card alipay wechat ]\n  alipay: str optional\n  wechat: str optional\n}\n";

    fn one_of_opts() -> ParseOptions {
        ParseOptions::new().with_modifier(sml_contract::OneOf).unwrap()
    }

    fn exactly_one_opts() -> ParseOptions {
        ParseOptions::new()
            .with_modifier(sml_contract::ExactlyOne)
            .unwrap()
    }

    /// `one_of`（至多一个）：一个都不写 ⇒ 放行；只写一个 ⇒ 放行；写两个 ⇒ 报错。
    #[test]
    fn one_of_allows_at_most_one() {
        let none = format!("{PAY_CONTRACT}doc {{\n @is Pay\n}}\n");
        crate::parse_with(&none, one_of_opts()).expect("一个都不写应放行");

        let one = format!("{PAY_CONTRACT}doc {{\n @is Pay\n card: \"x\"\n}}\n");
        crate::parse_with(&one, one_of_opts()).expect("只写一个应放行");

        let two = format!("{PAY_CONTRACT}doc {{\n @is Pay\n card: \"x\"\n alipay: \"y\"\n}}\n");
        let e = crate::parse_with(&two, one_of_opts()).expect_err("两个同时出现应报错");
        assert!(
            e.message().contains("one_of") && e.message().contains("同时出现"),
            "{e}"
        );
    }

    const EXACTLY_ONE_CONTRACT: &str = "@contract Pay {\n  card: str optional exactly_one [ card alipay wechat ]\n  alipay: str optional\n  wechat: str optional\n}\n";

    /// `exactly_one`（恰好一个）：一个都不写 ⇒ 报错；写一个 ⇒ 放行。
    #[test]
    fn exactly_one_requires_one() {
        let none = format!("{EXACTLY_ONE_CONTRACT}doc {{\n @is Pay\n}}\n");
        let e = crate::parse_with(&none, exactly_one_opts()).expect_err("一个都没写应报错");
        assert!(e.message().contains("exactly_one"), "{e}");

        let one = format!("{EXACTLY_ONE_CONTRACT}doc {{\n @is Pay\n wechat: \"z\"\n}}\n");
        crate::parse_with(&one, exactly_one_opts()).expect("恰好一个应放行");
    }

    /// 与 `loose` 叠加：`loose` 只放宽「未声明字段」，**不**放宽互斥。
    #[test]
    fn one_of_still_applies_under_loose() {
        let two = format!(
            "{PAY_CONTRACT}doc {{\n @is Pay loose\n card: \"x\"\n alipay: \"y\"\n}}\n"
        );
        assert!(
            crate::parse_with(&two, one_of_opts()).is_err(),
            "loose 不应放过互斥冲突"
        );
    }

    /// `default` 回填**不算**用户写出 —— 否则「组内成员都写 default」的契约会恒失败。
    #[test]
    fn default_backfill_does_not_count_as_present() {
        const DEF: &str = "@contract Pay {\n  card: str optional one_of [ card alipay ]\n  alipay: str optional default \"a\"\n}\n";
        let src = format!("{DEF}doc {{\n @is Pay\n card: \"x\"\n}}\n");
        crate::parse_with(&src, one_of_opts()).expect("alipay 的 default 不应被算成同时出现");
    }

    /// 组成员名写错（未在契约里声明）⇒ 响亮报错，而不是留下一条永远命中不了的约束。
    #[test]
    fn one_of_typoed_member_is_loud() {
        const BAD: &str = "@contract Pay {\n  card: str optional one_of [ card alipay cardd ]\n  alipay: str optional\n}\n";
        let src = format!("{BAD}doc {{\n @is Pay\n card: \"x\"\n}}\n");
        let e = crate::parse_with(&src, one_of_opts()).expect_err("成员名写错应报错");
        assert!(e.message().contains("cardd"), "{e}");
    }

    // ---- I-7 缺口②上半：数组元素**个数下限** `items_min` ----

    const MIN_CONTRACT: &str = "@contract R { tags: [str] items_min 2 }\n";

    fn items_min_opts() -> ParseOptions {
        ParseOptions::new()
            .with_modifier(sml_contract::ItemsMin)
            .unwrap()
    }

    /// 边界：恰好 n 项 ⇒ 通过；多于 n 项 ⇒ 通过；少于 n 项 ⇒ 报错。
    #[test]
    fn items_min_boundary_is_inclusive() {
        let exact = format!("{MIN_CONTRACT}doc {{\n @is R\n tags: [ a b ]\n}}\n");
        crate::parse_with(&exact, items_min_opts()).expect("恰好 2 项应放行");

        let more = format!("{MIN_CONTRACT}doc {{\n @is R\n tags: [ a b c ]\n}}\n");
        crate::parse_with(&more, items_min_opts()).expect("3 项应放行");

        let few = format!("{MIN_CONTRACT}doc {{\n @is R\n tags: [ a ]\n}}\n");
        let e = crate::parse_with(&few, items_min_opts()).expect_err("不足 2 项应报错");
        assert!(
            e.message().contains("items_min") && e.message().contains("至少 2 项"),
            "{e}"
        );
        assert_eq!(e.code(), "E-CONTRACT-011", "{e}");
    }

    /// 下限边界 0：`items_min 0` 放行空数组；`items_min 1` 拦住空数组。
    #[test]
    fn items_min_zero_boundary() {
        const ZERO: &str = "@contract R { tags: [str] items_min 0 }\n";
        let empty = format!("{ZERO}doc {{\n @is R\n tags: [ ]\n}}\n");
        crate::parse_with(&empty, items_min_opts()).expect("items_min 0 应放行空数组");

        const ONE: &str = "@contract R { tags: [str] items_min 1 }\n";
        let empty = format!("{ONE}doc {{\n @is R\n tags: [ ]\n}}\n");
        let e =
            crate::parse_with(&empty, items_min_opts()).expect_err("空数组应被 items_min 1 拦下");
        assert!(e.message().contains("至少 1 项"), "{e}");

        let one = format!("{ONE}doc {{\n @is R\n tags: [ a ]\n}}\n");
        crate::parse_with(&one, items_min_opts()).expect("1 项应放行");
    }

    /// 取值须为非负整数 —— `apply` 在**契约定义期**就拦下写坏的约束。
    #[test]
    fn items_min_rejects_bad_limit() {
        for bad in ["true", "-1", "nope"] {
            let src = format!(
                "@contract R {{ tags: [str] items_min {bad} }}\ndoc {{\n @is R\n tags: [ a b ]\n}}\n"
            );
            let e = crate::parse_with(&src, items_min_opts()).unwrap_err();
            assert!(
                e.message().contains("非负整数"),
                "取值 `{bad}` 应被拒绝：{e}"
            );
        }
    }

    /// 与 `default` 回填的交互：`items_min` 判定的是字段的**生效值**，
    /// 但当前契约语法里 `default` 只接受裸词 / 引号串 ⇒ 数组字段**写不出**
    /// `default [ ... ]`。把这条钉住，将来支持数组默认值时须回来重订语义。
    #[test]
    fn items_min_cannot_be_triggered_by_default_yet() {
        const DEF: &str = "@contract R { tags: [str] optional default [ a ] items_min 2 }\n";
        let src = format!("{DEF}doc {{\n @is R\n}}\n");
        let e = crate::parse_with(&src, items_min_opts())
            .expect_err("数组默认值当前不可表达，应在契约定义期就报错");
        assert!(
            e.message().contains("default"),
            "期望卡在 `default` 的取值上：{e}"
        );
    }

    /// 未注册 `items_min` 时它仍是非法写法 —— 零扩展 = 零影响。
    #[test]
    fn unregistered_items_min_still_errors() {
        let src = format!("{MIN_CONTRACT}doc {{\n @is R\n tags: [ a b ]\n}}\n");
        assert!(crate::parse_with(&src, ParseOptions::new()).is_err());
    }

    // ---- I-7 缺口②下半：`any_of`（组内**至少一个**）----

    const ANY_CONTRACT: &str = "@contract Contact {\n  email: str optional any_of [ email phone ]\n  phone: str optional\n}\n";

    fn any_of_opts() -> ParseOptions {
        ParseOptions::new().with_modifier(sml_contract::AnyOf).unwrap()
    }

    /// 0 个 ⇒ 报错；1 个 ⇒ 通过；多个 ⇒ 通过（这是它与 `one_of` 的分水岭）。
    #[test]
    fn any_of_requires_at_least_one() {
        let none = format!("{ANY_CONTRACT}doc {{\n @is Contact\n}}\n");
        let e = crate::parse_with(&none, any_of_opts()).expect_err("一个都没写应报错");
        assert!(
            e.message().contains("any_of") && e.message().contains("至少要出现一个"),
            "{e}"
        );
        // 三个新能力全部**复用既有码**，不新增错误码
        assert_eq!(e.code(), "E-CONTRACT-011", "{e}");

        for only in ["email: \"a@b\"", "phone: \"138\""] {
            let src = format!("{ANY_CONTRACT}doc {{\n @is Contact\n {only}\n}}\n");
            crate::parse_with(&src, any_of_opts())
                .unwrap_or_else(|e| panic!("只写一个应放行（{only}）：{e}"));
        }

        let both = format!(
            "{ANY_CONTRACT}doc {{\n @is Contact\n email: \"a@b\"\n phone: \"138\"\n}}\n"
        );
        crate::parse_with(&both, any_of_opts()).expect("多个同时出现应放行");
    }

    /// `default` 回填**不算**用户写出 ⇒ 组内成员全靠 `default` 时仍算「一个都没有」。
    #[test]
    fn any_of_counts_default_as_none() {
        const DEF: &str = "@contract Contact {\n  email: str optional any_of [ email phone ]\n  phone: str optional default \"0\"\n}\n";
        let none = format!("{DEF}doc {{\n @is Contact\n}}\n");
        let e = crate::parse_with(&none, any_of_opts())
            .expect_err("phone 的 default 回填不应让必选组通过");
        assert!(e.message().contains("phone"), "{e}");

        // 显式写出任一个 ⇒ 通过（另一个的 default 照旧回填，但不参与计数）
        let one = format!("{DEF}doc {{\n @is Contact\n email: \"a@b\"\n}}\n");
        crate::parse_with(&one, any_of_opts()).expect("显式写出 email 应放行");
    }

    /// 组成员名写错（未在契约里声明）⇒ 响亮报错。
    #[test]
    fn any_of_typoed_member_is_loud() {
        const BAD: &str = "@contract Contact {\n  email: str optional any_of [ email phonne ]\n  phone: str optional\n}\n";
        let src = format!("{BAD}doc {{\n @is Contact\n email: \"a@b\"\n}}\n");
        let e = crate::parse_with(&src, any_of_opts()).expect_err("成员名写错应报错");
        assert!(e.message().contains("phonne"), "{e}");
    }

    /// 退化的组（取值不是数组 / 只有一个成员）⇒ `apply` 在契约定义期就报错。
    #[test]
    fn any_of_rejects_degenerate_group() {
        for bad in ["email", "[ email ]", "[ email email ]"] {
            let src = format!(
                "@contract Contact {{\n  email: str optional any_of {bad}\n  phone: str optional\n}}\ndoc {{\n @is Contact\n email: \"a@b\"\n}}\n"
            );
            let e = crate::parse_with(&src, any_of_opts()).unwrap_err();
            assert!(
                e.message().contains("any_of"),
                "退化的组取值 `{bad}` 应被拒绝：{e}"
            );
        }
    }

    // ---- I-7 缺口③：跨字段依赖 `depends_on`（与块级 `@requires` 无关）----

    const DEP_CONTRACT: &str = "@contract Deploy {\n  replicas: int optional depends_on autoscaling\n  autoscaling: bool optional\n}\n";

    fn dep_opts() -> ParseOptions {
        ParseOptions::new()
            .with_modifier(sml_contract::DependsOn)
            .unwrap()
    }

    /// 语义表（`replicas` = 依赖方，`autoscaling` = 依赖目标）：
    /// 依赖方未出现 ⇒ 恒通过；出现了而目标缺失 ⇒ 报错。
    #[test]
    fn depends_on_semantics_table() {
        // ─ 依赖方未出现 + 目标未出现 ⇒ 通过
        let src = format!("{DEP_CONTRACT}doc {{\n @is Deploy\n}}\n");
        crate::parse_with(&src, dep_opts()).expect("依赖方未出现应放行");

        // ─ 依赖方未出现 + 目标出现 ⇒ 通过（本条不管「目标怎么单独出现」）
        let src = format!("{DEP_CONTRACT}doc {{\n @is Deploy\n autoscaling: true\n}}\n");
        crate::parse_with(&src, dep_opts()).expect("依赖方未出现时不约束目标");

        // ─ 两者都出现 ⇒ 通过
        let src = format!(
            "{DEP_CONTRACT}doc {{\n @is Deploy\n replicas: 3\n autoscaling: true\n}}\n"
        );
        crate::parse_with(&src, dep_opts()).expect("两者都出现应放行");

        // ─ 依赖方出现 + 目标缺失 ⇒ 报错
        let src = format!("{DEP_CONTRACT}doc {{\n @is Deploy\n replicas: 3\n}}\n");
        let e = crate::parse_with(&src, dep_opts()).expect_err("依赖目标缺失应报错");
        assert!(
            e.message().contains("depends_on") && e.message().contains("autoscaling"),
            "{e}"
        );
        assert_eq!(e.code(), "E-CONTRACT-011", "{e}");
    }

    /// 依赖目标的 `default` 回填不算出现 ⇒ 依赖方写了也照样报错。
    #[test]
    fn depends_on_default_backfill_does_not_count() {
        const DEF: &str = "@contract Deploy {\n  replicas: int optional depends_on autoscaling\n  autoscaling: bool optional default true\n}\n";
        let src = format!("{DEF}doc {{\n @is Deploy\n replicas: 3\n}}\n");
        let e = crate::parse_with(&src, dep_opts())
            .expect_err("被依赖字段只靠 default 回填不算出现");
        assert!(e.message().contains("autoscaling"), "{e}");
    }

    /// 依赖目标可以是一份清单（全部都要出现）。
    #[test]
    fn depends_on_multi_targets() {
        const JOB: &str = "@contract Job {\n  cron: str optional depends_on [ timezone retries ]\n  timezone: str optional\n  retries: int optional\n}\n";
        let all = format!(
            "{JOB}doc {{\n @is Job\n cron: \"0 0 * * *\"\n timezone: UTC\n retries: 3\n}}\n"
        );
        crate::parse_with(&all, dep_opts()).expect("依赖目标都在应放行");

        let partial = format!("{JOB}doc {{\n @is Job\n cron: \"0 0 * * *\"\n timezone: UTC\n}}\n");
        let e = crate::parse_with(&partial, dep_opts()).expect_err("少一个依赖目标应报错");
        assert!(e.message().contains("retries"), "{e}");
    }

    /// 依赖目标名写错（未在契约里声明）⇒ 响亮报错。
    #[test]
    fn depends_on_typoed_target_is_loud() {
        const BAD: &str = "@contract Deploy {\n  replicas: int optional depends_on autoscalng\n  autoscaling: bool optional\n}\n";
        let src = format!("{BAD}doc {{\n @is Deploy\n replicas: 3\n}}\n");
        let e = crate::parse_with(&src, dep_opts()).expect_err("依赖目标拼错应报错");
        assert!(
            e.message().contains("autoscalng"),
            "{e}"
        );
    }

    /// 自依赖恒为真 ⇒ 写了多半是笔误，必须报错而不是放行。
    #[test]
    fn depends_on_self_is_loud() {
        const SELF: &str = "@contract Deploy {\n  replicas: int optional depends_on replicas\n}\n";
        let src = format!("{SELF}doc {{\n @is Deploy\n replicas: 3\n}}\n");
        let e = crate::parse_with(&src, dep_opts()).expect_err("自依赖应报错");
        assert!(
            e.message().contains("不可是本字段自身"),
            "{e}"
        );
    }

    /// 空依赖清单（写了跟没写一样）⇒ 契约定义期就报错。
    #[test]
    fn depends_on_rejects_empty_targets() {
        const EMPTY: &str =
            "@contract Deploy {\n  replicas: int optional depends_on [ ]\n  autoscaling: bool optional\n}\n";
        let src = format!("{EMPTY}doc {{\n @is Deploy\n replicas: 3\n}}\n");
        let e = crate::parse_with(&src, dep_opts()).expect_err("空依赖清单应报错");
        assert!(
            e.message().contains("至少要有 1 个"),
            "{e}"
        );
    }

    /// 未注册 `depends_on` 时它仍是非法写法 —— 零扩展 = 零影响。
    #[test]
    fn unregistered_depends_on_still_errors() {
        let src = format!(
            "{DEP_CONTRACT}doc {{\n @is Deploy\n replicas: 3\n autoscaling: true\n}}\n"
        );
        assert!(crate::parse_with(&src, ParseOptions::new()).is_err());
    }
}
