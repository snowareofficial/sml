// SPDX-License-Identifier: MulanPSL-2.0
//! SML 注释语法集成测试：单行 `#` / `--`，多行 `/* */` / `_* *_`。

use sml::parse;

#[test]
fn single_hash_line_comment() {
    let v = parse(
        "a: 1 # 行尾注释
b: 2 # 另一个",
    )
    .unwrap();
    assert_eq!(v.get("a").unwrap(), &sml::Value::Int(1));
    assert_eq!(v.get("b").unwrap(), &sml::Value::Int(2));
}

#[test]
fn single_dash_dash_line_comment() {
    let v = parse(
        "a: 1 -- 行尾注释
b: 2 -- 另一个",
    )
    .unwrap();
    assert_eq!(v.get("a").unwrap(), &sml::Value::Int(1));
    assert_eq!(v.get("b").unwrap(), &sml::Value::Int(2));
}

#[test]
fn multi_block_slash_star() {
    let text = "/*
  多行注释
  a: 999  # 被忽略
*/
a: 1
b: 2";
    let v = parse(text).unwrap();
    assert_eq!(v.get("a").unwrap(), &sml::Value::Int(1));
    assert_eq!(v.get("b").unwrap(), &sml::Value::Int(2));
}

#[test]
fn multi_block_underscore_star() {
    let text = "/*
  错误写法说明：也支持 _* *_ 形式
*/
_* 这是另一种
   多行注释
*_
a: 1
b: 2";
    let v = parse(text).unwrap();
    assert_eq!(v.get("a").unwrap(), &sml::Value::Int(1));
    assert_eq!(v.get("b").unwrap(), &sml::Value::Int(2));
}

#[test]
fn comment_inside_block_and_array() {
    let text = "server {
  port: 8080 -- 监听端口
  /* hosts 列表 */
  hosts: [
    a -- 主
    b # 备
  ]
}";
    let v = parse(text).unwrap();
    let server = v.get("server").unwrap();
    assert_eq!(server.get("port").unwrap(), &sml::Value::Int(8080));
    assert_eq!(
        server.get("hosts").unwrap(),
        &sml::Value::Array(vec![sml::Value::Str("a".into()), sml::Value::Str("b".into())])
    );
}

#[test]
fn dash_not_comment_when_single() {
    // 单个 `-` 不是注释，应作为裸词字符保留（如负数、带连字符的词）
    let v = parse("a: -5
b: my-word").unwrap();
    assert_eq!(v.get("a").unwrap(), &sml::Value::Int(-5));
    assert_eq!(v.get("b").unwrap(), &sml::Value::Str("my-word".into()));
}

#[test]
fn slash_not_comment_when_single() {
    let v = parse("path: a/b/c").unwrap();
    assert_eq!(v.get("path").unwrap(), &sml::Value::Str("a/b/c".into()));
}

#[test]
fn underscore_not_comment_when_single() {
    let v = parse("id: foo_bar").unwrap();
    assert_eq!(v.get("id").unwrap(), &sml::Value::Str("foo_bar".into()));
}
