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
use sml_lex::{Tok, compute_string_spans, line_starts_in_string, tokenize};
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
/// - `@feature mode whitelist`       后续 enable 仅保留所列（基集先**清空**）
/// - `@feature mode blacklist`       后续 disable 仅移除所列（基集**全开**）
///
/// 不写 `@feature mode` 时基集 = **按版本派生**（`FeatureSet::for_version(v)`，即
/// 版本基线并按 v2+ 关掉「裸词字符串」），`enable` / `disable` 在其上增减 ——
/// 即「版本基集 + 我显式改的那几个」，**显式声明优先于版本规则**。
/// 版本在 `features_for` 里才可知（`@version` 先被剥掉），故基集的版本部分在那里补。
///
/// ⚠️ 这里改过一次语义：旧实现不写 mode 时基集起算是 `FeatureSet::all()`，于是文档里
/// 出现**任何**一行 `@feature`（哪怕只写 `@feature enable typed-block`）都会让**全部
/// 16 个特性生效** —— `when` / `for` / `glob-include` 以及 opt-in 的
/// `case-insensitive` 被顺手打开，与「enable X = 只开 X」的直觉相反。需要旧行为请显式
/// 声明 `@feature mode blacklist`（保留为逃生舱，未删除）。
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
    touched: &mut FeatureSet,
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
            match *mode {
                // 白名单：基集先清空，后续 enable 显式置位
                FeatureMode::Whitelist => *feats = FeatureSet::none(),
                // 黑名单：基集**全开**，后续 disable 才移除 —— 即旧的默认行为，
                // 现在需要文档显式声明才会走到这里（保持向后可得）。
                FeatureMode::Blacklist => *feats = FeatureSet::all(),
                FeatureMode::Default => {}
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
                *touched = touched.with(f);
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
                *touched = touched.with(f);
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
            // 紧凑黑名单：语义上就是 blacklist 模式（基集全开、移除所列）。
            // ⚠️ 必须显式置 mode：否则 `features_for` 会走 Default 分支、用版本规则
            // 补回「裸词字符串」等项，把这里的 `without` 抵消掉（既有用例
            // `feature::feature_blacklist_removes` 即钉这条）。
            *mode = FeatureMode::Blacklist;
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
/// 按「行 + 行尾」切分，返回每行的**字节区间**（含行终止符）。
///
/// 为什么不用 `str::lines()`：它会吃掉 `\r`、也丢掉「末行有没有换行」的信息 ——
/// 两者都会让剥离后的文本与原文**不等长**。而错误定位要求偏移与原文 1:1 对得上
/// （删一个指令行 ⇒ 其后所有偏移少 N 字节 ⇒ 行号整体前移，红线画到别的行上）。
/// JS 侧 `stripFeatureLines` 早已为此改成「换等长空格」，这里补上 Rust 那一半。
fn line_spans(text: &str) -> Vec<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    while start < bytes.len() {
        let end = match bytes[start..].iter().position(|&b| b == b'\n') {
            Some(off) => start + off + 1, // 含 '\n'
            None => bytes.len(),          // 末行无换行
        };
        out.push((start, end));
        start = end;
    }
    out
}

/// 去掉行尾的换行符（`\n` / `\r\n`），只留行内容 —— 与 `str::lines()` 的取法一致。
fn trim_line_end(raw: &str) -> &str {
    raw.trim_end_matches(|c| c == '\n' || c == '\r')
}

/// 与 `s` **等长**（按字节）的空格串：用于「剥离指令行但不改变偏移」。
fn blank_of(s: &str) -> String {
    " ".repeat(s.len())
}

pub fn strip_features(
    text: &str,
) -> Result<(String, FeatureSet, FeatureSet, FeatureMode, Option<Version>, bool), SmlError> {
    let mut out = String::new();
    let spans = compute_string_spans(text);
    // ⚠️ 基集 = **版本基线**（8 个核心特性），而不是「全开」。
    // 旧实现这里起算是 `FeatureSet::all()`：于是文档里出现**任何**一行 `@feature`
    // （哪怕只写 `@feature enable typed-block`），最终特性集都会变成「全部 16 个特性
    // 全开」—— 连 `when` / `for` / `glob-include` 以及 opt-in 的 `case-insensitive`
    // 都被顺手打开。这与「enable X = 只开 X」的直觉相反（`@feature` 是**开关**，
    // 不是「顺便把其余都打开」的开关）。
    // 需要旧行为（基集全开）时，文档可显式声明 `@feature mode blacklist`。
    let mut feats = FeatureSet::baseline();
    let mut mode = FeatureMode::Default;
    let mut base: Option<Version> = None;
    let mut had_feature = false;
    // 文档**显式**声明过（enable/disable 点过名）的特性。用于在 `features_for` 里把
    // 「基集（按版本派生）」与「文档显式增减」区分开 —— 显式声明优先于版本规则。
    let mut touched = FeatureSet::none();

    for (line_start, line_end) in line_spans(text) {
        let raw = &text[line_start..line_end];
        let line = trim_line_end(raw);
        // 多行字符串内的行（如 `note: "..."` 跨行）里的 `@feature` 不是指令，跳过以免破坏数据
        if !line_starts_in_string(text, line_start, &spans) {
            match apply_feature_directive(line, &mut feats, &mut mode, &mut base, &mut touched) {
                Ok(true) => {
                    had_feature = true;
                    // ⚠️ **等长替换**：指令行换成同长度空格，行尾原样保留。
                    // 直接 `continue`（删行）会让其后所有偏移比原文少 N 字节。
                    out.push_str(&blank_of(line));
                    out.push_str(&raw[line.len()..]);
                    continue;
                }
                Ok(false) => {}
                Err(e) => return Err(e), // 指令非法（如未知特性名）必须上浮，不能静默吞掉
            }
        }
        out.push_str(raw);
        // 末行没有换行时补一个 —— **历史行为**（`tests/error_codes.rs` 的
        // `lex_escape_at_eof_needs_the_lexer` 钉着它的可观察后果）。
        // 补在**文末**不影响任何既有偏移，故与「等长」不冲突。
        if !raw.ends_with('\n') {
            out.push('\n');
        }
    }
    Ok((out, feats, touched, mode, base, had_feature))
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
    // ⚠️ BOM 换成**等长的 3 个空格**（U+FEFF 在 UTF-8 里占 3 字节）而不是删掉：
    // 删掉会让它之后的所有偏移少 3 字节，首行/首列定位就不准了。
    // 空格对词法无影响（`\u{feff}` 才是那个会被吞进键名的字符）。
    let owned;
    let text = match text.strip_prefix('\u{feff}') {
        Some(rest) => {
            owned = format!("   {rest}");
            owned.as_str()
        }
        None => text,
    };
    let spans = compute_string_spans(text);
    let mut declared: Option<Version> = None;
    let mut rest = String::new();
    for (line_start, line_end) in line_spans(text) {
        let raw = &text[line_start..line_end];
        let line = trim_line_end(raw);
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
            // 同 `strip_features`：等长替换，保住后续所有偏移
            rest.push_str(&blank_of(line));
            rest.push_str(&raw[line.len()..]);
            continue;
        }
        rest.push_str(raw);
        // 同上：末行无换行时补一个（历史行为，文末追加不影响既有偏移）
        if !raw.ends_with('\n') {
            rest.push('\n');
        }
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
pub fn features_for(
    v: Version,
    feats: FeatureSet,
    touched: FeatureSet,
    mode: FeatureMode,
    had_feature: bool,
) -> FeatureSet {
    if !had_feature {
        return FeatureSet::for_version(v);
    }
    match mode {
        // 默认：**基集按版本派生**（`for_version(v)`）。`feats` 起算是 `baseline()`，
        // 与版本基线的差异只有「裸词字符串」这一项，这里补上；文档显式声明过该项时
        // 以文档为准（显式优先于版本规则）。
        FeatureMode::Default => {
            if touched.has(Feature::BarewordStr) {
                return feats;
            }
            if v.strict_strings() {
                feats.without(Feature::BarewordStr)
            } else {
                feats.with(Feature::BarewordStr)
            }
        }
        // whitelist / blacklist：基集已在 `strip_features` 里按模式定好（清空 / 全开），
        // 版本规则不参与 —— 保持这两个模式的既有语义不被改动。
        FeatureMode::Whitelist | FeatureMode::Blacklist => feats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sml_lex::tokenize_spans;

    /// 走一遍完整入口：`strip_version` → `strip_features`。
    fn strip_all(text: &str) -> String {
        let (rest, _) = strip_version(text).unwrap();
        let (rest, _, _, _, _, _) = strip_features(&rest).unwrap();
        rest
    }

    /// **核心不变量**：剥离后与原文**等长** —— 否则后面所有偏移/行号都会漂。
    #[test]
    fn stripping_is_length_preserving() {
        let cases = [
            "x: 1\n",
            "x: 1",                                   // 末行无换行
            "@version v1\nx: 1\n",
            "@feature enable top-level-array\nx: 1\n",
            "@version v1\r\nx: 1\r\n",                // CRLF
            "@version v1\r\nx: 1",                    // CRLF + 末行无换行
            "\u{feff}x: 1\n",                         // BOM
            "\u{feff}@version v1\nx: 1\n",            // BOM + 指令
            "note: \"a\nb\"\nx: 1\n",                 // 跨行字符串（其内的行不能被剥）
        ];
        for src in cases {
            let rest = strip_all(src);
            // 末行无换行时会在**文末**补一个 '\n'（历史行为）⇒ 允许 +1；
            // 在那之前的所有偏移必须与原文 1:1 对得上。
            let expected = if src.ends_with('\n') {
                src.len()
            } else {
                src.len() + 1
            };
            assert_eq!(
                rest.len(),
                expected,
                "输入 {src:?}：剥离后长度变了（{:?}）",
                rest
            );
        }
    }

    /// 指令行被换成**等长空格**而不是删掉 ⇒ 行数不变、行尾形态不变。
    #[test]
    fn directive_lines_are_blanked_not_deleted() {
        let src = "@version v1\r\nx: 1\r\n";
        let rest = strip_all(src);
        assert_eq!(rest.lines().count(), 2, "行数必须保持 2");
        let blank = " ".repeat("@version v1".len());
        assert!(
            rest.starts_with(&format!("{blank}\r\n")),
            "首行应是等长空格：{rest:?}"
        );
        assert_eq!(&rest[blank.len() + 2..], "x: 1\r\n", "CRLF 必须原样保留");
    }

    /// 这条才是目的：**指令行之后的 token 位置要对得上原文**。
    #[test]
    fn token_after_directives_is_located_at_original_offset() {
        let src = "@feature enable top-level-array\n@version v1\nab: 1\n";
        let rest = strip_all(src);
        let (toks, spans) = tokenize_spans(&rest).unwrap();
        let idx = toks
            .iter()
            .position(|t| matches!(t, Tok::Word(w) if w == "ab"))
            .expect("应能找到键 ab");
        // ⚠️ 不能写 `src.find("ab")`：enable 里就含 "ab"，会命中错的那处
        let expected = src.find("\nab:").unwrap() + 1;
        assert_eq!(
            spans[idx].start, expected,
            "键 ab 的偏移必须指向**原文**里的 ab"
        );
        assert_eq!(&src[spans[idx].start..spans[idx].end], "ab");
    }

    /// BOM 换成 3 个空格：键名不再被吞（这是 0.6.2 修过的那条），且长度不变。
    #[test]
    fn bom_becomes_three_spaces_and_key_survives() {
        let src = "\u{feff}x: 1\n";
        let rest = strip_all(src);
        assert_eq!(rest.len(), src.len());
        let (toks, _) = tokenize_spans(&rest).unwrap();
        assert_eq!(toks[0], Tok::Word("x".into()), "BOM 不得混进键名");
    }
}
