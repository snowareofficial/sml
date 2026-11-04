// SPDX-License-Identifier: MulanPSL-2.0
//! Guard for the cross-end vectors file / 跨端向量文件的守门用例。
//!
//! Same two-sided shape as `tests/canonical.rs` in the root package: the default build
//! must **fail loudly** (C2), and the `preserve_order` build must match the committed file.
//! 与根包的 `tests/canonical.rs` 同一套路：默认构建必须**明确报错**（C2），
//! 保序构建必须与入库的向量文件逐字节一致。
//!
//! Why a test and not only a CLI: CI runs `cargo test`, not `cargo run`. The vectors file is
//! the contract JS/Lua/C/C++ align to (T7), so drift has to turn red on its own.
//! 为什么不能只靠 CLI：CI 跑的是 `cargo test`，不是 `cargo run`。向量文件是 T7 五端
//! 要对齐的契约，漂移必须自己变红 —— 靠人记得手跑一次 `--check` 不算门禁。

#[cfg(feature = "preserve_order")]
#[test]
fn committed_vectors_match_the_code() {
    let path = smlb::vectors::resolve_default();
    smlb::vectors::check(&path).unwrap_or_else(|e| panic!("{e}"));
}

/// Precondition instead of a silent skip / 用「前提断言」代替静默跳过。
///
/// A skipped test is indistinguishable from a passing one in the log; asserting the error
/// keeps the C2 promise visible in the default build too.
/// 被跳过的用例在日志里与通过的长得一模一样；断言「确实报错」才能让 C2 的承诺在默认
/// 构建里也看得见。
#[cfg(not(feature = "preserve_order"))]
#[test]
fn without_preserve_order_rendering_refuses_to_sort() {
    let err = smlb::vectors::render().expect_err("默认构建必须报错，绝不按排序产出向量");
    assert!(matches!(err, smlb::CanonicalError::KeyOrderUnavailable), "{err:?}");
    assert_eq!(err.kind(), "key-order-unavailable");
}

/// Case ids are the cross-end join key / 用例 id 是跨端的对齐键。
#[test]
fn case_ids_are_unique_and_stable() {
    let mut seen: Vec<&str> = Vec::new();
    for c in smlb::vectors::CASES {
        assert!(!seen.contains(&c.id), "用例 id 重复：{}", c.id);
        // 只允许 `a-z0-9-`：五种语言的标识符都比较友好，也方便 shell 过滤
        assert!(
            c.id.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'),
            "用例 id 只允许小写字母/数字/连字符：{}",
            c.id
        );
        seen.push(c.id);
    }
    assert!(smlb::vectors::CASES.len() >= 20, "用例数应覆盖 C4–C10 各条规则");
}

/// The container inherits C2: no source order ⇒ `dump` refuses, it does not sort.
/// 容器层继承 C2：没有源序 ⇒ `dump` 报错，而不是偷偷排序。
///
/// Only checked in the default build (the `preserve_order` build has nothing to refuse).
/// 只在默认构建里检查（保序档没有可拒绝的东西）。
#[cfg(not(feature = "preserve_order"))]
#[test]
fn container_dump_also_refuses_to_sort() {
    let err = smlb::dump(&smlb::Value::Int(1)).expect_err("默认构建的 dump 必须报错");
    assert!(
        matches!(
            err,
            smlb::ContainerError::Canonical(smlb::CanonicalError::KeyOrderUnavailable)
        ),
        "{err:?}"
    );
}
