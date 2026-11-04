//! featag **S8**：`!any` 契约级开关（§10 Q7）。
//!
//! `any` 等于放弃类型校验 ⇒ **默认禁用**；契约里写 `!any` 才放行（含 `any` 里的 `null`）。
//! `!any` **不占特性位**：它是契约级开关，不是文档级特性。
use sml::{parse, Value};

const HEAD: &str = "@feature enable type.featag,typed-block,block-name\n";

fn err_of(body: &str) -> String {
    format!("{}", parse(&format!("{HEAD}{body}")).unwrap_err())
}

#[test]
fn any_is_rejected_without_the_switch() {
    let src = "@contract api { x: any }\napi a { x: 1 }\n";
    let err = err_of(src);
    // 因与「能力未放行」相同 ⇒ 复用 E-FEATURE-001
    assert!(err.contains("E-FEATURE-001"), "应报能力未放行：{err}");
    assert!(err.contains("!any"), "文案应告诉用户怎么放开：{err}");
    assert!(err.contains("api"), "文案应点名契约名：{err}");
}

#[test]
fn any_is_allowed_with_the_switch() {
    let src = "@contract api { !any x: any }\napi a { x: 1 }\n";
    let v = parse(&format!("{HEAD}{src}")).expect("写了 !any 应放行");
    assert_eq!(v.get("api.x").and_then(|x| x.as_i64()), Some(1));
}

#[test]
fn null_passes_under_any() {
    // `any` 里 null 放行（§10 Q7）
    let src = "@contract api { !any x: any }\napi a { x: null }\n";
    let v = parse(&format!("{HEAD}{src}")).expect("null 应放行");
    assert_eq!(v.get("api.x"), Some(&Value::Null));
}

#[test]
fn array_of_any_is_checked_too() {
    let src = "@contract api { xs: [any] }\napi a { xs: [1, \"two\"] }\n";
    let err = err_of(src);
    assert!(err.contains("E-FEATURE-001"), "`[any]` 也要拦：{err}");

    let ok = "@contract api { !any xs: [any] }\napi a { xs: [1, \"two\"] }\n";
    assert!(parse(&format!("{HEAD}{ok}")).is_ok(), "开了 !any 应放行");
}

#[test]
fn apply_switch_also_covers_the_is_path() {
    // `@is` 与块头标注是同一件事的两种写法，判据必须一致
    let src = "@contract api { x: any }\nblock { @is api\n x: 1 }\n";
    let err = err_of(src);
    assert!(err.contains("E-FEATURE-001"), "`@is` 路径也要拦：{err}");

    let ok = "@contract api { !any x: any }\nblock { @is api\n x: 1 }\n";
    assert!(parse(&format!("{HEAD}{ok}")).is_ok(), "@is 路径开了也应放行");
}

#[test]
fn nested_contract_any_is_checked() {
    let src = "@contract Inner { y: any }\n@contract api { inner: Inner }\napi a { inner: { y: 1 } }\n";
    let err = err_of(src);
    assert!(err.contains("E-FEATURE-001"), "嵌套契约也要查：{err}");
    assert!(err.contains("Inner"), "文案应点名是哪个契约：{err}");
}

#[test]
fn gated_off_means_any_still_works() {
    // 没开 `type.featag` ⇒ 不做这项检查（v4 文档的 `any` 照旧可用）
    let src = "@feature enable typed-block,block-name\n@contract api { x: any }\napi a { x: 1 }\n";
    assert!(
        parse(src).is_ok(),
        "未开 type.featag 时 `any` 应照旧可用：{:?}",
        parse(src).err()
    );
}

#[test]
fn recursive_contracts_do_not_loop() {
    // A 引用 B、B 引用 A：`seen` 去重，不能无限下钻。
    // 字段写 `optional`，否则「递归契约需要无限嵌套的数据」会先报必填缺失（那是另一回事）。
    let src = "@contract A { b: B optional }\n@contract B { a: A optional }\nA x { }\n";
    assert!(
        parse(&format!("{HEAD}{src}")).is_ok(),
        "递归契约不应卡死：{:?}",
        parse(&format!("{HEAD}{src}")).err()
    );
}
