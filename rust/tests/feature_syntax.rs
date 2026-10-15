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
