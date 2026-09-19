// SPDX-License-Identifier: MulanPSL-2.0
//! `@version` / `@feature` 指令的**扫描与剥离**。
//!
//! 位于本 crate 而非 `sml-feature`：需要 `Tok`（词法单元），
//! 而 `sml-lex` 又依赖 `sml-feature` 的 `FeatureSet`——
//! 放进 `sml-feature` 会形成循环依赖。

use sml_codes::{
    E_FEATURE_003, E_FEATURE_004, E_FEATURE_006, E_FEATURE_007, E_FEATURE_008, SmlError,
};
use sml_feature::{FEATURES, Feature, FeatureMode, FeatureSet, Version};
use sml_lex::{Tok, advance_line, compute_string_spans, line_starts_in_string, tokenize};
use sml_include::strip_line_comment;

/// 取出 token 的字符串内容（Word / Str 都取其文本；其余返回空串）。
pub fn tok_word(t: &Tok) -> String {
    match t {
        Tok::Word(s) | Tok::Str(s) => s.clone(),
        _ => String::new(),
    }
}

/// 若该行是 `@feature` 声明，则根据 `mode` / 操作更新 `feats`，并返回 true。
///
/// 支持语法（均不区分大小写，参数以空格分隔）：
/// - `@feature base v3`              设定基线版本（等价于 `@version`，仅用于特性派生）
/// - `@feature mode whitelist`       后续 enable 仅保留所列（基集先清空）
/// - `@feature mode blacklist`       后续 disable 仅移除所列（基集保持全开）
/// - `@feature enable <name>[,...]`  开启特性（可逗号批量）
/// - `@feature disable <name>[,...]` 关闭特性
/// - `@feature whitelist <a,b>`      紧凑白名单
/// - `@feature blacklist <a,b>`      紧凑黑名单
///
/// 未知特性名一律报错，避免拼写错误静默失效。
pub fn apply_feature_directive(
    line: &str,
    feats: &mut FeatureSet,
    mode: &mut FeatureMode,
    base: &mut Option<Version>,
) -> Result<bool, SmlError> {
    let content = strip_line_comment(line).trim();
    let toks = match tokenize(content) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    if toks.is_empty() || toks[0] != Tok::At {
        return Ok(false);
    }
    let words: Vec<String> = toks
        .iter()
        .map(|t| match t {
            Tok::At => "@".to_string(),
            other => tok_word(other),
        })
        .collect();
    // @feature 词法上拆成 [@, feature]，拼前两个 token 才是 "@feature"
    let head = format!("{}{}", words.first().map(|s| s.as_str()).unwrap_or(""), words.get(1).map(|s| s.as_str()).unwrap_or(""));
    if head != "@feature" {
        return Ok(false);
    }
    // 去掉首 token `@`，使后续 words[0]=="feature"
    let words: Vec<String> = words[1..].to_vec();
    if words.len() < 2 {
        return Err(SmlError::new(E_FEATURE_007, "@feature 指令缺少参数"));
    }
    let arg = words[1].as_str();
    // 把 `enable x,y,z` / `whitelist a,b` 的多名拆开
    let names = |from: usize| -> Vec<String> {
        words[from..]
            .join(",")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    match arg {
        "base" => {
            let v = Version::from_word(words.get(2).map(|s| s.as_str()).unwrap_or(""))
                .ok_or_else(|| {
                    SmlError::new(
                        E_FEATURE_004,
                        format!(
                            "@feature base 需要 v1/v2/v3/v4，收到 `{}`",
                            words.get(2).cloned().unwrap_or_default()
                        ),
                    )
                })?;
            *feats = FeatureSet::for_version(v);
            *base = Some(v);
            Ok(true)
        }
        "mode" => {
            let m = words.get(2).map(|s| s.as_str()).unwrap_or("");
            *mode = match m {
                "whitelist" => FeatureMode::Whitelist,
                "blacklist" => FeatureMode::Blacklist,
                _ => {
                    return Err(SmlError::new(
                        E_FEATURE_008,
                        format!("@feature mode 需要 whitelist/blacklist，收到 `{m}`"),
                    ))
                }
            };
            if *mode == FeatureMode::Whitelist {
                // 白名单：基集先清空，后续 enable 显式置位
                *feats = FeatureSet::none();
            }
            Ok(true)
        }
        "enable" => {
            // 直接在「当前特性集」上叠加开启（不切换白名单语义）。
            // 这样 `@feature enable regex-include` 在 `@version v1` 文档上会保留
            // bareword-string 等默认特性，而非收窄为仅所列项。
            // 真正的「收窄为仅所列」由显式 `@feature mode whitelist` 控制。
            for n in names(2) {
                let f = Feature::from_name(&n).ok_or_else(|| {
                    SmlError::new(
                        E_FEATURE_003,
                        format!(
                            "未知特性 `{n}`，可用：{}",
                            FEATURES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                        ),
                    )
                })?;
                *feats = feats.with(f);
            }
            Ok(true)
        }
        "disable" => {
            for n in names(2) {
                let f = Feature::from_name(&n).ok_or_else(|| {
                    SmlError::new(
                        E_FEATURE_003,
                        format!(
                            "未知特性 `{n}`，可用：{}",
                            FEATURES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                        ),
                    )
                })?;
                *feats = feats.without(f);
            }
            Ok(true)
        }
        "whitelist" => {
            *mode = FeatureMode::Whitelist;
            let mut s = FeatureSet::none();
            for n in names(2) {
                let f = Feature::from_name(&n).ok_or_else(|| {
                    SmlError::new(
                        E_FEATURE_003,
                        format!(
                            "未知特性 `{n}`，可用：{}",
                            FEATURES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                        ),
                    )
                })?;
                s = s.with(f);
            }
            *feats = s;
            Ok(true)
        }
        "blacklist" => {
            let mut s = FeatureSet::all();
            for n in names(2) {
                let f = Feature::from_name(&n).ok_or_else(|| {
                    SmlError::new(
                        E_FEATURE_003,
                        format!(
                            "未知特性 `{n}`，可用：{}",
                            FEATURES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                        ),
                    )
                })?;
                s = s.without(f);
            }
            *feats = s;
            Ok(true)
        }
        _ => Err(SmlError::new(
            E_FEATURE_006,
            format!("未知 @feature 子命令 `{arg}`，可用 base/mode/enable/disable"),
        )),
    }
}

/// 在解析前剥离全部 `@feature` 指令，返回剩余文本、推导出的特性集，以及
/// 由 `@feature base vN` 声明的基线版本（若文档未用 `@version` 则采用它）。
///
/// 文档内部的 `@feature` 只能收窄；调用方允许范围由 `parse_with_features`
/// / `parse_allowed` 的 `allowed` 参数在入口处再次交集。
///
/// 返回值三元组：(剩余文本, 特性集, @feature base 声明的版本, 是否出现过 @feature 指令)。
/// 若文档从未声明 `@feature`，则 `had_feature=false`，调用方应改以版本基线派生特性集
/// （例如 v3 默认关闭裸词字符串）。
pub fn strip_features(text: &str) -> Result<(String, FeatureSet, Option<Version>, bool), SmlError> {
    let mut out = String::new();
    let spans = compute_string_spans(text);
    let mut feats = FeatureSet::all();
    let mut mode = FeatureMode::Default;
    let mut base: Option<Version> = None;
    let mut had_feature = false;

    let mut line_start = 0usize;
    for line in text.lines() {
        // 多行字符串内的行（如 `note: "..."` 跨行）里的 `@feature` 不是指令，跳过以免破坏数据
        if !line_starts_in_string(text, line_start, &spans) {
            match apply_feature_directive(line, &mut feats, &mut mode, &mut base) {
                Ok(true) => {
                    had_feature = true;
                    line_start = advance_line(line_start, line, text);
                    continue; // 指令行被消费，不进入剩余文本
                }
                Ok(false) => {}
                Err(e) => return Err(e), // 指令非法（如未知特性名）必须上浮，不能静默吞掉
            }
        }
        out.push_str(line);
        out.push('\n');
        line_start = advance_line(line_start, line, text);
    }
    Ok((out, feats, base, had_feature))
}




/// 若该行是 `@version` 声明，返回版本字面量；否则返回 None。
///
/// `version` 是保留字：不允许作为片段名（`@version { }`）使用。
pub fn version_directive(line: &str) -> Result<Option<String>, SmlError> {
    let content = strip_line_comment(line).trim();
    // 词法失败的行（如未闭合引号）不是版本声明，交由主解析器报更准确的错
    let toks = match tokenize(content) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    match toks.as_slice() {
        [Tok::At, Tok::Word(w), Tok::Word(v)] if w == "version" => Ok(Some(v.clone())),
        [Tok::At, Tok::Word(w), Tok::Str(v)] if w == "version" => Ok(Some(v.clone())),
        [Tok::At, Tok::Word(w), ..] if w == "version" => Err(SmlError::new(
            E_FEATURE_004,
            "`@version` 是版本声明指令，须写作 `@version v1`；`version` 不可作为片段名",
        )),
        _ => Ok(None),
    }
}

/// 剥离 `@version` 声明行，返回剩余文本与声明的版本（未声明则为 None）。
///
/// 允许多次声明（include 进来的文件可各自声明），但必须一致；
/// 声明了实现不支持的版本时报错，避免静默按错误语法解析。
pub fn strip_version(text: &str) -> Result<(String, Option<Version>), SmlError> {
    // —— 文件级规范化：去掉开头的 **UTF-8 BOM**（U+FEFF）——
    //
    // 为什么放这里：本函数是**所有文本入口**（`parse` / `parse_with` / `parse_versioned` /
    // `parse_file` / `parse_file_versioned` / `parse_file_features`）的**唯一漏斗**
    // （每个入口第一步都调它）⇒ 一处收口，五端（Rust/C/C++/Lua/JS）语义才能一致。
    //
    // 为什么必须去：BOM **不是空白**（`char::is_whitespace('\u{feff}') == false`），
    // 于是 `\u{feff}x: 1` 会被词法当成单词 `\u{feff}x` ⇒ **第一个键名被静默改掉**
    // （本轮五端实测全中：Rust 输出 `"\u{feff}x": 1`、JS 输出 `{"\u{feff}x":1}`）。
    // 来源极常见：Windows 记事本「另存为 UTF-8」、Excel 导出的文本。
    //
    // ⚠️ 仓库里 `smltools` 的 **YAML / XML 导入器早就各自去了 BOM**（`yaml.rs:93`、`xml.rs:63`），
    // 只有 native SML 这条路径漏了 —— 属于"同一件事两套待遇"，本轮补齐。
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let spans = compute_string_spans(text);
    let mut declared: Option<Version> = None;
    let mut rest = String::new();
    let mut line_start = 0usize;
    for line in text.lines() {
        // 多行字符串内的行（如 `note: "..."` 跨行）里的 @version 不是指令，原样保留
        let is_directive = if line_starts_in_string(text, line_start, &spans) {
            false
        } else {
            matches!(version_directive(line)?, Some(_))
        };
        if is_directive {
            let lit = version_directive(line)?.unwrap();
            let v = Version::from_word(&lit).ok_or_else(|| {
                SmlError::new(
                    E_FEATURE_004,
                    format!(
                        "不支持的 SML 版本 `{lit}`（本实现支持 {}）",
                        Version::CURRENT.name()
                    ),
                )
            })?;
            match declared {
                None => declared = Some(v),
                Some(prev) if prev != v => {
                    return Err(SmlError::new(
                        E_FEATURE_004,
                        format!("@version 冲突：{} 与 {}", prev.name(), v.name()),
                    ))
                }
                Some(_) => {}
            }
            line_start = advance_line(line_start, line, text);
            continue;
        }
        rest.push_str(line);
        rest.push('\n');
        line_start = advance_line(line_start, line, text);
    }
    Ok((rest, declared))
}

/// 把 版本 + 文档 @feature 指令 合并为最终生效的特性集。
///
/// 规则：
/// - 若文档显式声明过 `@feature`（had_feature=true），则完全采用其推导的 `feats`；
/// - 否则（仅靠 `@version` 声明或默认），从版本基线派生（如 v3 关闭裸词字符串）。
/// 这样 v3 文档即使不写任何 `@feature` 也默认严格；调用方的 `allowed` 在
/// 入口处再与结果取交集，文档无法扩宽。
pub fn features_for(v: Version, feats: FeatureSet, had_feature: bool) -> FeatureSet {
    if had_feature {
        feats
    } else {
        FeatureSet::for_version(v)
    }
}
