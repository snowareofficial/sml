#![no_main]
//! L3 值层模糊：canonical 字节 + `STRTAB` 表 + 子块导航（零拷贝）。
//!
//! 为什么单独 fuzz 而不是只靠 L2（HANDOFF §4.2）：一个畸形 canonical 字节从
//! `smlb::load` 进去要穿过容器层才暴露，从 `Value::from_canonical_bytes` 直接进去
//! 一眼就能定位。
//!
//! 断言（HANDOFF §4.5）：
//! 1. 不 panic / 错误可分类 / 分配有预算；
//! 2. **固定点**：解码成功的值重新编码得到的字节，再解码再编码必须逐字节不变。
//!
//! ⚠️ 这里**不**断言「重新编码 == 原始输入」：解码器对 `STRTAB` 是宽松的
//! （允许重复表项），而编码器按 C12 去重。宽松输入 ⇒ 规范化输出是设计，不是 bug；
//! 真正的隐性 bug 是「规范化不收敛」，那一条用固定点抓。
mod common;

use libfuzzer_sys::fuzz_target;
use sml_value::{Path, SliceCursor, Value};

/// 固定路径集：覆盖点号、下标、空段、越界下标、未闭合括号等导航分支。
const PATHS: &[&str] = &[
    "a", "a.b", "a[0]", "[0]", "0", "a.0", "a..b", "a.", ".a", "", "[", "a[", "a[-1]",
    "a[999999999999999999999]", "a.b.c[0].d",
];

fuzz_target!(|data: &[u8]| {
    if data.len() > common::MAX_INPUT {
        return;
    }

    let (_, _, bytes) = common::count_allocs(|| {
        // —— STRTAB 段体：`(len:varint ‖ utf8)*`，无个数，长度自描述 ——
        if let Err(e) = sml_value::parse_strtab_table(data) {
            common::assert_canonical_error(&e);
        }

        // —— 裸 canonical 字节 → 值 ——
        let v = match Value::from_canonical_bytes(data) {
            Ok(v) => v,
            Err(e) => {
                common::assert_canonical_error(&e);
                return;
            }
        };

        // 解码成功的值必须能重新编码（对象在 canonical 路径上恒为有序态）。
        let b1 = match v.canonical_bytes() {
            Ok(b) => b,
            Err(e) => panic!("解码成功的值却编码失败：{e}"),
        };
        let text_ok = match v.canonical_text() {
            Ok(t) => t,
            Err(e) => panic!("解码成功的值却写不出 canonical 文本：{e}"),
        };
        assert!(!text_ok.is_empty(), "canonical 文本为空");

        // 固定点：`encode(decode(b1))` 必须与 b1 逐字节相同。
        let v1 = match Value::from_canonical_bytes(&b1) {
            Ok(v) => v,
            Err(e) => panic!("自己编出的字节却解不回来：{e}"),
        };
        assert!(v.eq_ordered(&v1), "canonical 字节往返后值/键序变了");
        let b2 = match v1.canonical_bytes() {
            Ok(b) => b,
            Err(e) => panic!("二次编码失败：{e}"),
        };
        assert_eq!(b1, b2, "canonical 字节不是固定点（规范化不收敛）");

        // —— 子块导航：`STRTAB` + `VALUE` 两个段体是容器层随机访问的输入 ——
        let blocks = match v.canonical_blocks() {
            Ok(b) => b,
            Err(e) => panic!("解码成功的值却切不出段块：{e}"),
        };
        let table = match sml_value::parse_strtab_table(&blocks.strtab) {
            Ok(t) => t,
            Err(e) => panic!("自己编出的 STRTAB 却解不出来：{e}"),
        };

        // 路径来源：固定集合 + 输入自身（后者让任意键名/引号/括号都能被走到）。
        let derived = std::str::from_utf8(data).ok();
        for p in PATHS.iter().copied().chain(derived.into_iter()) {
            let Ok(path) = Path::parse(p) else { continue };
            match sml_value::subtree_at(&blocks.strtab, &blocks.value, &path) {
                Ok(sub) => {
                    // 命中一个子树 ⇒ 它必须能单独解码（`subtree_len` 自描述的前提）。
                    match sml_value::value_from_subtree(&blocks.strtab, sub.bytes) {
                        Ok(v) => {
                            let _ = v.canonical_text();
                        }
                        Err(e) => panic!("subtree_at 命中的子树解不回来：{e}（路径 {p:?}）"),
                    }
                }
                Err(e) => common::assert_canonical_error(&e),
            }

            // 同一棵树走一遍游标接口（`Store` 走的是这条，而不是 `subtree_at`）。
            let mut cur = SliceCursor::new(&blocks.value);
            match sml_value::subtree_range(&mut cur, &table, path.segments(), 0) {
                Ok((start, end, _)) => {
                    if start >= end || end > blocks.value.len() {
                        panic!("subtree_range 返回非法区间 [{start}, {end})，长度 {}", blocks.value.len());
                    }
                }
                Err(e) => common::assert_canonical_error(&e),
            }
        }
    });

    common::assert_within_budget("value_canonical", data.len(), bytes);
});
