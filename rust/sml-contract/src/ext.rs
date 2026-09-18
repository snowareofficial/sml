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
