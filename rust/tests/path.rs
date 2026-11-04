//! Path 语法（S1a）门面测试：`sml::{Path, PathError, Seg}` 可从门面直接取用，
//! 且「读」在真实文档上跑通 —— 越界 / 缺键 / 类型不符**各报各的**，不再一律 `None`。
//!
//! 与 `value_accessors.rs` 的分工：那边钉**旧** `Value::get`（含「字面键优先」的兼容钉子），
//! 这边钉**新**严格路径（S6 写落点要复用的那一份）。
use sml::{parse, Path, PathError, Seg, Value};

fn doc() -> Value {
    parse(
        r#"
service: {
    name: "api"
    tls: { cert: "/etc/c.pem" }
    node: [ "a", "b" ]
}
"#,
    )
    .expect("示例文档应能解析")
}

#[test]
fn facade_reexports_path_types() {
    let p: Path = "service.node[1]".parse().expect("FromStr");
    assert_eq!(
        p.segments(),
        &[
            Seg::Key("service".into()),
            Seg::Key("node".into()),
            Seg::Index(1)
        ]
    );
    assert_eq!(doc().get_path(&p).map(|v| v.as_str()), Ok(Some("b")));
}

#[test]
fn out_of_range_is_an_error_not_none() {
    let d = doc();
    let p = Path::parse("service.node[9]").expect("语法合法");

    // 旧入口：只有 None，问不出原因
    assert!(d.get("service.node[9]").is_none());

    // 新入口：段号 + 越界数值 + 实际长度
    let err = d.get_path(&p).unwrap_err();
    assert_eq!(
        err,
        PathError::OutOfRange {
            at: 3,
            index: 9,
            len: 2
        }
    );
    assert!(err.to_string().contains("越界"), "{err}");
    assert!(!err.is_syntax(), "这是查找期错误，不是语法错误");
}

#[test]
fn missing_key_and_type_mismatch_are_distinguishable() {
    let d = doc();

    let missing = d.get_path(&Path::parse("service.nope").unwrap());
    assert_eq!(
        missing,
        Err(PathError::NotFound {
            at: 2,
            key: "nope".into()
        })
    );

    // `service.name` 是字符串，再往下取键 ⇒ 报「不是块」，而不是「键不存在」
    let mismatch = d.get_path(&Path::parse("service.name.x").unwrap());
    assert_eq!(
        mismatch,
        Err(PathError::NotAnObject {
            at: 3,
            actual: "字符串"
        })
    );
}

#[test]
fn syntax_errors_are_reported_before_lookup() {
    for (bad, at) in [("", 0), ("service..name", 9), ("service.node[x]", 14)] {
        let err = Path::parse(bad).unwrap_err();
        assert!(err.is_syntax(), "{bad} 应是语法错误：{err}");
        if at > 0 {
            assert!(
                err.to_string().contains(&format!("第 {at} 个字符")),
                "{bad} 的报错应定位到第 {at} 个字符：{err}"
            );
        }
    }
}

#[test]
fn writes_will_reuse_the_same_segments() {
    // S6 的写落点会按段遍历建父对象 —— 这里先钉住「段序列足够表达落点」这一前提：
    // 最后一段是键 ⇒ 落点是这个键；最后一段是下标 ⇒ 落点是数组元素。
    let p = Path::parse("service.tls.cert").unwrap();
    assert_eq!(p.len(), 3);
    assert_eq!(p.last(), Some(&Seg::Key("cert".into())));

    let q = Path::parse("service.node[0]").unwrap();
    assert_eq!(q.last(), Some(&Seg::Index(0)));

    // 手工拼的路径与解析出来的等价（`Path::new` 的用途：落点由程序拼装，不必过字符串）
    let manual = Path::new(vec![Seg::Key("service".into()), Seg::Key("tls".into())]);
    assert_eq!(manual, Path::parse("service.tls").unwrap());
    assert_eq!(manual.to_string(), "service.tls");
}
