// SPDX-License-Identifier: MulanPSL-2.0
//! 契约表回传（`ParseOptions::capture_contracts`）：内置契约表要能用 SML 写、
//! 再被宿主读回来注入（`docs/swsml/spec/weakbind.md` §6.1）。
//!
//! 钉两件事：
//! 1. **默认不开** ⇒ `contracts` 恒 `None`（与 `contract_map` 同一取舍，向后兼容的钉子）；
//! 2. 开启后拿到的是**已经 resolve 过的**契约表，字段规格原样可读。

use sml_parse::{parse_with, ParseOptions};

#[test]
fn off_by_default_no_contract_table() {
    let out = parse_with("@contract Tls { host: str }\n", ParseOptions::new()).unwrap();
    assert!(
        out.contracts.is_none(),
        "不开启时 contracts 必须是 None —— 这是「逐字一致」的一部分"
    );
}

#[test]
fn capture_returns_the_contract_table() {
    let src = "@contract Tls {\n  host: str required\n  port: int\n}\n\
               web { host: h port: 443 }\n";
    let out = parse_with(src, ParseOptions::new().capture_contracts(true)).unwrap();

    let cs = out.contracts.as_ref().expect("开启后应拿到契约表");
    let tls = cs.get("Tls").expect("应收集到 Tls");

    assert_eq!(tls.name, "Tls");
    assert_eq!(tls.fields.len(), 2, "两个字段都应收集到");
    assert!(tls.fields["host"].required, "`required` 修饰符要生效");
    assert!(tls.fields["port"].required, "未写 `optional` ⇒ 默认必填（FieldSpec 默认档）");
}

/// 默认必填是 `true`（`FieldSpec::required` 的默认档）：契约里不写 `optional` 就是必填。
#[test]
fn fields_default_to_required() {
    let out = parse_with(
        "@contract Cfg { a: str }\n",
        ParseOptions::new().capture_contracts(true),
    )
    .unwrap();
    let cfg = out.contracts.as_ref().unwrap().get("Cfg").unwrap();
    assert!(cfg.fields["a"].required, "未写 optional ⇒ 默认必填");
}
