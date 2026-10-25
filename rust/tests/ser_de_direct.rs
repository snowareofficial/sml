// SPDX-License-Identifier: MulanPSL-2.0
//! `sml::serde::to_string_direct` —— 直出 SML 文本（不经 `Value` 中转）。
//!
//! 运行：`cargo test --features serde --test ser_de_direct`

#![cfg(all(feature = "serde", feature = "sml"))]

use serde::Serialize;
use sml::Value;
use std::collections::BTreeMap;

// ---- 字节一致性：与 `to_sml` 逐字节相同（两份发射器不许漂移）----

fn obj(pairs: &[(&str, Value)]) -> Value {
    Value::Object(
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    )
}

fn cases() -> Vec<Value> {
    vec![
        // 扁平块
        obj(&[("a", Value::Int(1)), ("b", Value::Str("x".into()))]),
        // 嵌套块（非扁平 → 展开多行）
        obj(&[("a", obj(&[("x", Value::Int(1)), ("y", Value::Int(2))]))]),
        // 嵌套块但扁平 → 单行 { k: v }
        obj(&[("a", obj(&[("x", Value::Int(1))]))]),
        // 根块含 __type（走 dump_block 特例）
        obj(&[
            ("__type", Value::Str("Circle".into())),
            ("_value", Value::float(3.0)),
        ]),
        // 数组：标量 / 空 / 嵌对象
        obj(&[("t", Value::Array(vec![Value::Int(1), Value::Int(2)]))]),
        obj(&[("t", Value::Array(vec![]))]),
        obj(&[(
            "t",
            Value::Array(vec![obj(&[("k", Value::Str("v".into()))])]),
        )]),
        // 空块
        obj(&[("e", Value::Object(BTreeMap::new()))]),
        // 标量各型
        obj(&[
            ("n", Value::Null),
            ("t", Value::Bool(true)),
            ("f", Value::Bool(false)),
        ]),
        // 需要引号的键 / 值
        obj(&[("x y", Value::Str("a b".into())), ("0755", Value::Int(1))]),
        // 浮点：整数值保留 .0；NaN / inf 落成引号串
        obj(&[("a", Value::float(1.0)), ("b", Value::float(-0.5))]),
        obj(&[("nan", Value::float(f64::NAN)), ("inf", Value::float(f64::INFINITY))]),
        // 顶层非对象
        Value::Array(vec![Value::Int(1), Value::Str("s".into())]),
        Value::Int(7),
        Value::Str("bare".into()),
    ]
}

#[test]
fn byte_identical_with_to_sml() {
    for (i, v) in cases().into_iter().enumerate() {
        let expected = sml::to_sml(&v);
        let got = sml::serde::to_string_direct(&v)
            .unwrap_or_else(|e| panic!("第 {i} 例直出失败: {e}"));
        assert_eq!(
            expected, got,
            "第 {i} 例与 to_sml 不一致\n--- to_sml ---\n{expected}\n--- direct ---\n{got}"
        );
    }
}

// ---- 声明顺序：这是直出存在的首要理由 ----

#[derive(Serialize)]
struct Order {
    zebra: i32,
    alpha: i32,
    middle: i32,
}

#[test]
fn declaration_order_preserved() {
    let v = Order { zebra: 1, alpha: 2, middle: 3 };
    let direct = sml::serde::to_string_direct(&v).unwrap();
    assert_eq!(direct, "zebra: 1\nalpha: 2\nmiddle: 3\n");

    // 对照组：走 `Value` 那条路会被 BTreeMap 重排成字典序
    let via_value = sml::serde::to_string(&v).unwrap();
    assert_eq!(via_value, "alpha: 2\nmiddle: 3\nzebra: 1\n");
}

// ---- 大整数：直出能写，`Value` 那条路是拒收 ----

#[derive(Serialize)]
struct Big {
    i: i128,
    u: u128,
    big: u64,
}

#[test]
fn big_ints_are_written_not_rejected() {
    let v = Big {
        i: i128::MAX,
        u: u128::MAX,
        big: u64::MAX,
    };
    let direct = sml::serde::to_string_direct(&v).unwrap();
    assert!(direct.contains("170141183460469231731687303715884105727"), "{direct}");
    assert!(direct.contains("340282366920938463463374607431768211455"), "{direct}");
    assert!(direct.contains("18446744073709551615"), "{direct}");

    // 对照组：走 `Value` 会直接报错（E-DERIVE-002 口径）
    assert!(
        sml::serde::to_string(&v).is_err(),
        "Value 那条路应拒收超 i64 的整数"
    );
}

#[test]
fn big_int_reads_back_as_str() {
    // 直出保真，但 SML 回读按 coerce_word B10 规则得到 Str —— 往返不对称，文档已写明
    let text = sml::serde::to_string_direct(&u64::MAX.to_string()).unwrap();
    let _ = text;
    let v = sml::parse("x: 18446744073709551615\n").unwrap();
    assert_eq!(
        v.get("x"),
        Some(&Value::Str("18446744073709551615".into())),
        "超 i64 的裸词应读回为 Str"
    );
}

// ---- 深度闸门 ----

fn nested_arrays(depth: usize) -> Value {
    let mut v = Value::Int(1);
    for _ in 0..depth {
        v = Value::Array(vec![v]);
    }
    v
}

#[test]
fn depth_limit_is_error() {
    let err = sml::serde::to_string_direct(&nested_arrays(200))
        .expect_err("超过 MAX_VALUE_DEPTH 应返回 Err");
    assert!(err.contains("嵌套深度"), "实际: {err}");
}

#[test]
fn within_depth_matches_to_sml() {
    let v = nested_arrays(50);
    assert_eq!(
        sml::to_sml(&v),
        sml::serde::to_string_direct(&v).unwrap()
    );
}

// ---- 键必须是字符串 ----

#[test]
fn non_string_key_is_error() {
    let mut m = std::collections::HashMap::new();
    m.insert(1i32, "x".to_string());
    let err = sml::serde::to_string_direct(&m).expect_err("非字符串键应报错");
    assert!(err.contains("键必须是字符串"), "实际: {err}");
}

// ---- 与 derive 侧形状一致 ----

#[derive(Serialize)]
enum Shape {
    Unit,
    Circle(f64),
    Rect { w: f64, h: f64 },
}

#[test]
fn enum_unit_and_newtype_match_value_path() {
    // 单元变体 / newtype 变体不涉及字段顺序，两条路必然一致
    for s in [Shape::Unit, Shape::Circle(3.0)] {
        assert_eq!(
            sml::serde::to_string(&s).unwrap(),
            sml::serde::to_string_direct(&s).unwrap()
        );
    }
}

#[test]
fn enum_struct_variant_keeps_declaration_order() {
    let s = Shape::Rect { w: 4.0, h: 5.0 };
    let direct = sml::serde::to_string_direct(&s).unwrap();
    assert!(direct.contains("__type: Rect"), "{direct}");
    // 直出：声明顺序 w → h
    assert!(
        direct.find("w: 4.0").unwrap() < direct.find("h: 5.0").unwrap(),
        "直出应保留声明顺序: {direct}"
    );
    // 对照组：Value 那条路按 BTreeMap 排成 h → w
    let via_value = sml::serde::to_string(&s).unwrap();
    assert!(
        via_value.find("h: 5.0").unwrap() < via_value.find("w: 4.0").unwrap(),
        "Value 路应字典序重排: {via_value}"
    );
}

#[derive(Serialize)]
struct Opt {
    a: Option<i32>,
    b: Option<i32>,
}

#[test]
fn option_and_empty_containers() {
    let v = Opt { a: None, b: Some(7) };
    let text = sml::serde::to_string_direct(&v).unwrap();
    assert!(text.contains("a: null"), "{text}");
    assert!(text.contains("b: 7"), "{text}");

    // 顶层空数组：`is_flat`（空集为真）走单行 `dump_inline`，故是 `[  ]` 而非 `[]`。
    // 与 `to_sml` 一致 —— 这里刻意写成对照，避免两份发射器漂移。
    let empty: Vec<i32> = vec![];
    let direct = sml::serde::to_string_direct(&empty).unwrap();
    assert_eq!(direct, "[  ]");
    assert_eq!(sml::to_sml(&Value::Array(vec![])), direct);
}
