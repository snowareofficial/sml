// SPDX-License-Identifier: MulanPSL-2.0
//! fuzz target 共用件：分配预算 + 错误分类断言。
//!
//! 为什么每个 target 都要有这两样（HANDOFF §4.5「断言口径」）：
//! 「不 panic」只是及格线。畸形输入被 **静默接受**、错误码退化成 `""`、
//! 单次输入吃掉几个 GB —— 这三种都不会让 libFuzzer 报错，却都是真 bug。
//!
//! 分配计数照抄 `benches/perf.rs`：零依赖、只在本进程内计数。计数口径是
//! **分配请求的字节总量**（含 realloc），不是峰值 —— 对「平方级分配 / 解压炸弹」
//! 这类病态足够敏感，又不会因 allocator 实现差异抖动。

#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(l.size(), Ordering::Relaxed);
        System.alloc(l)
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new_size: usize) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        BYTES.fetch_add(new_size, Ordering::Relaxed);
        System.realloc(p, l, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// 单个 fuzz 输入的上限。超过就跳过：容器 / 解析器的闸门都按「输入规模可解释」设计，
/// 1 MiB 已能触发全部布局分支（4 GiB 的 `subtree_len` 上限另由代码内闸门守）。
pub const MAX_INPUT: usize = 1 << 20;

/// 跑一次 `f` 并记下分配次数与字节数。
pub fn count_allocs<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    let a0 = ALLOCS.load(Ordering::Relaxed);
    let b0 = BYTES.load(Ordering::Relaxed);
    let out = f();
    (
        out,
        ALLOCS.load(Ordering::Relaxed) - a0,
        BYTES.load(Ordering::Relaxed) - b0,
    )
}

/// 单次输入的分配预算：固定 64 MiB + 每输入字节 128 B（+ 显式的解压额度）。
///
/// 取值理由：合法路径的最坏放大率是个位数（编码 → 容器 → 摘要各一遍），
/// 128 B/B 留足余量；而「平方级拼接」一眼就是几个数量级。
///
/// `allowance` 只给**会解压**的入口：库自己把单段解压封在
/// [`amberice::container::MAX_DECOMPRESSED`]（256 MiB），一个文件最多 5 个可压缩段
/// （`VALUE` / `STRTAB` / `EMBED` / `FRAMES` / `MANIFEST`）—— 所以「一次调用」
/// 合法上限是 5 × 256 MiB。这不是放宽，而是把库声明的上限写进判据：
/// 真正的 OOM 由 libFuzzer 的 `-rss_limit_mb`（默认 2 GiB）兜底，
/// 这里抓的是「超出格式允许的放大」。
pub fn alloc_budget(input_len: usize, allowance: usize) -> usize {
    (64usize << 20)
        .saturating_add(input_len.saturating_mul(128))
        .saturating_add(allowance)
}

/// 断言单次**调用**没有吃爆内存（真触发即为 OOM 型 bug，不是性能问题）。
pub fn assert_within_budget_allow(what: &str, input_len: usize, bytes: usize, allowance: usize) {
    let limit = alloc_budget(input_len, allowance);
    assert!(
        bytes <= limit,
        "{what}: 单次调用分配 {bytes} B 超过预算 {limit} B（输入仅 {input_len} B）—— 疑似平方级分配"
    );
}

/// 不涉及解压的入口用这一版（额度为 0）。
pub fn assert_within_budget(what: &str, input_len: usize, bytes: usize) {
    assert_within_budget_allow(what, input_len, bytes, 0);
}

/// 跑一次入口并核对它的分配额度；返回入口的返回值。
pub fn check_call<T>(
    what: &str,
    input_len: usize,
    allowance: usize,
    f: impl FnOnce() -> T,
) -> T {
    let (out, _, bytes) = count_allocs(f);
    assert_within_budget_allow(what, input_len, bytes, allowance);
    out
}

/// 该文件声明了压缩 / 加密段时，单次调用允许的最大解压额度。
///
/// `info` 失败 ⇒ 文件连段表都读不出来，后续入口更走不到解压 ⇒ 额度 0。
pub fn decompression_allowance(bytes: &[u8]) -> usize {
    match amberice::info(bytes) {
        Ok(info)
            if info
                .segments
                .iter()
                .any(|s| s.header.comp != 0 || s.header.enc != 0 || s.header.seg_id == amberice::container::SEG_FRAMES) =>
        {
            5 * amberice::container::MAX_DECOMPRESSED
        }
        _ => 0,
    }
}

/// SML 文本层的错误分类断言：码非空、且必须登记在码表里。
pub fn assert_sml_error(e: &sml_codes::SmlError) {
    let code = e.code();
    assert!(!code.is_empty(), "SML 错误码为空（文案：{}）", e.message());
    assert!(
        sml_codes::error_info(code, "").is_some(),
        "未登记的 SML 错误码：{code}（文案：{}）",
        e.message()
    );
    assert!(!e.message().is_empty(), "[{code}] 错误文案为空");
}

/// 容器层错误分类：`kind()` 必须落在已知集合内（空串 = 分类漏了）。
pub fn assert_container_error(e: &amberice::ContainerError) {
    const KNOWN: &[&str] = &[
        "too-short",
        "bad-magic",
        "unsupported-fmt",
        "header-too-short",
        "bad-segment-table",
        "truncated-segment",
        "unsupported-algorithm",
        "missing-segment",
        "duplicate-segment",
        "digest-mismatch",
        "bad-value",
        "bad-path",
        "bad-manifest",
        "primary-segment-missing",
        "primary-not-value",
        "payload-read",
        "bad-capability-name",
        "duplicate-capability",
        "missing-capability",
        "manifest-mismatch",
        "unsupported-hash",
        "auth-failed",
        "key-required",
        "kdf-required",
        "kdf-mismatch",
        "kdf-unsupported",
        "encrypted-not-zero-copy",
        "crypto",
        "bad-layer",
        "compress-feature-disabled",
        "decompress-too-large",
        "compress-failed",
        "zero-copy-needs-plain",
        "bad-frame-index",
        "empty-archive",
        "duplicate-payload-name",
        "payload-not-found",
        "payload-table-mismatch",
    ];
    let kind = e.kind();
    assert!(!kind.is_empty(), "容器错误分类为空：{e}");
    assert!(KNOWN.contains(&kind), "未登记的容器错误分类：{kind}（{e}）");
}

/// canonical 层错误分类：同上。
pub fn assert_canonical_error(e: &sml_value::CanonicalError) {
    const KNOWN: &[&str] = &[
        "key-order-unavailable",
        "non-finite-float",
        "depth-exceeded",
        "subtree-too-large",
        "truncated",
        "varint-too-long",
        "unknown-tag",
        "bad-strtab",
        "str-id-out-of-range",
        "duplicate-key",
        "subtree-len-mismatch",
        "subtree-len-overflow",
        "trailing-bytes",
        "path-not-found",
        "source-unavailable",
        "path-type-mismatch",
    ];
    let kind = e.kind();
    assert!(!kind.is_empty(), "canonical 错误分类为空：{e}");
    assert!(KNOWN.contains(&kind), "未登记的 canonical 错误分类：{kind}（{e}）");
}
