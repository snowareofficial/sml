// SPDX-License-Identifier: MulanPSL-2.0
//! 保序解析的验收用例（方案 2 / B 之后：**不再由 feature 门控**）。
//!
//! 依据 `docs/swsml/spec/value-layout.md` §3/§5/§6 与 §12：
//! - 保序买到的是「键序 == 源序」，**不是**确定性字节（后者是 `to_sml_canonical`）；
//! - 合并规则（§6.3）：已存在的键**原位替换**、不存在的追加到尾部；
//! - `PartialEq` 两种键序下都按集合相等（§6.1），顺序敏感另走 `eq_ordered`。
//!
//! 入口是 **`parse_ordered()`**：显式要求源序。默认 `parse_ordered()` 仍按字节序
//! （由 `tests/emit.rs` 等既有用例钉着）—— 这是「少隐式」：要哪种顺序就写哪个函数，
//! 不看编译开关。
#![cfg(feature = "sml")]

use std::collections::BTreeMap;

use sml::{parse_ordered, to_sml, to_sml_canonical, Map, Value};

/// 取顶层对象的键序（按**迭代序**，不是排序后的）。
fn keys(src: &str) -> Vec<String> {
    let v = parse_ordered(src).expect("parse failed");
    match &v {
        Value::Object(m) => m.keys().map(|k| k.to_string()).collect(),
        other => panic!("顶层应为对象，得 {other:?}"),
    }
}

/// 取 `to_sml` 输出里每行的键名（只取行首到 `:` 的部分）。
fn line_keys(src: &str) -> Vec<String> {
    to_sml(&parse_ordered(src).expect("parse failed"))
        .lines()
        .filter_map(|l| l.split_once(':').map(|(k, _)| k.trim().to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// 1) 保序：键序 == 源序
// ---------------------------------------------------------------------------

/// 验收判据 ①：`parse → to_sml` 的键序 == 源序。
///
/// 这是保序唯一「肉眼可见」的收益点，也是最容易被写反的断言：
/// 若底层又换回排序容器，这里立刻红。
#[test]
fn to_sml_emits_keys_in_source_order() {
    let src = "z: 1\na: 2\nm: 3\nb: 4\n";
    assert_eq!(keys(src), vec!["z", "a", "m", "b"]);
    assert_eq!(line_keys(src), vec!["z", "a", "m", "b"]);
}

/// 验收判据 ①的下半：`parse → 改 → to_sml` 键序**仍是源序**。
///
/// 覆盖已有键的值不动位置（§6.3），故「改一个字段」不会把键挪到尾部 ——
/// 这正是配置编辑场景里 diff 干净的前提。
#[test]
fn edit_then_to_sml_keeps_source_order() {
    let v = parse_ordered("z: 1\na: 2\nm: 3\n").expect("parse failed");
    let mut v = v;
    if let Some(m) = v.as_object_mut() {
        m.insert("a".to_string(), Value::Int(99));
    }
    let out = to_sml(&v);
    let got: Vec<&str> = out
        .lines()
        .filter_map(|l| l.split_once(':').map(|(k, _)| k.trim()))
        .collect();
    assert_eq!(got, vec!["z", "a", "m"], "改值不得移动键的位置：{out}");
}

/// 嵌套块同样保序（对象是递归的，`Vec<Entry>` 在每一层）。
#[test]
fn nested_blocks_keep_source_order() {
    let src = "outer {\n  z: 1\n  a {\n    y: 2\n    b: 3\n  }\n}\n";
    let v = parse_ordered(src).expect("parse failed");
    let outer = v.get("outer").expect("outer");
    let ok: Vec<String> = match outer {
        Value::Object(m) => m.keys().map(|k| k.to_string()).collect(),
        other => panic!("应为对象，得 {other:?}"),
    };
    assert_eq!(ok, vec!["z", "a"]);
    let inner = outer.get("a").expect("a");
    let ik: Vec<String> = match inner {
        Value::Object(m) => m.keys().map(|k| k.to_string()).collect(),
        other => panic!("应为对象，得 {other:?}"),
    };
    assert_eq!(ik, vec!["y", "b"]);
}

// ---------------------------------------------------------------------------
// 2) 合并规则（§6.3）：已存在⇒原位，不存在⇒尾部
// ---------------------------------------------------------------------------

/// `&frag` 覆盖：**父块已有的键**被片段展开覆盖时**留在原位**（§6.3）。
///
/// 片段合并是**直接赋值**（已有键被片段值覆盖，不走重复键的数组提升 ——
/// 该语义由 `key_conflict.rs` 的既有用例钉着），故这里只钉「位置不变」。
#[test]
fn frag_overrides_in_place() {
    let src = "@base { host: \"new\" port: 8080 tls: true }\nserver {\n  host: \"old\"\n  port: 80\n  &base\n}\n";
    let v = parse_ordered(src).expect("parse failed");
    let sv = v.get("server").expect("server");
    let ks: Vec<String> = match sv {
        Value::Object(m) => m.keys().map(|k| k.to_string()).collect(),
        other => panic!("应为对象，得 {other:?}"),
    };
    assert_eq!(ks, vec!["host", "port", "tls"], "覆盖应原位、新键追加：{ks:?}");
    assert_eq!(sv.get("host").and_then(|x| x.as_str()), Some("new"));
    assert_eq!(sv.get("port").and_then(|x| x.as_i64()), Some(8080));
}

/// `&frag` 新增：**父块没有的键**追加到尾部。
#[test]
fn frag_appends_new_keys_at_tail() {
    let src = "@base { port: 80 }\nserver {\n  host: \"h1\"\n  &base\n}\n";
    let v = parse_ordered(src).expect("parse failed");
    let sv = v.get("server").expect("server");
    let ks: Vec<String> = match sv {
        Value::Object(m) => m.keys().map(|k| k.to_string()).collect(),
        other => panic!("应为对象，得 {other:?}"),
    };
    assert_eq!(ks, vec!["host", "port"], "新键应追加到尾部：{ks:?}");
}

/// `&frag` 混合：已有键原位替换 + 新键追加尾部，两者同时发生也不互相打乱。
#[test]
fn frag_mixed_override_and_append() {
    let src = "@base { port: 80 tls: true }\nserver {\n  host: \"h1\"\n  port: 8080\n  &base\n}\n";
    let v = parse_ordered(src).expect("parse failed");
    let sv = v.get("server").expect("server");
    let ks: Vec<String> = match sv {
        Value::Object(m) => m.keys().map(|k| k.to_string()).collect(),
        other => panic!("应为对象，得 {other:?}"),
    };
    assert_eq!(ks, vec!["host", "port", "tls"], "原位 + 追加：{ks:?}");
    assert_eq!(sv.get("port").and_then(|x| x.as_i64()), Some(80));
}

/// include 合并：与 `&frag` 同一套规则（已存在原位 / 不存在尾部）。
///
/// include 在 token 层拼接，故「已被父文档写过的键」再出现时走的是
/// `insert_field_val` 的原位替换分支 —— 这条用例把它钉住。
#[test]
fn include_merge_follows_same_rule() {
    let dir = std::env::temp_dir().join(format!("sml-po-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("建临时目录失败");
    std::fs::write(dir.join("base.sml"), "tls: true\n").unwrap();
    // include 只在**文件入口**展开（token 层拼接），故必须走文件入口（保序版）
    // —— 直接 `parse` 会把 `include` 当成普通键（既有语义，见 error_codes.rs）。
    let main = dir.join("main.sml");
    std::fs::write(&main, "include \"base.sml\"\nhost: \"h1\"\n").unwrap();
    let v = sml::parse_file_ordered(main.to_str().unwrap()).expect("parse_file failed");
    let ks: Vec<String> = match &v {
        Value::Object(m) => m.keys().map(|k| k.to_string()).collect(),
        other => panic!("应为对象，得 {other:?}"),
    };
    // 被 include 的键先落地（它们在源序上更早），host 追加在后
    assert_eq!(ks, vec!["tls", "host"], "include 合并序：{ks:?}");
    assert_eq!(v.get("tls").and_then(|x| x.as_bool()), Some(true));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 重复键提升为数组时**不挪位置**（保序档下 `remove` + `insert` 会把键甩到尾部，
/// 那是实现细节泄漏成行为差异 —— 这条用例挡住它）。
#[test]
fn duplicate_key_promotion_keeps_position() {
    let src = "a: 1\nb: 2\na: 3\n";
    let v = parse_ordered(src).expect("parse failed");
    let ks: Vec<String> = match &v {
        Value::Object(m) => m.keys().map(|k| k.to_string()).collect(),
        other => panic!("应为对象，得 {other:?}"),
    };
    assert_eq!(ks, vec!["a", "b"], "提升为数组不得移动键位：{ks:?}");
    assert!(matches!(v.get("a"), Some(Value::Array(x)) if x.len() == 2));
}

// ---------------------------------------------------------------------------
// 3) 相等仍按集合（§6.1）+ 顺序敏感另走 eq_ordered
// ---------------------------------------------------------------------------

/// 保序**不**改变相等语义：同键不同序的两个对象仍然相等（§6.1）。
/// 这条是「杜绝大面积红色」的守门用例。
#[test]
fn eq_is_still_set_equality() {
    let a = parse_ordered("a: 1\nb: 2\n").unwrap();
    let b = parse_ordered("b: 2\na: 1\n").unwrap();
    assert_eq!(a, b, "顺序不得参与 PartialEq");
    assert!(a.eq_ordered(&a.clone()));
    assert!(!a.eq_ordered(&b), "顺序敏感比较应另走 eq_ordered");
}

// ---------------------------------------------------------------------------
// 4) canonical 与保序解耦（§6.2）
// ---------------------------------------------------------------------------

/// 同值不同源 ⇒ `to_sml_canonical` 逐字节相同；而 `to_sml` 不同。
///
/// 二者是独立开关：保序回答「作者怎么写」，canonical 回答「同值同字节」。
#[test]
fn canonical_is_independent_of_source_order() {
    let a = parse_ordered("z: 1\na: 2\n").unwrap();
    let b = parse_ordered("a: 2\nz: 1\n").unwrap();
    assert_ne!(to_sml(&a), to_sml(&b), "保序档下源序不同 ⇒ 输出不同");
    assert_eq!(
        to_sml_canonical(&a),
        to_sml_canonical(&b),
        "canonical 必须与源序无关"
    );
    assert_eq!(to_sml_canonical(&a), "a: 2\nz: 1\n");
}

/// canonical 递归到嵌套层，且不改动原值（`sorted_by_key` 是拷贝语义）。
#[test]
fn canonical_recurses_and_does_not_mutate() {
    let src = "outer {\n  z: 1\n  a: 2\n}\ntop: 3\n";
    let v = parse_ordered(src).unwrap();
    let out = to_sml_canonical(&v);
    // 键后接块时 `dump` 会换行再写 `{`（与 `to_sml` 的既有排版一致）
    assert_eq!(out, "outer:\n{\n  a: 2\n  z: 1\n}\ntop: 3\n", "{out}");
    // 原值仍是源序（canonical 不回写）
    let ks: Vec<String> = match &v {
        Value::Object(m) => m.keys().map(|k| k.to_string()).collect(),
        _ => unreachable!(),
    };
    assert_eq!(ks, vec!["outer", "top"]);
}

// ---------------------------------------------------------------------------
// 5) 三档查找与体积（§5 / §12）
// ---------------------------------------------------------------------------

/// 三档阈值全覆盖：≤8 线性 / 9..=4096 排序下标 / >4096 哈希。
///
/// 只测中间档是这类结构最典型的漏测 —— 阈值写错时小规模用例全绿。
#[test]
fn lookup_is_correct_across_all_three_tiers() {
    for n in [0usize, 1, 8, 9, 64, 4096, 4097, 9000] {
        let mut m = Map::new();
        for i in 0..n {
            m.insert(format!("k{i}"), Value::Int(i as i64));
        }
        assert_eq!(m.len(), n);
        for i in 0..n {
            assert_eq!(m.get(&format!("k{i}")).and_then(|x| x.as_i64()), Some(i as i64), "n={n}");
        }
        assert!(m.get("nope").is_none());
    }
}

/// §12 验收：`Value` 不得因换载体而膨胀（48 B 是硬红线）。
#[test]
fn value_size_stays_within_budget() {
    assert!(
        std::mem::size_of::<Value>() <= 48,
        "size_of::<Value>() = {} 超过 48 B",
        std::mem::size_of::<Value>()
    );
}

/// 保序档与 `BTreeMap` 的互转：键集合一致（过渡期与测试用）。
#[test]
fn converts_to_and_from_btree_map() {
    let v = parse_ordered("z: 1\na: 2\n").unwrap();
    let m = v.as_object().expect("对象");
    let bt: BTreeMap<String, Value> = m
        .iter()
        .map(|(k, val)| (k.to_string(), val.clone()))
        .collect();
    assert_eq!(bt.len(), 2);
    assert_eq!(bt.get("z").and_then(|x| x.as_i64()), Some(1));
}
