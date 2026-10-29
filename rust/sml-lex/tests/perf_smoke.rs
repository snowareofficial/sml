// perf_smoke.rs —— 词法器**时间复杂度**护栏（人工跑的粗测，不是 CI 断言）。
//
// 为什么单独放一个被 `#[ignore]` 的文件：CI 上计时会抖，断言会假红；
// 但复杂度的量级变化（线性 ↔ 二次）在规模翻倍时是**肉眼可辨**的，
// 人工跑一次就能看出回归。
//
// 运行：
//     cargo test -p sml-lex --release -- --ignored --nocapture
//
// 判读：规模 ×4 时耗时若也约 ×4 ⇒ 线性；若约 ×16 ⇒ 二次（回归）。

use std::time::Instant;

/// 每行一个字符串：字符串区间数与字节数都随规模线性增长，
/// 正好能暴露「对每个字节扫一遍字符串区间」这类二次写法。
fn doc_with_many_strings(rows: usize) -> String {
    let mut s = String::with_capacity(rows * 16);
    for i in 0..rows {
        s.push_str(&format!("k{}: \"v{}\"\n", i, i));
    }
    s
}

#[test]
#[ignore]
fn perf_block_comment_spans() {
    for rows in [2_000usize, 8_000, 32_000] {
        let d = doc_with_many_strings(rows);
        let t = Instant::now();
        let n = sml_lex::compute_block_comment_spans(&d).len();
        println!(
            "block_comment_spans rows={:>6} bytes={:>8} spans={:>3} elapsed={:?}",
            rows,
            d.len(),
            n,
            t.elapsed()
        );
    }
}

#[test]
#[ignore]
fn perf_tokenize() {
    for rows in [2_000usize, 8_000, 32_000] {
        let d = doc_with_many_strings(rows);
        let t = Instant::now();
        let toks = sml_lex::tokenize(&d).unwrap();
        println!(
            "tokenize            rows={:>6} toks={:>7} elapsed={:?}",
            rows,
            toks.len(),
            t.elapsed()
        );
    }
}
