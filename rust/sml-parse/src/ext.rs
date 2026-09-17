// SPDX-License-Identifier: MulanPSL-2.0
//! 外置扩展点：让下游注册自定义 `@指令`，**无需修改本 crate 源码**。
//!
//! ## 动机
//!
//! SML 是通用数据格式，但下游常有「给文档挂带类型的元数据块，且**不进主数据树**」
//! 的需求（表单描述 / 处理流状态机 / 文章块等）。缺少扩展点时，下游只能 fork 出
//! 自己的子集解析器 —— 于是方言诞生、两端文档互不相通（实测：PVACIS 的
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    /// 仍被接受，但已不推荐的写法（如位置参数形式）。
    Deprecated,
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
#[derive(Clone, Default)]
pub struct ParseOptions {
    /// 外置 `@` 指令表
    pub directives: DirectiveTable,
    /// 外置契约类型表（`image` / `link` / `time` 这类领域类型）。
    /// 见 [`sml_contract::ext`]。
    pub contract_ext: ContractExt,
}

impl ParseOptions {
    pub fn new() -> Self {
        Self::default()
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
#[derive(Debug, Clone)]
pub struct ParseOutput {
    pub value: Value,
    /// 弃用提示等。为空表示文档未触发任何警告。
    pub diagnostics: Vec<Diagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 元数据块指令：照单全收、不进主树。等价于 PVACIS 的 @form/@policy/@flow 用法。
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

    /// 方言三指令（PVACIS 实际写法）应被收编：文档可解析，且不进主数据树。
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
    /// 这是 PVACIS 的 `images: [image]` 用例 —— 类型名与规则都留在下游，
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
        assert!(e.contains("ev-"), "错误信息应带上外置类型的理由：{e}");
        assert!(e.contains("images[0]"), "错误信息应带上字段路径：{e}");
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
        assert!(e.contains("items_max") && e.contains("最多 2 项"), "{e}");
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
}
