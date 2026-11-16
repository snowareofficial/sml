//! 外置修饰符（`E-CONTRACT-011`）的**集成**回归钉子。｜ Integration nails for external modifiers.
//!
//! `sml-parse/src/ext.rs` 里已有白盒单测；这里从公开 API 再钉一遍，覆盖 §7 P12 的验收格：
//! 注册 / 未注册 / 内置名冲突 / `apply` 改写 / `check` / `check_group`，
//! 外加四类曾漏跑的分支（组合字段、模式字段、默认值回填、数组元素契约的跨字段约束）。

use sml_contract::{ContractExt, FieldSpec, GroupView, Modifier};
use sml_parse::{parse_with, ParseOptions, Value};

#[derive(Debug)]
struct AlwaysFail(&'static str);

impl Modifier for AlwaysFail {
    fn name(&self) -> &str {
        self.0
    }
    fn apply(&self, _spec: &mut FieldSpec, _v: &Value) -> Result<(), String> {
        Ok(())
    }
    fn check(&self, _spec: &FieldSpec, _v: &Value) -> Result<(), String> {
        Err("钉子：check 失败".into())
    }
}

#[derive(Debug)]
struct FailGroup(&'static str);

impl Modifier for FailGroup {
    fn name(&self) -> &str {
        self.0
    }
    fn apply(&self, _spec: &mut FieldSpec, _v: &Value) -> Result<(), String> {
        Ok(())
    }
    fn check_group(&self, _view: &GroupView<'_>) -> Result<(), String> {
        Err("钉子：check_group 失败".into())
    }
}

#[derive(Debug)]
struct SetDefault;

impl Modifier for SetDefault {
    fn name(&self) -> &str {
        "set_default"
    }
    fn apply(&self, spec: &mut FieldSpec, _v: &Value) -> Result<(), String> {
        spec.default = Some(Value::Str("from-apply".into()));
        Ok(())
    }
}

fn err_of(src: &str, opts: ParseOptions) -> String {
    format!("{}", parse_with(src, opts).expect_err("本例应当报错"))
}

#[test]
fn registered_modifier_apply_rewrites_field_spec() {
    // apply 在**解析期**改写规格：这里把缺省值塞进去，值树应能看到它。
    let v = parse_with(
        "@contract F { title: str set_default 1 }\nd { @is F }\n",
        ParseOptions::new().with_modifier(SetDefault).unwrap(),
    )
    .expect("apply 成功应能解析");
    assert_eq!(
        v.value.get("d.title").and_then(|x| x.as_str()),
        Some("from-apply")
    );
}

#[test]
fn check_failure_is_e_contract_011() {
    let e = err_of(
        "@contract F { n: int boom 1 }\nd { @is F n: 1 }\n",
        ParseOptions::new().with_modifier(AlwaysFail("boom")).unwrap(),
    );
    assert!(e.contains("E-CONTRACT-011"), "{e}");
    assert!(e.contains("boom") && e.contains("check 失败"), "{e}");
}

#[test]
fn check_group_failure_is_e_contract_011() {
    let e = err_of(
        "@contract F { n: int boom 1 }\nd { @is F n: 1 }\n",
        ParseOptions::new().with_modifier(FailGroup("boom")).unwrap(),
    );
    assert!(e.contains("E-CONTRACT-011"), "{e}");
    assert!(e.contains("check_group 失败"), "{e}");
}

#[test]
fn unregistered_modifier_is_loud() {
    // 零扩展 = 零影响：未注册时该写法必须**报错**，不静默当普通字段放过。
    // ⚠️ 具体码由语法层判（当前落 E-PARSE-006），§7 的「一律 011」与
    //    `errors/codes.sml` 的 E-EXT-001 尚有规范冲突，故这里只钉「响亮拒绝」。
    let e = err_of(
        "@contract F { n: int boom 1 }\nd { @is F n: 1 }\n",
        ParseOptions::new(),
    );
    assert!(!e.is_empty(), "未注册修饰符不得静默放过");
}

#[test]
fn builtin_and_duplicate_registration_are_rejected() {
    let mut ext = ContractExt::new();
    #[derive(Debug)]
    struct Fake(&'static str);
    impl Modifier for Fake {
        fn name(&self) -> &str {
            self.0
        }
        fn apply(&self, _spec: &mut FieldSpec, _v: &Value) -> Result<(), String> {
            Ok(())
        }
    }
    for n in ["optional", "required", "default", "min", "max"] {
        let e = format!("{}", ext.register_modifier(Fake(n)).expect_err("内置名不可占用"));
        assert!(e.contains("E-EXT-002"), "{n}: {e}");
    }
    ext.register_modifier(Fake("dup")).unwrap();
    let e = format!("{}", ext.register_modifier(Fake("dup")).expect_err("重复注册不可放行"));
    assert!(e.contains("E-EXT-002"), "{e}");
}

#[test]
fn check_runs_on_composed_and_pattern_fields() {
    // 曾漏跑：`check_type` 的 ContractRef / Pattern / Ext 分支提前 return，
    // 挂在字段上的 `check` 永不执行。现在三类字段都要过外置钩子。
    let opts = || ParseOptions::new().with_modifier(AlwaysFail("boom")).unwrap();
    let composed = err_of(
        "@contract I { x: int }\n@contract O { inner: I boom 1 }\nd { @is O inner: { x: 1 } }\n",
        opts(),
    );
    assert!(composed.contains("E-CONTRACT-011"), "{composed}");

    let pattern = err_of(
        "@type name: Code { 字面: A }\n@contract P { code: Code boom 1 }\nd { @is P code: A }\n",
        opts(),
    );
    assert!(pattern.contains("E-CONTRACT-011"), "{pattern}");
}

#[test]
fn default_backfill_runs_check() {
    // 曾漏跑：缺字段时只 `insert(default)`，不经过 `check`，默认路径成了静默旁路。
    let e = err_of(
        "@contract F { n: int default 7 boom 1 }\nd { @is F }\n",
        ParseOptions::new().with_modifier(AlwaysFail("boom")).unwrap(),
    );
    assert!(e.contains("E-CONTRACT-011"), "{e}");
}

#[test]
fn array_element_contract_runs_group_checks() {
    // 曾漏跑：元素位置只调 `check_type`，没有块级 `check_ext_groups` ⇒
    // 写在元素契约上的互斥 / 计数 / 依赖约束被静默跳过。
    let e = err_of(
        "@contract E { x: str boom 1 }\n@contract C { peers: [E] }\nd { @is C peers: [ { x: a } ] }\n",
        ParseOptions::new().with_modifier(FailGroup("boom")).unwrap(),
    );
    assert!(e.contains("E-CONTRACT-011"), "{e}");
}

#[test]
fn mixin_with_different_modifiers_is_loud() {
    // 曾漏跑：合并视图只保留第一份 spec ⇒ 后一份契约挂的同名字段修饰符静默消失。
    // 现在纳入 E-CONTRACT-016 冲突预检（顺序无关）。
    let e = err_of(
        "@contract A { n: int boom 1 }\n@contract B { n: int }\nd { @is A B n: 1 }\n",
        ParseOptions::new().with_modifier(FailGroup("boom")).unwrap(),
    );
    assert!(e.contains("E-CONTRACT-016"), "{e}");
}
