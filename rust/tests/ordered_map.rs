// SPDX-License-Identifier: MulanPSL-2.0
//! 方案 2 第一步的判据：**默认构建里就同时存在两个显式对象类型**。
//! Evidence for step 1: both `Map` and `OrderedMap` exist without any feature.
//!
//! 这一步还没有改 `Value::Object` 的载体（仍由 `preserve_order` 决定），
//! 但两个类型已经无条件可用 —— 后续切换不再需要引入任何依赖。

use sml::{Map, OrderedMap, Value};

/// `OrderedMap` 在任何构建档都保源序、且能按名字取值。
#[test]
fn ordered_map_keeps_source_order_in_every_profile() {
    let mut m = OrderedMap::new();
    m.insert("b".to_string(), Value::Int(2));
    m.insert("a".to_string(), Value::Int(1));

    let keys: Vec<String> = m.iter().map(|(k, _)| k.to_string()).collect();
    assert_eq!(keys, vec!["b", "a"], "OrderedMap 必须保源序");
    assert_eq!(m.get("a"), Some(&Value::Int(1)));
    assert_eq!(m.len(), 2);

    // 覆盖已有键不改位置，这是 canonical 幂等的前提
    m.insert("b".to_string(), Value::Int(9));
    let keys2: Vec<String> = m.iter().map(|(k, _)| k.to_string()).collect();
    assert_eq!(keys2, vec!["b", "a"], "覆盖已有键不许改变位置");
}

/// **本轮最关键的一条判据**：默认构建（没有任何 feature）里，
/// 只要对象是**显式有序**的，就能签 canonical；无序对象则明确报错。
/// 「能不能签」由值决定，不再由编译开关决定。
#[test]
fn explicit_ordered_object_can_be_canonicalized_in_default_build() {
    use sml::CanonicalError;

    let mut o = OrderedMap::new();
    o.insert("b".to_string(), Value::Int(2));
    o.insert("a".to_string(), Value::Int(1));
    let v = Value::Object(Map::from(o));
    assert_eq!(v.canonical_text().unwrap(), "b: 2\na: 1\n");

    let mut u = Map::unordered();
    u.insert("b".to_string(), Value::Int(2));
    u.insert("a".to_string(), Value::Int(1));
    let e = Value::Object(u).canonical_text().unwrap_err();
    assert_eq!(e, CanonicalError::KeyOrderUnavailable);
    assert!(
        e.to_string().contains("Map::ordered") || e.to_string().contains("parse_ordered"),
        "错误必须指路显式入口：{e}"
    );
}

/// `parse_ordered()` 保源序，且在**默认构建**里就能签 canonical（不需要任何 feature）。
#[test]
fn parse_ordered_keeps_source_order_without_any_feature() {
    let v = sml::parse_ordered("z: 1\na: 2\n").unwrap();
    assert_eq!(v.canonical_text().unwrap(), "z: 1\na: 2\n");

    // `parse()` 的历史行为不变：键按字节序排序
    #[cfg(not(feature = "preserve_order"))]
    {
        let u = sml::parse("z: 1\na: 2\n").unwrap();
        assert_eq!(sml::to_sml(&u).replace(' ', "").contains("a:2"), true);
        assert_eq!(
            u.canonical_text().unwrap_err(),
            sml::CanonicalError::KeyOrderUnavailable
        );
    }

    assert_eq!(v.clone().into_ordered(), v);
}

/// `CanonicalValue` 把 C2 的检查挪到**转换点**：成功即保证有序，失败即明确拒绝。
#[test]
fn canonical_value_makes_key_order_a_type_level_fact() {
    let v = sml::parse_ordered("z: 1\na: 2\n").unwrap();
    let cv = sml::CanonicalValue::try_from(v).unwrap();
    assert_eq!(cv.canonical_text().unwrap(), "z: 1\na: 2\n");

    // 无序对象在**转换时**就失败 —— 不是等到 dump 才在运行期发现
    let u = sml::parse("z: 1\na: 2\n").unwrap();
    assert_eq!(
        sml::CanonicalValue::try_from(u).unwrap_err(),
        sml::CanonicalError::KeyOrderUnavailable
    );

    // 借用版本：先检查、成功才克隆
    let v2 = sml::parse_ordered("b: 1\n").unwrap();
    let cv2 = sml::CanonicalValue::try_from(&v2).unwrap();
    assert_eq!(cv2.as_value(), &v2);
    assert_eq!(cv2.into_value(), v2);
}

/// 默认档下 `Map` 是 BTreeMap（键按字节序），与 `OrderedMap` **语义不同**。
#[cfg(not(feature = "preserve_order"))]
#[test]
fn map_and_ordered_map_are_distinct_in_default_build() {
    let mut m = Map::unordered();
    m.insert("b".to_string(), Value::Int(2));
    m.insert("a".to_string(), Value::Int(1));

    let keys: Vec<String> = m.keys().map(|k| k.to_string()).collect();
    assert_eq!(keys, vec!["a", "b"], "默认档 Map 仍按 UTF-8 字节序");

    let mut o = OrderedMap::new();
    o.insert("b".to_string(), Value::Int(2));
    o.insert("a".to_string(), Value::Int(1));
    let ordered: Vec<String> = o.iter().map(|(k, _)| k.to_string()).collect();
    assert_eq!(ordered, vec!["b", "a"]);
    assert_ne!(ordered, keys, "两个类型必须能一眼看出区别");
}
