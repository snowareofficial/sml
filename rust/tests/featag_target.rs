//! featag **S6**：落点填充（§6.1 规则 1–5）。
//!
//! - 落点已存在 ⇒ 报错（**禁止覆盖**）；
//! - 父对象默认自动创建；`!strict-paths` 打开后必须已存在；
//! - `auto` ⇒ 取声明里的 `default`；
//! - 无落点 ⇒ 纯标签，只留在 `__featags`。
//!
//! 契约一律写 `loose`：这些用例只想验「落点怎么写」，不想被「未声明字段」干扰。
use sml::{parse, Value};

const HEAD: &str = "@feature enable type.featag,typed-block,block-name\n";

fn get<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    v.get(path)
}

#[test]
fn targets_are_filled_and_parents_created() {
    let src = format!(
        "{HEAD}@contract api loose {{\n  name: str\n  !critical\n  !cert: str -> tls.cert\n  !key: str -> tls.key\n}}\n\
         api !critical cert[\"c.pem\"] key[\"k.pem\"], web {{ name: api }}\n"
    );
    let v = parse(&src).expect("应能解析");
    let b = v.get("api").expect("有 api 块");
    // `tls` 原本不存在 ⇒ 按路径建出来
    assert_eq!(get(b, "tls.cert").and_then(|x| x.as_str()), Some("c.pem"));
    assert_eq!(get(b, "tls.key").and_then(|x| x.as_str()), Some("k.pem"));
    // 原有字段不动
    assert_eq!(get(b, "name").and_then(|x| x.as_str()), Some("api"));
    // 纯标签只进 __featags
    assert_eq!(
        get(b, "__featags").and_then(|x| x.as_array()).map(|a| a.len()),
        Some(3)
    );
}

#[test]
fn tag_without_target_writes_no_key() {
    let src = format!(
        "{HEAD}@contract api loose {{ !critical }}\napi !critical, x {{ a: 1 }}\n"
    );
    let v = parse(&src).expect("应能解析");
    let b = v.get("api").expect("有 api 块");
    assert!(get(b, "critical").is_none(), "纯标签不该写键：{b:?}");
    assert_eq!(get(b, "a").and_then(|x| x.as_i64()), Some(1));
}

#[test]
fn existing_target_is_rejected_not_overwritten() {
    let src = format!(
        "{HEAD}@contract api loose {{ !cert: str -> tls.cert }}\n\
         api cert[\"new.pem\"], x {{ tls: {{ cert: \"old.pem\" }} }}\n"
    );
    let err = format!("{}", parse(&src).unwrap_err());
    assert!(err.contains("E-CONTRACT-022"), "应报「落点已存在」：{err}");
    assert!(err.contains("不能覆盖"), "文案要说清禁止覆盖：{err}");
}

#[test]
fn strict_paths_requires_existing_parent() {
    // 默认：父对象自动创建
    let ok = format!(
        "{HEAD}@contract api loose {{ !cert: str -> deep.tls.cert }}\napi cert[\"c\"], x {{ }}\n"
    );
    let v = parse(&ok).expect("默认应自动创建父对象");
    assert_eq!(
        v.get("api.deep.tls.cert").and_then(|x| x.as_str()),
        Some("c")
    );

    // `!strict-paths`：父对象必须已存在
    let strict = format!(
        "{HEAD}@contract api loose {{ !strict-paths !cert: str -> deep.tls.cert }}\n\
         api cert[\"c\"], x {{ }}\n"
    );
    let err = format!("{}", parse(&strict).unwrap_err());
    assert!(err.contains("E-CONTRACT-023"), "应报父对象不存在：{err}");
    assert!(err.contains("strict-paths"), "文案要点明开关：{err}");

    // 父对象已存在时严格开关也放行
    let with_parent = format!(
        "{HEAD}@contract api loose {{ !strict-paths !cert: str -> tls.cert }}\n\
         api cert[\"c\"], x {{ tls: {{ }} }}\n"
    );
    let v = parse(&with_parent).expect("父对象存在应通过");
    assert_eq!(
        v.get("api.tls.cert").and_then(|x| x.as_str()),
        Some("c")
    );
}

#[test]
fn scalar_in_the_middle_is_rejected() {
    let src = format!(
        "{HEAD}@contract api loose {{ !cert: str -> tls.cert }}\n\
         api cert[\"c\"], x {{ tls: 5 }}\n"
    );
    let err = format!("{}", parse(&src).unwrap_err());
    assert!(err.contains("E-CONTRACT-023"), "路径中间是标量应报错：{err}");
    assert!(err.contains("没法往里写键"), "文案要说清原因：{err}");
}

#[test]
fn auto_takes_the_declared_default() {
    let src = format!(
        "{HEAD}@contract api loose {{ !cert: str default \"d.pem\" -> tls.cert }}\n\
         api cert[auto], x {{ }}\n"
    );
    let v = parse(&src).expect("应能解析");
    assert_eq!(
        v.get("api.tls.cert").and_then(|x| x.as_str()),
        Some("d.pem"),
        "auto 应取 default：{v:?}"
    );

    // 声明没有 default ⇒ 报错（同一个码：落点写不进去）
    let no_default = format!(
        "{HEAD}@contract api loose {{ !cert: str -> tls.cert }}\napi cert[auto], x {{ }}\n"
    );
    let err = format!("{}", parse(&no_default).unwrap_err());
    assert!(err.contains("E-CONTRACT-022"), "无默认值的 auto 应报错：{err}");
    assert!(err.contains("default"), "文案应点明缺 default：{err}");
}

#[test]
fn contract_name_resolves_before_metadata_is_written() {
    // 落点写的是 `__` 之外的名字，且写入发生在 `__type`/`__featags` 之前 ——
    // 这条只钉「元数据仍然完整」，防止将来调换顺序把它写坏。
    let src = format!(
        "{HEAD}@contract api loose {{ !cert: str -> tls.cert }}\napi cert[\"c\"], web {{ }}\n"
    );
    let v = parse(&src).expect("应能解析");
    let b = v.get("api").expect("有 api 块");
    assert_eq!(get(b, "__type").and_then(|x| x.as_str()), Some("api"));
    assert_eq!(get(b, "__name").and_then(|x| x.as_str()), Some("web"));
}
