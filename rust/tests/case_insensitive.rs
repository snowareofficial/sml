//! `case-insensitive` 特性：契约名查找的**首字母**大小写不敏感（opt-in）。
//!
//! 背景：默认（未开启）时契约名严格区分大小写。契约声明为 `Rule`、块级标注写成
//! `rule ssh { .. }`，首词不会命中契约表 ⇒ 契约**静默不生效**（不校验、不填默认值、
//! 也不报错）。本特性让「写错首字母」不再等于「校验形同虚设」。
//!
//! 口径：**只影响查找** —— 契约声明名、`__type` / `__name` 元数据、错误信息里的
//! 契约名一律保持书写原样；且只归一化**首字母**，不是整体忽略大小写。
//!
//! ⚠️ 用 `parse_with_features` 精确控制特性集来测「开 / 关」：因为 `@feature` 默认走
//! **blacklist 模式（基集全开）**，文档里一旦出现任何 `@feature` 指令，特性集就从
//! `FeatureSet::all()` 起算 —— 若用裸 `parse()`，只写 `@feature enable typed-block`
//! 也会把本特性一并打开，测不出开关效果（见文末
//! `feature_directive_implies_all_features_on`）。

use sml::{feature_names, parse, parse_with_features, Feature, FeatureSet};

const CONTRACT: &str = "@contract Rule { action: enum [ allow deny ] }\n";

/// 用 `allowed` 精确控制：baseline + typed-block（+ 可选 case-insensitive）。
/// 返回解析结果（Err 即校验被拦下）。
fn parse_ci(block: &str, ci: bool) -> Result<sml::Value, String> {
    let mut allowed = FeatureSet::baseline()
        .with(Feature::TypedBlock)
        .with(Feature::BlockName); // 实例名（`rule ssh`）需 block-name 特性
    if ci {
        allowed = allowed.with(Feature::CaseInsensitive);
    }
    // `allowed` 只能收窄、不能扩宽 ⇒ opt-in 特性必须由文档自己 `@feature enable` 声明
    let feat = if ci { "@feature enable case-insensitive\n" } else { "" };
    let src = format!(
        "@feature enable typed-block\n@feature enable block-name\n{feat}{CONTRACT}{block}\n"
    );
    parse_with_features(&src, allowed)
        .map(|(v, _)| v)
        .map_err(|e| e.to_string())
}

#[test]
fn feature_is_registered_and_opt_in() {
    assert_eq!(
        Feature::from_name("case-insensitive"),
        Some(Feature::CaseInsensitive)
    );
    assert!(feature_names().contains(&"case-insensitive"));
    // opt-in：不在 baseline 中
    assert!(!FeatureSet::baseline().has(Feature::CaseInsensitive));
}

#[test]
fn bit_position_is_stable() {
    // C-ABI 位序绑定：追加在末尾，不得插队（改动会破坏跨端位序）
    assert_eq!(Feature::TypedBlock as u8, 14);
    assert_eq!(Feature::CaseInsensitive as u8, 15);
    assert!(!FeatureSet::none()
        .with(Feature::TypedBlock)
        .has(Feature::CaseInsensitive));
}

#[test]
fn off_lower_case_silently_bypasses_contract() {
    // 关：违反 enum 却通过 —— 契约根本没生效，这正是本特性要修的坑
    assert!(parse_ci("rule ssh { action: bogus }", false).is_ok());
}

#[test]
fn on_lower_case_applies_contract() {
    // 开：`rule` 命中 `Rule` ⇒ 契约生效 ⇒ 拦下 enum 违规
    let err = parse_ci("rule ssh { action: bogus }", true).unwrap_err();
    assert!(err.contains("E-CONTRACT-006"), "实际报错：{err}");
}

#[test]
fn exact_match_unaffected_by_switch() {
    // 精确匹配路径与开关无关（开/关都拦下），且报错显示**声明名** `Rule`
    for ci in [false, true] {
        let err = parse_ci("Rule ssh { action: bogus }", ci).unwrap_err();
        assert!(err.contains("E-CONTRACT-006"), "ci={ci} 实际报错：{err}");
        assert!(err.contains("Rule"), "ci={ci} 应显示声明名，实际：{err}");
    }
}

#[test]
fn only_first_letter_is_normalized() {
    // 契约 `Rule`，写成 `RUle`：只有首字母参与归一化，其余仍区分 ⇒ 不命中
    assert!(parse_ci("RUle ssh { action: bogus }", true).is_ok());
}

#[test]
fn at_is_lower_case_follows_the_switch() {
    let mk = |ci: bool| {
        // `allowed` 只能**收窄**、不能扩宽（见 api.rs）：opt-in 特性必须由文档自己
        // `@feature enable` 声明，调用方再决定是否放行。
        let mut allowed = FeatureSet::all();
        if !ci {
            allowed = FeatureSet::baseline();
        }
        let feat = if ci { "@feature enable case-insensitive\n" } else { "" };
        let src = format!("{feat}{CONTRACT}ssh {{ @is rule action: allow }}\n");
        parse_with_features(&src, allowed)
            .map(|(v, _)| v)
            .map_err(|e| e.to_string())
    };
    // 关：`rule` 未定义
    let err = mk(false).unwrap_err();
    assert!(err.contains("E-CONTRACT-001"), "实际报错：{err}");
    // 开：命中契约 `Rule`，校验通过
    assert!(mk(true).is_ok(), "实际：{:?}", mk(true).unwrap_err());
}

#[test]
fn metadata_keeps_written_form() {
    // 只影响查找：结果树里的键与 `__type` 仍是**书写形式** `rule`，未被改写成 `Rule`
    let v = parse_ci("rule ssh { action: allow }", true).unwrap();
    let block = v.get("rule").expect("键应为书写形式 rule");
    assert_eq!(
        block.get("__type").and_then(|x| x.as_str()),
        Some("rule"),
        "__type 应保持书写原样"
    );
    assert_eq!(block.get("__name").and_then(|x| x.as_str()), Some("ssh"));
}

#[test]
fn defaults_are_filled_when_matched_by_first_letter() {
    // 兜底命中后走的是正常契约应用路径（默认值照常填充），不是另一套逻辑
    let src = format!(
        "@feature enable case-insensitive\n\
         @contract Cfg {{ host: str port: int default 8080 }}\n\
         cfg web {{ @is cfg host: h1 }}\n"
    );
    let v = parse(&src).expect("`@is cfg` 应命中契约 Cfg");
    assert_eq!(v.get("cfg.port"), Some(&sml::Value::Int(8080)));
}

/// 默认基集 = **版本基线**（不是"全开"）：只写 `@feature enable typed-block`
/// 不会顺手把 `case-insensitive` 一并打开。
///
/// 这是 0.6.3 的行为修正 —— 旧实现基集起算是 `FeatureSet::all()`，于是文档里出现
/// **任何**一行 `@feature` 都会让全部 16 个特性生效，与「enable X = 只开 X」的直觉
/// 相反。修正后 `@feature` 才是真正的"开关"。
#[test]
fn feature_enable_only_enables_what_is_listed() {
    let src = format!("@feature enable typed-block\n{CONTRACT}rule ssh {{ action: bogus }}\n");
    // 未声明 case-insensitive ⇒ `rule` 不命中 `Rule` ⇒ 契约不生效 ⇒ 静默通过
    assert!(parse(&src).is_ok(), "实际：{:?}", parse(&src).unwrap_err());
}

/// 旧行为（基集全开）仍可通过显式 `@feature mode blacklist` 取得 —— 向后可得的逃生舱。
#[test]
fn blacklist_mode_restores_all_on_base() {
    let src = format!(
        "@feature mode blacklist\n@feature enable typed-block\n{CONTRACT}rule ssh {{ action: bogus }}\n"
    );
    let err = parse(&src).unwrap_err().to_string();
    assert!(err.contains("E-CONTRACT-006"), "实际报错：{err}");
}
