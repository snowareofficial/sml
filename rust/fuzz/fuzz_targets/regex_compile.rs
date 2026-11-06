#![no_main]
//! L4 受限正则模糊：`compile_regex_checked` / `regex_matches_checked`。
//!
//! 为什么值得一个 target（HANDOFF §4.3 第 8 位，标注「可选」）：
//! 它是 `include "*.sml"` 的实现，输入来自用户的模式串；且 `sml-regex` 是**零依赖自研引擎**，
//! 手写解析器最容易在「未闭合字符类 / 悬空反斜杠 / 连续量词」上 panic。
//!
//! 输入布局：`<模式串> 0x00 <被匹配文本>`；没有 `0x00` 时整段当模式串、文本为空。
//!
//! 断言：
//! 1. 两个入口对同一 (模式, 文本) 必须**同答**（`checked` 只多一种失败：预算耗尽）；
//! 2. 错误必须落在 `RegexError` 的三类里且文案非空；
//! 3. 超长模式：`checked` 报 `TooLong`，不会失败的 `compile_regex` 必须退化成「不匹配」。
mod common;

use libfuzzer_sys::fuzz_target;
use sml_regex::{
    compile_regex, compile_regex_checked, regex_matches, regex_matches_checked, RegexError,
    MAX_REGEX_LEN,
};

fn assert_regex_error(e: &RegexError) {
    assert!(!e.to_string().is_empty(), "正则错误文案为空：{e:?}");
}

fuzz_target!(|data: &[u8]| {
    if data.len() > common::MAX_INPUT {
        return;
    }

    let (pat_raw, text_raw) = match data.iter().position(|b| *b == 0) {
        Some(i) => (&data[..i], &data[i + 1..]),
        None => (data, &data[data.len()..]),
    };
    let (Ok(pat), Ok(text)) = (std::str::from_utf8(pat_raw), std::str::from_utf8(text_raw)) else {
        return;
    };

    let (_, _, bytes) = common::count_allocs(|| {
        // —— 显式失败入口：错误必须分类 ——
        match compile_regex_checked(pat) {
            Ok(re) => {
                let silent = regex_matches(&re, text);
                match regex_matches_checked(&re, text) {
                    Ok(checked) => assert_eq!(
                        checked, silent,
                        "同一 (模式, 文本) 两个匹配入口给出不同答案：模式 {pat:?}"
                    ),
                    Err(RegexError::Budget { .. }) => {
                        // 预算耗尽：`checked` 报错，不会失败的入口静默判「不匹配」。
                        assert!(!silent, "预算耗尽时无失败入口必须判不匹配");
                    }
                    Err(e) => panic!("匹配阶段报出编译期才该有的错误：{e:?}"),
                }
            }
            Err(e) => {
                assert_regex_error(&e);
                // 编译失败的三类：过长 / 非法。`Budget` 只可能出现在匹配阶段。
                match e {
                    RegexError::TooLong { .. } | RegexError::Illegal { .. } => {}
                    RegexError::Budget { .. } => panic!("编译阶段不可能报预算耗尽"),
                }
            }
        }

        // —— 不会失败的入口：任何模式都不许 panic ——
        let loose = compile_regex(pat);
        let _ = std::hint::black_box(regex_matches(&loose, text));

        // —— 超长模式：显式入口报 TooLong，宽松入口退化成「不匹配」 ——
        // `compile_regex` 的占位模式是 `\u{0}`，所以文本含 NUL 时这条不成立，跳过。
        if pat.chars().count() > MAX_REGEX_LEN + 2 && !text.contains('\0') {
            assert!(
                matches!(compile_regex_checked(pat), Err(RegexError::TooLong { .. })),
                "超过长度上限的模式必须报 TooLong"
            );
            assert!(
                !regex_matches(&compile_regex(pat), text),
                "超长模式在宽松入口必须不匹配（占位模式不许命中）"
            );
        }
    });

    common::assert_within_budget("regex_compile", data.len(), bytes);
});
