//! 数字词法边界回归测试（对应《SML 数字字面量保真性审计报告》）。
//!
//! 核心原则：**数字形态的词才是数字，其余裸词一律是字符串。**
//!
//! 已守护：
//! - P3：科学计数法可解析（写法不保真是另一回事，见审计 2.3）
//! - P4：裸词 `inf` / `nan` 系列为字符串，不再是浮点数
//!        （Rust 的 f64 解析器接受它们且大小写不敏感，属挪威问题同类）
//! - 溢出（1e309）仍产生 Float(inf)：这是当前唯一会解析出非有限值的路径，
//!   因此序列化侧的 null 防护（c_abi::jsonify_nan_inf_becomes_null）仍然必要
//!
//! 已修复（用户实现）：
//! - P1：前导零/大整数保护的**符号不对称**
//!        根因是 `sml-lex/src/lib.rs` 判断前导零时用的是整个词 `w`，
//!        而 `-007` 首字符是 `-` 不是 `0`；`looks_int` 的字符集也漏了 `-`。
//!        修法：先 `strip_prefix(['+', '-'])` 得到 `digits`，后续判断统一用
//!        `digits`；`looks_int` 字符集补上 `-`。
//!        注意：符号本身**不需要**特殊处理 —— Rust 的 `parse::<i64>()`
//!        对 `007` / `+007` / `-007` 全部返回 Ok，符号由标准库消化。

use sml::{parse, Value};

/// 解析 `v: <literal>` 并返回标量值。
fn scalar_of(literal: &str) -> Value {
    let text = format!("v: {literal}\n");
    let root = parse(&text).unwrap_or_else(|e| panic!("解析失败 `{literal}`: {e}"));
    root.get("v").cloned().unwrap_or(Value::Null)
}

// ---------------------------------------------------------------------------
// P4：裸词 inf / nan 系列必须是字符串
// ---------------------------------------------------------------------------

#[test]
fn p4_inf_nan_family_is_string() {
    for lit in [
        "inf", "INF", "Inf", "infinity", "Infinity", "INFINITY",
        "-inf", "+inf", "nan", "NaN", "NAN", "-nan",
    ] {
        // Value 实现了 Drop，必须按引用匹配，否则 E0509
        match &scalar_of(lit) {
            Value::Str(s) => assert_eq!(s.as_str(), lit, "裸词 `{lit}` 应原样保留为字符串"),
            other => panic!("裸词 `{lit}` 应解析为 Str，实际 {other:?}"),
        }
    }
}

#[test]
fn p4_inf_nan_roundtrip_is_stable() {
    // 回归：修复前 `nan` -> Float(NaN) -> dump "NaN" -> 再解析成 Str("NaN")，
    // 类型与大小写双双改变。现在从一开始就是字符串，往返应完全稳定。
    let lit = "nan";
    let first = scalar_of(lit);
    let dumped = sml::to_sml(&parse(&format!("v: {lit}\n")).unwrap());
    let second = scalar_of(dumped.trim().strip_prefix("v: ").unwrap_or(&dumped));
    assert_eq!(first, second, "round-trip 后值改变: {first:?} -> {second:?}");
}

// ---------------------------------------------------------------------------
// 溢出仍是唯一的非有限值入口（numeric_head 闸门不得误伤）
// ---------------------------------------------------------------------------

#[test]
fn overflow_still_yields_non_finite_float() {
    for lit in ["1e309", "-1e309", "1e400"] {
        match scalar_of(lit) {
            Value::Float(f, _) => assert!(!f.is_finite(), "`{lit}` 应为非有限 Float，实际 {f}"),
            other => panic!("`{lit}` 应解析为 Float(inf)，实际 {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// 回归保护：正常与边界数字形态不受影响
// ---------------------------------------------------------------------------

#[test]
fn plain_numbers_unchanged() {
    assert_eq!(scalar_of("27"), Value::Int(27));
    assert_eq!(scalar_of("-3"), Value::Int(-3));
    assert_eq!(scalar_of("0"), Value::Int(0));
    assert_eq!(scalar_of("1.5"), Value::float(1.5));
    assert_eq!(scalar_of("0.5"), Value::float(0.5));
    assert_eq!(scalar_of("1.0"), Value::float(1.0));
}

#[test]
fn scientific_notation_still_parses() {
    // 写法能否 round-trip 是 P3 的问题；这里只守护「能解析成正确的值」
    assert_eq!(scalar_of("1e10"), Value::float(1e10));
    assert_eq!(scalar_of("1E10"), Value::float(1e10));
    assert_eq!(scalar_of("1e+10"), Value::float(1e10));
    assert_eq!(scalar_of("1e-10"), Value::float(1e-10));
}

#[test]
fn dot_forms_still_parse() {
    assert_eq!(scalar_of(".5"), Value::float(0.5));
    assert_eq!(scalar_of("5."), Value::float(5.0));
    assert_eq!(scalar_of("-.5"), Value::float(-0.5));
}

#[test]
fn unsigned_leading_zero_stays_string() {
    // P1-4 既有保护，不得被本次改动破坏
    assert_eq!(scalar_of("007"), Value::Str("007".into()));
    assert_eq!(scalar_of("0755"), Value::Str("0755".into()));
    assert_eq!(scalar_of("01234"), Value::Str("01234".into()));
}

// ---------------------------------------------------------------------------
// 政务数据实测（对应手册 3.4「⚠️ 推测、未实测」的风险清单）
//
// 实测结论见报告第七章。核心发现：Rust 侧 11 项中仅 3 项有风险，而其中
// 2 项（版本号 1.0、金额）是**判据过严导致的误报**——数值本身完全正确；
// 真正的风险只有 1 项：18 位身份证号在 Rust 侧精确，但一旦传给 JS 消费者
// 就会丢精度（超出 2^53）。
// ---------------------------------------------------------------------------

#[test]
fn gov_leading_zero_identifiers_stay_text() {
    // 前导零类编码：行政区划（补零形态）、机构代码、电话区号
    for lit in ["0571", "010101", "0571-87052651"] {
        match &scalar_of(lit) {
            Value::Str(s) => assert_eq!(s.as_str(), lit, "政务编码 `{lit}` 应保持原样"),
            other => panic!("政务编码 `{lit}` 应为 Str，实际 {other:?}"),
        }
    }
}

#[test]
fn gov_long_ids_beyond_i64_stay_text() {
    // 超过 i64 上界的编号：Rust 侧保精度
    let lit = "12345678901234567890";
    assert_eq!(scalar_of(lit), Value::Str(lit.into()));
}

#[test]
fn gov_id_card_is_exact_in_rust_but_not_in_js() {
    // 18 位身份证号：3.3e17 < i64::MAX，故 Rust 侧为精确 Int，不丢精度。
    let lit = "330106201503071234";
    assert_eq!(scalar_of(lit), Value::Int(330106201503071234));
    assert!(
        scalar_of(lit).as_float().is_none(),
        "身份证号在 Rust 侧是精确整数"
    );
    // 但它超出 JS 的 2^53，跨到 JS 侧即损坏 —— 见 report 与 _audit_gov.mjs 的对照。
    // 这是「Rust 正确」与「生态安全」之间的落差，需在使用层用 str 契约约束。
}

#[test]
fn gov_contract_can_pin_id_card_as_str() {
    // 实战解法：用契约把敏感长号约束为 str，解析期即拦截。
    // 这印证了「契约不能改变类型推断，但能把静默损坏变成必须处理的警报」。
    let e = parse(
        "@contract Person {\n  id_card: str\n}\np {\n  @is Person\n  id_card: 330106201503071234\n}\n",
    );
    assert!(e.is_err(), "未加引号的长号应被 str 契约拒绝: {e:?}");
}

#[test]
fn gov_credit_code_containing_letters_is_text() {
    // 统一社会信用代码含字母，两侧均为字符串
    let lit = "91330100MA2XXXXX7B";
    assert_eq!(scalar_of(lit), Value::Str(lit.into()));
}

// ---------------------------------------------------------------------------
// P3：书写形式保真（值模型存 raw 后）
// ---------------------------------------------------------------------------

/// 解析 `v: <literal>` 再 dump，返回输出的字面量文本。
fn roundtrip_literal(literal: &str) -> String {
    let v = parse(&format!("v: {literal}\n")).unwrap();
    let dumped = sml::to_sml(&v);
    dumped
        .trim()
        .strip_prefix("v: ")
        .unwrap_or_else(|| dumped.trim())
        .trim()
        .to_string()
}

#[test]
fn p3_scientific_notation_keeps_form() {
    // 修复前：1e10 -> 10000000000.0；1e100 -> 101 位定点数。
    // f64 无法表达「作者写的是科学计数法」，不存 raw 就必然丢失。
    for lit in ["1e10", "1E10", "1e+10", "1e-10", "1e2", "1e100", "1e-320"] {
        assert_eq!(roundtrip_literal(lit), lit, "科学计数法 `{lit}` 应原样往返");
    }
}

#[test]
fn p3_trailing_zero_and_dot_forms_kept() {
    for lit in ["1.10", "1.000", ".5", "5.", "1.0"] {
        assert_eq!(roundtrip_literal(lit), lit, "书写形式 `{lit}` 应原样往返");
    }
}

#[test]
fn parsed_float_carries_raw() {
    let v = parse("v: 1e10\n").unwrap();
    assert_eq!(
        v.get("v").and_then(|x| x.float_raw()),
        Some("1e10"),
        "解析出的 Float 应携带原始字面量"
    );
}

#[test]
fn program_constructed_float_has_no_raw() {
    // 失效规则：程序构造一律 raw = None
    assert!(Value::float(1e10).float_raw().is_none());
}

#[test]
fn raw_does_not_affect_equality() {
    // 相等性属「值」层面：解析出的（带 raw）== 程序构造的（无 raw）
    let parsed = parse("v: 1e10\n").unwrap();
    assert_eq!(
        parsed.get("v").cloned().unwrap(),
        Value::float(1e10),
        "raw 不应影响值的相等性"
    );
}

#[test]
fn stale_raw_is_ignored_not_emitted() {
    // 防御：构造一个「值与 raw 不符」的 Float。万一将来有人绕过失效规则，
    // dump 必须退回默认格式化，而不是输出一个与值不符的字面量 ——
    // 把「输出错误数据」降级为「丢失书写形式」。
    let v = Value::float_with_raw(2e10, "1e10");
    let out = sml::to_sml(&v);
    assert!(
        !out.contains("1e10"),
        "过期的 raw 不应被输出（否则值与写法不符）: {out}"
    );
}

#[test]
fn bigint_beyond_i64_stays_string() {
    // B10 既有保护（正数），不得被本次改动破坏
    assert_eq!(
        scalar_of("12345678901234567890"),
        Value::Str("12345678901234567890".into())
    );
}

// ---------------------------------------------------------------------------
// P1 待修：符号不对称。修复后删除 #[ignore] 即可验收。
// ---------------------------------------------------------------------------

#[test]
fn p1_signed_leading_zero_should_stay_string() {
    for lit in ["-007", "+007", "-0755"] {
        match &scalar_of(lit) {
            Value::Str(s) => assert_eq!(s.as_str(), lit, "带符号前导零 `{lit}` 应原样保留"),
            other => panic!("带符号前导零 `{lit}` 应解析为 Str，实际 {other:?}"),
        }
    }
}

#[test]
fn p1_negative_bigint_should_stay_string() {
    // 现状：-99999999999999999999 -> Float(-1e20)，精度丢失
    for lit in ["-12345678901234567890", "-99999999999999999999"] {
        match &scalar_of(lit) {
            Value::Str(s) => assert_eq!(s.as_str(), lit, "负大整数 `{lit}` 应保为字符串"),
            other => panic!("负大整数 `{lit}` 应解析为 Str，实际 {other:?}"),
        }
    }
}
