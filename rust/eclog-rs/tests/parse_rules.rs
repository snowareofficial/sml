// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! 语法规则单测：把 golden 覆盖不到的边界逐条钉住。
//! Rule-level tests for the edges the golden vectors do not cover.

use eclog::{
    parse, Error, ErrorKind, Number, NumberInfo, Object, ParseErrorKind, Str, StringNotation, Value,
};

fn ok(source: &str) -> Value {
    parse(source.as_bytes()).unwrap_or_else(|err| panic!("unexpected error: {}", err))
}

fn err(source: &str) -> Error {
    parse(source.as_bytes()).expect_err("expected a parse error")
}

#[test]
fn empty_input_is_an_empty_object() {
    for source in ["", "  \t\n", "# only a comment", "{}"] {
        assert_eq!(
            ok(source),
            Value::Object(Object::new()),
            "input {:?}",
            source
        );
    }
}

#[test]
fn duplicate_keys_keep_first_position_and_last_value() {
    let value = ok("a: 1\nb: 2\na: 3\n");
    let object = value.as_object().expect("root object");

    assert_eq!(object.len(), 2);
    assert_eq!(object.get_index(0).expect("first entry").0.text(), "a");
    assert_eq!(object.get_index(1).expect("second entry").0.text(), "b");
    assert_eq!(object.get("a").unwrap().as_number().unwrap().raw(), "3");
}

#[test]
fn duplicate_keys_take_the_last_key_notation() {
    let value = ok("a: 1\n\"a\": 2\n");
    let object = value.as_object().expect("root object");

    assert_eq!(object.len(), 1);
    assert_eq!(
        object.get_index(0).expect("entry").0.notation(),
        StringNotation::Quoted
    );
}

#[test]
fn trailing_dot_swallows_the_next_character() {
    // C++ `transferDecimalDigits` 先无条件吞一个字符：`1.e5` 不是指数形式。
    let value = ok("a: 1.e5");
    let number = value.get("a").unwrap().as_number().unwrap();
    assert_eq!(number.raw(), "1.e5");
    assert_eq!(number.info(), NumberInfo::FRAC);
    assert!(!number.has_exp());

    let value = ok("a: 1.x");
    assert_eq!(value.get("a").unwrap().as_number().unwrap().raw(), "1.x");
}

#[test]
fn leading_zero_is_rejected() {
    assert_eq!(
        err("a: 01").parse_kind(),
        Some(ParseErrorKind::UnexpectedCharacter)
    );
}

#[test]
fn inf_and_nan_carry_sign_bits() {
    let value = ok("a: inf\nb: +inf\nc: -nan\nd: nan");

    let a = value.get("a").unwrap().as_number().unwrap();
    assert_eq!(a.info(), NumberInfo::INF);
    assert_eq!(a.raw(), "inf");

    let b = value.get("b").unwrap().as_number().unwrap();
    assert_eq!(b.info(), NumberInfo::SIGN | NumberInfo::INF);
    assert_eq!(b.raw(), "+inf");

    let c = value.get("c").unwrap().as_number().unwrap();
    assert_eq!(
        c.info(),
        NumberInfo::SIGN | NumberInfo::NEG | NumberInfo::NAN
    );

    let d = value.get("d").unwrap().as_number().unwrap();
    assert_eq!(d.info(), NumberInfo::NAN);
}

#[test]
fn number_conversions() {
    let value = ok("a: 42\nb: -7\nc: 1e100\nd: +inf");

    assert_eq!(
        value.get("a").unwrap().as_number().unwrap().as_i64(),
        Some(42)
    );
    assert_eq!(
        value.get("b").unwrap().as_number().unwrap().as_i64(),
        Some(-7)
    );
    assert_eq!(value.get("b").unwrap().as_number().unwrap().as_u64(), None);
    assert_eq!(value.get("c").unwrap().as_number().unwrap().as_i64(), None);
    assert_eq!(value.get("c").unwrap().as_number().unwrap().as_f64(), 1e100);
    assert_eq!(
        value.get("d").unwrap().as_number().unwrap().as_f64(),
        f64::INFINITY
    );
}

#[test]
fn number_from_raw_validates_the_whole_literal() {
    assert_eq!(Number::from_raw("inf").unwrap().info(), NumberInfo::INF);
    assert_eq!(
        Number::from_raw("+1.5e-3").unwrap().info(),
        NumberInfo::SIGN | NumberInfo::FRAC | NumberInfo::EXP
    );
    assert!(Number::from_raw("01").is_none());
    assert!(Number::from_raw("1 ").is_none());
    assert!(Number::from_raw("").is_none());
}

#[test]
fn heredoc_dedents_by_the_closing_tag_indent() {
    let value = ok("a: |T\n    x\n    y\n  T\n");
    assert_eq!(value.get("a").unwrap().as_text(), Some("  x\n  y\n"));

    // 闭标记顶格时不减缩进。
    let value = ok("a: |T\n  x\nT\n");
    assert_eq!(value.get("a").unwrap().as_text(), Some("  x\n"));
}

#[test]
fn heredoc_requires_a_linebreak_after_the_closing_tag() {
    assert_eq!(
        err("a: |T\nT").parse_kind(),
        Some(ParseErrorKind::UnexpectedEndOfFile)
    );
    assert_eq!(
        err("a: |\nx\n").parse_kind(),
        Some(ParseErrorKind::EmptyStringDelimiter)
    );
}

#[test]
fn raw_string_backtracks_a_partial_delimiter() {
    let value = ok("a: @dd\"x\"d\"dd");
    assert_eq!(value.get("a").unwrap().as_text(), Some("x\"d"));

    // 空定界符：遇到第一个双引号即结束。
    let value = ok("a: @\"\"");
    assert_eq!(value.get("a").unwrap().as_text(), Some(""));
}

#[test]
fn unicode_escapes() {
    let value = ok("a: \"\\u{1f60D}\"\nb: \"\\ud83c\\uDf09\"");
    assert_eq!(value.get("a").unwrap().as_text(), Some("\u{1f60D}"));
    assert_eq!(value.get("b").unwrap().as_text(), Some("\u{1f309}"));
}

#[test]
fn malformed_escapes_report_specific_kinds() {
    assert_eq!(
        err("a: \"\\q\"").parse_kind(),
        Some(ParseErrorKind::UnrecognizedEscape)
    );
    assert_eq!(
        err("a: \"\\u{}\"").parse_kind(),
        Some(ParseErrorKind::InvalidUnicodeEscape)
    );
    assert_eq!(
        err("a: \"\\u{1234567}\"").parse_kind(),
        Some(ParseErrorKind::InvalidUnicodeEscape)
    );
    assert_eq!(
        err("a: \"\\udc37\"").parse_kind(),
        Some(ParseErrorKind::InvalidUtf16Surrogate)
    );
}

#[test]
fn reserved_words_are_not_unquoted_keys() {
    assert_eq!(
        err("true: 1").parse_kind(),
        Some(ParseErrorKind::InvalidKey)
    );

    let value = ok("\"true\": 1");
    assert_eq!(value.get("true").unwrap().as_number().unwrap().raw(), "1");
}

#[test]
fn concatenation_keeps_the_first_notation_and_rejects_bare_words() {
    let value = ok("a: \"x\" + @\"y\" + |T\nz\nT\n");
    let text = value.get("a").unwrap().as_str().expect("string");
    assert_eq!(text.text(), "xyz\n");
    assert_eq!(text.notation(), StringNotation::Quoted);

    assert_eq!(
        err("a: \"x\" + y").parse_kind(),
        Some(ParseErrorKind::UnexpectedCharacter)
    );
}

#[test]
fn same_line_fields_need_a_comma() {
    assert_eq!(
        err("a: 1 b: 2").parse_kind(),
        Some(ParseErrorKind::UnexpectedCharacter)
    );
    assert!(parse(b"a: 1, b: 2").is_ok());
}

#[test]
fn trailing_commas_are_allowed() {
    assert!(parse(b"a: [1, 2,],\nb: {c: 1,},\n").is_ok());
}

#[test]
fn root_must_be_an_object() {
    assert_eq!(
        err("[]").parse_kind(),
        Some(ParseErrorKind::UnexpectedCharacter)
    );
    assert_eq!(
        err("{}x").parse_kind(),
        Some(ParseErrorKind::UnexpectedCharacter)
    );
}

#[test]
fn invalid_utf8_is_a_decode_error() {
    let error = parse(b"a: \xff").expect_err("decode error");
    assert_eq!(error.kind(), ErrorKind::Decode);
    assert_eq!(error.parse_kind(), None);
    assert_eq!(error.offset(), 3);
}

#[test]
fn error_display_carries_position() {
    let error = err("a: 01");
    assert_eq!(error.line(), 1);
    assert_eq!(error.column(), 5);
    assert!(error.to_string().contains("unexpected character"));
}

#[test]
fn object_insert_keeps_position_and_takes_the_last_key() {
    let mut object = Object::new();
    object.insert(Str::new("a"), Value::from(1i64));
    object.insert(Str::new("b"), Value::from(2i64));

    let replaced = object.insert(Str::quoted("a"), Value::from(3i64));
    assert_eq!(replaced, Some(Value::from(1i64)));
    assert_eq!(object.len(), 2);
    assert_eq!(
        object.get_index(0).expect("first entry").0.notation(),
        StringNotation::Quoted
    );
    assert_eq!(object.get("a").unwrap().as_number().unwrap().raw(), "3");
}

#[test]
fn object_append_never_overwrites() {
    let mut object = Object::new();
    assert!(object.append(Str::new("a"), Value::from(1i64)));
    assert!(!object.append(Str::new("a"), Value::from(2i64)));
    assert_eq!(object.get("a").unwrap().as_number().unwrap().raw(), "1");
}

#[test]
fn notations_and_delimiters_survive_the_tree() {
    // `.check` 只看解码后的正文，记法与定界符由这条单测钉住（3b 渲染要用）。
    let value = ok("a: x\nb: \"x\"\nc: @dd\"x\"dd\nd: |T\nx\nT\ne: 1\n");
    let object = value.as_object().expect("root object");

    assert_eq!(
        object.get("a").unwrap().as_str().unwrap().notation(),
        StringNotation::Unquoted
    );
    assert_eq!(
        object.get("b").unwrap().as_str().unwrap().notation(),
        StringNotation::Quoted
    );

    let raw = object.get("c").unwrap().as_str().unwrap();
    assert_eq!(
        (raw.notation(), raw.delimiter()),
        (StringNotation::Raw, "dd")
    );
    assert_eq!(raw.text(), "x");

    let heredoc = object.get("d").unwrap().as_str().unwrap();
    assert_eq!(
        (heredoc.notation(), heredoc.delimiter()),
        (StringNotation::Heredoc, "T")
    );
    assert_eq!(heredoc.text(), "x\n");

    // 键的记法与定界符同样保留。
    let value = ok("|KEY\nv\nKEY\n: 1\n");
    let key = value.as_object().unwrap().get_index(0).unwrap().0;
    assert_eq!(
        (key.notation(), key.delimiter()),
        (StringNotation::Heredoc, "KEY")
    );
    assert_eq!(key.text(), "v\n");
}

#[test]
fn string_equality_ignores_notation() {
    assert_eq!(Str::new("x"), Str::quoted("x"));
    assert_eq!(Str::raw("x", "T"), Str::heredoc("x", "T"));
}

#[test]
fn parse_error_codes_are_unique() {
    let mut codes: Vec<&str> = ParseErrorKind::ALL.iter().map(|kind| kind.code()).collect();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), ParseErrorKind::ALL.len());
}
