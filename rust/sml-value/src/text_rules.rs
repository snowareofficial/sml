// SPDX-License-Identifier: MulanPSL-2.0
//! Text-atom rules: quoting/escaping decisions and float spelling.
//! 「文本原子」规则：引号/转义判定与浮点写法。
//!
//! Why a separate module / 为什么单独一个模块：three consumers now — `dump` (`to_sml`),
//! `ser_text` (serde direct emit) and `canonical` (the `.ambr` canonical text).
//! While these lived in `dump` the third one could not reach them (`dump` is gated behind
//! the `sml` feature, `canonical` is not), so the only options were “copy it” — exactly the
//! drift trap called out in `ser_text.rs`. Moved here: **one copy of the rules**.
//! 这三个规则现在有**三个**消费者 —— `dump`、`ser_text`、`canonical`。放在 `dump` 里时
//! `canonical` 拿不到（`dump` 被 `sml` 特性门控），复制一份就是「两份发射器各自漂移」的
//! 老坑。搬到这里后：**规则只有一份**。
//!
//! No feature gate / 不做 feature 门控：only `&str` / `f64`, no third-party, no IO.

/// Whether a string needs quotes — i.e. bare form would lose its type or break syntax.
/// 是否需要给字符串加引号（即：裸词形式会丢失类型或破坏语法）。
use crate::prelude::*;

pub(crate) fn needs_quote(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }
    match s {
        "true" | "false" | "null" | "inf" | "nan" => return true,
        _ => {}
    }
    // 纯数字 / 浮点会被字面量化成 Int/Float
    if s.parse::<i64>().is_ok() || s.parse::<f64>().is_ok() {
        return true;
    }
    // 注释前缀（仅出现在开头才危险）
    if s.starts_with("--")
        || s.starts_with("//")
        || s.starts_with("/*")
        || s.starts_with("*/")
        || s.starts_with("*")
        || s.starts_with("_*")
    {
        return true;
    }
    // 会破坏语法的字符
    if s.contains([' ', '\t', '\n', '\r', ':', '#', '{', '}', ',', '[', ']', '"', '\\', '/', '*'])
    {
        return true;
    }
    // 首字符必须是字母或下划线
    match s.chars().next() {
        Some(c) if c.is_alphabetic() || c == '_' => {}
        _ => return true,
    }
    // 其余字符只允许 [A-Za-z0-9_.-]（`.` 在中间安全，如 `web.example`；
    // 仅 `.` 开头才有歧义，已由首字符规则拦截）
    !s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// Emit a string per the SML bare-word rules: bare when safe, else quoted and escaped.
/// 按 SML 裸词规则输出字符串：能裸就裸，否则加引号并转义。
///
/// NOTE / 注：此处**不能**通过转义 `$` 来阻止 `$env.` 展开 —— `\u{24}` 之类转义在词法
/// 阶段就被解码，解码后的值与未转义形式不可区分，`Tok::Str` 也无从判断是否发生过转义。
/// 该 round-trip 限制已在 `to_sml` 文档中说明（禁用 env 特性即可规避）。
///
/// **Control chars must be escaped** (fixed 2026-11-03) / **控制字符必须转义**：
/// 此前只处理 `\` 与 `"`，于是 `\n` / `\t` / `\0` 会以**真实字节**写进引号串里 ——
/// `s: "u\0v"` dump 出来是 `"u` + 0x00 + `v"`：Rust 自己再读回时文本里带 NUL，
/// C 的 `sml_dump` 更是在 NUL 处**直接截断**（台账 B-02 记的就是这条）。
/// 词法层的转义集是 `\n \t \r \0 \" \\ \uXXXX`，故这四个走短形式，
/// 其余控制字符（0x01–0x1F、0x7F）走 `\u00XX`。
pub(crate) fn quote_if_needed(s: &str) -> Cow<'_, str> {
    if needs_quote(s) {
        let mut out = String::with_capacity(s.len() + 2);
        out.push('"');
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\t' => out.push_str("\\t"),
                '\r' => out.push_str("\\r"),
                '\0' => out.push_str("\\0"),
                c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                    out.push_str(&format!("\\u{:04X}", c as u32));
                }
                c => out.push(c),
            }
        }
        out.push('"');
        Cow::Owned(out)
    } else {
        // 绝大多数键/值不需要引号：借用原文，避免每次发射都分配一个 String。
        // ｜ Borrow when no quoting is needed: the common case costs zero allocations.
        Cow::Borrowed(s)
    }
}

/// 浮点值是否为整数（含 `±0`）：`true` 当且仅当 `f` 有限且没有小数部分。
///
/// 为什么不用 `f.fract() == 0.0`：`fract` / `trunc` / `floor` 都在 `std`（要 libm），
/// no_std 下拿不到。这里直接判指数位与尾数位，纯整数运算、无 unsafe、无 libm。
/// 判据：`|f| < 2^52` 时看尾数低 `52 - 阶码` 位是否全 0；阶码 ≥ 52 时小数位已用尽，
/// 一定是整数；`|f| < 1` 只可能是 `±0`。
/// ｜ Integrality test that works without libm (`fract`/`trunc` are std-only).
pub fn is_integral(f: f64) -> bool {
    let bits = f.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    if exp == 0x7ff {
        return false; // NaN / ±inf
    }
    let frac = bits & 0x000f_ffff_ffff_ffff;
    if exp == 0 {
        return frac == 0; // 次正规数：只有 ±0 是整数
    }
    let unbiased = exp - 1023;
    if unbiased < 0 {
        return false; // 0 < |f| < 1
    }
    if unbiased >= 52 {
        return true; // 小数位已用尽
    }
    frac & ((1u64 << (52 - unbiased)) - 1) == 0
}

/// Format a float — the only consumer of `raw`, i.e. where “keep the original spelling” lives (P3).
/// 格式化浮点值 —— `raw` 的消费点，也是「保留书写形式」的实现处（P3）。
///
/// `raw` 为解析器记录的原始字面量。若 raw 存在且**解析回来仍等于当前值**，
/// 则原样输出，从而保留 `1e10` / `1E10` / `1e+10` 这类书写形式。
/// Rust 的 `f64::Display` 从不输出科学计数法，不存 raw 就必然丢失。
///
/// Why validate “raw parses back to the current value” / **为何要校验「raw 解析回来是否
/// 等于当前值」**：失效规则要求程序构造的 Float 一律 `raw = None`，但万一将来有人绕过该
/// 规则（改了值却留着旧 raw），校验能让它退回默认格式化，而不是输出一个与值不符的字面量
/// —— 把「输出错误数据」降级为「丢失书写形式」。
///
/// **canonical never reads `raw`** (`spec §18 C5/C12`) / **canonical 不走 raw**：
/// raw 是解析期的书写痕迹、不参与相等比较，若让它进 canonical，「同一逻辑数据同字节」
/// 就不成立（同一份数据从文本读入与从 `.ambr` 读入会得到两串不同字节）。
/// canonical 一律 `raw = None` 调用本函数。
pub(crate) fn format_float(f: f64, raw: Option<&str>) -> String {
    if let Some(r) = raw {
        if let Ok(parsed) = r.parse::<f64>() {
            // NaN 自身不等于自身，需单独判定
            if parsed == f || (parsed.is_nan() && f.is_nan()) {
                return r.to_string();
            }
        }
    }
    // 强制保留小数点，避免 1.0 被 Display 写成 "1" 后 round-trip 成 Int
    if !f.is_finite() {
        // NaN/inf 没有合法的 SML 字面量；序列化为带引号字符串，
        // 回读得到 Str 而非非法/改变类型的字面量。
        format!("\"{}\"", f)
    } else if is_integral(f) {
        format!("{:.1}", f)
    } else {
        format!("{}", f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_words_stay_bare() {
        assert_eq!(quote_if_needed("hello"), "hello");
        assert_eq!(quote_if_needed("web.example"), "web.example");
        assert_eq!(quote_if_needed("中文键"), "中文键");
    }

    #[test]
    fn risky_words_are_quoted_and_escaped() {
        assert_eq!(quote_if_needed("007"), "\"007\"");
        assert_eq!(quote_if_needed("a b"), "\"a b\"");
        assert_eq!(quote_if_needed("u\0v"), "\"u\\0v\"");
        assert_eq!(quote_if_needed("\u{1}"), "\"\\u0001\"");
    }

    #[test]
    fn float_keeps_dot_zero_and_sign_of_negative_zero() {
        assert_eq!(format_float(1.0, None), "1.0");
        assert_eq!(format_float(-0.0, None), "-0.0");
        assert_eq!(format_float(1.5, None), "1.5");
    }

    /// `is_integral` 必须与 `f.fract() == 0.0` 逐点同结论 —— 它是后者的 no_std 替身。
    #[test]
    fn integral_matches_std_fract() {
        let cases = [
            0.0f64,
            -0.0,
            1.0,
            -1.0,
            1.5,
            -1.5,
            0.5,
            -0.5,
            1e-300,
            5e-324,
            2f64.powi(52),
            2f64.powi(53),
            -2f64.powi(53),
            4503599627370496.5, // 不可表示，会落到 2^52
            1e20,
            -1e20,
            f64::MAX,
            f64::MIN,
            f64::MIN_POSITIVE,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];
        for f in cases {
            assert_eq!(is_integral(f), f.fract() == 0.0, "value = {f:?}");
        }
        // 接缝两侧：`unbiased == 52` 起小数位用尽，再往下 0.5 还能表示
        assert!(is_integral(4503599627370496.0)); // 2^52
        assert!(!is_integral(4503599627370495.5)); // 2^52 - 0.5
    }
}
