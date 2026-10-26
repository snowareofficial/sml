// SPDX-License-Identifier: MulanPSL-2.0
// I-9：契约名写错时给出「你是不是想写 X」近似提示。
//
// 只验证报错文案的候选提示行为，不改动解析语义：错误码、是否报错、解析行为均不变。

use sml::parse;

/// 取解析失败的错误文案（含码）：`文案 [E-CONTRACT-001]`。
fn err(text: &str) -> String {
    match parse(text) {
        Ok(_) => panic!("应校验失败，实际通过了:\n{text}"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn typo_one_char_suggests_correct_contract() {
    // ① 拼错一个字符：Securty -> Security
    let e = err("@contract Security { host: str }\n x { @is Securty }");
    assert!(e.contains("未定义的契约 `Securty`"), "got: {e}");
    assert!(
        e.contains("你是不是想写 `Security`？"),
        "拼错一个字符应给出正确候选，got: {e}"
    );
}

#[test]
fn unrelated_name_has_no_suggestion() {
    // ② 完全无关的名字：不应给出任何候选
    let e = err("@contract Security { host: str }\n x { @is Zzzztop }");
    assert!(e.contains("未定义的契约 `Zzzztop`"), "got: {e}");
    assert!(
        !e.contains("你是不是想写"),
        "完全无关的名字不应给出候选，got: {e}"
    );
}

#[test]
fn empty_contract_table_no_suggestion_no_panic() {
    // ③ 契约表为空：任何 @is 都未定义，但不应 panic、不应追加提示
    let e = err("x { @is Whatever }");
    assert!(e.contains("未定义的契约 `Whatever`"), "got: {e}");
    assert!(
        !e.contains("你是不是想写"),
        "空契约表不应追加候选提示，got: {e}"
    );
}

#[test]
fn composition_ref_typo_suggests_correct_contract() {
    // 额外覆盖 lib.rs 的「字段引用未定义契约」路径：Addres -> Address
    // 注意 `@is Server` 必须单独成行，否则 `addr` 会被当成 `@is` 的第二个契约参数。
    let e = err(
        "@contract Address { city: str }
         @contract Server { addr: Addres }
         db {
           @is Server
           addr { city: x }
         }",
    );
    assert!(e.contains("未定义的契约 `Addres`"), "got: {e}");
    assert!(
        e.contains("你是不是想写 `Address`？"),
        "组合字段引用拼错应给出正确候选，got: {e}"
    );
}
