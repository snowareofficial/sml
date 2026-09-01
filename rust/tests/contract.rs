// SPDX-License-Identifier: MulanPSL-2.0
// 契约（Contract）行为测试
//
// 契约是 SML 的可选 schema 层：给块加上结构约束（必填/类型/枚举/数值区间/默认值）。
// 不使用契约时行为完全不变（见 `no_contract_behaviour_unchanged`）。

use sml::parse;

fn ok(text: &str) -> sml::Value {
    match parse(text) {
        Ok(v) => v,
        Err(e) => panic!("应解析成功，实际失败: {e}\n---\n{text}\n---"),
    }
}

fn err(text: &str) -> String {
    match parse(text) {
        Ok(_) => panic!("应校验失败，实际通过了:\n{text}"),
        Err(e) => e,
    }
}

#[test]
fn defaults_are_filled() {
    let v = ok(
        "@contract Server {
             host: str
             port: int default 8080
             tls: bool default true
         }
         db {
             @is Server
             host: db1.internal
         }",
    );
    assert_eq!(v.get("db.host").unwrap().as_str(), Some("db1.internal"));
    assert_eq!(v.get("db.port"), Some(&sml::Value::Int(8080)), "缺失字段应填 default");
    assert_eq!(v.get("db.tls"), Some(&sml::Value::Bool(true)));
}

#[test]
fn missing_required_field_fails() {
    let e = err(
        "@contract Server { host: str }
         db { @is Server }",
    );
    assert!(e.contains("host"), "错误信息应指出缺失字段，got: {e}");
    assert!(e.contains("必填"), "got: {e}");
}

// ---------------------------------------------------------------------------
// 错误定位：嵌套字段报错要给出**完整路径**（`api.port`），而不是只报最内层
// 字段名（`port`）—— 深层配置里多个契约都可能有 `port`，只报字段名无法判断
// 出错位置。
// ---------------------------------------------------------------------------

/// 组合契约：`App { db: Database, api: Service }`，其中 Database 还嵌了 Credentials。
const NESTED: &str = "
@contract Credentials { user: str  pass: str }
@contract Database { host: str default localhost
                     port: int min 1 max 65535
                     creds: Credentials }
@contract Service { name: str
                    port: int min 1 max 65535
                    tags: [str] optional }
@contract App { db: Database  api: Service }
";

#[test]
fn error_path_covers_nested_contract_field() {
    let e = err(&format!(
        "{NESTED}
         app {{ @is App
              db {{ host: h  port: 5432  creds {{ user: admin  pass: 12345 }} }}
              api {{ name: gw  port: 443 }} }}"
    ));
    // 最内层的 pass 类型错，路径应追溯到 db.creds.pass
    assert!(e.contains("db.creds.pass"), "应给出完整路径，got: {e}");
}

#[test]
fn error_path_covers_child_block_field() {
    let e = err(&format!(
        "{NESTED}
         app {{ @is App
              db {{ host: h  port: 5432  creds {{ user: a  pass: b }} }}
              api {{ name: gw  port: \"443\" }} }}"
    ));
    assert!(e.contains("api.port"), "应给出完整路径，got: {e}");
}

#[test]
fn error_path_covers_array_element_index() {
    let e = err(&format!(
        "{NESTED}
         app {{ @is App
              db {{ host: h  port: 5432  creds {{ user: a  pass: b }} }}
              api {{ name: gw  port: 443  tags: [web, 8080] }} }}"
    ));
    // 第 2 个元素（下标 1）是 int，应为 str
    assert!(e.contains("api.tags[1]"), "数组错误应带下标，got: {e}");
}

#[test]
fn error_path_covers_undeclared_field_in_child_block() {
    let e = err(&format!(
        "{NESTED}
         app {{ @is App
              db {{ host: h  port: 5432  creds {{ user: a  pass: b }}  extra: 1 }}
              api {{ name: gw  port: 443 }} }}"
    ));
    assert!(e.contains("db.extra"), "未声明字段应带所在块路径，got: {e}");
}

#[test]
fn error_path_covers_missing_composed_block() {
    let e = err(&format!(
        "{NESTED}
         app {{ @is App
              db {{ host: h  port: 5432 }}
              api {{ name: gw  port: 443 }} }}"
    ));
    assert!(e.contains("db.creds"), "必填组合字段应带路径，got: {e}");
}

#[test]
fn error_path_covers_range_violation() {
    let e = err(&format!(
        "{NESTED}
         app {{ @is App
              db {{ host: h  port: 99999  creds {{ user: a  pass: b }} }}
              api {{ name: gw  port: 443 }} }}"
    ));
    assert!(e.contains("db.port"), "超上界应带路径，got: {e}");
    assert!(e.contains("65535"), "应提示上界值，got: {e}");
}

/// 顶层块自身的直接字段没有路径前缀：顶层块不进 ns_stack（否则 `@is App`
/// 会被 qualify 成 `app.App` 而找不到契约），故只报字段名。
#[test]
fn top_level_field_has_no_path_prefix() {
    let e = err(&format!(
        "{NESTED}
         app {{ @is App
              db {{ host: h  port: 5432  creds {{ user: a  pass: b }} }}
              api {{ name: gw  port: 443 }}
              extra: 1 }}"
    ));
    assert!(e.contains("extra"), "got: {e}");
    assert!(!e.contains("app.extra"), "顶层字段不应带前缀，got: {e}");
}

#[test]
fn type_mismatch_fails() {
    // port 声明为 int，实际给了字符串
    let e = err(
        "@contract Server { port: int }
         db { @is Server
              port: \"not-a-number\" }",
    );
    assert!(e.contains("port"), "got: {e}");
    assert!(e.contains("int"), "应提示期望类型，got: {e}");
}

#[test]
fn enum_accepts_declared_value() {
    let v = ok(
        "@contract Server { status: enum [ active retired ] }
         db { @is Server
              status: active }",
    );
    assert_eq!(v.get("db.status").unwrap().as_str(), Some("active"));
}

#[test]
fn enum_rejects_undeclared_value() {
    let e = err(
        "@contract Server { status: enum [ active retired ] }
         db { @is Server
              status: deleted }",
    );
    assert!(e.contains("status"), "got: {e}");
}

#[test]
fn numeric_bounds_enforced() {
    ok("@contract C { ratio: num min 0 max 1 }
        x { @is C
            ratio: 0.5 }");

    let e = err(
        "@contract C { ratio: num min 0 max 1 }
         x { @is C
             ratio: 5 }",
    );
    assert!(e.contains("上界") || e.contains("大于"), "got: {e}");
}

#[test]
fn array_element_type_checked() {
    ok("@contract C { tags: [str] }
        x { @is C
            tags: [ a b c ] }");

    let e = err(
        "@contract C { tags: [str] }
         x { @is C
             tags: [ a 2 c ] }",
    );
    assert!(e.contains("tags"), "数组元素类型应被校验，got: {e}");
}

#[test]
fn optional_field_may_be_absent() {
    let v = ok(
        "@contract C { note: str optional }
         x { @is C }",
    );
    // optional 且无默认值 -> 字段不出现
    assert_eq!(v.get("x.note"), None);
}

#[test]
fn unknown_contract_is_error() {
    let e = err("x { @is Nonexistent\n     a: 1 }");
    assert!(e.contains("未定义的契约"), "got: {e}");
}

#[test]
fn no_contract_behaviour_unchanged() {
    // 不使用契约时，解析行为与以往完全一致（向后兼容）
    let v = ok("host: db1.internal\nport: 8080\n");
    assert_eq!(v.get("host").unwrap().as_str(), Some("db1.internal"));
    assert_eq!(v.get("port"), Some(&sml::Value::Int(8080)));
}

#[test]
fn contract_definition_not_in_tree() {
    // @contract 定义本身不应进入解析结果
    let v = ok(
        "@contract C { a: str }
         x: 1",
    );
    assert_eq!(v.get("x"), Some(&sml::Value::Int(1)));
    assert_eq!(v.get("contract"), None, "契约定义不应进主树");
    assert_eq!(v.get("C"), None, "契约定义不应进主树");
}

#[test]
fn contract_applies_to_multiple_blocks() {
    let v = ok(
        "@contract C { port: int default 80 }
         a { @is C }
         b { @is C
             port: 9090 }",
    );
    assert_eq!(v.get("a.port"), Some(&sml::Value::Int(80)));
    assert_eq!(v.get("b.port"), Some(&sml::Value::Int(9090)));
}

// ---------------------------------------------------------------------------
// 组合（而非继承）
// ---------------------------------------------------------------------------

#[test]
fn composition_nested_contract_checked() {
    // address 字段的类型是另一个契约 Address（组合）
    let v = ok(
        "@contract Address {
             city: str
             zip: str optional
         }
         @contract Server {
             host: str
             address: Address
         }
         db {
             @is Server
             host: db1.internal
             address { city: Beijing }
         }",
    );
    assert_eq!(v.get("db.host").unwrap().as_str(), Some("db1.internal"));
    assert_eq!(
        v.get("db.address.city").unwrap().as_str(),
        Some("Beijing"),
        "组合字段应被递归解析"
    );
}

#[test]
fn composition_fills_nested_defaults() {
    let v = ok(
        "@contract Address { city: str  country: str default CN }
         @contract Server { address: Address }
         db {
             @is Server
             address { city: Shanghai }
         }",
    );
    assert_eq!(
        v.get("db.address.country").unwrap().as_str(),
        Some("CN"),
        "子块缺失字段应填被引用契约的 default"
    );
}

#[test]
fn composition_rejects_violation_in_nested_contract() {
    let e = err(
        "@contract Address { city: str }
         @contract Server { address: Address }
         db {
             @is Server
             address { city: 123 }
         }",
    );
    assert!(e.contains("city"), "嵌套契约的类型违规应报错，got: {e}");
}

#[test]
fn composition_rejects_scalar_where_block_expected() {
    let e = err(
        "@contract Address { city: str }
         @contract Server { address: Address }
         db {
             @is Server
             address: not-a-block
         }",
    );
    assert!(e.contains("address"), "got: {e}");
}

#[test]
fn composition_rejects_unknown_referenced_contract() {
    let e = err(
        "@contract Server { address: Nonexistent }
         db {
             @is Server
             address { city: x }
         }",
    );
    assert!(e.contains("未定义的契约"), "got: {e}");
}

#[test]
fn referenced_contract_may_be_defined_later() {
    // 契约引用在 @is 时才解析，因此可先引用后定义
    let v = ok(
        "@contract Server { address: Address }
         @contract Address { city: str }
         db {
             @is Server
             address { city: Chengdu }
         }",
    );
    assert_eq!(v.get("db.address.city").unwrap().as_str(), Some("Chengdu"));
}

// ---------------------------------------------------------------------------
// 严格模式（默认严格，loose 显式放宽）
// ---------------------------------------------------------------------------

#[test]
fn strict_mode_rejects_undeclared_field() {
    let e = err(
        "@contract Server { host: str }
         db {
             @is Server
             host: db1.internal
             prot: 5432
         }",
    );
    assert!(e.contains("prot"), "拼错的字段应被拒绝，got: {e}");
    assert!(e.contains("严格"), "got: {e}");
}

#[test]
fn loose_mode_allows_undeclared_field() {
    // 显式写 loose 才允许额外字段
    let v = ok(
        "@contract Server loose { host: str }
         db {
             @is Server
             host: db1.internal
             extra: anything
         }",
    );
    assert_eq!(v.get("db.host").unwrap().as_str(), Some("db1.internal"));
    assert!(v.get("db.extra").is_some(), "loose 下额外字段应保留");
}

#[test]
fn loose_still_validates_declared_fields() {
    // loose 只放宽「未声明字段」，已声明字段照样校验
    let e = err(
        "@contract Server loose { port: int }
         db {
             @is Server
             port: not-an-int
             extra: 1
         }",
    );
    assert!(e.contains("port"), "loose 下已声明字段仍须校验，got: {e}");
}

// ---------------------------------------------------------------------------
// 数值边界 NaN/inf 防护（审计报告 #2）
// ---------------------------------------------------------------------------

#[test]
fn nan_min_max_bound_rejected() {
    // 边界本身为 NaN/inf 时，min/max 校验不应被静默绕过
    let e = err(
        "@contract S { ratio: num min nan max nan }
         x { @is S
             ratio: 99999 }",
    );
    assert!(!e.is_empty(), "NaN 边界应被拒绝，实际通过了约束: {e}");
    assert!(e.contains("有限") || e.contains("nan") || e.contains("数字边界"), "got: {e}");
}

#[test]
fn nan_value_rejected_by_bounds() {
    let e = err(
        "@contract S { ratio: num min 0 max 1 }
         x { @is S
             ratio: nan }",
    );
    assert!(!e.is_empty(), "NaN 值应被 min/max 校验拒绝，实际穿透: {e}");
    assert!(e.contains("非有限") || e.contains("NaN"), "got: {e}");
}

#[test]
fn inf_value_rejected_by_bounds() {
    let e = err(
        "@contract S { ratio: num min 0 max 1 }
         x { @is S
             ratio: inf }",
    );
    assert!(!e.is_empty(), "inf 值应被 min/max 校验拒绝，实际穿透: {e}");
}

#[test]
fn normal_bounds_still_work() {
    // 回归：正常边界仍生效
    let e = err(
        "@contract S { ratio: num min 0 max 1 }
         x { @is S
             ratio: 5 }",
    );
    assert!(e.contains("上界") || e.contains("大于"), "got: {e}");
    // 正常范围内应通过
    ok("@contract S { ratio: num min 0 max 1 }
       x { @is S ratio: 0.5 }");
}

#[test]
fn undefined_fragment_reference_is_error() {
    // 修复前：未定义的片段引用静默降级为字符串 Str("&nope")，
    // 拼错的片段名（如 &prod-db）不会报错，下游 .get 取到 None，难以排查。
    // 现在应与契约引用一致地报错。
    let e = err("k: &nope");
    assert!(
        e.contains("未定义的片段引用"),
        "未定义片段应报错，实际: {e}"
    );

    // 片段名拼错（&prod 写成 &prod-db）也报错，而非静默得到 "&prod-db"
    let e2 = err("cfg { host: &prod-db }");
    assert!(
        e2.contains("未定义的片段引用"),
        "拼错片段名应报错，实际: {e2}"
    );

    // 定义体内的自引用（@base { x: &base }）同样报错，不会无限递归降级为字符串
    let e3 = err("@base { x: &base } k: &base");
    assert!(
        e3.contains("未定义的片段引用"),
        "自引用片段应报错，实际: {e3}"
    );

    // 已定义的片段引用仍正常工作
    ok("@db { host: \"h\" port: 5432 } k: &db");
}
