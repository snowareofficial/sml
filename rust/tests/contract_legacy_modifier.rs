// SPDX-License-Identifier: MulanPSL-2.0
//! 遗留契约符号修饰符 `?` / `=` 的**五端统一拒绝**回归（Rust 侧）。
//!
//! 背景：`?`（可选）与 `=`（默认值）已废弃（2026-10-30），迁移到
//! `optional` / `default`。Rust 侧的坑是**粘连式**（`str?`、`int=5`）：
//! 词法把这两个字符并进 Word，于是 `str?` 被当成普通类型名 —— 字段照旧必填，
//! 用户拿到的是 `E-CONTRACT-003`（必填缺失）而不是 `E-PARSE-006`，与另外三端不一致。
//! `tests/conformance/cases/35_contract_modifier_question_only.sml` 就是为钉住这格而设。
//!
//! ⚠️ 为什么不在词法层切开 `?`：`?` 在 `@pattern` 里是量词（`sml-pattern`），
//! 全局切分会把 `a?` 这类合法模式拆坏。故在契约字段的**类型位置**定点拦截。

use sml::parse;

fn code_of(src: &str) -> &'static str {
    match parse(src) {
        Ok(_) => panic!("应当解析失败，实际成功了：{src:?}"),
        Err(e) => e.code(),
    }
}

fn assert_code(src: &str, want: &str) {
    let got = code_of(src);
    assert_eq!(got, want, "源码 {src:?} 的错误码不符");
}

/// 粘连式 `?`：此前报 E-CONTRACT-003（把 `str?` 当类型名），现应为 E-PARSE-006。
#[test]
fn glued_question_is_rejected() {
    assert_code("@contract S {\n x: str?\n}\na { @is S }\n", "E-PARSE-006");
}

/// 带空格的 `?`：词法上已是独立 Word，此路径本来就报同码 —— 留着防回归。
#[test]
fn spaced_question_is_rejected() {
    assert_code("@contract S {\n x: str ?\n}\na { @is S }\n", "E-PARSE-006");
}

/// 带空格的 `=`（默认值遗留写法）。
#[test]
fn spaced_eq_is_rejected() {
    assert_code("@contract S {\n p: int = 5\n}\na { @is S }\n", "E-PARSE-006");
}

/// 粘连式 `=`：此前同样被当类型名吞掉（`int=5` 成一个 Word）。
#[test]
fn glued_eq_is_rejected() {
    assert_code("@contract S {\n p: int=5\n}\na { @is S }\n", "E-PARSE-006");
}

/// 数组元素类型位置也一并拦（`[ str? ]` 不该被当成类型名）。
#[test]
fn glued_symbol_in_array_elem_is_rejected() {
    assert_code("@contract S {\n t: [ str? ]\n}\na { @is S }\n", "E-PARSE-006");
}

/// 正对照：单词式不受影响（不能把合法写法一起拦掉）。
#[test]
fn word_modifiers_still_work() {
    let v = parse("@contract S {\n x: str optional\n p: int default 5432\n}\na { @is S }\n")
        .expect("单词式修饰符应正常解析");
    let s = format!("{v:?}");
    assert!(s.contains("5432"), "default 应回填：{s}");
}
