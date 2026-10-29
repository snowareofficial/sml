// spans_equivalence.rs —— 把「优化后的算法」与「朴素实现」钉成逐字等价。
//
// 为什么需要它：`compute_block_comment_spans` 与 `line_starts_in_string` 都从
// 「对每个位置扫一遍区间表」改成了二分 / 单调游标（消除二次复杂度）。
// 这类替换最容易在**边界**上悄悄漂移（区间首尾、空区间、相邻区间、未闭合串），
// 所以这里保留一份 O(n·m) 的朴素参照实现，对同一批输入逐条比对结果。

use sml_lex::{compute_block_comment_spans, compute_string_spans, line_starts_in_string};

/// 朴素参照：对每个字节全扫字符串区间（优化前的写法）。
fn naive_block_comment_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let strings = compute_string_spans(text);
    let in_string = |i: usize| strings.iter().any(|(s, e)| i >= *s && i < *e);
    let mut spans = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if in_string(i) {
            i += 1;
            continue;
        }
        let b = bytes[i];
        let skip_to_eol = b == b'#'
            || (b == b'/' && bytes.get(i + 1) == Some(&b'/'))
            || (b == b'-' && bytes.get(i + 1) == Some(&b'-'));
        if skip_to_eol {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        let closer = match (b, bytes.get(i + 1)) {
            (b'/', Some(b'*')) => Some((b'*', b'/')),
            (b'_', Some(b'*')) => Some((b'*', b'_')),
            _ => None,
        };
        if let Some((c1, c2)) = closer {
            let mut j = i + 2;
            while j + 1 < bytes.len() && !(bytes[j] == c1 && bytes[j + 1] == c2) {
                j += 1;
            }
            let end = if j + 1 < bytes.len() { j + 2 } else { bytes.len() };
            spans.push((i, end));
            i = end;
            continue;
        }
        i += 1;
    }
    spans
}

fn naive_line_starts_in_string(start: usize, spans: &[(usize, usize)]) -> bool {
    spans.iter().any(|(s, e)| start >= *s && start < *e)
}

/// 刻意覆盖边界：注释里含引号、字符串里含注释起始符、未闭合字符串、跨行字符串、
/// 行注释三种写法、CRLF、空输入、以及区间正好相邻/首尾相接的情形。
fn cases() -> Vec<&'static str> {
    vec![
        "",
        "a: 1\n",
        "/* c */\nk: 1\n",
        "_* c *_\nk: 1\n",
        "k: \"/* not a comment */\"\n",
        "k: \"_* not either *_\"\n",
        "k: \"unterminated\n/* c */\n",
        "k: \"multi\nline\nstring\"\n/* real */\n",
        "# hash\n// slash\n-- dash\nk: 1\n",
        "/* a */ /* b */\n",
        "/**/\n",
        "/*\nmulti\nline\n*/\n",
        "k: \"\"\n/* c */\n",
        "k: \"a\" \"b\"\n",
        "/* \" */\nk: 1\n",
        "a\r\nb\r\n/* c */\r\n",
        "k: \"/*\"\n/* real */\n",
    ]
}

#[test]
fn block_comment_spans_matches_naive() {
    for c in cases() {
        assert_eq!(
            compute_block_comment_spans(c),
            naive_block_comment_spans(c),
            "块注释区间与朴素实现不一致，输入：{c:?}"
        );
    }
}

#[test]
fn line_starts_in_string_matches_naive() {
    for c in cases() {
        let spans = compute_string_spans(c);
        // 每个字节偏移都查一遍（不只是行首）—— 二分的下标边界最容易被写错一格
        for start in 0..=c.len() {
            assert_eq!(
                line_starts_in_string(c, start, &spans),
                naive_line_starts_in_string(start, &spans),
                "字符串区间判定不一致，输入：{c:?} start={start}"
            );
        }
    }
}
