// SPDX-License-Identifier: MulanPSL-2.0
//! T3 random-access reader tests / T3 随机访问读取的验收用例。
//!
//! 用例里的对象由 `Map::ordered()` 显式构造，canonical 字节在任何 feature 组合下都可产出，
//! 故本文件不设档位门控。
//! ｜ Objects here are built with `Map::ordered()`; canonical bytes are available in every
//!    feature profile, so this file is not gated.

use amberice::{ContainerError, Map, Path, Store, Value};

/// 整树解码后的取值口径（对照组）：`Store::get` 必须与它一致。
fn at(v: &Value, path: &str) -> Value {
    let p = Path::parse(path).unwrap();
    v.get_path(&p).unwrap().clone()
}

/// A document with 500 sibling objects — big enough that "did not decode everything" is
/// observable, small enough to keep the test fast (< 100 KB).
/// 500 个兄弟对象的文档：够大到能观察「没有全解」，又够小到跑得快。
fn wide_doc() -> Value {
    let items: Vec<Value> = (0..500)
        .map(|i| {
            let mut m = Map::ordered();
            m.insert("idx".to_string(), Value::Int(i));
            m.insert(
                "pad".to_string(),
                Value::Str(format!("padding-{i}-{}", "x".repeat(120))),
            );
            Value::Object(m)
        })
        .collect();
    let mut root = Map::ordered();
    root.insert("arr".to_string(), Value::Array(items));
    root.insert("head".to_string(), Value::Str("根".into()));
    Value::Object(root)
}

/// `get` must agree with a full decode, and `subtree_bytes` must be a decodable node.
/// `get` 必须与整树解码一致，`subtree_bytes` 必须是能独立解码的一个节点。
#[test]
fn get_matches_full_decode() {
    let doc = wide_doc();
    let f = amberice::dump( &cv(&doc) ).unwrap();
    let store = Store::open(&f).unwrap();
    let full = amberice::load(&f).unwrap();

    for path in ["head", "arr[0].idx", "arr[499].idx", "arr[250].pad"] {
        assert_eq!(store.get(path).unwrap(), at(&full, path), "路径 {path}");
    }

    // 子树字节可以单独解码（含 6 B 节点头），且只属于这棵子树
    let sub = store.subtree_bytes("arr[42]").unwrap();
    let decoded = amberice::value_from_subtree(store.strtab_body(), sub).unwrap();
    assert_eq!(decoded, at(&full, "arr[42]"));
    assert!(
        sub.len() < store.value_body().len() / 10,
        "子树 {} B 不该接近整树 {} B",
        sub.len(),
        store.value_body().len()
    );
}

/// The strong form of “does not traverse the whole tree”: corrupt a **sibling's tag** and the
/// lookup of another sibling must still work (the broken one is skipped by `subtree_len`
/// without ever being decoded). ｜ 「不遍历全树」的强形式：把某个兄弟的 tag 改坏，取别的兄弟仍成功。
#[test]
fn siblings_are_skipped_without_decoding() {
    let f = amberice::dump( &cv(&wide_doc()) ).unwrap();
    let store = Store::open(&f).unwrap();

    // 找到 arr[7] 那棵子树的偏移：用零拷贝的子树字节去定位
    let target = store.subtree_bytes("arr[7]").unwrap();
    let base = store.value_body().as_ptr() as usize;
    let off = target.as_ptr() as usize - base;
    assert!(off < store.value_body().len());

    // 把它的 tag 改成非法值（0xFF）：整树解码必然失败，跳过去则应毫无影响
    let mut broken = f.clone();
    let at = (store.value_body().as_ptr() as usize - f.as_ptr() as usize) + off;
    broken[at] = 0xFF;

    let store2 = Store::open_unverified(&broken).unwrap();
    assert!(amberice::load(&broken).is_err(), "改坏 tag 后整树解码必须失败");
    assert!(
        store2.get("arr[0].idx").is_ok(),
        "arr[0] 在坏兄弟之前，跳过它即可"
    );
    assert!(
        store2.get("arr[499].idx").is_ok(),
        "arr[499] 在坏兄弟之后，同样靠 subtree_len 跳过"
    );
    assert!(
        store2.get("arr[7].idx").is_err(),
        "坏掉的那棵自己当然解不开"
    );
}

/// `nodes_visited` measures the sibling scan only; the decoded work equals the target subtree.
/// `nodes_visited` 只反映兄弟扫描；解码量等于目标子树。
#[test]
fn stats_show_sibling_scan_not_full_decode() {
    let f = amberice::dump( &cv(&wide_doc()) ).unwrap();
    let store = Store::open(&f).unwrap();

    let (_, near) = store.get_with_stats("arr[1]").unwrap();
    let (_, far) = store.get_with_stats("arr[499]").unwrap();
    assert!(near < 10, "靠前的元素只需少量兄弟扫描，实得 {near}");
    assert!(far > 400, "靠后的元素要扫过前面所有兄弟，实得 {far}");

    // 但两条路径**解码**的节点数一样（都只有目标子树），所以「分配量 == 目标大小」
    let small = store.subtree_bytes("arr[1]").unwrap().len();
    let big = store.subtree_bytes("arr[499]").unwrap().len();
    assert!(
        small.abs_diff(big) < 32,
        "两棵子树的字节数应接近（{small} vs {big}）"
    );
}

/// `text_of` must equal the canonical text of that subtree (and `text` of the whole doc).
/// `text_of` 必须等于该子树的 canonical 文本；`text` 是整份。
#[test]
fn text_of_is_the_subtree_canonical_text() {
    let f = amberice::dump( &cv(&wide_doc()) ).unwrap();
    let store = Store::open(&f).unwrap();

    let sub = store.get("arr[3]").unwrap();
    assert_eq!(store.text_of("arr[3]").unwrap(), sub.canonical_text().unwrap());

    let whole = store.text().unwrap();
    assert_eq!(whole, amberice::load(&f).unwrap().canonical_text().unwrap());
    assert!(whole.starts_with("arr: [") || whole.starts_with("head: "));
}

/// Opening verifies digests by default; `open_unverified` skips that (for `check` runs).
/// 默认打开会验摘要；`open_unverified` 不验（留给 `check` 那种要自己报错的场景）。
#[test]
fn open_verifies_digests_by_default() {
    let f = amberice::dump( &cv(&wide_doc()) ).unwrap();
    let info = amberice::info(&f).unwrap();
    let value_seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == amberice::SEG_VALUE)
        .unwrap();
    let mut bad = f.clone();
    bad[value_seg.offset as usize + 10] ^= 0x01;

    let e = Store::open(&bad).expect_err("段体被改，默认打开必须报错");
    assert_eq!(e.kind(), "digest-mismatch", "{e}");
    assert!(Store::open_unverified(&bad).is_ok(), "不验档应当能打开");
}

/// Path errors are reported precisely (missing key / index out of range / type mismatch).
/// 路径错误要说清是哪一段：缺键 / 下标越界 / 类型不对。
#[test]
fn path_errors_name_the_segment() {
    let f = amberice::dump( &cv(&wide_doc()) ).unwrap();
    let store = Store::open(&f).unwrap();

    let e = store.get("nope").unwrap_err();
    assert!(matches!(
        e,
        ContainerError::Canonical(amberice::CanonicalError::PathNotFound { .. })
    ));
    let e = store.get("arr[9999]").unwrap_err();
    assert!(e.to_string().contains("[9999]"), "{e}");
    let e = store.get("head.deeper").unwrap_err();
    assert!(e.to_string().contains("标量"), "{e}");
}

/// With layers: `get` sees patch overrides (top-level keys), `history_at` replays.
/// 有层时：`get` 看得到顶层补丁覆盖，`history_at` 能回放。
#[cfg(feature = "sign")]
#[test]
fn patches_are_visible_through_get() {
    use amberice::{HashAlgo, KeyPair};

    let f = amberice::dump( &cv(&wide_doc()) ).unwrap();
    let k = KeyPair::from_seed(&[9u8; 32]);
    let l0 = amberice::append_layer(&f, &k, None, None, HashAlgo::Sha256).unwrap();

    // 补丁把顶层键 head 换掉
    let mut p = Map::ordered();
    p.insert("head".to_string(), Value::Str("改过".into()));
    let patch = Value::Object(p);
    let l1 = amberice::append_layer(&l0, &k, Some(patch), None, HashAlgo::Sha256).unwrap();

    let store = Store::open(&l1).unwrap();
    assert_eq!(store.get("head").unwrap(), Value::Str("改过".into()));
    assert_eq!(
        store.get("arr[0].idx").unwrap(),
        Value::Int(0),
        "没被补丁碰到的键仍走零拷贝导航"
    );

    // 历史：第 0 层还没有补丁
    let h0 = store.history_at(0).unwrap();
    assert_eq!(at(&h0, "head"), Value::Str("根".into()));
}

/// 测试用：把值断言成 `CanonicalValue`（无序在测试里就该 panic，而不是静默通过）。
fn cv(v: &amberice::Value) -> amberice::CanonicalValue {
    amberice::CanonicalValue::try_from(v).expect("测试值必须是有序构造的")
}
