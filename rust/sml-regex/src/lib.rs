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
///
/// `W16` 顺带给它加上 `Debug` / `Clone`：`compile_regex_checked` 返回
/// `Result<MiniRegex, RegexError>`，没有 `Debug` 时调用方连 `unwrap_err()` 都用不了。
#[derive(Debug, Clone)]
pub struct MiniRegex {
    pattern: String,
}

/// 受限正则的模式长度上限：防止超长模式构造的灾难性回溯（ReDoS）。
pub const MAX_REGEX_LEN: usize = 256;
/// 单次匹配的最大回溯步数：无记忆化回溯在恶意模式（如 `a*a*a*...*b`）下会指数级
/// 爆炸，故设步数上限——超出即视为不匹配，避免挂死整个解析（审计 #3）。
pub const MAX_REGEX_STEPS: u64 = 2_000_000;

/// 编译/匹配失败的原因（W16）。
///
/// **本 crate 是零依赖的引擎层，故不带码** —— 只报「为什么失败」，由调用方
/// （`sml-include`）按 `errors/codes.sml` 映射成码：
/// `TooLong ⇒ E-LIMIT-007`、`Illegal ⇒ E-PARSE-025`、`Budget ⇒ E-LIMIT-002`。
/// 这与各端「引擎给原因、上层给码」的分层一致。
///
/// 为什么要有这套入口：改前 [`compile_regex`] / [`regex_matches`] 对
/// 「模式过长」「模式非法」「步数超预算」**一律静默判「不匹配」** ——
/// 于是「模式写错了」在用户眼里表现成「目录里没有匹配的文件」，无从排查。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegexError {
    /// 模式源码超过 [`MAX_REGEX_LEN`]（⇒ `E-LIMIT-007`）
    TooLong { len: usize, max: usize },
    /// 模式语法非法：量词之前没有可重复的原子（含连续两个量词）、字符类未闭合、
    /// 以反斜杠结尾（⇒ `E-PARSE-025`）
    Illegal { why: &'static str },
    /// 回溯步数预算耗尽（⇒ `E-LIMIT-002`）
    Budget { steps: u64, max: u64 },
}

impl std::fmt::Display for RegexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegexError::TooLong { len, max } => {
                write!(f, "受限正则模式过长（{len} > {max}）")
            }
            RegexError::Illegal { why } => write!(f, "受限正则模式语法非法：{why}"),
            RegexError::Budget { steps, max } => {
                write!(f, "受限正则匹配超出步数预算（{steps} > {max} 步），疑似病态模式")
            }
        }
    }
}

impl std::error::Error for RegexError {}

/// 去掉首尾 `^` / `$` 锚（锚由 matcher 解释，不参与长度与语法检查）。
fn strip_anchors(pat: &str) -> &str {
    let s = pat.strip_prefix('^').unwrap_or(pat);
    s.strip_suffix('$').unwrap_or(s)
}

pub fn compile_regex(pat: &str) -> MiniRegex {
    // 去掉可能的首尾 `^`/`$` 锚（由 matcher 解释）后做长度上限检查
    let inner = strip_anchors(pat);
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

/// [`compile_regex`] 的**显式失败**版本（W16）：模式过长或语法非法都返回原因，
/// 而不是悄悄给一个「永不匹配」的正则。生产路径（regex-include）应走这个入口。
pub fn compile_regex_checked(pat: &str) -> Result<MiniRegex, RegexError> {
    let inner = strip_anchors(pat);
    let len = inner.chars().count();
    if len > MAX_REGEX_LEN {
        return Err(RegexError::TooLong {
            len,
            max: MAX_REGEX_LEN,
        });
    }
    validate_pattern(inner)?;
    Ok(MiniRegex {
        pattern: pat.to_string(),
    })
}

/// 模式语法预检：与 [`backtrack_match`] 的判定**同一条规则**（那里遇到非法就
/// `return None` = 不匹配），只是把「不匹配」提前成「编译期报错」。
///
/// 规则（两边必须一致，否则预检会放过运行期才炸的模式）：
/// - 量词（`*` `+` `?`）必须紧跟在一个原子之后，且不能连续出现两个量词；
/// - 字符类 `[` 必须闭合；
/// - 反斜杠必须后跟一个字符（模式不能以 `\` 结尾）。
fn validate_pattern(inner: &str) -> Result<(), RegexError> {
    let pchars: Vec<char> = inner.chars().collect();
    let mut i = 0usize;
    let mut prev_was_atom = false;
    while i < pchars.len() {
        let c = pchars[i];
        if matches!(c, '*' | '+' | '?') {
            if !prev_was_atom {
                return Err(RegexError::Illegal {
                    why: "量词之前没有可重复的原子（或连续出现两个量词）",
                });
            }
            prev_was_atom = false; // `a*` 之后不能再跟量词
            i += 1;
            continue;
        }
        match parse_atom(&pchars, i) {
            Some((n, _)) => {
                prev_was_atom = true;
                i += n;
            }
            None => {
                return Err(RegexError::Illegal {
                    why: "字符类 `[` 未闭合，或模式以孤立的反斜杠结尾",
                })
            }
        }
    }
    Ok(())
}

/// 用受限正则匹配整个 `text`（默认全匹配，支持 `^`/`$` 锚点）。
///
/// ⚠️ 步数预算耗尽时它**静默判「不匹配」**（既有行为，被 40+ 处断言依赖）。
/// 需要「预算耗尽要报错」的调用方走 [`regex_matches_checked`]。
pub fn regex_matches(re: &MiniRegex, text: &str) -> bool {
    matches_inner(re, text).0
}

/// [`regex_matches`] 的**显式失败**版本（W16）：步数预算耗尽时返回
/// `RegexError::Budget`（⇒ `E-LIMIT-002`），而不是静默算作「不匹配」。
pub fn regex_matches_checked(re: &MiniRegex, text: &str) -> Result<bool, RegexError> {
    let (matched, steps) = matches_inner(re, text);
    if steps > MAX_REGEX_STEPS {
        return Err(RegexError::Budget {
            steps,
            max: MAX_REGEX_STEPS,
        });
    }
    Ok(matched)
}

/// 内部：返回 `(是否匹配, 实际步数)`；步数超过预算时匹配结果按「不匹配」返回，
/// 由两个公开入口决定是静默（[`regex_matches`]）还是报错（[`regex_matches_checked`]）。
fn matches_inner(re: &MiniRegex, text: &str) -> (bool, u64) {
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
    let matched = if anchored_start {
        // ⚠️ 这里必须和下面的循环一样校验 `anchored_end`：只判 `is_some()` 的话，
        // `^...$` 会退化成「前缀匹配」——`^conf\.sml$` 能匹配 `conf.sml.bak`。
        // 这是与量词 off-by-one 同一次修复中翻出来的第二个既有缺陷（见 AUDIT_REPORT）。
        match backtrack_match(p, text, 0, &mut steps) {
            Some(end) => !anchored_end || end == text.len(),
            None => false,
        }
    } else {
        let mut hit = false;
        for start in 0..=text.len() {
            if steps > MAX_REGEX_STEPS {
                // 预算耗尽：整次匹配判定为「不匹配」，不再尝试剩余起点。
                break;
            }
            // 每个起点只算一次：既省一半开销，也避免「是否匹配到末端」
            // 被两次独立判定（原写法对同一起点调用两次，可能得出不同结论）。
            match backtrack_match(p, text, start, &mut steps) {
                Some(end) if !anchored_end || end == text.len() => {
                    hit = true;
                    break;
                }
                _ => {}
            }
        }
        hit
    };
    (matched, steps)
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

