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

/// 默认档下 `Map` 是 BTreeMap（键按字节序），与 `OrderedMap` **语义不同**。
#[cfg(not(feature = "preserve_order"))]
#[test]
fn map_and_ordered_map_are_distinct_in_default_build() {
    let mut m = Map::new();
    m.insert("b".to_string(), Value::Int(2));
    m.insert("a".to_string(), Value::Int(1));

    let keys: Vec<String> = m.keys().cloned().collect();
    assert_eq!(keys, vec!["a", "b"], "默认档 Map 仍按 UTF-8 字节序");

    let mut o = OrderedMap::new();
    o.insert("b".to_string(), Value::Int(2));
    o.insert("a".to_string(), Value::Int(1));
    let ordered: Vec<String> = o.iter().map(|(k, _)| k.to_string()).collect();
    assert_eq!(ordered, vec!["b", "a"]);
    assert_ne!(ordered, keys, "两个类型必须能一眼看出区别");
}
