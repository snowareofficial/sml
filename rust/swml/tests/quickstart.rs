// SPDX-License-Identifier: MulanPSL-2.0
//! 这一层只承诺 4 个概念能跑通。**每条用例都对应 README 里的一句话**，
//! 所以 README 与行为不会各说各话。

use swml::easy::Doc;

#[test]
fn read_a_field_by_dotted_path() {
    let doc = Doc::read("server { host: localhost port: 8080 }").unwrap();
    assert_eq!(doc.str("server.host"), Some("localhost"));
    assert_eq!(doc.int("server.port"), Some(8080));
}

#[test]
fn missing_field_is_none_not_an_error() {
    let doc = Doc::read("a: 1").unwrap();
    assert_eq!(doc.str("nope"), None);
    assert_eq!(doc.int("a.deep.deeper"), None);
    assert!(!doc.has("nope"));
    // 存在但类型不符：同样只是 None
    assert!(!doc.str("a").is_some_and(|s| s.is_empty()));
    assert_eq!(doc.str("a"), None);
}

#[test]
fn errors_carry_a_stable_code() {
    let err = Doc::read("42").unwrap_err();
    assert_eq!(err.code(), "E-PARSE-008");
    assert!(!err.to_string().is_empty());
}

#[test]
fn numeric_looking_strings_are_not_ints() {
    // README「坑 2」：前导零与大整数在 SML 里都是**字符串**（保语义、保精度）
    let doc = Doc::read("mode: 0755 big: 9223372036854775808 normal: 42").unwrap();
    assert_eq!(doc.int("mode"), None);
    assert_eq!(doc.int("big"), None);
    assert_eq!(doc.int("normal"), Some(42));
    // 但也确实拿得到原文
    assert_eq!(doc.str("mode"), Some("0755"));
}

#[test]
fn list_has_no_index_path() {
    let doc = Doc::read("tags: [ a b ]").unwrap();
    let tags = doc.list("tags").expect("tags 应是数组");
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[1].as_str(), Some("b"));
    // 路径语法里没有下标 —— 这条钉住「别以为能这么写」
    assert_eq!(doc.str("tags.0"), None);
}

#[test]
fn read_file_expands_include() {
    let dir = std::env::temp_dir().join(format!("smlkit_easy_inc_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sub.sml"), "sub: 2\n").unwrap();
    std::fs::write(dir.join("main.sml"), "include \"sub.sml\"\nroot: 1\n").unwrap();

    let doc = Doc::read_file(dir.join("main.sml")).unwrap();
    assert_eq!(doc.int("root"), Some(1));
    assert_eq!(doc.int("sub"), Some(2), "include 应被真正展开（不是留在值树里）");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn escape_hatch_and_conversions() {
    use swml::easy::Error;

    let doc = Doc::read("a: 1").unwrap();
    // raw() 给到底层 Value
    assert_eq!(doc.raw().get("a").and_then(|v| v.as_i64()), Some(1));
    // Display 与 to_sml 同源
    assert_eq!(doc.to_string(), doc.to_sml());
    // 能整体转成 Value
    let value: swml::easy::Value = doc.clone().into();
    assert_eq!(value.get("a").and_then(|v| v.as_i64()), Some(1));

    // Error 就是 SmlError 的别名（同一类型，不是包装）
    let e: Result<Doc, Error> = Doc::read("42");
    let err = e.unwrap_err();
    assert_eq!(err.code(), "E-PARSE-008");
}
