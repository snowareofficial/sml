// SPDX-License-Identifier: MulanPSL-2.0
//! D53：写前脱敏是纯值变换 —— 删 / 遮盖 / 哈希，键序与其余值不变。

use amberice::{Map, RedactAction, RedactError, RedactRules, Value};

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::ordered();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}

fn doc() -> Value {
    obj(&[
        (
            "user",
            obj(&[
                ("id_card", s("110101199001011234")),
                ("phone", s("13800001111")),
                ("name", s("ada")),
            ]),
        ),
        ("session", obj(&[("token", s("t-1234567890"))])),
        (
            "list",
            Value::Array(vec![
                obj(&[("secret", s("a")), ("keep", Value::Int(1))]),
                obj(&[("secret", s("b")), ("keep", Value::Int(2))]),
            ]),
        ),
    ])
}

fn user(v: &Value) -> &Map {
    let Value::Object(top) = v else { panic!("root") };
    let Some(Value::Object(u)) = top.get("user") else {
        panic!("user")
    };
    u
}

#[test]
fn drop_mask_hash_and_array_drop() {
    let mut rules = RedactRules::new();
    rules.drop_path("user.id_card").expect("drop");
    rules.mask_path("user.phone").expect("mask");
    rules.hash_path("session.token").expect("hash");
    rules.drop_path("list[0]").expect("drop array");

    let mut v = doc();
    rules.apply(&mut v).expect("apply");

    let u = user(&v);
    assert!(!u.contains_key("id_card"), "id_card 应被删");
    assert_eq!(u.get("phone"), Some(&s("***")), "phone 应遮盖");
    assert_eq!(u.get("name"), Some(&s("ada")), "其余字段不变");

    // 删键不改变剩余键序（canonical 仍是确定的）
    let keys: Vec<&str> = u.iter().map(|(k, _)| k).collect();
    assert_eq!(keys, vec!["phone", "name"]);

    let Value::Object(top) = &v else { panic!("root") };
    let Some(Value::Object(session)) = top.get("session") else {
        panic!("session")
    };
    let Some(Value::Str(token)) = session.get("token") else {
        panic!("token")
    };
    assert_eq!(token.len(), 64, "sha256 十六进制");
    assert!(token.chars().all(|c| c.is_ascii_hexdigit()));

    let Some(Value::Array(list)) = top.get("list") else {
        panic!("list")
    };
    assert_eq!(list.len(), 1, "数组元素被删");
    let Value::Object(first) = &list[0] else {
        panic!("list[0]")
    };
    assert_eq!(first.get("secret"), Some(&s("b")));

    // 同规则两次应用必须逐字节相同（确定性）
    let mut again = doc();
    rules.apply(&mut again).expect("apply again");
    assert_eq!(
        again.canonical_bytes().expect("bytes"),
        v.canonical_bytes().expect("bytes")
    );
}

/// D53：只有「写前脱敏 + 新文件」才叫抹除 —— 新字节里不应出现被删/遮盖的原文。
#[test]
fn redacted_dump_does_not_contain_the_secrets() {
    let mut rules = RedactRules::new();
    rules.drop_path("user.id_card").expect("drop");
    rules.mask_path("user.phone").expect("mask");
    rules.hash_path("session.token").expect("hash");

    let mut v = doc();
    rules.apply(&mut v).expect("apply");
    let cv = amberice::CanonicalValue::try_from(&v).expect("ordered");
    let bytes = amberice::dump(&cv).expect("dump");

    for secret in ["110101199001011234", "13800001111", "t-1234567890"] {
        assert!(
            !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
            "原文 `{secret}` 不应出现在新文件里"
        );
    }
}

#[test]
fn hash_is_deterministic_and_value_sensitive() {
    let mut a = doc();
    let mut b = doc();
    let mut rules = RedactRules::new();
    rules.hash_path("session.token").expect("hash");
    rules.apply(&mut a).expect("apply");
    rules.apply(&mut b).expect("apply");
    assert_eq!(
        a.canonical_bytes().unwrap(),
        b.canonical_bytes().unwrap()
    );

    let mut c = obj(&[("session", obj(&[("token", s("different"))]))]);
    rules.apply(&mut c).expect("apply");
    assert_ne!(
        a.canonical_bytes().unwrap(),
        c.canonical_bytes().unwrap()
    );
}

#[test]
fn missing_path_is_a_noop_but_shape_mismatch_is_an_error() {
    let mut rules = RedactRules::new();
    rules.drop_path("user.not_there").expect("rule");
    let mut v = doc();
    let before = v.canonical_text().expect("text");
    rules.apply(&mut v).expect("missing path is fine");
    assert_eq!(v.canonical_text().expect("text"), before);

    let mut bad = RedactRules::new();
    bad.drop_path("user.phone.deep").expect("rule");
    let mut v2 = doc();
    assert!(matches!(bad.apply(&mut v2), Err(RedactError::BadRules(_))));
}

#[test]
fn duplicate_paths_are_rejected() {
    let mut rules = RedactRules::new();
    rules.drop_path("user.phone").expect("first");
    assert!(matches!(
        rules.mask_path("user.phone"),
        Err(RedactError::DuplicatePath(_))
    ));
}

#[test]
fn rules_from_value() {
    let rules_v = Value::Array(vec![
        obj(&[("path", s("session.token")), ("action", s("hash"))]),
        obj(&[("path", s("user.phone")), ("action", s("mask"))]),
        obj(&[("path", s("user.id_card"))]),
    ]);
    let rules = RedactRules::from_value(&rules_v).expect("rules");
    assert_eq!(rules.len(), 3);

    let mut v = doc();
    rules.apply(&mut v).expect("apply");
    let u = user(&v);
    assert!(!u.contains_key("id_card"), "缺 action 默认 drop");
    assert_eq!(u.get("phone"), Some(&s("***")));

    // 未知 action 必须报错，不静默当 drop
    let bad = Value::Array(vec![obj(&[
        ("path", s("a")),
        ("action", s("blind")),
    ])]);
    assert!(matches!(
        RedactRules::from_value(&bad),
        Err(RedactError::BadRules(_))
    ));

    // 根路径：Mask 整棵树（空路径只能经 `add_rule`，`Path::parse("")` 会拒绝）
    let mut root_rules = RedactRules::new();
    root_rules
        .add_rule(amberice::RedactRule {
            path: amberice::Path::new(vec![]),
            action: RedactAction::Mask,
        })
        .expect("root");
    let mut v3 = doc();
    root_rules.apply(&mut v3).expect("apply");
    assert_eq!(v3, s("***"));
}
