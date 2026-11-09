// SPDX-License-Identifier: MulanPSL-2.0
//! Layer chain + signature tests (T4) / 层链与签名验收（T4）。
//!
//! 门控：`preserve_order`（没有源序就没有 canonical 字节可签）+ `sign`（Ed25519 实现）。
//! Gated on `preserve_order` + `sign`: no canonical bytes without the former, no crypto
//! without the latter.
#![cfg(all(feature = "preserve_order", feature = "sign"))]

use amberice::layer::{self, Delegation, Trailer};
use amberice::{ContainerError, HashAlgo, KeyPair, Map, SignError, Value};

fn base_doc() -> Value {
    let mut cfg = Map::ordered();
    cfg.insert("port".to_string(), Value::Int(8080));
    cfg.insert("name".to_string(), Value::Str("demo".into()));
    Value::Object({
        let mut m = Map::ordered();
        m.insert("config".to_string(), Value::Object(cfg));
        m
    })
}

fn patch(port: i64) -> Value {
    let mut cfg = Map::ordered();
    cfg.insert("port".to_string(), Value::Int(port));
    Value::Object({
        let mut m = Map::ordered();
        m.insert("config".to_string(), Value::Object(cfg));
        m
    })
}

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(&[seed; 32])
}

/// `--trust`：链头被钉住 ⇒ 换掉整条链会被发现。
/// Pinning the head catches a wholesale chain swap.
///
/// 这是 §21.1c 那条已知边界的**唯一**补法：没有外部锚点时，「本来没签」与「签了又被剥掉」
/// 在字节上一模一样；记住过 head，两件事就分开了。
#[test]
fn trust_pins_the_chain_head() {
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let signed = amberice::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();

    let head = layer::chain_head(&signed, HashAlgo::Sha256)
        .unwrap()
        .expect("签过名就一定有链头");

    // 对得上 ⇒ 通过
    let ok = amberice::verify_trusted(&signed, HashAlgo::Sha256, &head).unwrap();
    assert!(ok.ok, "{:?}", ok.lines);
    assert!(ok.lines.join("\n").contains("与期望一致"), "{:?}", ok.lines);

    // 期望值错了 ⇒ 不过是哪一条错，必须是「未通过」
    let wrong = [0xAAu8; 32];
    let bad = amberice::verify_trusted(&signed, HashAlgo::Sha256, &wrong).unwrap();
    assert!(!bad.ok, "{:?}", bad.lines);
    assert!(bad.lines.join("\n").contains("head 不符"), "{:?}", bad.lines);

    // 链头是**最后一层**的哈希：再追加一层，链头必须变
    let deeper =
        amberice::append_layer(&signed, &key(1), Some(patch(9090)), None, HashAlgo::Sha256).unwrap();
    let head2 = layer::chain_head(&deeper, HashAlgo::Sha256).unwrap().unwrap();
    assert_ne!(head, head2, "追加一层后链头必须变");
    let bad2 = amberice::verify_trusted(&deeper, HashAlgo::Sha256, &head).unwrap();
    assert!(!bad2.ok, "拿旧锚点验新文件必须不通过");
}

/// 整条层链被剥离（拿未签名文件冒充）⇒ 锚点必须把它抓出来。
/// Stripping every layer is caught by the anchor — and by nothing else.
#[test]
fn stripped_chain_is_caught_by_the_anchor() {
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let signed = amberice::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();
    let head = layer::chain_head(&signed, HashAlgo::Sha256).unwrap().unwrap();

    // 未签名的原文件：`verify` 自己会判「通过（0 层）」—— 这正是需要锚点的原因
    assert!(amberice::verify(&f, HashAlgo::Sha256).unwrap().ok);
    assert_eq!(layer::chain_head(&f, HashAlgo::Sha256).unwrap(), None);

    let anchored = amberice::verify_trusted(&f, HashAlgo::Sha256, &head).unwrap();
    assert!(!anchored.ok, "{:?}", anchored.lines);
    let text = anchored.lines.join("\n");
    assert!(text.contains("一层都没有"), "{text}");
    assert!(text.contains("剥离"), "{text}");
}

/// 甲方案的核心收益：**换掉基文档**会被验签当场发现，不必动层区一个字节。
/// The base document is bound into the signed chain (§21.1b E).
#[test]
fn swapping_the_base_document_is_detected() {
    let algo = HashAlgo::Sha256;
    let k = key(1);
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let v1 = amberice::append_layer(&f, &k, None, None, algo).unwrap();
    assert!(amberice::verify(&v1, algo).unwrap().ok);

    // 只改**基文档区**的一个字节（层记录、Trailer、签名一个都没动）
    let mut bad = v1.clone();
    bad[200] ^= 0x01;
    assert_ne!(bad, v1);

    let report = amberice::verify(&bad, algo).unwrap();
    assert!(!report.ok, "{:?}", report.lines);
    assert!(
        report.lines.join("\n").contains("base_hash"),
        "错误必须点名 base_hash：{:?}",
        report.lines
    );

    // 没有 `sign` feature 的结构校验也能抓到（链头只用到摘要）
    let chain = layer::verify_chain(&bad, algo).unwrap();
    assert!(!chain.ok, "{:?}", chain.lines);
    assert!(chain.lines.join("\n").contains("base_hash"), "{:?}", chain.lines);
}

/// compact 换了基文档 ⇒ 链头必须**变**（这正是甲方案修掉的那个反直觉现象）。
#[test]
fn compact_changes_the_head() {
    let algo = HashAlgo::Sha256;
    let k = key(1);
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let v1 = amberice::append_layer(&f, &k, None, None, algo).unwrap();
    let head1 = layer::chain_head(&v1, algo).unwrap().unwrap();
    let v2 = amberice::append_layer(&v1, &k, Some(patch(9090)), None, algo).unwrap();

    let merged = amberice::compact(&v2, &k, algo).unwrap();
    let head2 = layer::chain_head(&merged, algo).unwrap().unwrap();
    assert_ne!(head1, head2, "基文档变了，链头不能还是同一个");

    // 旧锚点因此正确失效（之前会误判通过）
    assert!(!amberice::verify_trusted(&merged, algo, &head1).unwrap().ok);
    assert!(amberice::verify_trusted(&merged, algo, &head2).unwrap().ok);
}

/// D20 写者锁：两个写者基于**同一个链头**工作，后者必须拿到明确冲突。
/// Optimistic writer lock: a stale head must fail loudly, not append silently.
#[test]
fn stale_head_gets_an_explicit_append_conflict() {
    let algo = HashAlgo::Sha256;
    let k = key(1);
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let v1 = amberice::append_layer(&f, &k, None, None, algo).unwrap();
    let head1 = layer::chain_head(&v1, algo).unwrap().unwrap();

    // A：基于 head1 追加 ⇒ 成功
    let v2 = amberice::append_layer_checked(&v1, &k, Some(patch(9090)), None, algo, &head1).unwrap();
    assert_eq!(layer::layers(&v2).unwrap().len(), 2);

    // B：也基于 head1（已经陈旧）⇒ 明确冲突，且消息点名两个链头
    let e = amberice::append_layer_checked(&v2, &k, Some(patch(1111)), None, algo, &head1)
        .unwrap_err();
    assert!(
        matches!(e, SignError::AppendConflict { .. }),
        "应报 AppendConflict，实得 {e:?}"
    );
    assert!(e.to_string().contains("追加冲突"), "{e}");

    // 未签名文件的「当前头」按约定是全 0
    let v0 = amberice::append_layer_checked(&f, &k, None, None, algo, &[0u8; 32]).unwrap();
    assert_eq!(layer::layers(&v0).unwrap().len(), 1);
    // 反之，拿非零期望去追加未签名件同样冲突
    assert!(matches!(
        amberice::append_layer_checked(&f, &k, None, None, algo, &head1).unwrap_err(),
        SignError::AppendConflict { .. }
    ));
}

/// D8 的例外：compact 合并多层、丢弃历史、**重签**。
/// `compact` rewrites the base, drops history, and must re-sign.
#[test]
fn compact_merges_history_and_resigns() {
    let algo = HashAlgo::Sha256;
    let k = key(1);
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let v1 = amberice::append_layer(&f, &k, None, None, algo).unwrap();
    let v2 = amberice::append_layer(&v1, &k, Some(patch(9090)), None, algo).unwrap();
    assert_eq!(layer::layers(&v2).unwrap().len(), 2);

    let before: Value = amberice::load(&v2).unwrap();
    let old_head = layer::chain_head(&v2, algo).unwrap().unwrap();

    let merged = amberice::compact(&v2, &k, algo).unwrap();

    // 值不变、只剩一层、链能通过
    assert!(amberice::load(&merged).unwrap().eq_ordered(&before));
    assert_eq!(layer::layers(&merged).unwrap().len(), 1);
    assert!(amberice::verify(&merged, algo).unwrap().ok);

    // 链条被换过 ⇒ 旧锚点必须失效（这是正确的，调用方要重建锚点）
    let new_head = layer::chain_head(&merged, algo).unwrap().unwrap();
    assert_ne!(new_head, old_head);
    assert!(!amberice::verify_trusted(&merged, algo, &old_head).unwrap().ok);
    assert!(amberice::verify_trusted(&merged, algo, &new_head).unwrap().ok);

    // 合并后仍可继续追加（带写者锁）
    let again = amberice::append_layer_checked(
        &merged,
        &k,
        Some(patch(1111)),
        None,
        algo,
        &new_head,
    )
    .unwrap();
    assert!(amberice::verify(&again, algo).unwrap().ok);
}

/// 签一层 → 验签通过 → 当前值不变。
#[test]
fn sign_then_verify_passes() {
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let signed = amberice::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();

    let report = amberice::verify(&signed, HashAlgo::Sha256).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    assert_eq!(report.layers, 1);

    assert!(amberice::load(&signed).unwrap().eq_ordered(&base_doc()));

    // 追加不改旧字节：新文件的前缀必须逐字节等于原文件
    assert_eq!(&signed[..f.len()], &f[..]);
}

/// 追加补丁 ⇒ 当前值变、历史值不变（P2-a「值来自哪一层」）。
#[test]
fn append_patch_changes_current_value_but_not_history() {
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let k = key(1);
    let l0 = amberice::append_layer(&f, &k, None, None, HashAlgo::Sha256).unwrap();
    let l1 = amberice::append_layer(&l0, &k, Some(patch(9090)), None, HashAlgo::Sha256).unwrap();

    let report = amberice::verify(&l1, HashAlgo::Sha256).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    assert_eq!(report.layers, 2);

    // 当前值：port = 9090
    let cur = amberice::get(&l1, "config.port").unwrap();
    assert_eq!(cur, Value::Int(9090));

    // 历史：第 0 层（只有第 0 层的补丁 = 无补丁）→ 8080；第 1 层 → 9090
    let base = amberice::base_value(&l1).unwrap();
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
    let cur = amberice::load(&l1).unwrap();
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
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let mut signed = amberice::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();
    let t = Trailer::parse(&signed).expect("应有 Trailer");
    let body_at = t.last_layer_off as usize + amberice::layer::LAYER_HEADER_LEN;
    signed[body_at + 2] ^= 0x01; // 层体里翻一位

    let report = amberice::verify(&signed, HashAlgo::Sha256).unwrap();
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
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let a = key(1);
    let b = key(2);
    let l0 = amberice::append_layer(&f, &a, None, None, HashAlgo::Sha256).unwrap();

    // 没有 delegation 直接换钥 ⇒ 拒
    let no_deleg = amberice::append_layer(&l0, &b, None, None, HashAlgo::Sha256).unwrap();
    let r = amberice::verify(&no_deleg, HashAlgo::Sha256).unwrap();
    assert!(!r.ok);
    assert!(r.lines.join("\n").contains("没有 delegation"), "{:?}", r.lines);

    // 有 delegation（A → B，覆盖到第 1 层）⇒ 通过
    let d: Delegation = a.delegate(&b.public_bytes(), 1);
    let with_deleg = amberice::append_layer(&l0, &b, None, Some(d.clone()), HashAlgo::Sha256).unwrap();
    let r2 = amberice::verify(&with_deleg, HashAlgo::Sha256).unwrap();
    assert!(r2.ok, "{:?}", r2.lines);
    assert!(r2.lines.join("\n").contains("delegation 有效"), "{:?}", r2.lines);

    // 层号超过 max_index ⇒ 拒
    let d_tight = a.delegate(&b.public_bytes(), 0); // 只覆盖第 0 层，但这次是第 1 层
    let too_late = amberice::append_layer(&l0, &b, None, Some(d_tight), HashAlgo::Sha256).unwrap();
    let r3 = amberice::verify(&too_late, HashAlgo::Sha256).unwrap();
    assert!(!r3.ok);
    assert!(r3.lines.join("\n").contains("超出 delegation"), "{:?}", r3.lines);
}

/// Trailer 被改（层数对不上）⇒ 层链读不出来，而不是静默少读。
#[test]
fn trailer_count_mismatch_is_rejected() {
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let mut signed = amberice::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();
    let n = signed.len();
    // Trailer: magic(8) + count(4) + last_off(8) + head(32) → count 在倒数第 56 字节
    let count_at = n - amberice::layer::TRAILER_LEN + 8;
    signed[count_at] = 5; // 声称 5 层

    let e = layer::layers(&signed).expect_err("层数对不上必须报错");
    assert!(
        format!("{e}").contains("层"),
        "错误信息应点明层数：{e}"
    );
}

/// `amberice check` 会把层链结构也算进结论（不需要密钥那部分）。
#[test]
fn check_reports_the_chain() {
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let signed = amberice::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();
    let report = amberice::check(&signed).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    let text = report.lines.join("\n");
    assert!(text.contains("Trailer   head_hash OK"), "{text}");
}

/// 已知边界：**整条层链可被剥离**（没有外部锚点就无法察觉）。
/// 这条用例把「现状如此」钉住 —— 别以为有了签名就能防住「谁把签名删了」。
#[test]
fn stripping_the_trailer_silently_unsigned() {
    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let signed = amberice::append_layer(&f, &key(1), None, None, HashAlgo::Sha256).unwrap();
    let stripped = &signed[..f.len()];
    assert!(layer::layers(stripped).unwrap().is_empty());
    let r = amberice::verify(stripped, HashAlgo::Sha256).unwrap();
    assert!(r.ok && r.layers == 0, "剥离后看起来是「未签名文件」");
    // 唯一能发现它的办法是**外部**记住 head_hash（D18：信任根交给调用者）
    assert_ne!(Trailer::parse(&signed).unwrap().head_hash, [0u8; 32]);
}

/// 密钥：种子 → 公钥稳定；签名可复现（同一层同一 key 两次签出同样的字节）。
#[test]
fn keys_and_signatures_are_deterministic() {
    let k1 = KeyPair::from_seed_hex(&key(7).seed_hex()).unwrap();
    assert_eq!(k1.public_hex(), key(7).public_hex());

    let f = amberice::dump( &cv(&base_doc()) ).unwrap();
    let a = amberice::append_layer(&f, &key(7), None, None, HashAlgo::Sha256).unwrap();
    let b = amberice::append_layer(&f, &key(7), None, None, HashAlgo::Sha256).unwrap();
    assert_eq!(a, b, "签名是确定性的（Ed25519 无随机数）");
}

/// 归档文件签名后仍是归档（`load` 给 PrimaryNotValue，载荷照旧取得出来）。
#[test]
fn archives_can_be_signed_too() {
    let blobs: [(&str, &[u8]); 1] = [("a.txt", b"hello")];
    let arc = amberice::pack(&blobs).unwrap();
    let signed = amberice::append_layer(&arc, &key(3), None, None, HashAlgo::Sha256).unwrap();
    let r = amberice::verify(&signed, HashAlgo::Sha256).unwrap();
    assert!(r.ok, "{:?}", r.lines);
    assert_eq!(&*amberice::payload(&signed, "a.txt").unwrap(), b"hello");
    assert!(matches!(
        amberice::load(&signed).unwrap_err(),
        ContainerError::PrimaryNotValue { .. }
    ));
}

/// 测试用：把值断言成 `CanonicalValue`（无序在测试里就该 panic，而不是静默通过）。
fn cv(v: &amberice::Value) -> amberice::CanonicalValue {
    amberice::CanonicalValue::try_from(v).expect("测试值必须是有序构造的")
}
