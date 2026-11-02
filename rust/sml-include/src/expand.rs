// SPDX-License-Identifier: MulanPSL-2.0
//! SML 模块化的**展开**部分：把 include 指令替换为被包含文件的内容。
//!
//! 与 [`crate`]（指令解析）分开，是因为这部分要读文件系统、递归展开，
//! 且受 `MAX_INCLUDE_DEPTH` / `MAX_INCLUDE_EXPANSIONS` 双重限深保护。

use std::path::{Path, PathBuf};

use sml_codes::{
    E_FEATURE_001, E_INCLUDE_001, E_INCLUDE_002, E_INCLUDE_003, E_INCLUDE_004, E_INCLUDE_010,
    E_INCLUDE_011, E_IO_001, E_LIMIT_003, SmlError,
};
use sml_feature::{Feature, FeatureSet};
use sml_lex::{
    Span, Tok, advance_line, compute_block_comment_spans, compute_string_spans,
    line_starts_in_string, tokenize, tokenize_spans,
};

use crate::{
    MAX_INCLUDE_DEPTH, MAX_INCLUDE_EXPANSIONS, parse_include_line,
    resolve_target_paths, strip_line_comment,
};

/// Expand every `include` and return one **flat** token stream ｜ 展开所有 `include`，返回**单层扁平**的 token 流。
///
/// `base` 是**被包含文件自身所在目录**的起点（与 C 预处理器一致，不是进程工作目录），
/// 因此嵌套 include 的路径行为是可预期的。
///
/// # 沙箱：`base` 之外一律拒绝
///
/// 所有命中的文件规范化后**必须仍在 `base` 之内**，否则拒绝（`E-INCLUDE-003`）——
/// 这是防 `../` 与 glob/regex 模式越界读任意文件。`base` 自身规范化失败时也**拒绝**，
/// 而不是回落到未规范化的路径（否则 `starts_with` 比较会静默失效，`E-INCLUDE-010`）。
/// 所以「include 到上级目录的同级工程」是**设计上不支持**的，不是 bug。
///
/// # 限制与错误码（都是**硬顶**，不是建议值）
///
/// - 嵌套层数 ≥ `MAX_INCLUDE_DEPTH` ⇒ `E-INCLUDE-004`；
/// - 全局展开次数 > `MAX_INCLUDE_EXPANSIONS` ⇒ `E-LIMIT-003`（同一文件被多处包含**也计数**，
///   防指数膨胀 DoS）；
/// - 命中正在展开的文件链 ⇒ 成环 ⇒ `E-INCLUDE-002`；
/// - 目标无法规范化（不存在 / 权限）⇒ `E-INCLUDE-001`；读到但读不出文本 ⇒ `E-IO-001`；
/// - 文档未开 `include` 特性却写了 include ⇒ `E-FEATURE-001`。
///
/// ⚠️ 本函数**丢掉 span**：展开后的 token 来自多个文件，没有可映射的单一原文。
/// 需要错误定位请用 [`resolve_includes_spans`]（它在「整篇无 include」时才给 span）。
pub fn resolve_includes(
    text: &str,
    base: &Path,
    features: FeatureSet,
) -> Result<Vec<Tok>, SmlError> {
    let mut stack: Vec<PathBuf> = Vec::new();
    let mut toks: Vec<Tok> = Vec::new();
    let mut expansions: u64 = 0;
    expand_includes(text, base, &mut stack, features, &mut toks, &mut expansions)?;
    Ok(toks)
}

/// Same as [`resolve_includes`], but hands back `spans` **when the document has no include at all** ｜ 同 [`resolve_includes`]，但整篇无 include 时额外交出 `spans`
/// （与 token 流一一对应）——供文件入口把错误定位到「第 N 行第 M 列」。
///
/// 有 include 时 token 来自多个文件、**没有可映射的单一原文**，返回 `None`
/// （调用方应按「不带位置」处理，**不凭空造位置**）。
///
/// 判据与 [`expand_includes`] 的快速路径一致：无 include ⇒ 展开是恒等操作，
/// 直接整篇 `tokenize_spans`，避免「展开 + 再补位置」重复词法。
pub fn resolve_includes_spans(
    text: &str,
    base: &Path,
    features: FeatureSet,
) -> Result<(Vec<Tok>, Option<Vec<Span>>), SmlError> {
    let string_spans = compute_string_spans(text);
    let mut has_include = false;
    for seg in segments(text, &string_spans) {
        if matches!(parse_include_line(seg, features)?, Some(_)) {
            has_include = true;
            break;
        }
    }
    if !has_include {
        let (toks, spans) = tokenize_spans(text)?;
        return Ok((toks, Some(spans)));
    }
    let toks = resolve_includes(text, base, features)?;
    Ok((toks, None))
}

/// Recursively expand includes into the `out` token stream (the worker behind [`resolve_includes`]) ｜ 递归展开 include 到 `out` token 流。
/// `expansions` 为「全局已展开文件计数」，用于防御指数膨胀 DoS（同一文件被多处包含仍计数）。
pub fn expand_includes(
    text: &str,
    base: &Path,
    stack: &mut Vec<PathBuf>,
    features: FeatureSet,
    out: &mut Vec<Tok>,
    expansions: &mut u64,
) -> Result<(), SmlError> {
    if stack.len() >= MAX_INCLUDE_DEPTH {
        return Err(SmlError::from_code_msg(E_INCLUDE_004)
            .with_detail(format!("超过 {MAX_INCLUDE_DEPTH} 层")));
    }
    // 沙箱根：所有 include 命中的文件必须位于 base（规范化为绝对路径）之内，
    // 否则拒绝，防止 `../` 或 glob/regex 模式越界读取任意文件（路径遍历漏洞）。
    //
    // 规范化失败时**必须拒绝**而不是回落到未规范化的 base —— 回落后
    // `starts_with` 比较的对象就变成一个可能含 `..` 的路径，越界校验会静默失效
    // （安全默认、无法校验即不放行）。
    let base_canon = base.canonicalize().map_err(|e| {
        SmlError::from_code_msg(E_INCLUDE_010)
            .with_detail(format!("{}（{e}）", base.display()))
    })?;
    let spans = compute_string_spans(text);

    // —— 快速路径：整篇**没有** include 指令 ⇒ 整篇词法一次，不进逐段循环 ——
    //
    // 为什么必须有这条路：下面的慢路径按**段**词法，而"段"的切分再准也仍是分片的；
    // 无 include 时"展开"本就是恒等操作，整篇词法一次最保险（语义与 `parse` 完全一致，
    // 多行块注释 / 多行字符串自然全部处理正确）。
    {
        let mut has_include = false;
        for seg in segments(text, &spans) {
            if matches!(parse_include_line(seg, features)?, Some(_)) {
                has_include = true;
                break;
            }
        }
        if !has_include {
            out.extend(tokenize(text)?);
            return Ok(());
        }
    }
    // ⚠️ 这里**不再** `for line in text.lines()`：逐行 `tokenize` 会让任何**跨行的词法单元**
    // （多行块注释 `/* … */`、多行字符串）在**第 1 行**就报"未闭合" —— 实测曾让 4 份语料
    // 在文件入口（`parse_file` / C-ABI `sml_load_file` / 编辑器）直接读不了。
    // 现在按**段**遍历：跨行单元与它所在的那些行粘成一段，段内是自洽的词法单元（见 `segments`）。
    // 顺带把"字符串/注释里写一行 `include \"x.sml\"` 被真的展开"这条也堵住了：
    // 那种行所在段的**开头**不是 `include`，`parse_include_line` 自然返回 `None`。
    for line in segments(text, &spans) {
        match parse_include_line(line, features)? {
            Some(targets) => {
                if !features.has(Feature::Include) {
                    return Err(SmlError::from_code_msg(E_FEATURE_001)
                        .with_detail("include 特性"));
                }
                for t in targets {
                    if t.namespace.is_some() && !features.has(Feature::Namespace) {
                        return Err(SmlError::from_code_msg(E_FEATURE_001)
                            .with_detail("namespace 特性"));
                    }
                    // 把一个 target 解析为 0..N 个实际文件路径（支持 glob/regex/ext-rewrite）
                    let paths = resolve_target_paths(&t, base, features)?;
                    for path in paths {
                        let canon = path.canonicalize().map_err(|e| {
                            SmlError::from_code_msg(E_INCLUDE_001)
                                .with_detail(format!("{}: {e}", path.display()))
                        })?;
                        // 路径遍历防护：规范化后必须仍位于沙箱根 base 之内
                        if !canon.starts_with(&base_canon) {
                            return Err(SmlError::from_code_msg(E_INCLUDE_003).with_detail(
                                format!(
                                    "{} 不在基准目录 {} 内",
                                    canon.display(),
                                    base_canon.display()
                                ),
                            ));
                        }
                        // stack 是「当前正在展开的文件链」，命中即成环
                        if stack.iter().any(|p| p == &canon) {
                            return Err(SmlError::from_code_msg(E_INCLUDE_002)
                                .with_detail(canon.display().to_string()));
                        }
                        let content = std::fs::read_to_string(&canon).map_err(|e| {
                            SmlError::from_code_msg(E_IO_001)
                                .with_detail(format!("{}: {e}", canon.display()))
                        })?;
                        *expansions += 1;
                        if *expansions > MAX_INCLUDE_EXPANSIONS {
                            return Err(SmlError::from_code_msg(E_LIMIT_003).with_detail(
                                format!(
                                    "上限 {}（疑似指数膨胀 DoS）",
                                    MAX_INCLUDE_EXPANSIONS
                                ),
                            ));
                        }
                        let child_base = canon
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| PathBuf::from("."));
                        stack.push(canon.clone());
                        // 展开子文件 tokens（共享全局展开计数，防钻石型重复包含爆炸）
                        let mut inner = expand_file_tokens(
                            &content,
                            &child_base,
                            stack,
                            features,
                            expansions,
                        )?;
                        // 部分引用：仅保留指定顶层键（命名空间包裹时同样只挑这些）
                        if let Some(keys) = &t.keys {
                            inner = filter_top_level_keys(inner, keys);
                        }
                        // 命名空间包含：用 `ns { ... }` 包裹子文件 tokens（零拷贝）
                        if let Some(ns) = &t.namespace {
                            for seg in ns.split('.') {
                                out.push(Tok::Word(seg.to_string()));
                                out.push(Tok::LBrace);
                            }
                            out.extend(inner);
                            for _ in ns.split('.') {
                                out.push(Tok::RBrace);
                            }
                        } else {
                            out.extend(inner);
                        }
                        stack.pop();
                    }
                }
            }
            None => {
                // 非 include 行：直接 tokenize 该行并追加（保持行级语义，零拷贝）
                let line_toks = tokenize(line).map_err(|e| {
                    SmlError::from_code_msg(E_INCLUDE_011)
                        .with_detail(format!("{e}（于行：{line}）"))
                })?;
                out.extend(line_toks);
            }
        }
    }
    Ok(())
}

/// 把文本切成若干「**词法单元不跨段**」的段。
///
/// - 普通行各自成一段；
/// - **跨行**的词法单元（多行字符串、多行块注释）与它覆盖到的那些行**粘成一段**。
///
/// 为什么需要它：include 展开要逐段 `tokenize`，而逐行 tokenize 会把跨行单元在第 1 行
/// 就判成"未闭合"（`E-LEX-001` / `E-LEX-002`）—— 那正是本函数要根治的架构缺陷。
/// 它同时取代了原先"这一行是否在多行字符串内部"的判据（`line_starts_in_string`）：
/// 跨行单元所在的段，其**开头**必然不是 `include`，于是 `parse_include_line` 自然返回
/// `None` —— **注释里伪造的 include 不会被读文件**（与字符串内伪造 include 同等防护）。
fn segments<'a>(text: &'a str, string_spans: &[(usize, usize)]) -> Vec<&'a str> {
    let comment_spans = compute_block_comment_spans(text);
    let mut multi: Vec<(usize, usize)> = string_spans
        .iter()
        .chain(comment_spans.iter())
        .copied()
        .filter(|(s, e)| text[*s..*e].contains('\n'))
        .collect();
    // ⚠️ 原来 `multi.iter().any(...)` 对文本的**每一个字节**全扫一遍跨行单元区间
    // ⇒ O(字节数 × 区间数) 的二次复杂度（与 sml-lex 那两处同源）。
    // 两个来源各自有序，但 `chain` 之后整体**未必**有序 ⇒ 先排序（O(m log m)），
    // 再用只前进的游标：`i` 单调递增，摊还 O(1)，总复杂度回到 O(n log n)。
    // 区间互不重叠（块注释区间本身就跳过了字符串内部），游标判据与原来逐字相同。
    multi.sort_unstable();
    let mut mi = 0usize;
    let mut segs = Vec::new();
    let mut start = 0usize;
    for (i, b) in text.as_bytes().iter().enumerate() {
        while mi < multi.len() && multi[mi].1 <= i {
            mi += 1;
        }
        let inside_multi = mi < multi.len() && multi[mi].0 <= i;
        // 只在"不在跨行单元内部"的换行处切段
        if *b == b'\n' && !inside_multi {
            segs.push(&text[start..i]);
            start = i + 1;
        }
    }
    if start <= text.len() {
        segs.push(&text[start..]);
    }
    segs
}

/// Keep only the top-level entries whose key is in `keys`; other top-level entries are dropped ｜ 仅保留 `toks` 中顶层键名属于 `keys` 的条目。
///
/// 嵌套层级（块 `{}` / 数组 `[]`）内的键**不受影响** —— 只有 depth == 0 的顶层键被过滤。
/// 用于 `import "x" { a, b }` 部分引用：避免整文件内联。
///
/// （原文档前两行讲的是「读文件 + 剥 `@version`」，属另一个函数，早已错挂在此 —— 2026-11-02 删除。）
pub fn filter_top_level_keys(toks: Vec<Tok>, keys: &[String]) -> Vec<Tok> {
    let key_set: std::collections::HashSet<&str> = keys.iter().map(|s| s.as_str()).collect();
    let mut out: Vec<Tok> = Vec::with_capacity(toks.len());
    let mut i = 0;
    let n = toks.len();
    while i < n {
        // 顶层必须是键（Word/Str）起始；非键 token 原样保留以免破坏结构
        if !matches!(toks[i], Tok::Word(_) | Tok::Str(_)) {
            out.push(toks[i].clone());
            i += 1;
            continue;
        }
        let key_name = match &toks[i] {
            Tok::Word(w) => w.clone(),
            Tok::Str(s) => s.clone(),
            _ => unreachable!(),
        };
        // 计算该顶层条目 [i, j) 的结束位置
        let j = if i + 1 < n {
            match &toks[i + 1] {
                // key: value —— 值从其后的 token 开始
                Tok::Colon => {
                    if i + 2 < n {
                        match &toks[i + 2] {
                            // 值为块/数组：配对括号
                            Tok::LBrace | Tok::LBrack => {
                                let mut depth = 1i32;
                                let mut k = i + 3;
                                while k < n {
                                    match &toks[k] {
                                        Tok::LBrace | Tok::LBrack => depth += 1,
                                        Tok::RBrace | Tok::RBrack => {
                                            depth -= 1;
                                            if depth == 0 {
                                                break;
                                            }
                                        }
                                        _ => {}
                                    }
                                    k += 1;
                                }
                                (k + 1).min(n)
                            }
                            // 单 token 值
                            _ => i + 3,
                        }
                    } else {
                        i + 2
                    }
                }
                // key { ... } / key [ ... ] —— 直接配对括号
                Tok::LBrace | Tok::LBrack => {
                    let mut depth = 1i32;
                    let mut k = i + 2;
                    while k < n {
                        match &toks[k] {
                            Tok::LBrace | Tok::LBrack => depth += 1,
                            Tok::RBrace | Tok::RBrack => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        k += 1;
                    }
                    (k + 1).min(n)
                }
                // 裸词独立行等：单 token 条目
                _ => i + 1,
            }
        } else {
            i + 1
        };
        if key_set.contains(key_name.as_str()) {
            for t in &toks[i..j] {
                out.push(t.clone());
            }
        }
        i = j;
    }
    out
}

/// Tokenize an included **child file** and expand its own includes ｜ 对**被包含的子文件**做词法并递归展开它自己的 include。
///
/// 与主文件走的是同一条路，唯独多了两步「子文件专属」的清洗，都在这里收口：
///
/// - **去 BOM**：记事本「另存为 UTF-8」会带 `\u{feff}`，不去掉的话第一个键名会变成
///   `\u{feff}x`。主文件那条路由 `sml-parse::scan` 处理，子文件只能在这里去（否则同项目两套待遇）。
/// - **剥 `@version` / `@feature` 行**：子文件里的这两类指令不该污染 token 流；
///   但**多行字符串内部**的行要保留（否则字符串数据被破坏）。
///
/// `stack` 是被包含文件链（用于成环检测），`expansions` 是全局计数（跨文件累加，不在本层清零）。
pub fn expand_file_tokens(
    content: &str,
    base: &Path,
    stack: &mut Vec<PathBuf>,
    features: FeatureSet,
    expansions: &mut u64,
) -> Result<Vec<Tok>, SmlError> {
    // 子文件也可能带 **UTF-8 BOM**（记事本「另存为 UTF-8」）：不去掉的话，
    // 它的第一个键名会变成 `\u{feff}x`。
    // 主文件那条路由 `sml-parse::scan::strip_version` 收口（文本入口的唯一漏斗），
    // 被 include 进来的**子文件**只能在这里去 —— 否则同一项目里主/子文件两套待遇。
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    // 剥离子文件内的版本/特性指令行，避免污染 token 流。
    // 多行字符串内部的行不算指令，须跳过（否则会破坏字符串数据，如 "line\n@version\n..."）。
    let spans = compute_string_spans(content);
    let mut line_start = 0usize;
    let cleaned: String = content
        .lines()
        .filter(|l| {
            let inside = line_starts_in_string(content, line_start, &spans);
            line_start = advance_line(line_start, l, content);
            if inside {
                return true; // 字符串内：保留
            }
            let t = strip_line_comment(l).trim();
            let t = t.strip_prefix('@').unwrap_or(t).trim_start();
            !(t.starts_with("version") || t.starts_with("feature"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut toks = Vec::new();
    expand_includes(&cleaned, base, stack, features, &mut toks, expansions)?;
    Ok(toks)
}

#[cfg(test)]
mod tests {
    use super::segments;
    use sml_lex::{compute_block_comment_spans, compute_string_spans};

    /// 朴素参照：对每个字节全扫跨行单元区间（本次优化**前**的写法）。
    ///
    /// 保留它是为了把「排序 + 单调游标」的新实现钉成逐字等价 —— 这类替换最容易在
    /// 区间首尾、相邻区间、区间末尾正好是行尾这些边界上漂移一格。
    fn naive_segments<'a>(text: &'a str, string_spans: &[(usize, usize)]) -> Vec<&'a str> {
        let comment_spans = compute_block_comment_spans(text);
        let multi: Vec<(usize, usize)> = string_spans
            .iter()
            .chain(comment_spans.iter())
            .copied()
            .filter(|(s, e)| text[*s..*e].contains('\n'))
            .collect();
        let inside_multi = |i: usize| multi.iter().any(|(s, e)| i >= *s && i < *e);
        let mut segs = Vec::new();
        let mut start = 0usize;
        for (i, b) in text.as_bytes().iter().enumerate() {
            if *b == b'\n' && !inside_multi(i) {
                segs.push(&text[start..i]);
                start = i + 1;
            }
        }
        if start <= text.len() {
            segs.push(&text[start..]);
        }
        segs
    }

    /// 覆盖跨行单元的两类来源（多行字符串 / 跨行块注释）与它们的边界：
    /// 单元末尾正好在行尾、注释里伪造 include、CRLF、空输入、字符串内嵌注释起始符。
    fn cases() -> Vec<&'static str> {
        vec![
            "",
            "a: 1\n",
            "a: 1\nb: 2\n",
            "k: \"line1\nline2\"\nb: 2\n",
            "k: \"line1\nline2\"",
            "/* c1\nc2 */\nk: 1\n",
            "/* include \"evil.sml\" */\nk: 1\n",
            "k: \"/* not a comment\nstill string */\"\nb: 2\n",
            "a: 1\r\nb: 2\r\n",
            "k: \"a\"\n/* x\ny */\nk2: \"b\nc\"\n",
        ]
    }

    #[test]
    fn segments_matches_naive() {
        for c in cases() {
            let sp = compute_string_spans(c);
            assert_eq!(
                segments(c, &sp),
                naive_segments(c, &sp),
                "切段结果与朴素实现不一致，输入：{c:?}"
            );
        }
    }
}

