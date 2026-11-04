//! featag **S7**：`from-featag` 来源规则（§6.2）。
//!
//! 两档来源：默认（谁都能写）/ `from-featag`（**只能 featag 落点写**）。
//! 文档里直接给该键赋值 ⇒ `E-CONTRACT-024`。
use sml::parse;

const HEAD: &str = "@feature enable type.featag,typed-block,block-name\n";

fn err_of(body: &str) -> String {
    format!("{}", parse(&format!("{HEAD}{body}")).unwrap_err())
}

#[test]
fn document_assignment_to_a_from_featag_field_is_rejected() {
    let src = "\
@contract Tls { cert: str from-featag }
@contract api loose { tls: Tls }
api x { tls: { cert: \"fake.pem\" } }
";
    let err = err_of(src);
    assert!(err.contains("E-CONTRACT-024"), "文档手写应被拒：{err}");
    assert!(err.contains("tls.cert"), "错误应定位到字段全路径：{err}");
    assert!(err.contains("from-featag"), "文案应点明来源修饰符：{err}");
}

#[test]
fn featag_writing_the_same_field_is_fine() {
    // featag 落点写 ⇒ 唯一合法来源
    let src = "\
@contract Tls { cert: str from-featag }
@contract api loose { tls: Tls !cert: str -> tls.cert }
api cert[\"c.pem\"], x { }
";
    let v = parse(&format!("{HEAD}{src}")).expect("featag 写入应通过");
    assert_eq!(
        v.get("api.tls.cert").and_then(|x| x.as_str()),
        Some("c.pem")
    );
}

#[test]
fn optional_from_featag_field_may_be_absent() {
    // 没有 featag 提供、文档也不写 ⇒ 不报来源错（`optional` 的意义）
    let src = "\
@contract Tls { cert: str from-featag optional }
@contract api loose { tls: Tls }
api x { tls: { } }
";
    assert!(
        parse(&format!("{HEAD}{src}")).is_ok(),
        "缺失的 from-featag 可选字段不该报错：{:?}",
        parse(&format!("{HEAD}{src}")).err()
    );
}

#[test]
fn nested_contracts_are_checked_too() {
    // 两层嵌套：`inner.secret` 声明 from-featag，文档写 `inner.secret` ⇒ 报错
    let src = "\
@contract Inner { secret: str from-featag }
@contract Middle { inner: Inner }
@contract api loose { mid: Middle }
api x { mid: { inner: { secret: \"s\" } } }
";
    let err = err_of(src);
    assert!(err.contains("E-CONTRACT-024"), "嵌套字段也要查：{err}");
    assert!(err.contains("mid.inner.secret"), "路径应带层级：{err}");
}

#[test]
fn gated_off_means_document_may_write_it() {
    // 没开 featag ⇒ `from-featag` 只是普通未知修饰符？不：它仍被解析，但不做来源检查。
    // 关掉 `type.featag` 时，检查整段不跑 ⇒ 文档手写照常（v4 行为不变）。
    let src = "\
@feature enable typed-block,block-name
@contract Tls { cert: str from-featag }
@contract api loose { tls: Tls }
api x { tls: { cert: \"fake.pem\" } }
";
    assert!(
        parse(src).is_ok(),
        "未开 type.featag 时不应做来源检查：{:?}",
        parse(src).err()
    );
}
