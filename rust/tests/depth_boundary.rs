// SPDX-License-Identifier: MulanPSL-2.0
//! 深度边界：**解析 / 序列化**两套口径必须一致（TODO 雷 9）。
//!
//! 改前：`max_value_depth()` 把根算作 1，而解析器 `Parser::depth` 与各 emit 后端的
//! `depth` 形参都从 0 起 ⇒ 恰好 `MAX_VALUE_DEPTH`(128) 层的文档
//! **能解析、`--to json` 能输出，却 `to_sml` 报 `E-LIMIT-004`**（说 129 > 128）。
//! 属静默类的边界不一致：用户拿到的是"这一份能读进来、却写不回去"的文档。
//!
//! 判据（实测钉住，不写死 128 之外的假设）：
//!   * `MAX_VALUE_DEPTH` 层：**解析成功** 且 **序列化成功**；
//!   * `MAX_VALUE_DEPTH + 1` 层：解析阶段就报 `E-LIMIT-001`。
//! ⚠️ 必须用**闭合**嵌套：不闭合会被 `E-PARSE-001`（未闭合块）先抓住，测到的不是深度。

use sml::{parse, to_sml, to_sml_checked, MAX_VALUE_DEPTH};

/// 闭合的 N 层嵌套块。
fn nest(n: usize) -> String {
    "a { ".repeat(n) + &"} ".repeat(n)
}

#[test]
fn at_limit_parses_and_dumps() {
    let v = parse(&nest(MAX_VALUE_DEPTH))
        .unwrap_or_else(|e| panic!("{} 层应能解析，实得 {}", MAX_VALUE_DEPTH, e.code()));
    to_sml_checked(&v).unwrap_or_else(|e| panic!("{} 层应能序列化（与解析同口径），实得 {e}", MAX_VALUE_DEPTH));
    assert!(!to_sml(&v).is_empty(), "to_sml 不应为空");
}

#[test]
fn over_limit_is_rejected_at_parse() {
    let e = parse(&nest(MAX_VALUE_DEPTH + 1))
        .expect_err("超过上限的嵌套必须报错，而不是栈溢出或静默通过");
    assert_eq!(e.code(), "E-LIMIT-001", "超深嵌套应报 E-LIMIT-001");
}

#[test]
fn just_under_limit_roundtrips() {
    // 上限内两格逐格钉住，防止「改一处守卫就差一格」再次发生
    for n in [MAX_VALUE_DEPTH - 1, MAX_VALUE_DEPTH - 2] {
        let v = parse(&nest(n)).unwrap_or_else(|e| panic!("{n} 层应能解析，实得 {}", e.code()));
        to_sml_checked(&v).unwrap_or_else(|e| panic!("{n} 层应能序列化，实得 {e}"));
    }
}
