// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
//! golden 对拍：Eclog-CPP `Tests/Text/`（MIT，Copyright (c) Vallest Systems LLC）。
//! Golden conformance against Eclog-CPP `Tests/Text/` (MIT).
//!
//! 判据两条，缺一不可：
//! Two checks, both required:
//!
//! 1. `parse_with` 产生的事件序列与 `.check` 逐行一致；
//!    the event stream from `parse_with` matches `.check` line by line;
//! 2. 同一输入经 `parse` 建树后再序列化成事件，与 `.check` 也一致 —— 保证两个出口
//!    不会各自漂移。
//!    the tree from `parse`, serialized back into events, matches too — so the two
//!    outlets cannot drift apart.
//!
//! 向量必须保持上游的 LF 字节（`.gitattributes` 已钉 `*.ecl` / `*.check`）。
//! Vectors must keep the upstream LF bytes.

use eclog::{parse, parse_with, render, NumberInfo, ParseHandler, RendererConfig, Str, Value};

/// 与 C++ `DebugParseHandler` 同形的输出器。
/// The same shape as C++ `DebugParseHandler`.
struct CheckWriter {
    out: String,
}

impl CheckWriter {
    fn new() -> Self {
        Self { out: String::new() }
    }

    fn line(&mut self, text: &str) {
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn key_line(&mut self, text: &str) {
        let mut line = String::from("key ");
        escape_into(&mut line, text);
        self.line(&line);
    }
}

impl ParseHandler for CheckWriter {
    fn on_object_begin(&mut self) {
        self.line("object");
    }

    fn on_object_end(&mut self) {
        self.line("object_end");
    }

    fn on_array_begin(&mut self) {
        self.line("array");
    }

    fn on_array_end(&mut self) {
        self.line("array_end");
    }

    fn on_key(&mut self, key: &Str) {
        self.key_line(key.text());
    }

    fn on_value(&mut self, value: &Value) {
        let mut line = String::from("value ");
        match value {
            Value::Str(s) => escape_into(&mut line, s.text()),
            Value::Number(n) => line.push_str(n.raw()),
            Value::Bool(true) => line.push_str("true"),
            Value::Bool(false) => line.push_str("false"),
            Value::Null => line.push_str("null"),
            Value::Array(_) | Value::Object(_) => {
                unreachable!("containers are reported through begin/end")
            }
        }
        line.push_str(" type ");
        line.push_str(value.type_name());
        if let Some(number) = value.as_number() {
            line.push_str(" info ");
            line.push_str(&number.info().to_string());
        }
        self.line(&line);
    }
}

/// C++ `DebugParseHandler::escapeString` 的等价实现。
/// Equivalent of C++ `DebugParseHandler::escapeString`.
fn escape_into(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\0' => out.push_str("\\0"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
}

fn events_of(source: &str) -> String {
    let mut writer = CheckWriter::new();
    parse_with(source.as_bytes(), &mut writer).expect("event parse failed");
    writer.out
}

fn events_of_tree(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value);
    out
}

fn write_value(out: &mut String, value: &Value) {
    match value {
        Value::Object(object) => {
            out.push_str("object\n");
            for (key, child) in object.iter() {
                out.push_str("key ");
                escape_into(out, key.text());
                out.push('\n');
                write_value(out, child);
            }
            out.push_str("object_end\n");
        }
        Value::Array(items) => {
            out.push_str("array\n");
            for child in items {
                write_value(out, child);
            }
            out.push_str("array_end\n");
        }
        Value::Null => out.push_str("value null type null\n"),
        Value::Bool(true) => out.push_str("value true type boolean\n"),
        Value::Bool(false) => out.push_str("value false type boolean\n"),
        Value::Number(number) => {
            out.push_str("value ");
            out.push_str(number.raw());
            out.push_str(" type number info ");
            out.push_str(&number.info().to_string());
            out.push('\n');
        }
        Value::Str(text) => {
            out.push_str("value ");
            escape_into(out, text.text());
            out.push_str(" type string\n");
        }
    }
}

/// 14 组 / 23 个输入，与上游 `Tests/Source/ParseTests.cpp` 的清单一致。
/// 14 groups / 23 inputs, matching the upstream `ParseTests.cpp` list.
const CASES: &[(&str, &[&str])] = &[
    ("Array", &["ecl", "json"]),
    ("Boolean", &["ecl", "json"]),
    ("Heredoc", &["ecl"]),
    ("Inline", &["ecl", "json"]),
    ("Nesting", &["ecl", "json"]),
    ("Null", &["ecl", "json"]),
    ("Number", &["ecl", "json"]),
    ("Object", &["ecl", "json"]),
    ("Person", &["ecl", "json"]),
    ("QuotedString", &["ecl", "json"]),
    ("RawString", &["ecl"]),
    ("StringConcatenation", &["ecl"]),
    ("TrailingComma", &["ecl"]),
    ("UnquotedString", &["ecl"]),
];

#[test]
fn golden_events_and_tree_match() {
    let mut checked = 0usize;

    for (name, extensions) in CASES {
        for extension in *extensions {
            let input_path = format!("tests/golden/{}.{}", name, extension);
            let bytes = std::fs::read(&input_path)
                .unwrap_or_else(|err| panic!("read {}: {}", input_path, err));
            let expectation_path = format!("{}.check", input_path);
            let expected = std::fs::read_to_string(&expectation_path)
                .unwrap_or_else(|err| panic!("read {}: {}", expectation_path, err));

            assert!(
                !expected.contains('\r'),
                "golden must stay LF: {}",
                expectation_path
            );

            let source = std::str::from_utf8(&bytes).expect("golden input is UTF-8");

            let events = events_of(source);
            assert_eq!(
                events, expected,
                "event stream mismatch: {}.{}",
                name, extension
            );

            let tree = parse(&bytes).expect("tree parse failed");
            assert_eq!(
                events_of_tree(&tree),
                expected,
                "tree/event drift: {}.{}",
                name,
                extension
            );

            checked += 1;
        }
    }

    assert_eq!(checked, 23, "expected 23 golden inputs");
}

#[test]
fn golden_directory_is_complete() {
    let files = std::fs::read_dir("tests/golden")
        .expect("golden directory")
        .filter(|entry| entry.as_ref().map(|e| e.path().is_file()).unwrap_or(false))
        .count();
    assert_eq!(files, 46, "23 inputs + 23 expectations");

    let renders = std::fs::read_dir("tests/golden/render")
        .expect("render expectations")
        .count();
    assert_eq!(renders, 48, "23 inputs x 2 schemes + 2 edge cases");
}

#[test]
fn render_matches_oracle() {
    let mut checked = 0usize;

    for (name, extensions) in CASES {
        for extension in *extensions {
            let input_path = format!("tests/golden/{}.{}", name, extension);
            let bytes = std::fs::read(&input_path)
                .unwrap_or_else(|err| panic!("read {}: {}", input_path, err));
            let tree = parse(&bytes).expect("tree parse failed");

            for (suffix, config) in [
                ("typical", RendererConfig::typical()),
                ("json", RendererConfig::json_compatible()),
            ] {
                let path = format!(
                    "tests/golden/render/{}.{}.{}.render",
                    name, extension, suffix
                );
                let expected = std::fs::read_to_string(&path)
                    .unwrap_or_else(|err| panic!("read {}: {}", path, err));

                assert!(!expected.contains('\r'), "golden must stay LF: {}", path);
                assert_eq!(
                    render(&tree, &config),
                    expected,
                    "render mismatch: {}",
                    path
                );

                checked += 1;
            }
        }
    }

    assert_eq!(checked, 46, "23 inputs x 2 schemes");
}

#[test]
fn number_info_display_matches_check_names() {
    let info = NumberInfo::SIGN | NumberInfo::NEG | NumberInfo::INF;
    assert_eq!(info.to_string(), "sign|neg|inf");
    assert_eq!(NumberInfo::NONE.to_string(), "none");
}
