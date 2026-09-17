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
        backtrack_match(p, text, 0, &mut steps).is_some()
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
            match pchars[pi] {
                '\\' => {
                    // 转义下一个字符（如 \. 匹配字面的 .）
                    if pi + 1 >= pchars.len() {
                        return None;
                    }
                    let pc = pchars[pi + 1];
                    if ti >= tchars.len() || tchars[ti] != pc {
                        return None;
                    }
                    pi += 2;
                    ti += 1;
                }
                '.' => {
                    if ti >= tchars.len() {
                        return None;
                    }
                    pi += 1;
                    ti += 1;
                }
                '*' => {
                    // 匹配前一个原子零次或多次（贪婪）
                    // 回退：尝试匹配零次（跳过 * 与前一原子），或匹配一次后继续
                    let prev = if pi >= 1 { Some(pchars[pi - 1]) } else { None };
                    // 零次：跳过 '*'（以及其前的普通原子已由上层处理，这里仅跳过 '*'）
                    // 但为简化，* 作用于前一原子：先尝试消耗一字符再递归
                    if ti < tchars.len() {
                        // 贪婪：尽量多匹配
                        let mut end = ti;
                        match prev {
                            Some('.') => {
                                while end < tchars.len() {
                                    end += 1;
                                }
                            }
                            Some(c) if c != '\\' => {
                                while end < tchars.len() && tchars[end] == c {
                                    end += 1;
                                }
                            }
                            _ => {}
                        }
                        // 从 end 回退尝试让后续模式匹配
                        let mut e = end;
                        while e >= ti {
                            if let Some(r) = go(pchars, tchars, pi + 1, e, steps) {
                                return Some(r);
                            }
                            if e == ti {
                                break;
                            }
                            e -= 1;
                        }
                    }
                    // 零次匹配：跳过 '*'
                    return go(pchars, tchars, pi + 1, ti, steps);
                }
                '+' => {
                    if ti >= tchars.len() {
                        return None;
                    }
                    let prev = pchars.get(pi.wrapping_sub(1)).copied();
                    // 延迟初始化：下面 match 的每条非返回分支都会赋值，
                    // 原先的 `= 0` 初值永远不会被读到（unused_assignments）。
                    let mut consumed;
                    match prev {
                        Some('.') => {
                            if ti >= tchars.len() {
                                return None;
                            }
                            consumed = 1;
                        }
                        Some(c) if c != '\\' => {
                            if tchars[ti] != c {
                                return None;
                            }
                            consumed = 1;
                            while ti + consumed < tchars.len()
                                && tchars[ti + consumed] == c
                            {
                                consumed += 1;
                            }
                        }
                        _ => return None,
                    }
                    pi += 1;
                    ti += consumed;
                }
                '?' => {
                    // 前一原子的零或一
                    let prev = pchars.get(pi.wrapping_sub(1)).copied();
                    if ti < tchars.len() {
                        match prev {
                            Some('.') => {
                                pi += 1;
                                ti += 1;
                            }
                            Some(c) if c != '\\' => {
                                if tchars[ti] == c {
                                    pi += 1;
                                    ti += 1;
                                } else {
                                    pi += 1; // 零次
                                }
                            }
                            _ => {
                                pi += 1; // 零次
                            }
                        }
                    } else {
                        pi += 1;
                    }
                }
                '[' => {
                    // 字符类 [abc] 或 [^abc] 或 [a-z]
                    let mut j = pi + 1;
                    let negate = if j < pchars.len() && pchars[j] == '^' {
                        j += 1;
                        true
                    } else {
                        false
                    };
                    let mut cls = Vec::new();
                    while j < pchars.len() && pchars[j] != ']' {
                        if j + 2 < pchars.len()
                            && pchars[j + 1] == '-'
                            && pchars[j + 2] != ']'
                        {
                            let lo = pchars[j];
                            let hi = pchars[j + 2];
                            cls.push((lo, hi));
                            j += 3;
                        } else {
                            cls.push((pchars[j], pchars[j]));
                            j += 1;
                        }
                    }
                    if j >= pchars.len() {
                        return None; // 未闭合
                    }
                    if ti >= tchars.len() {
                        return None;
                    }
                    let c = tchars[ti];
                    let in_cls = cls.iter().any(|(lo, hi)| c >= *lo && c <= *hi);
                    let ok = if negate { !in_cls } else { in_cls };
                    if !ok {
                        return None;
                    }
                    pi = j + 1;
                    ti += 1;
                }
                c => {
                    if ti >= tchars.len() || tchars[ti] != c {
                        return None;
                    }
                    pi += 1;
                    ti += 1;
                }
            }
        }
        Some(ti)
    }
    go(&pchars, &tchars, 0, ti, steps)
}

