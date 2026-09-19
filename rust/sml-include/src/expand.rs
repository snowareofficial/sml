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
use sml_lex::{Tok, advance_line, compute_string_spans, line_starts_in_string, tokenize};

use crate::{
    MAX_INCLUDE_DEPTH, MAX_INCLUDE_EXPANSIONS, parse_include_line,
    resolve_target_paths, strip_line_comment,
};

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

/// 递归展开 include 到 `out` token 流。
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
        return Err(SmlError::new(
            E_INCLUDE_004,
            format!("include 嵌套超过 {MAX_INCLUDE_DEPTH} 层"),
        ));
    }
    // 沙箱根：所有 include 命中的文件必须位于 base（规范化为绝对路径）之内，
    // 否则拒绝，防止 `../` 或 glob/regex 模式越界读取任意文件（路径遍历漏洞）。
    //
    // 规范化失败时**必须拒绝**而不是回落到未规范化的 base —— 回落后
    // `starts_with` 比较的对象就变成一个可能含 `..` 的路径，越界校验会静默失效
    // （安全默认、无法校验即不放行）。
    let base_canon = base.canonicalize().map_err(|e| {
        SmlError::new(
            E_INCLUDE_010,
            format!(
                "include 基准目录不可解析，无法做越界校验，已拒绝继续：{}（{e}）",
                base.display()
            ),
        )
    })?;
    let spans = compute_string_spans(text);
    let mut line_start = 0usize;

    // —— 快速路径：整篇**没有** include 指令 ⇒ 整篇词法一次，不进逐行循环 ——
    //
    // 为什么必须有这条路：下面的慢路径是**逐行** `tokenize(line)`，因此任何**跨行的
    // 词法单元**（多行块注释 `/* … */`、多行字符串）都会在第 1 行被判"未闭合"。
    // 实测后果（文件入口全中：`parse_file` / C-ABI `sml_load_file` / 编辑器）：
    //   · `examples/common.sml`（多行块注释）⇒ `E-LEX-002`；
    //   · `examples/doc-demo/yuntianming_original.sml`、`examples/micro/CH32V103xx.sml`、
    //     `examples/slint/calculator.sml`（多行字符串）⇒ `E-LEX-001`；
    //   而 `parse(text)` 没这个问题（它整篇词法）。无 include 时"展开"本就是恒等操作，
    //   故整篇词法即可，语义与 `parse` 完全一致。
    //
    // ⚠️ 已知遗留：**既有 include、又有跨行词法单元**的文档仍会走慢路径而报错
    //   （见 TODO「逐行 tokenize 的架构缺陷」）—— 那条要改成"整篇词法 + 按行插入"。
    {
        let mut ls = 0usize;
        let mut has_include = false;
        for line in text.lines() {
            let inside = line_starts_in_string(text, ls, &spans);
            ls = advance_line(ls, line, text);
            if inside {
                continue; // 多行字符串内部的行不是指令（与下面的循环同判据）
            }
            if matches!(parse_include_line(line, features)?, Some(_)) {
                has_include = true;
                break;
            }
        }
        if !has_include {
            out.extend(tokenize(text)?);
            return Ok(());
        }
    }
    for line in text.lines() {
        // 多行字符串内部的行（如 `"...\ninclude \"x\"\n..."`）里的 include 不是指令，
        // 更不能被当作文件读取（防止字符串内伪造 include 触发任意文件读取）。
        let inside_string = line_starts_in_string(text, line_start, &spans);
        line_start = advance_line(line_start, line, text);
        if inside_string {
            // 当作普通行 tokenize（保持与字符串片段一致），不进入 include 解析分支
            // 内层是词法错误（E-LEX-*），外壳标 E-INCLUDE-011；内层码保留在文案里。
            let line_toks = tokenize(line).map_err(|e| {
                SmlError::new(
                    E_INCLUDE_011,
                    format!("include 预处理词法错误：{e}（于行：{line}）"),
                )
            })?;
            out.extend(line_toks);
            continue;
        }
        match parse_include_line(line, features)? {
            Some(targets) => {
                if !features.has(Feature::Include) {
                    return Err(SmlError::new(
                        E_FEATURE_001,
                        "sml: 当前特性集禁用了 include（include 特性）",
                    ));
                }
                for t in targets {
                    if t.namespace.is_some() && !features.has(Feature::Namespace) {
                        return Err(SmlError::new(
                            E_FEATURE_001,
                            "sml: 当前特性集禁用了命名空间包含（namespace 特性）",
                        ));
                    }
                    // 把一个 target 解析为 0..N 个实际文件路径（支持 glob/regex/ext-rewrite）
                    let paths = resolve_target_paths(&t, base, features)?;
                    for path in paths {
                        let canon = path.canonicalize().map_err(|e| {
                            SmlError::new(
                                E_INCLUDE_001,
                                format!("include 无法定位 {}: {e}", path.display()),
                            )
                        })?;
                        // 路径遍历防护：规范化后必须仍位于沙箱根 base 之内
                        if !canon.starts_with(&base_canon) {
                            return Err(SmlError::new(
                                E_INCLUDE_003,
                                format!(
                                    "include 越界拒绝：{} 不在基准目录 {} 内",
                                    canon.display(),
                                    base_canon.display()
                                ),
                            ));
                        }
                        // stack 是「当前正在展开的文件链」，命中即成环
                        if stack.iter().any(|p| p == &canon) {
                            return Err(SmlError::new(
                                E_INCLUDE_002,
                                format!("include 循环引用: {}", canon.display()),
                            ));
                        }
                        let content = std::fs::read_to_string(&canon).map_err(|e| {
                            SmlError::new(
                                E_IO_001,
                                format!("include 读取失败 {}: {e}", canon.display()),
                            )
                        })?;
                        *expansions += 1;
                        if *expansions > MAX_INCLUDE_EXPANSIONS {
                            return Err(SmlError::new(
                                E_LIMIT_003,
                                format!(
                                    "include 展开次数超过上限 {}（疑似指数膨胀 DoS）",
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
                    SmlError::new(
                        E_INCLUDE_011,
                        format!("include 预处理词法错误：{e}（于行：{line}）"),
                    )
                })?;
                out.extend(line_toks);
            }
        }
    }
    Ok(())
}

/// 读取单个文件内容，剥离其自身的 `@version`/`@feature` 行后 tokenize。
/// 子文件不引入新特性维度，由主文件/调用方统一控制。
/// 仅保留 `toks` 中顶层键名属于 `keys` 的条目；其余顶层条目被丢弃。
/// 嵌套层级（块 `{}` / 数组 `[]`）内的键不受影响——只有 depth==0 的顶层键被过滤。
/// 用于 `import "x" { a, b }` 部分引用：避免整文件内联。
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

pub fn expand_file_tokens(
    content: &str,
    base: &Path,
    stack: &mut Vec<PathBuf>,
    features: FeatureSet,
    expansions: &mut u64,
) -> Result<Vec<Tok>, SmlError> {
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

