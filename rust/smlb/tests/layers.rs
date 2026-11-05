// SPDX-License-Identifier: MulanPSL-2.0
//! Layer chain + signature tests (T4) / 层链与签名验收（T4）。
//!
//! 门控：`preserve_order`（没有源序就没有 canonical 字节可签）+ `sign`（Ed25519 实现）。
//! Gated on `preserve_order` + `sign`: no canonical bytes without the former, no crypto
//! without the latter.
#![cfg(all(feature = "preserve_order", feature = "sign"))]

use smlb::layer::{self, Delegation};
use smlb::{ContainerError, HashAlgo, KeyPair, Map, Trailer, Value};

fn base_doc() -> Value {
    let mut cfg = Map::new();
    cfg.insert("port".to_string(), Value::Int(8080));
    cfg.insert("name".to_string(), Value::Str("demo".into()));
    Value::Object({
        let mut m = Map::new();
        m.insert("config".to_string(), Value::Object(cfg));
        m
    })
}

fn patch(port: i64) -> Value {
    let mut cfg = Map::new();
    cfg.insert("port".to_string(), Value::Int(port));
    Value::Object({
        let mut m = Map::new();
        m.insert("config".to_string(), Value::Object(cfg));
        m
    })
}

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(&[seed; 32])
}

/// 签一层 → 验签通过 → 当前值不变。
#[test]
fn sign_then_verify_passes() {
    let f = smlb::dump(&base_doc()).unwrap();
    let signed = smlb::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();

    let report = smlb::verify(&signed, HashAlgo::Sha256).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    assert_eq!(report.layers, 1);

    assert!(smlb::load(&signed).unwrap().eq_ordered(&base_doc()));

    // 追加不改旧字节：新文件的前缀必须逐字节等于原文件
    assert_eq!(&signed[..f.len()], &f[..]);
}

/// 追加补丁 ⇒ 当前值变、历史值不变（P2-a「值来自哪一层」）。
#[test]
fn append_patch_changes_current_value_but_not_history() {
    let f = smlb::dump(&base_doc()).unwrap();
    let k = key(1);
    let l0 = smlb::append_layer(&f, &k, None, None, HashAlgo::Sha256).unwrap();
    let l1 = smlb::append_layer(&l0, &k, Some(patch(9090)), None, HashAlgo::Sha256).unwrap();

    let report = smlb::verify(&l1, HashAlgo::Sha256).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    assert_eq!(report.layers, 2);

    // 当前值：port = 9090
    let cur = smlb::get(&l1, "config.port").unwrap();
    assert_eq!(cur, Value::Int(9090));

    // 历史：第 0 层（只有第 0 层的补丁 = 无补丁）→ 8080；第 1 层 → 9090
    let base = smlb::base_value(&l1).unwrap();
    assert_eq!(
        layer::history_value_at(&l1, &base, 0, "config.port").unwrap(),
        Value::Int(8080)
    );
    assert_eq!(
        layer::history_value_at(&l1, &base, 1, "config.port").unwrap(),
        Value::Int(9090)
    );
    assert_eq!(
        layer::history_value_at(&l1, &base, 2, "config.port").unwrap(),
        Value::Int(9090),
        "层号超过链长时按链尾算"
    );

    // 位置语义：`config` 键仍在原位（原位覆盖，D22）
    let cur = smlb::load(&l1).unwrap();
    let keys: Vec<String> = cur
        .as_object()
        .expect("应为对象")
        .keys()
        .map(|k| k.to_string())
        .collect();
    assert_eq!(keys, vec!["config".to_string()]);
}

/// 改层体里一个字节 ⇒ 验签必须失败（签名 + 链两条都会响）。
#[test]
fn tampered_layer_is_detected() {
    let f = smlb::dump(&base_doc()).unwrap();
    let mut signed = smlb::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();
    let t = Trailer::parse(&signed).expect("应有 Trailer");
    let body_at = t.last_layer_off as usize + smlb::LAYER_HEADER_LEN;
    signed[body_at + 2] ^= 0x01; // 层体里翻一位

    let report = smlb::verify(&signed, HashAlgo::Sha256).unwrap();
    assert!(!report.ok, "{:?}", report.lines);
    let text = report.lines.join("\n");
    // 报错形式取决于翻在哪一位：可能是「签名验不过」，也可能是层体直接解不开
    assert!(
        text.contains("验不过") || text.contains("不符") || text.contains("读不出来"),
        "{text}"
    );
}

/// 换钥必须有前一位作者的授权（D16），且层号不得超过 max_index。
#[test]
fn delegation_is_required_for_key_change() {
    let f = smlb::dump(&base_doc()).unwrap();
    let a = key(1);
    let b = key(2);
    let l0 = smlb::append_layer(&f, &a, None, None, HashAlgo::Sha256).unwrap();

    // 没有 delegation 直接换钥 ⇒ 拒
    let no_deleg = smlb::append_layer(&l0, &b, None, None, HashAlgo::Sha256).unwrap();
    let r = smlb::verify(&no_deleg, HashAlgo::Sha256).unwrap();
    assert!(!r.ok);
    assert!(r.lines.join("\n").contains("没有 delegation"), "{:?}", r.lines);

    // 有 delegation（A → B，覆盖到第 1 层）⇒ 通过
    let d: Delegation = a.delegate(&b.public_bytes(), 1);
    let with_deleg = smlb::append_layer(&l0, &b, None, Some(d.clone()), HashAlgo::Sha256).unwrap();
    let r2 = smlb::verify(&with_deleg, HashAlgo::Sha256).unwrap();
    assert!(r2.ok, "{:?}", r2.lines);
    assert!(r2.lines.join("\n").contains("delegation 有效"), "{:?}", r2.lines);

    // 层号超过 max_index ⇒ 拒
    let d_tight = a.delegate(&b.public_bytes(), 0); // 只覆盖第 0 层，但这次是第 1 层
    let too_late = smlb::append_layer(&l0, &b, None, Some(d_tight), HashAlgo::Sha256).unwrap();
    let r3 = smlb::verify(&too_late, HashAlgo::Sha256).unwrap();
    assert!(!r3.ok);
    assert!(r3.lines.join("\n").contains("超出 delegation"), "{:?}", r3.lines);
}

/// Trailer 被改（层数对不上）⇒ 层链读不出来，而不是静默少读。
#[test]
fn trailer_count_mismatch_is_rejected() {
    let f = smlb::dump(&base_doc()).unwrap();
    let mut signed = smlb::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();
    let n = signed.len();
    // Trailer: magic(8) + count(4) + last_off(8) + head(32) → count 在倒数第 56 字节
    let count_at = n - smlb::TRAILER_LEN + 8;
    signed[count_at] = 5; // 声称 5 层

    let e = layer::layers(&signed).expect_err("层数对不上必须报错");
    assert!(
        format!("{e}").contains("层"),
        "错误信息应点明层数：{e}"
    );
}

/// `smlb check` 会把层链结构也算进结论（不需要密钥那部分）。
#[test]
fn check_reports_the_chain() {
    let f = smlb::dump(&base_doc()).unwrap();
    let signed = smlb::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();
    let report = smlb::check(&signed).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    let text = report.lines.join("\n");
    assert!(text.contains("Trailer   head_hash OK"), "{text}");
}

/// ⚠️ 已知边界：**整条层链可被剥离**（没有外部锚点就无法察觉）。
/// 这条用例把「现状如此」钉住 —— 别以为有了签名就能防住「谁把签名删了」。
#[test]
fn stripping_the_trailer_silently_unsigned() {
    let f = smlb::dump(&base_doc()).unwrap();
    let signed = smlb::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();
    let stripped = &signed[..f.len()];
    assert!(layer::layers(stripped).unwrap().is_empty());
    let r = smlb::verify(stripped, HashAlgo::Sha256).unwrap();
    assert!(r.ok && r.layers == 0, "剥离后看起来是「未签名文件」");
    // 唯一能发现它的办法是**外部**记住 head_hash（D18：信任根交给调用者）
    assert_ne!(Trailer::parse(&signed).unwrap().head_hash, [0u8; 32]);
}

/// 密钥：种子 → 公钥稳定；签名可复现（同一层同一 key 两次签出同样的字节）。
#[test]
fn keys_and_signatures_are_deterministic() {
    let k1 = KeyPair::from_seed_hex(&key(7).seed_hex()).unwrap();
    assert_eq!(k1.public_hex(), key(7).public_hex());

    let f = smlb::dump(&base_doc()).unwrap();
    let a = smlb::append_layer(&f, &key(7), None, None, HashAlgo::Sha256).unwrap();
    let b = smlb::append_layer(&f, &key(7), None, None, HashAlgo::Sha256).unwrap();
    assert_eq!(a, b, "签名是确定性的（Ed25519 无随机数）");
}

/// 归档文件签名后仍是归档（`load` 给 PrimaryNotValue，载荷照旧取得出来）。
#[test]
fn archives_can_be_signed_too() {
    let blobs: [(&str, &[u8]); 1] = [("a.txt", b"hello")];
    let arc = smlb::pack(&blobs).unwrap();
    let signed = smlb::append_layer(&arc, &key(3), None, None, HashAlgo::Sha256).unwrap();
    let r = smlb::verify(&signed, HashAlgo::Sha256).unwrap();
    assert!(r.ok, "{:?}", r.lines);
    assert_eq!(&*smlb::payload(&signed, "a.txt").unwrap(), b"hello");
    assert!(matches!(
        smlb::load(&signed).unwrap_err(),
        ContainerError::PrimaryNotValue { .. }
    ));
}
