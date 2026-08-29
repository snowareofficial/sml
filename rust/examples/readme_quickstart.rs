// SPDX-License-Identifier: MulanPSL-2.0
//! 验证 README「快速开始」与「契约」两节的示例确实可运行。
//!
//! README 里的代码若无法运行就是误导，故固化为可执行的 example。

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // —— Quick start ——
    let v = sml::parse("name: John\nage: 27")?;
    assert_eq!(v.get("name").and_then(|x| x.as_str()), Some("John"));
    assert_eq!(v.get("age"), Some(&sml::Value::Int(27)));
    println!("to_sml: {}", sml::to_sml(&v));
    println!("[ok] quick start");

    // —— Syntax at a glance ——
    let text = r#"
@version v1
firstName: John
age: 27
address {
    streetAddress: "21 2nd Street"
    state: NY
}
phoneNumbers: [ { type: home } { type: office } ]
@base { region: cn-north-1 }
region: &base
contact {
    to: a@b.c
    from: "sal <sal@mail.swebase.cn>"
}
"#;
    let v = sml::parse(text)?;
    assert_eq!(v.get("address.state").unwrap().as_str(), Some("NY"));
    assert_eq!(
        v.get("region.region").unwrap().as_str(),
        Some("cn-north-1"),
        "fragment referenced as a value should expand"
    );
    assert_eq!(v.get("contact.to").unwrap().as_str(), Some("a@b.c"));
    println!("[ok] syntax at a glance");

    // —— Top-level array ——
    let v = sml::parse("[ { ts: \"2026-01-01\" to: \"a@b.c\" } ]")?;
    assert!(matches!(v, sml::Value::Array(ref a) if a.len() == 1));
    println!("[ok] top-level array");

    // —— Contract: defaults, enum, composition, strict ——
    let text = r#"
@contract Address {
    city: str
    country: str default CN
}
@contract Server {
    host: str
    port: int default 5432
    tls: bool default false
    tags: [str] optional
    status: enum [ active standby retired ]
    weight: num min 0 max 100
    address: Address
}
db {
    @is Server
    host: db1.internal
    status: active
    weight: 80
    address { city: Beijing }
}
"#;
    let v = sml::parse(text)?;
    assert_eq!(v.get("db.host").unwrap().as_str(), Some("db1.internal"));
    assert_eq!(v.get("db.port"), Some(&sml::Value::Int(5432)), "default filled");
    assert_eq!(v.get("db.tls"), Some(&sml::Value::Bool(false)), "default filled");
    assert_eq!(v.get("db.status").unwrap().as_str(), Some("active"));
    // 组合：子块递归校验并回填默认值
    assert_eq!(v.get("db.address.city").unwrap().as_str(), Some("Beijing"));
    assert_eq!(
        v.get("db.address.country").unwrap().as_str(),
        Some("CN"),
        "nested default filled by referenced contract"
    );
    println!("[ok] contract (defaults / enum / composition)");

    // 严格模式：未声明字段被拒绝
    let bad = "@contract C { a: str }\nx { @is C\n  a: 1\n  b: 2 }";
    assert!(sml::parse(bad).is_err(), "strict mode must reject undeclared field");
    // loose 允许
    let loose = "@contract C loose { a: str }\nx { @is C\n  a: v\n  b: 2 }";
    assert!(sml::parse(loose).is_ok(), "loose must allow undeclared field");
    println!("[ok] strict mode / loose");

    // —— Version declaration ——
    let (_v, ver) = sml::parse_versioned("@version v1\nname: John")?;
    assert_eq!(ver, sml::Version::V1);
    println!("[ok] version declaration");

    println!("\nALL README EXAMPLES PASSED");
    Ok(())
}
