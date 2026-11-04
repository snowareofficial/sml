//! featag **S2 + S3**：块头 `!` 区的语法识别，以及落成 `__featags`（保序数组）。
//!
//! 片界：S2 = 认区 + B1/B2/B3 三条歧义；S3 = 落数据（**不做声明校验**，那是 S4/S5）。
//! 门控：一切都在 `@feature enable type.featag` 后面（默认关 ⇒ 关掉即回到 v4 行为）。
use sml::{parse, Value};

/// 开 featag 的文档头。`10`（= 1.0）保留 `bareword-string`，方便对照 B2 的「不豁免」。
const ON: &str = "@feature enable type.featag\n";

fn block<'a>(v: &'a Value, key: &str) -> &'a Value {
    v.get(key).unwrap_or_else(|| panic!("缺少块 `{key}`：{v:?}"))
}

fn featags<'a>(v: &'a Value, key: &str) -> &'a Vec<Value> {
    block(v, key)
        .get("__featags")
        .and_then(|x| x.as_array())
        .unwrap_or_else(|| panic!("块 `{key}` 应有 `__featags`：{:?}", block(v, key)))
}

// ---------------------------------------------------------------------------
// 正例：区被识别，并落成保序数组
// ---------------------------------------------------------------------------

#[test]
fn region_becomes_ordered_featags_array() {
    let v = parse(&format!(
        "{ON}wea !critical obey[1, 2], name {{ a: 1 }}\n"
    ))
    .expect("应能解析");
    let b = block(&v, "wea");
    // 块头其余部分照旧
    assert_eq!(b.get("__type").and_then(|x| x.as_str()), Some("wea"));
    assert_eq!(b.get("__name").and_then(|x| x.as_str()), Some("name"));
    assert_eq!(b.get("a").and_then(|x| x.as_i64()), Some(1));

    let f = featags(&v, "wea");
    assert_eq!(f.len(), 2, "两个 featag → 两项");
    assert_eq!(f[0].get("name").and_then(|x| x.as_str()), Some("critical"));
    assert!(f[0].get("args").is_none(), "无参项不带 `args` 键");
    assert_eq!(f[1].get("name").and_then(|x| x.as_str()), Some("obey"));
    let args = f[1]
        .get("args")
        .and_then(|x| x.as_array())
        .expect("带参项有 args");
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].as_i64(), Some(1));
    assert_eq!(args[1].as_i64(), Some(2));
}

#[test]
fn duplicates_and_order_are_preserved() {
    // 用**数组**而不是映射的理由就在这里：同名可重复、顺序不丢。
    let v = parse(&format!("{ON}wea !a !a !b, x {{ }}\n")).expect("应能解析");
    let names: Vec<&str> = featags(&v, "wea")
        .iter()
        .map(|f| f.get("name").and_then(|x| x.as_str()).unwrap_or("<无>"))
        .collect();
    assert_eq!(names, vec!["a", "a", "b"], "顺序与重复都要保留");
}

#[test]
fn block_name_may_be_omitted() {
    let v = parse(&format!("{ON}wea !critical, {{ a: 1 }}\n")).expect("应能解析");
    let b = block(&v, "wea");
    assert!(b.get("__name").is_none(), "没写块名就不该有 __name");
    assert_eq!(b.get("a").and_then(|x| x.as_i64()), Some(1));
    assert_eq!(featags(&v, "wea").len(), 1);
}

#[test]
fn args_may_nest_brackets_and_braces() {
    // 参数是真值，不是被跳过的 token：嵌套数组 / 块都要原样立起来
    let v = parse(&format!("{ON}wea obey[[1, 2], 3], name {{ }}\n")).expect("应能解析");
    let f = featags(&v, "wea");
    let args = f[0].get("args").and_then(|x| x.as_array()).expect("有 args");
    assert_eq!(args.len(), 2);
    assert_eq!(
        args[0].as_array().map(|a| a.len()),
        Some(2),
        "第一个参数应是嵌套数组"
    );

    let v = parse(&format!("{ON}wea obey[{{ a: 1 }}], name {{ }}\n")).expect("应能解析");
    let f = featags(&v, "wea");
    let args = f[0].get("args").and_then(|x| x.as_array()).expect("有 args");
    assert_eq!(
        args[0].get("a").and_then(|x| x.as_i64()),
        Some(1),
        "参数里的块应成对象"
    );
}

#[test]
fn explicit_empty_args_are_kept() {
    // `obey[]` 与「没有参数槽」是两回事：前者显式写了空数组
    let v = parse(&format!("{ON}wea obey[], name {{ }}\n")).expect("应能解析");
    let f = featags(&v, "wea");
    assert_eq!(
        f[0].get("args").and_then(|x| x.as_array()).map(|a| a.len()),
        Some(0),
        "显式 `[]` 应保留空数组"
    );
}

// ---------------------------------------------------------------------------
// B1：空 featag 区
// ---------------------------------------------------------------------------

#[test]
fn b1_empty_region_errors() {
    for src in ["wea ! , name { }", "wea !, name { }", "wea ! { }", "wea !!x, name { }"] {
        let err = format!("{}", parse(&format!("{ON}{src}\n")).unwrap_err());
        assert!(err.contains("E-PARSE-026"), "`{src}` 应报 B1：{err}");
    }
}

// ---------------------------------------------------------------------------
// B2：值位置的 `!` 裸词（开了 bareword-string 也不豁免）
// ---------------------------------------------------------------------------

#[test]
fn b2_bang_bareword_in_value_position_errors() {
    // `10`（1.0）= 裸词即字符串，这条正是「不豁免」的证据：换成 `20` 也一样报
    for head in ["@feature enable type.featag\n", "@version 20\n@feature enable type.featag\n"] {
        let err = format!("{}", parse(&format!("{head}a: !b\n")).unwrap_err());
        assert!(err.contains("E-PARSE-027"), "应报 B2：{err}");
    }
}

#[test]
fn b2_is_gated_by_the_feature() {
    // 没开 featag ⇒ 一字不改：裸词仍是字符串
    let v = parse("a: !b\n").expect("未开特性时不报错");
    assert_eq!(v.get("a").and_then(|x| x.as_str()), Some("!b"));
    // 引号写法始终可用
    let v = parse(&format!("{ON}a: \"!b\"\n")).expect("加引号应通过");
    assert_eq!(v.get("a").and_then(|x| x.as_str()), Some("!b"));
}

// ---------------------------------------------------------------------------
// B3：块头里以 `!` 开头的名字
// ---------------------------------------------------------------------------

#[test]
fn b3_bang_in_block_name_position_errors() {
    // 区已结束（`,` 之后是块名位置）⇒ 块名以 `!` 开头
    for src in ["wea !critical, !web { }", "wea !critical !x, !web { }"] {
        let err = format!("{}", parse(&format!("{ON}{src}\n")).unwrap_err());
        assert!(err.contains("E-PARSE-028"), "`{src}` 应报 B3：{err}");
    }
    // 首词（类型词）位置同理
    let err = format!("{}", parse(&format!("{ON}!wea name {{ }}\n")).unwrap_err());
    assert!(err.contains("E-PARSE-028"), "类型词以 `!` 开头应报 B3：{err}");
}

#[test]
fn consecutive_bang_items_are_all_featags_not_names() {
    // 逗号**之前**的 `!x` 一律是 featag ⇒ `wea !critical !bad { }` 是「两个 featag + 无块名」，
    // 不是 B3。这条钉住「区分块名与 featag 的是逗号，不是 `!` 的有无」。
    let v = parse(&format!("{ON}wea !critical !bad {{ a: 1 }}\n")).expect("应能解析");
    assert!(block(&v, "wea").get("__name").is_none(), "逗号前没有块名");
    assert_eq!(featags(&v, "wea").len(), 2, "两个 `!x` 都是 featag");
}

// ---------------------------------------------------------------------------
// 与契约共存：`__featags` 是语法元数据，不该被 strict 契约当成未声明字段
// ---------------------------------------------------------------------------

#[test]
fn featags_survive_a_strict_contract() {
    // `block-name` 也要开：typed-block 模式下「契约词之后的首个非契约词」默认被当错误
    // （那是 D2 的拆雷，与 featag 无关），`name` 作为块名必须显式放行。
    let src = "\
@feature enable type.featag,typed-block,block-name
@contract wea { a: int }
wea !critical, name { a: 1 }
";
    let v = parse(src).expect("strict 契约不应因 __featags 报未声明字段");
    assert_eq!(featags(&v, "wea").len(), 1);
    assert_eq!(
        block(&v, "wea").get("a").and_then(|x| x.as_i64()),
        Some(1)
    );
}

// ---------------------------------------------------------------------------
// 回归红线：关掉特性 ⇒ 一字不改
// ---------------------------------------------------------------------------

#[test]
fn without_the_feature_nothing_changes() {
    // 裸块元数据的老语义（回归红线）：`server web prod { }`
    let v = parse("server web prod { }\n").expect("应能解析");
    let b = block(&v, "server");
    assert_eq!(b.get("__name").and_then(|x| x.as_str()), Some("web"));
    assert_eq!(
        b.get("__args").and_then(|x| x.as_array()).map(|a| a.len()),
        Some(1)
    );
    assert!(b.get("__featags").is_none(), "未开特性不该有 __featags");
    // 没开 featag 时 `wea !critical, name { }` 仍是**旧错误**（块头里不许有逗号），
    // 不是新的 B1 —— 新码只在特性开启后出现。
    let err = format!("{}", parse("wea !critical, name { }\n").unwrap_err());
    assert!(!err.contains("E-PARSE-026"), "未开特性不该报新码：{err}");
}

#[test]
fn unbalanced_args_are_reported() {
    // 配不平 ⇒ 预扫描判定「不是块头」⇒ 回落旧路径，由既有 PARSE 码报错。
    // 这里只钉「一定报错、且带码」，不钉具体是哪个码 —— 那是旧路径的既有行为，不属本片。
    let err = format!("{}", parse(&format!("{ON}wea obey[1, name {{ }}\n")).unwrap_err());
    assert!(err.contains("E-PARSE-"), "括号不配平应报码：{err}");
}
