// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! 渲染边界用例（记法降级 / compact / inline / 注释）与 RFC 7396 merge patch。
//! Renderer edge cases (notation fallback / compact / inline / comments) and RFC 7396.
//!
//! `tests/golden/render/edge.*.render` 由 C++ oracle（同一份 `ValueDesc` 建树）生成，
//! 是本文件的唯一判据来源。
//! The `edge.*.render` expectations come from the C++ oracle building the same tree,
//! and are the sole source of truth here.

use eclog::{
    merge_patch, parse, to_json_compatible, to_typical, Formatting, Number, NumberInfo, Object,
    Renderer, RendererConfig, Str, StringNotation, Value,
};

fn number(raw: &str, info: NumberInfo) -> Value {
    Value::Number(Number::new(raw, info))
}

fn text(raw: &str, notation: StringNotation, delimiter: &str) -> Value {
    Value::Str(Str::from_parts(raw, notation, delimiter))
}

fn raw(raw: &str, delimiter: &str) -> Value {
    text(raw, StringNotation::Raw, delimiter)
}

fn heredoc(raw: &str, delimiter: &str) -> Value {
    text(raw, StringNotation::Heredoc, delimiter)
}

fn unquoted(raw: &str) -> Value {
    text(raw, StringNotation::Unquoted, "")
}

/// 与 `_scratch/eclog-oracle/edge.cpp` 逐项对应。
/// Mirrors `_scratch/eclog-oracle/edge.cpp` item by item.
fn edge_tree() -> Value {
    let mut object = Object::new();

    object.insert("raw_empty", raw("", ""));
    object.insert("raw_ctrl", raw("\u{1f}", ""));
    object.insert("raw_newline", raw("Hello\nWorld!", ""));
    object.insert("raw_path", raw("C:\\Program Files\\Internet Explorer", ""));
    object.insert("raw_quote", raw("\"Hello World!\"", ""));
    object.insert("raw_delim_quote", raw("\"Hello World!\"", "dd"));
    object.insert("raw_delim_hit", raw("\"dd", "dd"));

    object.insert("hd_empty", heredoc("", "T"));
    object.insert("hd_ctrl", heredoc("\u{1f}", "T"));
    object.insert("hd_simple", heredoc("Hello World!", "T"));
    object.insert("hd_tab", heredoc("\tx\n    y\nz", "T"));
    object.insert("hd_tag_line", heredoc("a\nT\nb", "T"));
    object.insert("hd_tag_indent", heredoc("a\n  T\nb", "T"));
    object.insert("hd_tag_inside", heredoc("a\nT99\nb", "T"));
    object.insert("hd_no_delim", heredoc("x", ""));

    object.insert("uq_true", unquoted("true"));
    object.insert("uq_inf", unquoted("inf"));
    object.insert("uq_bare", unquoted("localhost"));
    object.insert("uq_space", unquoted("with space"));
    object.insert("uq_empty", unquoted(""));
    object.insert("uq_lead_digit", unquoted("9lead"));
    object.insert("uq_tail", unquoted("a-b.c"));
    object.insert("uq_utf8", unquoted("你好"));

    object.insert("num_zero", number("0", NumberInfo::NONE));
    object.insert(
        "num_neg_zero",
        number(
            "-0.0",
            NumberInfo::SIGN | NumberInfo::NEG | NumberInfo::FRAC,
        ),
    );
    object.insert(
        "num_seventh",
        number("0.14285714285714285", NumberInfo::FRAC),
    );
    object.insert("num_max", number("1.7976931348623157e308", NumberInfo::EXP));
    object.insert(
        "num_i64max",
        number("9223372036854775807", NumberInfo::NONE),
    );
    object.insert("num_nan", number("nan", NumberInfo::NAN));
    object.insert(
        "num_neg_nan",
        number("-nan", NumberInfo::SIGN | NumberInfo::NEG | NumberInfo::NAN),
    );
    object.insert("num_inf", number("inf", NumberInfo::INF));
    object.insert(
        "num_neg_inf",
        number("-inf", NumberInfo::SIGN | NumberInfo::NEG | NumberInfo::INF),
    );

    // 数值排版边界：定点/指数分界、补零、u64 边界、次正规数
    object.insert("n_1e-4", number("1e-4", NumberInfo::EXP));
    object.insert("n_1e-5", number("1e-5", NumberInfo::EXP));
    object.insert(
        "n_neg_1e-4",
        number(
            "-1e-4",
            NumberInfo::SIGN | NumberInfo::NEG | NumberInfo::EXP,
        ),
    );
    object.insert("n_100", number("100.0", NumberInfo::FRAC));
    object.insert("n_0_001", number("0.001", NumberInfo::FRAC));
    object.insert("n_1e16", number("1e16", NumberInfo::EXP));
    object.insert("n_1e17", number("1e17", NumberInfo::EXP));
    object.insert("n_1234_5678", number("1234.5678", NumberInfo::FRAC));
    object.insert("n_0_1", number("0.1", NumberInfo::FRAC));
    object.insert("n_1_5", number("1.5", NumberInfo::FRAC));
    object.insert("n_min_subnormal", number("5e-324", NumberInfo::EXP));
    object.insert("n_1e-308", number("1e-308", NumberInfo::EXP));
    object.insert(
        "n_123456789012345678",
        number("123456789012345678", NumberInfo::NONE),
    );
    object.insert("n_u64max", number("18446744073709551615", NumberInfo::NONE));
    object.insert(
        "n_i64_plus_one",
        number("9223372036854775808", NumberInfo::NONE),
    );
    object.insert("n_1e18_double", number("1e18", NumberInfo::EXP));

    object.insert(
        Str::from_parts("", StringNotation::Unquoted, ""),
        unquoted("empty_key"),
    );
    object.insert(Str::quoted("q"), unquoted("quoted_key"));
    object.insert(Str::raw("r", ""), unquoted("raw_key"));
    object.insert(Str::heredoc("h", "T"), unquoted("heredoc_key"));

    Value::Object(object)
}

fn render_edge(config: RendererConfig) -> String {
    let tree = edge_tree();
    let object = tree.as_object().expect("object root");

    let mut renderer = Renderer::new(config);
    renderer.begin_root_object();

    for (key, value) in object.iter() {
        renderer.render_member(key, value);
    }

    renderer.render_comment("line one\nline two");

    renderer.begin_array_with_key(&Str::new("inline"));
    renderer.begin_inline();
    for index in 0..3 {
        renderer.render_value(&Value::from(index as i64));
    }
    renderer.end_inline();
    renderer.end_array();

    renderer.end_root_object();
    renderer.close();
    renderer.into_string()
}

fn golden(name: &str) -> String {
    let path = format!("tests/golden/render/{}", name);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {}", path, err));
    assert!(!text.contains('\r'), "golden must stay LF: {}", path);
    text
}

#[test]
fn edge_cases_match_oracle_typical() {
    assert_eq!(
        render_edge(RendererConfig::typical()),
        golden("edge.typical.render")
    );
}

#[test]
fn edge_cases_match_oracle_compact() {
    let mut config = RendererConfig::typical();
    config.formatting = Formatting::Compact;
    assert_eq!(render_edge(config), golden("edge.compact.render"));
}

#[test]
fn empty_root_object_round_trips() {
    // C++ `Renderer::close()` 在未渲染根对象时补一个空对象。
    for (expected, config) in [
        ("{\n}\n", {
            let mut config = RendererConfig::typical();
            config.enclose_root_object_with_curly_brackets = true;
            config
        }),
        ("{}", {
            let mut config = RendererConfig::typical();
            config.formatting = Formatting::Compact;
            config.enclose_root_object_with_curly_brackets = true;
            config
        }),
    ] {
        let mut renderer = Renderer::new(config);
        renderer.close();
        assert_eq!(renderer.into_string(), expected);
    }
}

#[test]
fn empty_document_renders_like_oracle() {
    // oracle: typical → 0 字节；json_compatible → "{\n}\n"
    let doc = parse(b"").expect("empty input");
    assert_eq!(to_typical(&doc), "");
    assert_eq!(to_json_compatible(&doc), "{\n}\n");
}

// ---- RFC 7396 merge patch：移植自 C++ DocumentTests.cpp:1230-1313 ----

fn merged(target: &str, patch: &str) -> Value {
    let mut target = parse(target.as_bytes()).expect("target parses");
    let patch = parse(patch.as_bytes()).expect("patch parses");

    merge_patch(
        target.as_object_mut().expect("target root"),
        patch.as_object().expect("patch root"),
    );

    target
}

fn assert_merge(target: &str, patch: &str, expected: &str) {
    let actual = merged(target, patch);
    let expected = parse(expected.as_bytes()).expect("expected parses");
    assert_eq!(
        actual,
        expected,
        "merge({:?}, {:?}) rendered as {:?}",
        target,
        patch,
        eclog::to_typical(&actual)
    );
}

#[test]
fn merge_patch_replaces_scalars() {
    assert_merge("a: b", "a: c", "a: c");
}

#[test]
fn merge_patch_adds_new_keys() {
    assert_merge("a: b", "b: c", "a: b, b: c");
}

#[test]
fn merge_patch_deletes_with_null() {
    assert_merge("a: b", "a: null", "");
    assert_merge("a: b, b: c", "a: null", "b: c");
}

#[test]
fn merge_patch_replaces_containers() {
    assert_merge("a: [b]", "a: c", "a: c");
    assert_merge("a: c", "a: [b]", "a: [b]");
    assert_merge("a: [{b: c}]", "a: [1]", "a: [1]");
}

#[test]
fn merge_patch_recurses_into_objects() {
    assert_merge("a: {b: c}", "a: {b: d, c: null}", "a: {b: d}");
}

#[test]
fn merge_patch_keeps_existing_null_members() {
    assert_merge("e: null", "a: 1", "e: null, a: 1");
}

#[test]
fn merge_patch_strips_nulls_from_inserted_subtrees() {
    assert_merge("", "a: {bb: {ccc: null}}", "a: {bb: {}}");
    assert_merge(
        "a: {b: {c: true}}",
        "a: {b: {c: {d: null}}}",
        "a: {b: {c: {}}}",
    );
}
