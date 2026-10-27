//! Value 访问器层补全测试（I-3）：as_object / as_array / as_i64 / as_bool，
//! 以及 *_mut、遍历能力（iter / len / is_empty）、get() 数组下标。
//!
//! 约定：类型不符必须返回 None，绝不 panic / unwrap（库内 unwrap 须有不变量守护）。
use sml_value::Value;
use std::collections::BTreeMap;

#[test]
fn as_object_returns_map_or_none() {
    let obj = Value::Object(BTreeMap::new());
    assert!(obj.as_object().is_some());
    // 反例：非对象返回 None，不 panic
    assert!(Value::Str("x".into()).as_object().is_none());
    assert!(Value::Array(vec![]).as_object().is_none());
    assert!(Value::Int(1).as_object().is_none());
    assert!(Value::Bool(true).as_object().is_none());
    assert!(Value::float(1.0).as_object().is_none());
    assert!(Value::Null.as_object().is_none());
}

#[test]
fn as_array_returns_vec_or_none() {
    let arr = Value::Array(vec![]);
    assert!(arr.as_array().is_some());
    assert!(Value::Str("x".into()).as_array().is_none());
    assert!(Value::Object(BTreeMap::new()).as_array().is_none());
    assert!(Value::Int(1).as_array().is_none());
    assert!(Value::Bool(true).as_array().is_none());
    assert!(Value::float(1.0).as_array().is_none());
    assert!(Value::Null.as_array().is_none());
}

#[test]
fn as_i64_returns_int_or_none() {
    assert_eq!(Value::Int(7).as_i64(), Some(7));
    assert_eq!(Value::Int(-3).as_i64(), Some(-3));
    // 反例：Float 不降级为 i64
    assert_eq!(Value::float(7.0).as_i64(), None);
    assert_eq!(Value::float(7.5).as_i64(), None);
    assert_eq!(Value::Str("7".into()).as_i64(), None);
    assert_eq!(Value::Bool(true).as_i64(), None);
    assert_eq!(Value::Object(BTreeMap::new()).as_i64(), None);
    assert_eq!(Value::Array(vec![]).as_i64(), None);
    assert_eq!(Value::Null.as_i64(), None);
}

#[test]
fn as_bool_returns_bool_or_none() {
    assert_eq!(Value::Bool(true).as_bool(), Some(true));
    assert_eq!(Value::Bool(false).as_bool(), Some(false));
    // 反例
    assert_eq!(Value::Int(1).as_bool(), None);
    assert_eq!(Value::Str("true".into()).as_bool(), None);
    assert_eq!(Value::float(1.0).as_bool(), None);
    assert_eq!(Value::Object(BTreeMap::new()).as_bool(), None);
    assert_eq!(Value::Array(vec![]).as_bool(), None);
    assert_eq!(Value::Null.as_bool(), None);
}

#[test]
fn as_object_mut_returns_mut_map_or_none() {
    let mut v = Value::Object(BTreeMap::new());
    if let Some(m) = v.as_object_mut() {
        m.insert("k".into(), Value::Int(1));
    }
    // 写回后读取可见
    assert_eq!(v.get("k").and_then(|x| x.as_i64()), Some(1));
    // 反例：非对象返回 None，不 panic
    let mut s = Value::Str("x".into());
    assert!(s.as_object_mut().is_none());
}

#[test]
fn as_array_mut_returns_mut_vec_or_none() {
    let mut v = Value::Array(vec![Value::Int(1)]);
    if let Some(a) = v.as_array_mut() {
        a.push(Value::Int(2));
    }
    assert_eq!(v.as_array().map(|a| a.len()), Some(2));
    // 反例
    let mut s = Value::Str("x".into());
    assert!(s.as_array_mut().is_none());
}

#[test]
fn iter_yields_children_or_none() {
    let arr = Value::Array(vec![Value::Int(1), Value::Int(2)]);
    let collected: Vec<_> = if let Some(it) = arr.iter() {
        it.map(|v| v.as_i64()).collect()
    } else {
        Vec::new()
    };
    assert_eq!(collected, vec![Some(1), Some(2)]);
    let obj = Value::Object(BTreeMap::new());
    assert!(obj.iter().is_some());
    // 反例：非 Object/Array 返回 None，不 panic
    assert!(Value::Str("x".into()).iter().is_none());
    assert!(Value::Int(1).iter().is_none());
    assert!(Value::Bool(true).iter().is_none());
    assert!(Value::float(1.0).iter().is_none());
    assert!(Value::Null.iter().is_none());
}

#[test]
fn len_and_is_empty_for_object_array() {
    let arr = Value::Array(vec![Value::Int(1)]);
    assert_eq!(arr.len(), Some(1));
    assert_eq!(arr.is_empty(), Some(false));
    let empty = Value::Array(vec![]);
    assert_eq!(empty.len(), Some(0));
    assert_eq!(empty.is_empty(), Some(true));
    let obj = Value::Object(BTreeMap::from([("a".into(), Value::Int(1))]));
    assert_eq!(obj.len(), Some(1));
    assert_eq!(obj.is_empty(), Some(false));
    let empty_obj = Value::Object(BTreeMap::new());
    assert_eq!(empty_obj.is_empty(), Some(true));
    // 反例：标量返回 None，不 panic
    assert_eq!(Value::Str("x".into()).len(), None);
    assert_eq!(Value::Str("x".into()).is_empty(), None);
    assert_eq!(Value::Int(1).len(), None);
    assert_eq!(Value::Null.len(), None);
    assert_eq!(Value::Null.is_empty(), None);
}

#[test]
fn get_supports_array_index() {
    use std::collections::BTreeMap;
    let doc = Value::Object(BTreeMap::from([(
        "a".into(),
        Value::Object(BTreeMap::from([(
            "b".into(),
            Value::Array(vec![
                Value::Int(10),
                Value::Object(BTreeMap::from([("c".into(), Value::Int(99))])),
            ]),
        )])),
    )]));
    // 点路径 + 下标
    assert_eq!(doc.get("a.b[0]").and_then(|x| x.as_i64()), Some(10));
    // 下标后再点路径
    assert_eq!(doc.get("a.b[1].c").and_then(|x| x.as_i64()), Some(99));
    // 链式下标
    let nested = Value::Array(vec![Value::Array(vec![Value::Int(7)])]);
    assert_eq!(nested.get("[0][0]").and_then(|x| x.as_i64()), Some(7));
    // 反例：越界 / 类型不符 / 下标非数字 => None，不 panic
    assert!(doc.get("a.b[9]").is_none());
    assert!(doc.get("a.b[x]").is_none());
    assert!(doc.get("a.b[]").is_none());
    assert!(doc.get("a.b[-1]").is_none());
    assert!(doc.get("a.b[0].z").is_none());
}

/// 回归钉子：`get()` 新增 `[idx]` 下标后，键名**本身含方括号**的文档
/// 不得从 `Some` 变 `None` —— 字面键名优先于下标语法。
#[test]
fn bracket_in_key_name_is_not_rewritten() {
    use std::collections::BTreeMap;
    let doc = Value::Object(BTreeMap::from([
        ("a[0]".into(), Value::Int(1)),
        ("x[".into(), Value::Int(2)),
        ("plain".into(), Value::Str("ok".into())),
    ]));
    // 键名就叫 a[0]：按字面取，不能当成"键 a 的下标 0"
    assert_eq!(doc.get("a[0]").and_then(|x| x.as_i64()), Some(1));
    // 未闭合方括号同理，按字面键名取
    assert_eq!(doc.get("x[").and_then(|x| x.as_i64()), Some(2));
    // 普通键不受影响
    assert_eq!(doc.get("plain").and_then(|x| x.as_str()), Some("ok"));
    // 没有字面匹配的仍走下标语法（见 get_supports_array_index）
    let arr = Value::Object(BTreeMap::from([("b".into(), Value::Array(vec![Value::Int(5)]))]));
    assert_eq!(arr.get("b[0]").and_then(|x| x.as_i64()), Some(5));
}
