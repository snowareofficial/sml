// SPDX-License-Identifier: MulanPSL-2.0
//! Guard for the cross-end vectors file / 跨端向量文件的守门用例。
//!
//! 方案 2 / B 之后：向量对象是**显式有序**构造的（`Map::ordered()`），所以
//! **默认构建（没有任何 feature）也能产出向量**并与入库文件逐字节一致 —— 这正是
//! 「正确性不再是配置项」。仍然拒的是**显式无序**的对象（C2：绝不偷偷按排序写）。
//!
//! Why a test and not only a CLI: CI runs `cargo test`, not `cargo run`. The vectors file is
//! the contract JS/Lua/C/C++ align to (T7), so drift has to turn red on its own.
//! 为什么不能只靠 CLI：CI 跑的是 `cargo test`，不是 `cargo run`。向量文件是 T7 五端
//! 要对齐的契约，漂移必须自己变红 —— 靠人记得手跑一次 `--check` 不算门禁。

#[test]
fn committed_vectors_match_the_code() {
    let path = amberice::vectors::resolve_default();
    amberice::vectors::check(&path).unwrap_or_else(|e| panic!("{e}"));
}

/// 默认构建也能产出向量：向量里的对象是 `Map::ordered()` 显式构造的。
/// 这条在**任何 feature 组合**下都必须通过 —— 它就是「正确性不是配置项」的证据。
#[test]
fn rendering_works_without_any_feature() {
    let got = amberice::vectors::render().expect("显式有序的对象在任何构建档都能编出向量");
    assert!(!got.is_empty());
}

/// Case ids are the cross-end join key / 用例 id 是跨端的对齐键。
#[test]
fn case_ids_are_unique_and_stable() {
    let mut seen: Vec<&str> = Vec::new();
    for c in amberice::vectors::CASES {
        assert!(!seen.contains(&c.id), "用例 id 重复：{}", c.id);
        // 只允许 `a-z0-9-`：五种语言的标识符都比较友好，也方便 shell 过滤
        assert!(
            c.id.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
            "用例 id 只允许小写字母/数字/连字符：{}",
            c.id
        );
        seen.push(c.id);
    }
    assert!(amberice::vectors::CASES.len() >= 20, "用例数应覆盖 C4–C10 各条规则");
}

/// 容器层继承 C2，但判定看**值**：标量没有键序问题，任何构建档都能写。
/// 旧实现「连标量也拒」是「正确性靠 feature」的副作用，现已更正。
#[test]
fn container_dump_accepts_a_scalar_in_every_build() {
    let f = amberice::dump( &cv(&amberice::Value::Int(1)) ).expect("标量没有键序问题，任何档都能写");
    assert!(amberice::check(&f).map(|r| r.ok).unwrap_or(false));
}

/// 测试用：把值断言成 `CanonicalValue`（无序在测试里就该 panic，而不是静默通过）。
fn cv(v: &amberice::Value) -> amberice::CanonicalValue {
    amberice::CanonicalValue::try_from(v).expect("测试值必须是有序构造的")
}
