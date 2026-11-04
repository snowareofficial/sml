//! featag **S6 的数组落点**（§10 Q20，2026-11-04 用户定）：
//!
//! - `n < len` 且元素为 `null` ⇒ **补缺**（填入）；
//! - `n < len` 且元素非 null ⇒ **禁止覆盖**（`E-CONTRACT-022`）；
//! - `n == len` ⇒ **追加**；`!strict-paths` 下 ⇒ `E-CONTRACT-023`；
//! - `n > len` ⇒ `E-CONTRACT-025`（**绝不补 null**）；
//! - 数组本身不存在 ⇒ `E-CONTRACT-023`（数组不凭空创建）。
use sml::{parse, Value};

const HEAD: &str = "@feature enable type.featag,typed-block,block-name\n";

fn err_of(body: &str) -> String {
    format!("{}", parse(&format!("{HEAD}{body}")).unwrap_err())
}

#[test]
fn null_slot_is_filled() {
    let src = "\
@contract api loose { !name: str -> rules[0].name }
api name[\"r0\"], x { rules: [ null ] }
";
    let v = parse(&format!("{HEAD}{src}")).expect("null 空位应可填");
    assert_eq!(
        v.get("api.rules[0].name").and_then(|x| x.as_str()),
        Some("r0")
    );
}

#[test]
fn non_null_element_is_not_overwritten() {
    let src = "\
@contract api loose { !name: str -> rules[0].name }
api name[\"r0\"], x { rules: [ { name: \"keep\" } ] }
";
    let err = err_of(src);
    assert!(err.contains("E-CONTRACT-022"), "非 null 元素应禁止覆盖：{err}");
}

#[test]
fn append_at_len_is_allowed() {
    // `n == len` ⇒ 追加（这里带上后续段 ⇒ 先把空对象追加进去再写键）
    let src = "\
@contract api loose { !name: str -> rules[2].name }
api name[\"r2\"], x { rules: [ { name: \"a\" }, { name: \"b\" } ] }
";
    let v = parse(&format!("{HEAD}{src}")).expect("应能追加");
    let rules = v.get("api.rules").and_then(|x| x.as_array()).expect("有数组");
    assert_eq!(rules.len(), 3, "应追加到 3 个元素");
    assert_eq!(
        rules[2].get("name").and_then(|x| x.as_str()),
        Some("r2"),
        "追加出的元素里应写入值"
    );
}

#[test]
fn append_is_blocked_by_strict_paths() {
    let src = "\
@contract api loose { !strict-paths !name: str -> rules[2].name }
api name[\"r2\"], x { rules: [ { name: \"a\" }, { name: \"b\" } ] }
";
    let err = err_of(src);
    assert!(err.contains("E-CONTRACT-023"), "strict-paths 下不许追加：{err}");
    assert!(err.contains("追加"), "文案应点明是追加：{err}");
}

#[test]
fn index_beyond_len_is_out_of_range() {
    let src = "\
@contract api loose { !name: str -> rules[5].name }
api name[\"r5\"], x { rules: [ ] }
";
    let err = err_of(src);
    assert!(err.contains("E-CONTRACT-025"), "越界应报 025：{err}");
    assert!(err.contains("越界"), "文案应说清越界：{err}");
    assert!(err.contains("不补 null"), "文案应点明不补 null：{err}");
}

#[test]
fn missing_array_is_not_created() {
    let src = "\
@contract api loose { !name: str -> rules[0].name }
api name[\"r0\"], x { }
";
    let err = err_of(src);
    assert!(err.contains("E-CONTRACT-023"), "数组不存在应报 023：{err}");
    assert!(
        err.contains("数组不会凭空创建"),
        "文案应解释为什么不建：{err}"
    );
}

#[test]
fn nested_index_and_key_mix() {
    // `cells[0][1]`：数组套数组
    let src = "\
@contract api loose { !v: int -> cells[0][1] }
api v[7], x { cells: [ [ 0, null ] ] }
";
    let v = parse(&format!("{HEAD}{src}")).expect("嵌套下标应可写");
    assert_eq!(
        v.get("api.cells[0][1]").and_then(|x| x.as_i64()),
        Some(7)
    );
}

#[test]
fn index_on_non_array_is_rejected() {
    let src = "\
@contract api loose { !name: str -> rules[0] }
api name[\"r0\"], x { rules: 5 }
";
    let err = err_of(src);
    assert!(err.contains("E-CONTRACT-023"), "下标作用在标量上应报错：{err}");
    assert!(err.contains("不是数组"), "文案应说清类型：{err}");
}

#[test]
fn declaration_accepts_index_targets() {
    // S4 曾经拒绝 `-> rules[0]`，Q20 之后必须收
    let src = "@contract api loose { !name: str -> rules[0].name }\napi name[\"a\"], x { rules: [ null ] }\n";
    assert!(parse(&format!("{HEAD}{src}")).is_ok());
    let _ = Value::Null; // 保持 `Value` 在本文件的引用不被裁剪（其余用例都用 get）
}
