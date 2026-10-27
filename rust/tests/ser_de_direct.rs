// SPDX-License-Identifier: MulanPSL-2.0
//! `sml::serde::to_string_direct` —— 直出 SML 文本（不经 `Value` 中转）。
//!
//! 运行：`cargo test --features serde --test ser_de_direct`

#![cfg(all(feature = "serde", feature = "sml"))]

use serde::Serialize;
use sml::Value;
use std::cell::RefCell;
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

// ---- 浮点 raw：两条发射器的不对称（TODO.md 雷 5 B 组 2-3 行）----
//
// 先说结论：**做不到让 serde 侧也保留 `1e10`**。
// serde 的 `Serializer` 对浮点只有一个入口 `serialize_f64(v: f64)`
// （`serde_core-1.0.229/src/ser/mod.rs:681`），`f64` **按值**传入 ——
// 数据模型里没有「数值 + 原始书写文本」这个位置，序列化器拿不到 raw。
// 下面的 `probe` 把这个事实**实测**出来（不靠印象）。

/// 什么都不做，只把 serde 调过来的方法名与参数记下来的探针 `Serializer`。
mod probe {
    use serde::de::value::Error;
    use serde::ser::{Impossible, Serializer};
    use std::cell::RefCell;

    pub struct Probe<'a>(pub &'a RefCell<Vec<String>>);

    /// 标量通道：记下 `方法名(参数)` 后正常返回。
    macro_rules! record {
        ($(fn $m:ident($($a:ident: $t:ty),*) { $e:expr })*) => {
            $(
                fn $m(self, $($a: $t),*) -> Result<Self::Ok, Self::Error> {
                    self.0.borrow_mut().push(format!("{}({})", stringify!($m), $e));
                    Ok(())
                }
            )*
        };
    }

    /// 容器通道：本探针只观测标量，一律拒收。
    macro_rules! reject {
        ($(fn $m:ident($($a:ident: $t:ty),*);)*) => {
            $(
                fn $m(self, $($a: $t),*) -> Result<Impossible<(), Error>, Error> {
                    Err(<Error as serde::de::Error>::custom("probe: 容器不在观测范围"))
                }
            )*
        };
    }

    impl<'a> Serializer for Probe<'a> {
        type Ok = ();
        type Error = Error;
        type SerializeSeq = Impossible<(), Error>;
        type SerializeTuple = Impossible<(), Error>;
        type SerializeTupleStruct = Impossible<(), Error>;
        type SerializeTupleVariant = Impossible<(), Error>;
        type SerializeMap = Impossible<(), Error>;
        type SerializeStruct = Impossible<(), Error>;
        type SerializeStructVariant = Impossible<(), Error>;

        record! {
            fn serialize_bool(v: bool) { v }
            fn serialize_i8(v: i8) { v }
            fn serialize_i16(v: i16) { v }
            fn serialize_i32(v: i32) { v }
            fn serialize_i64(v: i64) { v }
            fn serialize_u8(v: u8) { v }
            fn serialize_u16(v: u16) { v }
            fn serialize_u32(v: u32) { v }
            fn serialize_u64(v: u64) { v }
            fn serialize_f32(v: f32) { v }
            fn serialize_f64(v: f64) { v }
            fn serialize_char(v: char) { v }
            fn serialize_str(v: &str) { v }
            fn serialize_bytes(v: &[u8]) { format!("{v:?}") }
            fn serialize_none() { "" }
            fn serialize_unit() { "" }
            fn serialize_unit_struct(_n: &'static str) { "" }
            fn serialize_unit_variant(_n: &'static str, _i: u32, v: &'static str) { v }
        }

        fn serialize_some<T: ?Sized + serde::Serialize>(self, v: &T) -> Result<(), Error> {
            v.serialize(self)
        }
        fn serialize_newtype_struct<T: ?Sized + serde::Serialize>(
            self,
            _n: &'static str,
            v: &T,
        ) -> Result<(), Error> {
            v.serialize(self)
        }
        fn serialize_newtype_variant<T: ?Sized + serde::Serialize>(
            self,
            _n: &'static str,
            _i: u32,
            v: &'static str,
            x: &T,
        ) -> Result<(), Error> {
            self.0
                .borrow_mut()
                .push(format!("serialize_newtype_variant({v})"));
            x.serialize(self)
        }

        reject! {
            fn serialize_seq(_l: Option<usize>);
            fn serialize_tuple(_l: usize);
            fn serialize_tuple_struct(_n: &'static str, _l: usize);
            fn serialize_tuple_variant(_n: &'static str, _i: u32, _v: &'static str, _l: usize);
            fn serialize_map(_l: Option<usize>);
            fn serialize_struct(_n: &'static str, _l: usize);
            fn serialize_struct_variant(_n: &'static str, _i: u32, _v: &'static str, _l: usize);
        }
    }
}

/// 想把 `1e10` 的**写法**带过去的唯一通道：走字符串（`collect_str`）。
///
/// 代价见下一个用例 —— 在别的 serde 格式里这个浮点就不再是数字了。
struct RawFloat(f64, &'static str);

impl Serialize for RawFloat {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let _ = self.0; // 数值根本传不进这个通道
        s.collect_str(&format_args!("{}", self.1))
    }
}

/// Q1 的实测答案：**没有钩子**。serde 递给序列化器的就只是一个 `f64`。
#[test]
fn serde_hands_the_emitter_only_an_f64() {
    let log = RefCell::new(Vec::new());

    // ① 裸 f64：只有一个入口
    1e10f64.serialize(probe::Probe(&log)).unwrap();

    // ② 带 raw 的 `Value::Float`：走的**还是** `serialize_f64(f64)` ——
    //    raw 在 `impl Serialize for Value` 处就被丢掉了，任何序列化器都见不到它
    Value::float_with_raw(1e10, "1e10")
        .serialize(probe::Probe(&log))
        .unwrap();

    // ③ 唯一能携带文本的通道是 `collect_str`，而它落在 `serialize_str` 上
    RawFloat(1e10, "1e10").serialize(probe::Probe(&log)).unwrap();

    assert_eq!(
        log.into_inner(),
        vec![
            "serialize_f64(10000000000)".to_string(),
            "serialize_f64(10000000000)".to_string(),
            "serialize_str(1e10)".to_string(),
        ],
        "serde 侧拿到的只有 f64：raw 无槽位可放"
    );
}

/// 反证：真要走字符串通道来「保住写法」，浮点在别的格式里会变成**字符串** ——
/// 那是改数据（比丢书写形式糟），所以这条路不能采用。
#[test]
fn carrying_raw_via_the_str_channel_would_change_the_type() {
    assert_eq!(
        serde_json::to_string(&RawFloat(1e10, "1e10")).unwrap(),
        "\"1e10\"",
        "走字符串通道后，JSON 里是带引号的字符串，不再是数字"
    );
    // 对照：正常的 f64 在 JSON 里是数字（不带引号）
    let plain = serde_json::to_string(&1e10f64).unwrap();
    assert!(!plain.starts_with('"'), "正常 f64 应是数字：{plain}");
}

/// 雷 5 B 组 2-3 行：浮点 `raw` **只活在 `to_sml` 这一条发射器上**。
///
/// 这是「承认不对称」之后钉住现状的测试。将来谁让 serde 侧也保住了 `1e10`，
/// 这条会红 —— 那时请回来改这里，并改 `ser_text.rs` 模块文档里「做不到」的定性。
#[test]
fn float_raw_only_survives_to_sml() {
    let v = sml::parse("x: 1e10\n").unwrap();
    // parser 确实把书写形式记下来了（raw 的唯一生产点：`sml-lex` 的 `coerce_word`）
    assert_eq!(v.get("x").and_then(Value::float_raw), Some("1e10"));

    // ① 直接写 `Value`：raw 在 ⇒ 保留 `1e10`
    assert_eq!(sml::to_sml(&v), "x: 1e10\n");

    // ② 经 serde 的两条发射器：raw 在 `impl Serialize for Value` 处即已丢弃
    let lost = "x: 10000000000.0\n";
    assert_eq!(sml::serde::to_string(&v).unwrap(), lost);
    assert_eq!(sml::serde::to_string_direct(&v).unwrap(), lost);

    // 正对照：丢的只是**书写形式**，值没变（回读仍是同一个 f64）
    let back = sml::parse(lost).unwrap();
    assert_eq!(back.get("x").and_then(Value::as_float), Some(1e10));
}
