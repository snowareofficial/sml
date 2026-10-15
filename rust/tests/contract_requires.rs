// SPDX-License-Identifier: MulanPSL-2.0
//! 契约级 `@requires`：定义时校验、批量写法、跨端拒绝所需的判据，与回归红线。
//!
//! 三组关注点（对应任务要求 2.6）：
//!   1. **定义时**校验（不是应用时）：契约体里 `@requires` 声明的任一特性未开启
//!      ⇒ 报 `E-CONTRACT-018`；未知特性名 ⇒ `E-FEATURE-003`。
//!   2. **不得静默失效**：`@requires` 必须被当作指令消费，绝不能退化成「名为
//!      requires 的片段」；契约体里其余 `@xxx` 必须报错而非静默变片段。
//!   3. **回归**：不带 `@requires` 的既有契约（含名为 `requires` 的**字段**）逐字不变。

use sml::parse;

fn ok(text: &str) -> sml::Value {
    match parse(text) {
        Ok(v) => v,
        Err(e) => panic!("应解析成功，实际失败: {e}\n---\n{text}\n---"),
    }
}

/// 跑一段源码，返回 (错误码, 文案)；解析成功则 panic（用例写错了）。
fn err_info(text: &str) -> (String, String) {
    match parse(text) {
        Ok(_) => panic!("应当失败，实际通过了:\n{text}"),
        Err(e) => (e.code().to_string(), e.message().to_string()),
    }
}

fn err_code(text: &str) -> String {
    err_info(text).0
}

// ── 1. 定义时校验：特性未开启 ⇒ 报错 ────────────────────────────────────

/// `typed-block` 是 opt-in（不在 `baseline()` 里），未 `@feature enable` 即应在
/// **契约定义时**失败 —— 注意此例契约**从未被 `@is` 应用**，证明校验点在定义而非应用。
#[test]
fn unmet_requires_fails_at_definition_not_application() {
    let doc = "\
@contract Rule {
    @requires typed-block
    action: enum [ allow deny ]
}
";
    let (code, msg) = err_info(doc);
    assert_eq!(code, "E-CONTRACT-018", "未开启的 requires 应报 E-CONTRACT-018：{msg}");
    assert!(msg.contains("Rule"), "错误信息须写清是哪个契约：{msg}");
    assert!(msg.contains("typed-block"), "错误信息须写清缺哪个特性：{msg}");
}

/// 多个 requires 里只要**有一个**未开启，就必须在定义时报错。
#[test]
fn any_single_unmet_requirement_fails() {
    let doc = "\
@contract Rule {
    @requires contract, typed-block
    action: str
}
";
    let (code, msg) = err_info(doc);
    assert_eq!(code, "E-CONTRACT-018");
    assert!(msg.contains("typed-block"), "须点出未开启的那一个：{msg}");
}

/// `@requires` 里写未知特性名 ⇒ `E-FEATURE-003`（与 `@feature enable nosuch` 同码）。
#[test]
fn unknown_feature_name_in_requires_errors() {
    let doc = "\
@contract Rule {
    @requires nosuch-feature
    action: str
}
";
    let (code, msg) = err_info(doc);
    assert_eq!(code, "E-FEATURE-003", "未知特性名应报 E-FEATURE-003：{msg}");
    assert!(msg.contains("nosuch-feature"), "须点出拼错的特性名：{msg}");
}

// ── 2. 特性已开启 ⇒ 契约定义与应用均正常 ───────────────────────────────

/// 满足 requires 后，契约照常定义、照常应用（默认值回填 / 类型校验 / 严格模式都照旧）。
#[test]
fn satisfied_requires_defines_and_applies() {
    let doc = "\
@feature enable typed-block
@contract Rule {
    @requires typed-block, contract
    action: enum [ allow deny ]
    note: str default \"-\"
}
ssh {
    @is Rule
    action: allow
}
";
    let v = ok(doc);
    assert_eq!(v.get("ssh.action"), Some(&sml::Value::Str("allow".into())));
    // 默认值仍照旧回填
    assert_eq!(v.get("ssh.note"), Some(&sml::Value::Str("-".into())));
    // 严格模式仍照旧拒绝未声明字段
    let bad = doc.replace("action: allow", "action: allow\n    extra: 1");
    let (code, _msg) = err_info(&bad);
    assert_eq!(code, "E-CONTRACT-004", "满足 requires 后严格性不应被削弱");
}

// ── 3. 逗号批量 / 空格批量两种写法都要认 ───────────────────────────────

/// 逗号分隔、空格分隔、以及「逗号但无空格」三种写法等价且都生效。
/// 认不出来就是**静默失效**：requires 会被当成普通契约字段一样被吞掉而不报错。
#[test]
fn comma_and_space_batches_both_recognized() {
    for decl in [
        "@requires typed-block, contract",
        "@requires typed-block contract",
        "@requires typed-block,contract",
    ] {
        let doc = format!(
            "@feature enable typed-block\n@contract Rule {{\n    {decl}\n    action: str\n}}\n\
             ssh {{\n    @is Rule\n    action: allow\n}}\n"
        );
        let v = ok(&doc);
        assert_eq!(
            v.get("ssh.action"),
            Some(&sml::Value::Str("allow".into())),
            "写法 {decl:?} 应被识别为 @requires 指令"
        );
    }
}

/// 反向探针：若某种批量写法没被认出来，空格批量会把字吞成 fieldless 垃圾。
/// 这里用「未开启的特性」做石蕊试纸 —— 能被检测到就说明整串都被收集到了。
#[test]
fn batch_forms_collect_every_name_not_just_the_first() {
    // 逗号批量里 typed-block 排在**第二位**，只有整个列表都被收集才会触发定义时校验
    let doc = "\
@contract Rule {
    @requires contract, typed-block
    action: str
}
";
    let (code, msg) = err_info(doc);
    assert_eq!(code, "E-CONTRACT-018", "第二个名字也必须收到：{msg}");
    assert!(msg.contains("typed-block"));
}

// ── 4. 不得静默变片段 ──────────────────────────────────────────────────

/// `@requires` 必须被当作**指令**消费掉，而不是退化成「名为 requires 的片段」。
///
/// 探针：若它被静默注册成片段，下面的 `&requires` 引用会成立；正确实现下
/// 它从未进过片段表 ⇒ 引用未定义片段 ⇒ `E-INCLUDE-006`。
#[test]
fn requires_is_not_silently_registered_as_a_fragment() {
    let doc = "\
@feature enable typed-block
@contract Rule {
    @requires typed-block
    action: str
}
x {
    &requires
}
";
    let (code, msg) = err_info(doc);
    assert_eq!(
        code, "E-INCLUDE-006",
        "@requires 不该变成名为 requires 的片段（那样 &requires 就成立）：{msg}"
    );
    assert!(msg.contains("requires"), "须点出未命中的片段名：{msg}");
}

/// 契约体里写不认识的 `@xxx` ⇒ 报错，绝不静默当成名为 xxx 的片段。
#[test]
fn unknown_directive_in_contract_body_errors() {
    let doc = "\
@contract Rule {
    @bogus whatever
    action: str
}
";
    let (code, msg) = err_info(doc);
    assert_eq!(code, "E-PARSE-005", "契约体里的未知 @xxx 必须报错：{msg}");
    assert!(msg.contains("bogus"), "须点出不支持的指令名：{msg}");
}

/// 同一个道理，对**看起来像合法指令**的名字也成立 —— 契约体只认 `@requires`。
#[test]
fn non_requires_directive_in_contract_body_errors() {
    let doc = "\
@contract Rule {
    @when whatever
    action: str
}
";
    let code = err_code(doc);
    assert_eq!(code, "E-PARSE-005", "契约体不支持 @when，须响亮拒绝");
}

// ── 5. 回归：既有行为逐字不变 ──────────────────────────────────────────

/// 不带 `@requires` 的契约：默认值回填、严格模式、类型校验全部照旧。
#[test]
fn regression_contract_without_requires_is_unchanged() {
    let doc = "\
@contract Server {
    host: str
    port: int default 8080
}
db {
    @is Server
    host: h1
}
";
    let v = ok(doc);
    assert_eq!(v.get("db.host"), Some(&sml::Value::Str("h1".into())));
    assert_eq!(v.get("db.port"), Some(&sml::Value::Int(8080)));
}

/// 关键回归：`requires` **作为字段名**（`requires: [...]`）是既有合法写法
///（`examples/probe_capability.rs` 在用），不得被 `@requires` 指令抢占 —— 区分
/// 判据是 `@` 符号，字段名路径永远走不到 At 分支。
#[test]
fn regression_contract_field_named_requires_still_works() {
    let doc = "\
@contract Cap {
    requires: [str]
    provides: [str]
}
x {
    @is Cap
    requires: [\"tls\"]
    provides: [\"enc\"]
}
";
    let v = ok(doc);
    assert_eq!(
        v.get("x.requires"),
        Some(&sml::Value::Array(vec![sml::Value::Str("tls".into())])),
        "名为 requires 的字段必须照旧工作"
    );
    assert_eq!(
        v.get("x.provides"),
        Some(&sml::Value::Array(vec![sml::Value::Str("enc".into())]))
    );
    // 字段规格仍是 [str]：给非字符串应照旧报类型错
    let bad = doc.replace("requires: [\"tls\"]", "requires: [1]");
    let code = err_code(&bad);
    assert_eq!(code, "E-CONTRACT-002", "字段名 requires 的类型校验须照旧");
}

/// 契约体里走 paren 形式的 enum、组合引用等既有语法不受影响。
#[test]
fn regression_existing_contract_body_syntax_unaffected() {
    let doc = "\
@contract Addr { city: str }
@contract Srv {
    addr: Addr
    st: enum [ a b ]
    tags: [str] optional
    r: num min 0 max 1
}
x {
    @is Srv
    addr { city: bj }
    st: a
    r: 0.5
}
";
    let v = ok(doc);
    assert_eq!(v.get("x.addr.city"), Some(&sml::Value::Str("bj".into())));
    assert_eq!(v.get("x.st"), Some(&sml::Value::Str("a".into())));
    // Float 的 raw 不参与相等比较，故用无 raw 的构造器断言纯值
    assert_eq!(v.get("x.r"), Some(&sml::Value::float(0.5)));
}
