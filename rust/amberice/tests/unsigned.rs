// SPDX-License-Identifier: MulanPSL-2.0
//! D53：无签名追加层 —— 链结构照常，`verify` 仍会明确说「无签名」。

use amberice::{append_unsigned, dump, layers, load, CanonicalValue, HashAlgo, Map, Value};

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::ordered();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}

fn base() -> Vec<u8> {
    let v = obj(&[
        ("host", Value::Str("a.example".into())),
        ("port", Value::Int(8080)),
    ]);
    dump(&CanonicalValue::try_from(&v).expect("ordered")).expect("dump")
}

#[test]
fn unsigned_layer_links_and_merges() {
    let b = base();
    let patch = obj(&[("port", Value::Int(9090))]);
    let one = append_unsigned(&b, Some(patch), HashAlgo::Sha256).expect("append");

    // 旧字节逐字节保留（不变量 3）
    assert_eq!(&one[..b.len()], &b[..], "追加必须只写 EOF 之后");

    let ls = layers(&one).expect("layers");
    assert_eq!(ls.len(), 1);
    assert_eq!(ls[0].index, 0);
    assert_eq!(ls[0].author, [0u8; 32], "无签名层的作者必须是全 0");
    assert!(ls[0].sigs.is_empty(), "无签名层没有 sig");
    assert_eq!(ls[0].prev_hash, [0u8; 32]);

    let v = load(&one).expect("load");
    let Value::Object(m) = &v else { panic!("root") };
    assert_eq!(m.get("port"), Some(&Value::Int(9090)));
    assert_eq!(
        m.get("host"),
        Some(&Value::Str("a.example".into()))
    );

    // 第二层：链指针连到第一层
    let two = append_unsigned(&one, Some(obj(&[("port", Value::Int(9091))])), HashAlgo::Sha256)
        .expect("append 2");
    let ls2 = layers(&two).expect("layers 2");
    assert_eq!(ls2.len(), 2);
    assert_eq!(ls2[1].index, 1);
    assert_ne!(ls2[1].prev_hash, [0u8; 32], "第二层必须指向第一层");
    let v2 = load(&two).expect("load 2");
    let Value::Object(m2) = &v2 else { panic!("root") };
    assert_eq!(m2.get("port"), Some(&Value::Int(9091)));
}

/// D53：`verify --allow-unsigned` 只把「没有签名」从失败改成明确接受，
/// 其余检查（Trailer / 链指针 / 已签名层）一律照旧。
#[cfg(feature = "sign")]
#[test]
fn verify_allows_unsigned_only_when_explicit() {
    let b = base();
    let one = append_unsigned(&b, Some(obj(&[("port", Value::Int(9090))])), HashAlgo::Sha256)
        .expect("append");

    let strict = amberice::verify(&one, HashAlgo::Sha256).expect("verify");
    assert!(!strict.ok, "默认必须把无签名层判为失败");
    assert!(strict.lines.iter().any(|l| l.contains("没有签名")));

    let allowed =
        amberice::verify_with(&one, HashAlgo::Sha256, true).expect("verify allow");
    assert!(allowed.ok, "{:?}", allowed.lines);
    assert!(allowed.lines.iter().any(|l| l.contains("--allow-unsigned")));

    // 允许无签名 ≠ 允许被改：Trailer head 仍要严丝合缝。
    let mut broken = one.clone();
    // Trailer 末尾 12 B 是保留位（不参与校验）；改 head_hash 的最后一个字节。
    let last = broken.len() - 20;
    broken[last] ^= 0xff;
    let report =
        amberice::verify_with(&broken, HashAlgo::Sha256, true).expect("verify broken");
    assert!(!report.ok, "链头被改必须失败：{:?}", report.lines);
}

#[test]
fn unsigned_layer_without_patch_is_allowed() {
    let b = base();
    let one = append_unsigned(&b, None, HashAlgo::Sha256).expect("append");
    let ls = layers(&one).expect("layers");
    assert_eq!(ls.len(), 1);
    assert!(ls[0].patch.is_none());
    assert_eq!(load(&one).unwrap().canonical_text().unwrap(), load(&b).unwrap().canonical_text().unwrap());
}
