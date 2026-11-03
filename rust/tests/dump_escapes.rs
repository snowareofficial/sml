// SPDX-License-Identifier: MulanPSL-2.0
//! `to_sml` 的**转义**回归：控制字符必须写成转义序列，不能以真实字节落进引号串。
//!
//! 为什么单独立文件：这条曾长期是「能跑但偷偷坏」的形态 ——
//! 2026-11-03 之前 `quote_if_needed` 只处理 `\` 与 `"`，于是
//! `s: "u\0v"` dump 出来是 `"u` + **真实 0x00** + `v"`：
//! - Rust 写进文件后文本里带 NUL（再读回要看编码运气）；
//! - C 的 `sml_dump` 在 NUL 处**直接截断**（台账 B-02 记的就是这条）；
//! - `\n` / `\t` 同样是原样字节，输出肉眼看着就断行了。
//!
//! 判据一句话：**dump 出来的东西必须能自己读回去**。

use sml::{to_sml, Value};

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

#[test]
fn control_chars_are_escaped_not_raw() {
    let v = s("x\ny\tz\0w\r");
    let out = to_sml(&v);
    assert!(!out.contains('\n'), "输出里不许有真实换行：{out:?}");
    assert!(!out.contains('\t'), "输出里不许有真实制表符：{out:?}");
    assert!(!out.contains('\u{0}'), "输出里不许有真实 NUL：{out:?}");
    assert!(!out.contains('\r'), "输出里不许有真实回车：{out:?}");
    assert!(out.contains("\\n") && out.contains("\\t") && out.contains("\\0") && out.contains("\\r"));
}

#[test]
fn control_chars_survive_round_trip() {
    let origin = "a\nb\tc\0d";
    let dumped = to_sml(&s(origin));
    // 顶层字符串不是合法 SML 文档，包成键值再读
    let doc = format!("k: {dumped}\n");
    let back = sml::parse(&doc).expect("dump 出来的内容必须能读回去");
    assert_eq!(
        back.get("k").and_then(|x| x.as_str()),
        Some(origin),
        "往返后内容应逐字一致，dump 得到 {dumped:?}"
    );
}

#[test]
fn other_c0_controls_use_unicode_escape() {
    // 词法短形式只有 \n \t \r \0；其余控制字符走 \u00XX
    let v = s("\u{1}\u{1f}\u{7f}");
    let out = to_sml(&v);
    assert!(out.contains("\\u0001"), "0x01 应落成 \\u0001，实际 {out:?}");
    assert!(out.contains("\\u001F") || out.contains("\\u001f"), "0x1F 应落成 \\u001F，实际 {out:?}");
    assert!(out.contains("\\u007F") || out.contains("\\u007f"), "0x7F 应落成 \\u007F，实际 {out:?}");
}
