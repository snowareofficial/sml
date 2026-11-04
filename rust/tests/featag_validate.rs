//! featag **S5**：校验开闸 —— 未声明即拒 + 参数个数/类型与声明相符。
//!
//! 门控：`type.featag`（语法）+ `typed-block`/`block-name`（让块头首词命中契约）。
//! 库侧放宽：`ParseOptions::apply_contracts(false)` —— 与契约校验**同一个开关**。
use sml::{parse, parse_with, ParseOptions};

/// 一个声明齐全的文档头：`!critical` 无参、`!obey: [int]` 数组、`!cert: str` 单值。
const DECL: &str = "\
@feature enable type.featag,typed-block,block-name
@contract wea { a: int optional !critical !obey: [int] !cert: str }
";

fn err_of(body: &str) -> String {
    format!("{}", parse(&format!("{DECL}{body}")).unwrap_err())
}

// ---------------------------------------------------------------------------
// 未声明即拒
// ---------------------------------------------------------------------------

#[test]
fn undeclared_featag_is_rejected_with_a_hint() {
    let err = err_of("wea !nope, x { }\n");
    assert!(err.contains("E-CONTRACT-021"), "应报未声明：{err}");
    assert!(err.contains("wea.nope"), "诊断须写全 `契约.featag`：{err}");
    assert!(
        err.contains("!critical") && err.contains("!obey"),
        "应列出该契约可用的 featag：{err}"
    );
}

#[test]
fn featag_without_any_contract_is_rejected() {
    // 块头首词不是契约 ⇒ 没有任何声明表 ⇒ featag 无主
    let err = err_of("server !critical, api { }\n");
    assert!(err.contains("E-CONTRACT-021"), "应报未声明：{err}");
    assert!(
        err.contains("没有应用任何契约"),
        "文案应点明「featag 作用域契约私有」：{err}"
    );
}

// ---------------------------------------------------------------------------
// 用法与声明不符
// ---------------------------------------------------------------------------

#[test]
fn args_on_a_tag_only_featag_are_rejected() {
    let err = err_of("wea critical[1], x { }\n");
    assert!(err.contains("E-CONTRACT-021"), "无参声明带参应报错：{err}");
    assert!(err.contains("无参"), "文案应说清是「声明为无参」：{err}");
}

#[test]
fn missing_args_on_a_typed_featag_are_rejected() {
    let err = err_of("wea !obey, x { }\n");
    assert!(err.contains("E-CONTRACT-021"), "带参声明缺参应报错：{err}");
    assert!(err.contains("obey[…]"), "文案应给出正确写法：{err}");
}

#[test]
fn non_array_declaration_takes_exactly_one_arg() {
    let err = err_of("wea cert[a, b], x { }\n");
    assert!(err.contains("E-CONTRACT-021"), "单值声明给两个参数应报错：{err}");
    assert!(err.contains("1 个参数"), "文案应说清个数限制：{err}");
}

// ---------------------------------------------------------------------------
// 参数类型校验（复用字段类型链条 ⇒ 报的是既有类型错误码）
// ---------------------------------------------------------------------------

#[test]
fn arg_type_is_checked() {
    // `[int]` 声明 ↔ `[1, "x"]`：第 2 个元素类型不符
    let err = err_of("wea obey[1, \"x\"], name { }\n");
    assert!(
        err.starts_with("sml:") || err.contains("E-CONTRACT-"),
        "应走类型错误码：{err}"
    );
    assert!(err.contains("wea.obey"), "错误应定位到 `契约.featag`：{err}");

    // `str` 声明 ↔ 数字
    let err = err_of("wea cert[1], name { }\n");
    assert!(err.contains("E-CONTRACT-"), "单值类型不符也应报类型错误：{err}");
}

#[test]
fn well_formed_usage_passes() {
    for body in [
        "wea !critical, x { }\n",
        "wea obey[1, 2], x { }\n",
        "wea cert[\"/etc/c.pem\"], x { }\n",
        "wea !critical obey[] cert[\"k\"], { }\n",
    ] {
        assert!(
            parse(&format!("{DECL}{body}")).is_ok(),
            "应通过：{body} → {:?}",
            parse(&format!("{DECL}{body}")).err()
        );
    }
}

// ---------------------------------------------------------------------------
// 库侧放宽：与契约校验同一个开关
// ---------------------------------------------------------------------------

#[test]
fn apply_contracts_off_disables_featag_validation() {
    let src = format!("{DECL}wea !nope, x {{ }}\n");
    // 默认：拒
    assert!(parse(&src).is_err());
    // 关掉契约应用 ⇒ featag 校验一并不做（否则「关契约」形同虚设）
    let out = parse_with(&src, ParseOptions::new().apply_contracts(false))
        .expect("放宽后应能解析");
    assert!(out.errors.is_empty(), "放宽后不应报错：{:?}", out.errors);
    // 数据照样落盘：`__featags` 是语法层的产物，与校验收不开无关
    assert!(out.value.get("wea").is_some(), "值树应仍产出：{:?}", out.value);
}
