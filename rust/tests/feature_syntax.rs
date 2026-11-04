//! `@feature enable` 能否一行多个：逗号批量 / 空格批量 的实测。

use sml::parse;

#[test]
fn enable_accepts_comma_separated_names() {
    // 逗号批量：一行开多个
    let src = "\
@feature enable typed-block,block-name
@contract Rule { action: enum [ allow deny ] }
Rule ssh { action: allow }
";
    assert!(parse(src).is_ok(), "实际：{:?}", parse(src).unwrap_err());
}

#[test]
fn enable_accepts_space_separated_names() {
    // 空格批量（与逗号等价）
    let src = "\
@feature enable typed-block block-name
@contract Rule { action: enum [ allow deny ] }
Rule ssh { action: allow }
";
    assert!(parse(src).is_ok(), "实际：{:?}", parse(src).unwrap_err());
}

#[test]
fn unknown_feature_name_still_errors() {
    let err = parse("@feature enable typed-block,nope\nx: 1\n")
        .unwrap_err()
        .to_string();
    assert!(err.contains("E-FEATURE-003"), "实际报错：{err}");
}

// ===========================================================================
// 带域名（N4）：`<域>.<名>`，只在第一个点切一次；老名保持平铺、**不补**别名（N2）
// ===========================================================================

#[test]
fn dotted_name_is_accepted() {
    // `type.featag` 是第一个带域名的特性（S2 才接语法，现在只占位与开关）
    assert!(parse("@feature enable type.featag\nx: 1\n").is_ok());
    assert!(sml::feature_names().contains(&"type.featag"));
    assert_eq!(
        sml::Feature::parse_name("type.featag"),
        Ok(sml::Feature::Featag)
    );
}

#[test]
fn three_segments_are_rejected_with_reason() {
    let err = format!("{}", parse("@feature enable a.b.c\nx: 1\n").unwrap_err());
    assert!(err.contains("E-FEATURE-003"), "实际报错：{err}");
    assert!(err.contains("最多两段"), "应说清层数限制：{err}");
}

#[test]
fn unknown_domain_is_rejected_with_whitelist() {
    let err = format!("{}", parse("@feature enable foo.bar\nx: 1\n").unwrap_err());
    assert!(err.contains("E-FEATURE-003"), "实际报错：{err}");
    assert!(err.contains("未知域 `foo`"), "应指出是域的问题：{err}");
    assert!(err.contains("lex"), "应列出白名单：{err}");
}

#[test]
fn old_features_get_no_dotted_alias() {
    // 域合法，但 `type.typed-block` 不是注册名 —— N2：同一特性不给第二个名字
    let err = format!(
        "{}",
        parse("@feature enable type.typed-block\nx: 1\n").unwrap_err()
    );
    assert!(err.contains("E-FEATURE-003"), "实际报错：{err}");
    assert!(err.contains("可用"), "应回落到「未注册」并给清单：{err}");
    // 平铺写法仍然可用
    assert!(parse("@feature enable typed-block\nx: 1\n").is_ok());
}

#[test]
fn misconfigured_requires_reports_the_same_shape_rule() {
    let src = "\
@feature enable type.featag
@contract C {
    @requires a.b.c
    x: int
}
";
    let err = format!("{}", parse(src).unwrap_err());
    assert!(err.contains("E-FEATURE-003"), "实际报错：{err}");
    assert!(err.contains("最多两段"), "`@requires` 应与 `enable` 同判据：{err}");
}
