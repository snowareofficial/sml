// SPDX-License-Identifier: MulanPSL-2.0
//! SML 的**受限正则引擎**，零依赖。
//!
//! 只服务于 `include /re/` 这一处：匹配文件名，不追求完整正则语义。
//! 刻意不引入 `regex` crate —— 后者体积大且为 SML 的唯一功能点引入
//! 一个重量级依赖并不划算；同时自研实现便于实施下面两道硬限制。
//!
//! 安全约束（审计 #3）：
//! - `MAX_REGEX_LEN`：模式长度上限，杜绝超长模式；
//! - `MAX_REGEX_STEPS`：回溯步数上限。无记忆化回溯在恶意模式
//!   （如 `a*a*a*…*b`）下会指数级爆炸，超限即判为不匹配，而非挂死解析。

/// 编译后的受限正则（只有模式串，匹配由 [`regex_matches`] 解释执行）。
pub struct MiniRegex {
    pattern: String,
}

/// 受限正则的模式长度上限：防止超长模式构造的灾难性回溯（ReDoS）。
const MAX_REGEX_LEN: usize = 256;
/// 单次匹配的最大回溯步数：无记忆化回溯在恶意模式（如 `a*a*a*...*b`）下会指数级
/// 爆炸，故设步数上限——超出即视为不匹配，避免挂死整个解析（审计 #3）。
const MAX_REGEX_STEPS: u64 = 2_000_000;

pub fn compile_regex(pat: &str) -> MiniRegex {
    // 去掉可能的首尾 `^`/`$` 锚（由 matcher 解释）后做长度上限检查
    let inner = pat
        .strip_prefix('^')
        .or_else(|| Some(pat))
        .map(|s| s.strip_suffix('$').unwrap_or(s))
        .unwrap_or(pat);
    if inner.chars().count() > MAX_REGEX_LEN {
        // 超出上限：用不可能匹配的模式占位（调用方会得到 false），不 panic
        return MiniRegex {
            pattern: "\u{0}".to_string(),
        };
    }
    MiniRegex {
        pattern: pat.to_string(),
    }
}

/// 用受限正则匹配整个 `text`（默认全匹配，支持 `^`/`$` 锚点）。
pub fn regex_matches(re: &MiniRegex, text: &str) -> bool {
    let pat = &re.pattern;
    let anchored_start = pat.starts_with('^');
    let anchored_end = pat.ends_with('$');
    let p = if anchored_start { &pat[1..] } else { pat };
    let p = if anchored_end { &p[..p.len().saturating_sub(1)] } else { p };
    // 步数预算必须**跨所有起始位置共享**：预算若是每次调用的局部变量，
    // 每个起点都能重新拿到 MAX_REGEX_STEPS 步，实际总开销 = 起点数 × 2M。
    // 而 glob/regex include 会对目录中每个文件名各调用一次本函数，
    // 一个恶意模式即可把整个解析挂死（安全审计 P3-1）。故在此统一持有并传递。
    let mut steps: u64 = 0;
    if anchored_start {
        // ⚠️ 这里必须和下面的循环一样校验 `anchored_end`：只判 `is_some()` 的话，
        // `^...$` 会退化成「前缀匹配」——`^conf\.sml$` 能匹配 `conf.sml.bak`。
        // 这是与量词 off-by-one 同一次修复中翻出来的第二个既有缺陷（见 AUDIT_REPORT）。
        match backtrack_match(p, text, 0, &mut steps) {
            Some(end) => !anchored_end || end == text.len(),
            None => false,
        }
    } else {
        for start in 0..=text.len() {
            if steps > MAX_REGEX_STEPS {
                // 预算耗尽：整次匹配判定为「不匹配」，不再尝试剩余起点。
                break;
            }
            // 每个起点只算一次：既省一半开销，也避免「是否匹配到末端」
            // 被两次独立判定（原写法对同一起点调用两次，可能得出不同结论）。
            match backtrack_match(p, text, start, &mut steps) {
                Some(end) if !anchored_end || end == text.len() => return true,
                _ => {}
            }
        }
        false
    }
}

/// 一个「原子」：可被量词作用的单位（一个字符，或一个字符类）。
///
/// 把「匹配一个字符」从「重复几次」里**拆出来**，是修量词 off-by-one 的关键：
/// 旧实现让主循环先按普通字符消费掉一个，量词分支再要求「至少再来一个」，
/// 于是三个量词全错了一格 —— `x+` ≡ `xx*`、`x*` ≡ `xx*`、`x?` ≡ `xx?`
/// （`^ab+c$` 匹配 `abbc` 却不匹配 `abc`）。
#[derive(Debug)]
enum Atom {
    /// 字面字符（含 `\.` 这类转义后的字符）
    Lit(char),
    /// `.`：任意字符
    Any,
    /// `[...]`：区间表 + 是否被 `^` 取反
    Class(Vec<(char, char)>, bool),
}

impl Atom {
    fn matches(&self, c: char) -> bool {
        match self {
            Atom::Lit(want) => c == *want,
            Atom::Any => true,
            Atom::Class(ranges, negate) => {
                let inside = ranges.iter().any(|(lo, hi)| c >= *lo && c <= *hi);
                inside != *negate
            }
        }
    }
}

/// 解析 `pchars[pi..]` 处的一个原子，返回 `(占用的模式字符数, 原子)`。
///
/// 非法（`\` 结尾、`[...` 未闭合）返回 `None`，调用方据此判为「不匹配」——
/// 本引擎对非法模式一律走「不匹配」，不 panic。
fn parse_atom(pchars: &[char], pi: usize) -> Option<(usize, Atom)> {
    match pchars.get(pi)? {
        '\\' => {
            let c = *pchars.get(pi + 1)?;
            Some((2, Atom::Lit(c)))
        }
        '.' => Some((1, Atom::Any)),
        '[' => {
            let mut j = pi + 1;
            let negate = if pchars.get(j) == Some(&'^') {
                j += 1;
                true
            } else {
                false
            };
            let mut cls = Vec::new();
            while j < pchars.len() && pchars[j] != ']' {
                if j + 2 < pchars.len() && pchars[j + 1] == '-' && pchars[j + 2] != ']' {
                    cls.push((pchars[j], pchars[j + 2]));
                    j += 3;
                } else {
                    cls.push((pchars[j], pchars[j]));
                    j += 1;
                }
            }
            if j >= pchars.len() {
                return None; // 未闭合
            }
            Some((j + 1 - pi, Atom::Class(cls, negate)))
        }
        c => Some((1, Atom::Lit(*c))),
    }
}

/// 回溯匹配：从 `text[ti]` 开始尝试匹配 `pat[pi]`，返回成功时 text 的消耗终点（usize）。
///
/// `steps` 由调用方持有并在**整个匹配过程**中共享累积，用于跨起点统一限流。
fn backtrack_match(pat: &str, text: &str, ti: usize, steps: &mut u64) -> Option<usize> {
    // 递归实现，模式索引 pi 通过 chars 迭代
    let pchars: Vec<char> = pat.chars().collect();
    let tchars: Vec<char> = text.chars().collect();
    fn go(pchars: &[char], tchars: &[char], pi: usize, ti: usize, steps: &mut u64) -> Option<usize> {
        // 步数预算：每次进入一个匹配决策都计一步，超出上限即中止（防 ReDoS）
        *steps += 1;
        if *steps > MAX_REGEX_STEPS {
            return None;
        }
        let mut pi = pi;
        let mut ti = ti;
        while pi < pchars.len() {
            // 前面没有原子的量词（`*a`、`+`）是非法模式。必须先挡掉：
            // 否则 `*` 会被 `parse_atom` 当成本义字符 `*` 而去匹配文本里的星号。
            if matches!(pchars[pi], '*' | '+' | '?') {
                return None;
            }
            let (alen, atom) = parse_atom(pchars, pi)?;
            let qpos = pi + alen;
            let quant = match pchars.get(qpos) {
                Some(q @ ('*' | '+' | '?')) => Some(*q),
                _ => None,
            };
            let after = if quant.is_some() { qpos + 1 } else { qpos };

            match quant {
                // 无量词：恰好一次，不匹配即失败
                None => {
                    if ti >= tchars.len() || !atom.matches(tchars[ti]) {
                        return None;
                    }
                    pi = after;
                    ti += 1;
                }
                // `?`：零或一次（贪婪，先试一次）
                Some('?') => {
                    if ti < tchars.len() && atom.matches(tchars[ti]) {
                        if let Some(r) = go(pchars, tchars, after, ti + 1, steps) {
                            return Some(r);
                        }
                    }
                    pi = after; // 零次：继续在当前栈帧里往下走
                }
                // `*` / `+`：零（一）或多次，贪婪 + 回退
                Some(q @ ('*' | '+')) => {
                    let min = if q == '+' { 1 } else { 0 };
                    let mut end = ti;
                    while end < tchars.len() && atom.matches(tchars[end]) {
                        end += 1;
                    }
                    let total = end - ti;
                    if total < min {
                        return None;
                    }
                    let mut take = total; // 从最长开始回退，保证贪婪
                    loop {
                        if let Some(r) = go(pchars, tchars, after, ti + take, steps) {
                            return Some(r);
                        }
                        if take == min {
                            break;
                        }
                        take -= 1;
                    }
                    return None;
                }
                // 不可达（quant 只有三种取值），防御性返回而非 unreachable!()
                Some(_) => return None,
            }
        }
        Some(ti)
    }
    go(&pchars, &tchars, 0, ti, steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(pat: &str, text: &str) -> bool {
        regex_matches(&compile_regex(pat), text)
    }

    /// 审计报告里的原始反例：`^ab+c$` 曾匹配 `abbc` 却不匹配 `abc`。
    #[test]
    fn plus_means_one_or_more() {
        assert!(m("^a+$", "a"));
        assert!(m("^a+$", "aaa"));
        assert!(!m("^a+$", ""));
        assert!(m("^ab+c$", "abc"), "`+` 应接受恰好一个 b");
        assert!(m("^ab+c$", "abbc"));
        assert!(!m("^ab+c$", "ac"), "`+` 至少一个 b");
        assert!(!m("^ab+c$", "abbbx"));
    }

    #[test]
    fn question_means_zero_or_one() {
        assert!(m("^ab?c$", "ac"), "`?` 应允许零个 b");
        assert!(m("^ab?c$", "abc"));
        assert!(!m("^ab?c$", "abbc"), "`?` 至多一个 b");
    }

    #[test]
    fn star_means_zero_or_more() {
        assert!(m("^a*b$", "b"), "`*` 应允许零个 a");
        assert!(m("^a*b$", "aaab"));
        assert!(!m("^a*b$", "aaac"));
    }

    /// 修复前量词只作用于「前一个字符」，字符类后面跟 `+` 会失效；
    /// 现在量词作用于一整个原子，`[0-9]+`、`.+` 都成立。
    #[test]
    fn quantifier_applies_to_class_and_any() {
        assert!(m("^[0-9]+$", "01"));
        assert!(m("^[0-9]+$", "7"));
        assert!(!m("^[0-9]+$", ""), "`+` 至少一个");
        assert!(!m("^[0-9]+$", "x1"));
        assert!(m("^widget_[0-9]+$", "widget_012"));
        assert!(m("^.+$", "ab"));
        assert!(!m("^.+$", ""));
        assert!(m("^[^0-9]+$", "abc"));
        assert!(!m("^[^0-9]+$", "a1"));
    }

    #[test]
    fn escaped_metachar_is_literal() {
        assert!(m(r"^a\*b$", "a*b"));
        assert!(!m(r"^a\*b$", "aab"));
        assert!(m(r"^a\+$", "a+"));
    }

    /// 非法模式（独立量词、未闭合字符类、结尾悬空转义）一律「不匹配」，不 panic。
    #[test]
    fn malformed_patterns_do_not_panic() {
        assert!(!m("^*a$", "a"));
        assert!(!m("^+$", "a"));
        assert!(!m("^[0-9$", "0"));
        assert!(!m("^a\\$", "a"));
        assert!(!m(r"^a\$", "a"));
    }

    #[test]
    fn non_anchored_substring_still_works() {
        assert!(m("b+c", "xxbbcyy"));
        assert!(!m("b+c", "xxbyy"));
        assert!(m("[0-9]+", "ab12cd"));
    }

    #[test]
    fn anchors_and_empty_pattern() {
        assert!(m("^$", ""));
        assert!(!m("^$", "a"));
        assert!(m("", "abc"), "空模式零宽匹配");
        assert!(m("$", "abc"));
    }

    /// 同一次修复中翻出的第二个既有缺陷：同时出现 `^` 与 `$` 时，
    /// 只检查了「能匹配」而没检查「匹配到结尾」，`^...$` 退化成前缀匹配。
    #[test]
    fn end_anchor_is_enforced_together_with_start_anchor() {
        assert!(!m("^a$", "ab"), "`$` 必须匹配到文本结尾");
        assert!(!m("^[^0-9]+$", "a1"));
        assert!(m(r"^conf\.sml$", "conf.sml"));
        assert!(
            !m(r"^conf\.sml$", "conf.sml.bak"),
            "修复前会误匹配这个（只匹配了前缀）"
        );
        assert!(!m(r"^conf\.sml$", "x.conf.sml"), "`^` 仍须从头匹配");
    }
}

