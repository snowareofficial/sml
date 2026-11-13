// SPDX-License-Identifier: MulanPSL-2.0
//! `@version` / `@feature` 指令的**扫描与剥离**。
//!
//! 位于本 crate 而非 `sml-feature`：需要 `Tok`（词法单元），
//! 而 `sml-lex` 又依赖 `sml-feature` 的 `FeatureSet`——
//! 放进 `sml-feature` 会形成循环依赖。

use sml_codes::{
    E_FEATURE_003, E_FEATURE_004, E_FEATURE_006, E_FEATURE_007, E_FEATURE_008, E_FEATURE_010,
    SmlError,
};
use sml_feature::{
    edition_by_codename, Edition, EditionStatus, FEATURES, Feature, FeatureMode, FeatureNameError,
    FeatureSet,
};
use sml_lex::{Tok, compute_string_spans, line_starts_in_string, tokenize};
use sml_include::strip_line_comment;

/// The token's string content (`Word` / `Str` yield their text; anything else yields `""`) ｜ 取出 token 的字符串内容。
pub fn tok_word(t: &Tok) -> String {
    match t {
        Tok::Word(s) | Tok::Str(s) => s.clone().into_owned(),
        _ => String::new(),
    }
}

/// If the line is a `@feature` directive, update `feats` per its `mode` / operation and return `true` ｜ 若该行是 `@feature` 声明，则更新 `feats` 并返回 true。
///
/// 支持语法（均不区分大小写，参数以空格分隔）：
/// - `@feature base v3`              设定基线版本集（等价于 `@version`，仅用于特性派生）
/// - `@feature mode whitelist`       后续 enable 仅保留所列（基集先**清空**）
/// - `@feature mode blacklist`       后续 disable 仅移除所列（基集**全开**）
///
/// 不写 `@feature mode` 时基集 = **按版本集查表**（`FeatureSet::for_edition(e)`，即
/// `2.0` 起关掉「裸词字符串」），`enable` / `disable` 在其上增减 ——
/// 即「该代的默认位 + 我显式改的那几个」，**显式声明优先于版本规则**。
/// 版本集在 `features_for` 里才可知（`@version` 先被剥掉），故基集的版本部分在那里补。
///
/// 这里改过一次语义：旧实现不写 mode 时基集起算是 `FeatureSet::all()`，于是文档里
/// 出现**任何**一行 `@feature`（哪怕只写 `@feature enable typed-block`）都会让**全部
/// 16 个特性生效** —— `when` / `for` / `glob-include` 以及 opt-in 的
/// `case-insensitive` 被顺手打开，与「enable X = 只开 X」的直觉相反。需要旧行为请显式
/// 声明 `@feature mode blacklist`（保留为逃生舱，未删除）。
/// - `@feature enable <name>[,...]`  开启特性（可逗号批量）
/// - `@feature disable <name>[,...]` 关闭特性
/// - `@feature whitelist <a,b>`      紧凑白名单
/// - `@feature blacklist <a,b>`      紧凑黑名单
///
/// 特性名 → 位：**唯一**入口（命名空间形态 + 注册表都在这里判）。
///
/// `enable` / `disable` / `whitelist` / `blacklist` 四处，以及契约里的 `@requires`，
/// 全部走这里 —— 否则「哪个指令对带点名的处理不一样」这种漂移迟早发生。
///
/// 失败一律 `E-FEATURE-003`（未知特性名），但 detail 分三档：
/// 三段名 / 未知域 / 没注册（附可用清单）—— 用户看一眼就知道是「多点了个点」
/// 还是「名字拼错」还是「这门语言还没这特性」。
fn lookup_feature(n: &str) -> Result<Feature, SmlError> {
    Feature::parse_name(n).map_err(|e| {
        let detail = match e {
            FeatureNameError::Unknown => format!(
                "`{n}`，可用：{}",
                FEATURES
                    .iter()
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            other => format!("`{n}`：{other}"),
        };
        SmlError::from_code_title(E_FEATURE_003).with_detail(detail)
    })
}

/// 未知特性名一律报错，避免拼写错误静默失效。
pub fn apply_feature_directive(
    line: &str,
    feats: &mut FeatureSet,
    mode: &mut FeatureMode,
    base: &mut Option<Edition>,
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
        return Err(SmlError::from_code_msg(E_FEATURE_007));
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
            let word = words.get(2).map(|s| s.as_str()).unwrap_or("");
            // 与 `@version` 共用同一个判据（`resolve_edition`）：字面量 / 代号 → 一代，
            // 并检查状态。`@feature base` 没有 `dev` 槽位，故 Dev 代在这里必然报「须写 dev」。
            let v = resolve_edition(Some(word), None, false, word).map_err(|e| {
                // 起点是 `@feature base`，不是 `@version` —— 报错文案要指对地方。
                e.suffix("（来自 `@feature base`）")
            })?;
            *feats = FeatureSet::for_edition(v);
            *base = Some(v);
            Ok(true)
        }
        "mode" => {
            let m = words.get(2).map(|s| s.as_str()).unwrap_or("");
            *mode = match m {
                "whitelist" => FeatureMode::Whitelist,
                "blacklist" => FeatureMode::Blacklist,
                _ => {
                    return Err(SmlError::from_code_title(E_FEATURE_008)
                        .with_detail(format!("@feature mode 需要 whitelist/blacklist，收到 `{m}`")))
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
                let f = lookup_feature(&n)?;
                *feats = feats.with(f);
                *touched = touched.with(f);
            }
            Ok(true)
        }
        "disable" => {
            for n in names(2) {
                let f = lookup_feature(&n)?;
                *feats = feats.without(f);
                *touched = touched.with(f);
            }
            Ok(true)
        }
        "whitelist" => {
            *mode = FeatureMode::Whitelist;
            let mut s = FeatureSet::none();
            for n in names(2) {
                let f = lookup_feature(&n)?;
                s = s.with(f);
            }
            *feats = s;
            Ok(true)
        }
        "blacklist" => {
            // 紧凑黑名单：语义上就是 blacklist 模式（基集全开、移除所列）。
            // 必须显式置 mode：否则 `features_for` 会走 Default 分支、用版本规则
            // 补回「裸词字符串」等项，把这里的 `without` 抵消掉（既有用例
            // `feature::feature_blacklist_removes` 即钉这条）。
            *mode = FeatureMode::Blacklist;
            let mut s = FeatureSet::all();
            for n in names(2) {
                let f = lookup_feature(&n)?;
                s = s.without(f);
            }
            *feats = s;
            Ok(true)
        }
        _ => Err(SmlError::from_code_title(E_FEATURE_006)
            .with_detail(format!("`{arg}`，可用 base/mode/enable/disable"))),
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
    raw.trim_end_matches(['\n', '\r'])
}

/// 与 `s` **等长**（按字节）的空格串：用于「剥离指令行但不改变偏移」。
fn blank_of(s: &str) -> String {
    " ".repeat(s.len())
}

/// 末行补换行是历史行为，但源文本**以 `\` 结尾**时不能补。
///
/// 补了就把「转义符后遇文件结束」（`E-LEX-006`）改写成「未知转义」（`E-LEX-004`）——
/// 那是「按行重建」的实现副作用，不是语义差别。T21 已裁决统一到 006（2026-11-06），
/// 故这里只放过这一种收尾：EOF 之后的那个位置**本来就没有字符**，不该凭空造一个。
///
/// 不在字符串里时补不补都一样（`\` 只是裸词里的普通字节），所以判据可以这么粗。
fn needs_trailing_newline(raw: &str) -> bool {
    !raw.ends_with('\n') && !raw.ends_with('\\')
}

/// Strip `@version` / `@feature` lines and report what they said ｜ 剥离 `@version` / `@feature` 指令行，并汇报它们说了什么。
///
/// 返回六元组（顺序即为契约，改顺序会静默打乱所有调用方）：
///
/// 1. 剥掉指令行后的正文 —— 每条指令行被替换成**等长空格**，故偏移量与原文**一一对应**
///    （诊断的行列号因此还能对上原文，别改成「删行」）；
/// 2. `feats`：文档声明的特性集，**基集是 [`FeatureSet::baseline`]（按版本）而非全开**；
/// 3. `touched`：文档**显式点名**增减过的特性 —— 用来把「按版本派生的基集」与
///    「作者明确要求」分开，后者优先（喂给 `features_for`）；
/// 4. `mode`：enable/disable 的语义模式（`Default` / `Whitelist` / `Blacklist` 三档，
///    只有 `@feature mode …` 能改）；
/// 5. `base`：**仅**由 `@feature base` 给出的基线版本集（`@version` 不在这里，
///    由调用方另行解析并在优先级上压过它），`None` ⇒ 调用方回落到 `1.0`；
/// 6. `had`：是否**出现过**任何 `@feature` 行。
///
/// 多行字符串内部形如 `@feature` 的文本不算指令（靠 `compute_string_spans` 判定），
/// 否则 `note: "..."` 里写一行 `@feature enable x` 就能改掉整份文档的语义。
/// `strip_features` 的六元组结果；抽成别名压住 `clippy::type_complexity`。
type StrippedFeatures = (String, FeatureSet, FeatureSet, FeatureMode, Option<Edition>, bool);

pub fn strip_features(text: &str) -> Result<StrippedFeatures, SmlError> {
    // ⚡ 快路径：`feature` 一词都不在文本里 ⇒ 不可能有 `@feature`（含 `@ feature` 写法）。
    // 逐行 push_str、字符串区间表、等长替换全可跳过；判据保守，误命中只会走慢路径。
    // ｜ Conservative gate; a false positive merely falls back to the line loop.
    if !text.contains("feature") {
        let mut out = String::with_capacity(text.len() + 1);
        out.push_str(text);
        if needs_trailing_newline(text) {
            out.push('\n'); // 与逐行分支相同的「末行补换行」历史行为
        }
        return Ok((
            out,
            FeatureSet::baseline(),
            FeatureSet::none(),
            FeatureMode::Default,
            None,
            false,
        ));
    }
    let mut out = String::new();
    // ⚡ **惰性求值**（IDEAS I-23 第二处）：`compute_string_spans` 是 O(n) 全文扫描，
    // 存在的唯一目的是回答「这个 `@` 是不是落在多行字符串里」。而**首字符不是 `@` 的行
    // 根本不可能是指令** ⇒ 真遇到候选行时才扫，别为一份没有指令的文档白扫全文。
    let mut spans: Option<Vec<(usize, usize)>> = None;
    // 基集 = **版本基线**（8 个核心特性），而不是「全开」。
    // 旧实现这里起算是 `FeatureSet::all()`：于是文档里出现**任何**一行 `@feature`
    // （哪怕只写 `@feature enable typed-block`），最终特性集都会变成「全部 20 个特性
    // 全开」—— 连 `when` / `for` / `glob-include` 以及 opt-in 的 `case-insensitive`
    // 都被顺手打开。这与「enable X = 只开 X」的直觉相反（`@feature` 是**开关**，
    // 不是「顺便把其余都打开」的开关）。
    // 需要旧行为（基集全开）时，文档可显式声明 `@feature mode blacklist`。
    let mut feats = FeatureSet::baseline();
    let mut mode = FeatureMode::Default;
    let mut base: Option<Edition> = None;
    let mut had_feature = false;
    // 文档**显式**声明过（enable/disable 点过名）的特性。用于在 `features_for` 里把
    // 「基集（按版本派生）」与「文档显式增减」区分开 —— 显式声明优先于版本规则。
    let mut touched = FeatureSet::none();

    for (line_start, line_end) in line_spans(text) {
        let raw = &text[line_start..line_end];
        let line = trim_line_end(raw);
        // 多行字符串内的行（如 `note: "..."` 跨行）里的 `@feature` 不是指令，跳过以免破坏数据。
        // 先做一次**零成本**的候选判断（首字符是 `@`），命中才需要字符串区间表。
        if line.trim_start().starts_with('@') {
            let spans = spans.get_or_insert_with(|| compute_string_spans(text));
            if !line_starts_in_string(text, line_start, spans) {
                match apply_feature_directive(line, &mut feats, &mut mode, &mut base, &mut touched) {
                    Ok(true) => {
                        had_feature = true;
                        // **等长替换**：指令行换成同长度空格，行尾原样保留。
                        // 直接 `continue`（删行）会让其后所有偏移比原文少 N 字节。
                        out.push_str(&blank_of(line));
                        out.push_str(&raw[line.len()..]);
                        continue;
                    }
                    Ok(false) => {}
                    Err(e) => return Err(e), // 指令非法（如未知特性名）必须上浮，不能静默吞掉
                }
            }
        }
        out.push_str(raw);
        // 末行没有换行时补一个 —— **历史行为**（`tests/error_codes.rs` 的
        // `lex_escape_at_eof_stays_006` 钉着它的可观察后果）。
        // 补在**文末**不影响任何既有偏移，故与「等长」不冲突。
        if needs_trailing_newline(raw) {
            out.push('\n');
        }
    }
    Ok((out, feats, touched, mode, base, had_feature))
}




/// `@version` 声明里的三个槽位 ｜ the three slots of an `@version` declaration.
///
/// 目标语法（edition 模型，见 `docs/swsml/spec/version.md`）：
///
/// ```text
/// @version 50 "camelia" dev
///          ↑  ↑          ↑
///        数字 代号      dev 标记（Dev 状态的版本必写）
/// ```
///
/// 旧写法 `@version v1` / `@version 4` / `@version "4"` 一律仍然接受（都是别名）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VersionDecl {
    /// 数字字面量（裸词或引号串皆可），如 `50` / `5.0` / `v4` / `4`。
    pub literal: Option<String>,
    /// 开发代号（**只**接受引号串），如 `camelia`。
    pub codename: Option<String>,
    /// 末尾的 `dev` 标记。
    pub dev: bool,
}

impl VersionDecl {
    /// 用于错误文案的原始写法 ｜ the raw text for diagnostics.
    fn written(&self) -> String {
        self.literal
            .clone()
            .or_else(|| self.codename.clone())
            .unwrap_or_else(|| "<空>".to_string())
    }
}

/// 本实现**已发布**的版本集名字（用 `, ` 连接，用于错误文案）｜ supported edition names.
fn supported_edition_names() -> String {
    sml_feature::EDITIONS
        .iter()
        .filter(|r| r.status == EditionStatus::Supported)
        .map(|r| r.edition.name())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 迁移规则的 id（`5.0` → `v50-version`）｜ migration rule id for one edition.
///
/// 与 `docs/swsml/spec/version.md` §4.5 的规则命名一致：`--rule v50-version` 就是「改 `@version` 行」那条。
/// 规则登记处将来只有一处（V8 / §4.5 第 4 条），此处不另存表 —— 由代号的数字形式拼出来即可。
pub fn migration_rule_id(e: Edition) -> String {
    format!("v{}{}-version", e.major, e.minor)
}

/// `E-FEATURE-010`：版本集**已被移除**（与「未知版本集」`E-FEATURE-004` 分开）｜ edition removed.
///
/// 为什么必须两个码：用户要能分清「我写错了」（004）和「我该迁移了」（010）。
/// 文案固定带三样东西 —— 代号（人记得住）、规则 id（能直接 `--rule` 跑）、迁移命令。
fn edition_removed(e: Edition) -> SmlError {
    SmlError::from_code_title(E_FEATURE_010).with_detail(format!(
        "版本集 `{}`（代号 `{}`）已被移除，不再接受；请运行 `smltools --migrate scan` 按规则 `{}` 升级文档",
        e.name(),
        e.codename().unwrap_or("-"),
        migration_rule_id(e)
    ))
}

/// 把「字面量 / 代号 + `dev` 标记」解析成一代版本集，并检查它的生命周期状态 ｜ resolve an edition.
///
/// **单一判据点**（`docs/swsml/spec/version.md` V10）：`@version` 与 `@feature base` 都走这里，
/// 于是「哪些标识能认」「Dev 要不要写 `dev`」「Removed 报哪个码」只有一份实现。
///
/// 三态处理：
/// - `Supported`：可用（写了 `dev` ⇒ 报错，因为发布版没有开发期）；
/// - `Dev`：必须写 `dev`；定稿前**还没有功能集** ⇒ 明确报「尚未启用」；
/// - `Removed`：**专用码** [`E_FEATURE_010`] + 迁移工具指引；
/// - 未登记：`E-FEATURE-004`（「从未存在或写法不合法」）。
fn resolve_edition(
    literal: Option<&str>,
    codename: Option<&str>,
    dev: bool,
    written: &str,
) -> Result<Edition, SmlError> {
    // 1) 字面量 / 代号 → 一代
    let by_literal = literal.and_then(Edition::parse);
    let by_codename = codename.and_then(edition_by_codename).map(|r| r.edition);

    let ed = match (by_literal, by_codename) {
        (Some(a), Some(b)) if a != b => {
            return Err(SmlError::from_code_title(E_FEATURE_004).with_detail(format!(
                "版本声明里的代号与数字不指向同一代：`{}` vs `{}`",
                codename.unwrap_or_default(),
                a.name()
            )))
        }
        (Some(a), None) | (None, Some(a)) => a,
        (Some(a), Some(_)) => a,
        (None, None) => {
            return Err(SmlError::from_code_title(E_FEATURE_004).with_detail(format!(
                "不支持的 SML 版本集 `{}`（本实现支持 {}）",
                written,
                supported_edition_names()
            )))
        }
    };

    // 2) 状态与 `dev` 标记
    match ed.status() {
        Some(EditionStatus::Supported) => {
            if dev {
                return Err(SmlError::from_code_title(E_FEATURE_004).with_detail(format!(
                    "`{}` 已是发布版本，不需要 `dev` 标记（去掉末尾的 `dev`）",
                    ed.name()
                )));
            }
            Ok(ed)
        }
        Some(EditionStatus::Dev) => {
            if !dev {
                let ver = format!("{}{}", ed.major, ed.minor);
                return Err(SmlError::from_code_title(E_FEATURE_004).with_detail(format!(
                    "`{}` 尚未定稿，须显式写 `dev` 标记：`@version {} \"{}\" dev`",
                    ed.name(),
                    ver,
                    ed.codename().unwrap_or("代号")
                )));
            }
            // 定稿前，本实现**还没有**该代的功能集 ⇒ 明确报「未启用」而不是「不支持」。
            Err(SmlError::from_code_title(E_FEATURE_004).with_detail(format!(
                "版本集 `{}`（代号 `{}`）仍在开发中，本实现尚未启用；请继续用已发布版本（{}）",
                ed.name(),
                ed.codename().unwrap_or("-"),
                supported_edition_names()
            )))
        }
        Some(EditionStatus::Removed) => Err(edition_removed(ed)),
        None => Err(SmlError::from_code_title(E_FEATURE_004).with_detail(format!(
            "不支持的 SML 版本集 `{}`（本实现支持 {}）",
            written,
            supported_edition_names()
        ))),
    }
}

/// 三个槽位 → 一代版本集（`@version` 的入口，转发到 [`resolve_edition`]）｜ resolve `@version` slots.
fn resolve_version_decl(decl: &VersionDecl) -> Result<Edition, SmlError> {
    resolve_edition(
        decl.literal.as_deref(),
        decl.codename.as_deref(),
        decl.dev,
        &decl.written(),
    )
}

/// If the line is a `@version` directive, return its slots; otherwise `None` ｜ 若该行是 `@version` 声明，返回三个槽位；否则 None。
///
/// `version` 是保留字：不允许作为片段名（`@version { }`）使用。
pub fn version_directive(line: &str) -> Result<Option<VersionDecl>, SmlError> {
    let content = strip_line_comment(line).trim();
    // 词法失败的行（如未闭合引号）不是版本声明，交由主解析器报更准确的错
    let toks = match tokenize(content) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let mut it = toks.iter();
    match (it.next(), it.next()) {
        (Some(Tok::At), Some(Tok::Word(w))) if w == "version" => {}
        _ => return Ok(None),
    }
    let rest: Vec<&Tok<'_>> = it.collect();
    if rest.is_empty() {
        return Err(SmlError::from_code_title(E_FEATURE_004).with_detail(
            "`@version` 须带版本字面量或代号（如 `@version 4` / `@version 50 \"camelia\" dev`）；`version` 不可作为片段名",
        ));
    }
    if rest.len() > 3 {
        return Err(SmlError::from_code_title(E_FEATURE_004).with_detail(format!(
            "`@version` 最多三个槽位（数字 代号 dev），实际给了 {} 个",
            rest.len()
        )));
    }
    let mut decl = VersionDecl::default();
    for (idx, t) in rest.iter().enumerate() {
        match t {
            Tok::Str(s) => {
                // 引号串既可能是数字（`@version "4"`，历史写法），也可能是代号
                if Edition::parse(s).is_some() {
                    if decl.literal.replace(s.clone().into_owned()).is_some() {
                        return Err(SmlError::from_code_title(E_FEATURE_004)
                            .with_detail("`@version` 出现了两个数字字面量"));
                    }
                } else if decl.codename.replace(s.clone().into_owned()).is_some() {
                    return Err(SmlError::from_code_title(E_FEATURE_004)
                        .with_detail("`@version` 出现了两个代号"));
                }
            }
            Tok::Word(w) if w == "dev" => {
                if idx + 1 != rest.len() {
                    return Err(SmlError::from_code_title(E_FEATURE_004).with_detail(
                        "`dev` 只能出现在 `@version` 的**最后**（`@version 50 \"camelia\" dev`）",
                    ));
                }
                decl.dev = true;
            }
            Tok::Word(w) => {
                if decl.literal.replace(w.clone().into_owned()).is_some() {
                    return Err(SmlError::from_code_title(E_FEATURE_004)
                        .with_detail("`@version` 出现了两个数字字面量"));
                }
            }
            _ => {
                return Err(SmlError::from_code_title(E_FEATURE_004)
                    .with_detail("`@version` 后面只能是版本字面量、代号（引号串）或 `dev`"))
            }
        }
    }
    Ok(Some(decl))
}

/// Strip `@version` lines, returning the remaining text and the declared version (`None` if none) ｜ 剥离 `@version` 声明行。
///
/// 允许多次声明（include 进来的文件可各自声明），但必须一致；
/// 声明了实现不支持的版本时报错，避免静默按错误语法解析。
pub fn strip_version(text: &str) -> Result<(String, Option<Edition>), SmlError> {
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
    // 仓库里 `smltools` 的 **YAML / XML 导入器早就各自去了 BOM**（`yaml.rs:93`、`xml.rs:63`），
    // 只有 native SML 这条路径漏了 —— 属于"同一件事两套待遇"，本轮补齐。
    // BOM 换成**等长的 3 个空格**（U+FEFF 在 UTF-8 里占 3 字节）而不是删掉：
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
    // ⚡ 快路径：指令词 `version` 根本不在文本里 ⇒ 不可能有 `@version`（哪怕写成
    // `@ version` 也含这个词）。逐行 push_str 与字符串区间表都可以整段跳过。
    // 判据刻意保守：`version` 出现在普通数据里只会误走慢路径，不会漏判。
    // ｜ Conservative gate: a false positive only costs the old path, never a wrong parse.
    if !text.contains("version") {
        let mut rest = String::with_capacity(text.len() + 1);
        rest.push_str(text);
        if needs_trailing_newline(text) {
            rest.push('\n'); // 与下面逐行分支相同的「末行补换行」历史行为
        }
        return Ok((rest, None));
    }
    // ⚡ 惰性求值（同 `strip_features`）：只有首字符是 `@` 的行才可能是指令
    let mut spans: Option<Vec<(usize, usize)>> = None;
    let mut declared: Option<Edition> = None;
    let mut rest = String::new();
    for (line_start, line_end) in line_spans(text) {
        let raw = &text[line_start..line_end];
        let line = trim_line_end(raw);
        // 多行字符串内的行（如 `note: "..."` 跨行）里的 @version 不是指令，原样保留。
        // 只解析一次：原写法先 `matches!(.., Some(_))` 再 `unwrap()`，既多解析一遍，
        // 又把「一定为 Some」这个不变量藏进 unwrap（改坏前半句就会 panic）。
        // ｜ Parse once; the old code called version_directive twice and unwrapped.
        let direct_decl = if line.trim_start().starts_with('@') {
            let spans = spans.get_or_insert_with(|| compute_string_spans(text));
            if line_starts_in_string(text, line_start, spans) {
                None
            } else {
                version_directive(line)?
            }
        } else {
            None
        };
        if let Some(decl) = direct_decl {
            let ed = resolve_version_decl(&decl)?;
            match declared {
                None => declared = Some(ed),
                Some(prev) if prev != ed => {
                    return Err(SmlError::from_code_title(E_FEATURE_004)
                        .with_detail(format!("@version 冲突：{} 与 {}", prev.name(), ed.name())))
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
        if needs_trailing_newline(raw) {
            rest.push('\n');
        }
    }
    Ok((rest, declared))
}

/// Merge the declared edition with the document's `@feature` directives into the effective feature set ｜ 把版本集与文档 `@feature` 指令合并为最终生效的特性集。
///
/// 规则：
/// - 若文档显式声明过 `@feature`（had_feature=true），则完全采用其推导的 `feats`；
/// - 否则（仅靠 `@version` 声明或默认），取该代版本的**默认功能集**
///   （[`FeatureSet::for_edition`] 查表：`2.0` 起不含 `bareword-string`）。
///
/// 这样 v3 文档即使不写任何 `@feature` 也默认严格；调用方的 `allowed` 在
/// 入口处再与结果取交集，文档无法扩宽。
pub fn features_for(
    e: Edition,
    feats: FeatureSet,
    touched: FeatureSet,
    mode: FeatureMode,
    had_feature: bool,
) -> FeatureSet {
    if !had_feature {
        return FeatureSet::for_edition(e);
    }
    match mode {
        // 默认：**基集按版本集查表**（`for_edition(e)`）。`feats` 起算是 `baseline()`，
        // 与版本基线的差异只有「裸词字符串」这一项，这里补上；文档显式声明过该项时
        // 以文档为准（显式优先于版本规则）。
        FeatureMode::Default => {
            if touched.has(Feature::BarewordStr) {
                return feats;
            }
            if e.strict_strings() {
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
        // 不能写 `src.find("ab")`：enable 里就含 "ab"，会命中错的那处
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

    // =======================================================================
    // S0c：`Removed` 专用码 + 「单一判据点」`resolve_edition`
    // =======================================================================

    /// `Removed` 必须用**专用码**（`E-FEATURE-010`），文案带代号 + 迁移规则 id。
    ///
    /// 登记表里现在还没有 `Removed` 行 ⇒ 只能直接测构造函数。加一行即接上，
    /// 这条用例同时钉住「码与文案长什么样」，防止将来接分支时又复用 004。
    #[test]
    fn removed_edition_has_its_own_code() {
        let e = edition_removed(Edition::new(5, 0));
        assert_eq!(e.code(), E_FEATURE_010, "Removed 不能复用 E-FEATURE-004");
        let msg = format!("{e}");
        assert!(msg.contains("camelia"), "文案应带代号：{msg}");
        assert!(msg.contains("v50-version"), "文案应带迁移规则 id：{msg}");
        assert!(msg.contains("--migrate scan"), "文案应给迁移命令：{msg}");
    }

    /// 「从未存在」与「已被移除」是**两个码** —— 这是本轮最核心的一条区分。
    #[test]
    fn unknown_edition_stays_on_004() {
        let e = resolve_edition(Some("99"), None, false, "99").unwrap_err();
        assert_eq!(e.code(), E_FEATURE_004);
        assert!(format!("{e}").contains("不支持"), "{e}");
    }

    /// `@feature base` 与 `@version` 共用一个判据点 ⇒ Dev 代在这里同样要求 `dev`。
    #[test]
    fn feature_base_uses_the_same_edition_gate() {
        // 已发布代：正常
        let (_, _, _, _, base, _) = strip_features("@feature base 30\nx: 1\n").unwrap();
        assert_eq!(base, Some(Edition::new(3, 0)));
        // Dev 代：没有 `dev` 槽位可选 ⇒ 必须报「须显式写 dev」
        let e = strip_features("@feature base 50\nx: 1\n").unwrap_err();
        assert_eq!(e.code(), E_FEATURE_004);
        assert!(format!("{e}").contains("dev"), "{e}");
    }

    /// 3.0/4.0 的默认功能集照旧关掉裸词字符串（查表路径不能比旧 `>=` 判据更松）。
    #[test]
    fn edition_table_drives_strictness() {
        let (_, _, _, _, base, _) = strip_features("x: 1\n").unwrap();
        assert_eq!(base, None);
        assert!(!Edition::V1.strict_strings());
        assert!(Edition::V3.strict_strings());
        assert!(Edition::new(5, 0).strict_strings(), "5.0 也已是引号严格");
    }
}
