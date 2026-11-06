#![no_main]
//! L1 文本层模糊：任意 UTF-8 喂给**所有**文本入口，而不是只喂 `parse`。
//!
//! 为什么不是一个入口（HANDOFF §4.4）：`parse_ordered` / `parse_with(collect_errors)`
//! / `parse_with_warnings` / `tokenize` / `parse_include_line` 走的是**不同的状态机**
//! （保序容器构造、同层恢复、词法层、include 指令解析）。只喂 `parse` 的话，
//! 其余四条路径的 panic 与「畸形输入被静默接受」永远撞不出来。
//!
//! 断言口径（HANDOFF §4.5）：
//! 1. 不 panic —— libFuzzer 天然检查；
//! 2. 错误可分类 —— 码非空且登记在码表里；
//! 3. 不 OOM —— 单次输入分配有预算；
//! 4. **往返闭合** —— `parse` 成功 ⇒ `to_sml` ⇒ 再 `parse` ⇒ `eq_ordered` 相等。
//!    注意比的是**有序相等**而不是字节相同：默认 `parse` 产出显式无序对象（HANDOFF §3 第 4 条）。
mod common;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // SML 是 UTF-8 文本格式：非 UTF-8 字节不进文本路径。
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if text.len() > common::MAX_INPUT {
        return;
    }

    let (_, _, bytes) = common::count_allocs(|| {
        // —— 词法层：parse 的上游。词法层 panic 会掩盖语法层的判断 ——
        if let Err(e) = sml_lex::tokenize(text) {
            common::assert_sml_error(&e);
        }

        // —— include 指令行（纯文本入口，不落盘）——
        let features = sml_feature::FeatureSet::for_edition(sml_feature::Edition::V1);
        if let Err(e) = sml_include::parse_include_line(text, features) {
            common::assert_sml_error(&e);
        }

        // —— 历史默认入口：无序对象 ——
        match sml_parse::parse(text) {
            Ok(v) => {
                // 往返闭合：to_sml 的产物必须能重新解析回同一个值（含键序判定）。
                let rendered = sml_value::to_sml(&v);
                match sml_parse::parse(&rendered) {
                    Ok(back) => assert!(
                        v.eq_ordered(&back),
                        "parse→to_sml→parse 值不相等\n原文：{text:?}\n渲染：{rendered:?}"
                    ),
                    Err(e) => panic!("parse→to_sml 的产物重新 parse 失败：{e}\n渲染：{rendered:?}"),
                }
            }
            Err(e) => common::assert_sml_error(&e),
        }

        // —— 保序入口：走 Map::ordered() 构造，容器行为与默认路径不同 ——
        match sml_parse::parse_ordered(text) {
            Ok(v) => {
                let rendered = sml_value::to_sml(&v);
                match sml_parse::parse_ordered(&rendered) {
                    Ok(back) => assert!(
                        v.eq_ordered(&back),
                        "parse_ordered→to_sml→parse_ordered 值不相等\n原文：{text:?}"
                    ),
                    Err(e) => panic!("parse_ordered→to_sml 的产物重新 parse 失败：{e}"),
                }
            }
            Err(e) => common::assert_sml_error(&e),
        }

        // —— collect_errors：同层恢复是另一套状态机；错误数必须被 64 上限夹住 ——
        match sml_parse::parse_with(text, sml_parse::ParseOptions::new().collect_errors(true)) {
            Ok(out) => {
                assert!(
                    out.errors.len() <= 64,
                    "collect_errors 收集了 {} 条错误，超过 64 上限",
                    out.errors.len()
                );
                for e in &out.errors {
                    common::assert_sml_error(e);
                }
            }
            Err(e) => common::assert_sml_error(&e),
        }

        // —— 诊断入口：与 parse 只差返回值，但走的是另一条组装链 ——
        match sml_parse::parse_with_warnings(text) {
            Ok((_v, diags)) => assert!(diags.len() <= 4096, "诊断条数异常：{}", diags.len()),
            Err(e) => common::assert_sml_error(&e),
        }
    });

    common::assert_within_budget("parse 家族", text.len(), bytes);
});
