#![no_main]
//! L2 层链 / 签名模糊：任何「读得出来」的文件都要能被**签名、验证、压缩历史**。
//!
//! 为什么单独一个 target（HANDOFF §4.3 第 6 位）：`verify` / `append_layer` 是安全核心，
//! 但它的输入绝大多数是「被改过的合法文件」—— 只靠 `amberice_load` 的只读断言碰不到写侧。
//! 这里以**固定密钥**为轴：能读的文件 ⇒ 签一层 ⇒ 必须验通过 ⇒ 值不变 ⇒ `compact` 后仍不变。
//!
//! 断言：
//! 1. 自己 `append_layer` 的结果 `verify` 必须 `ok = true`（签出来的东西自己验不过就是 bug）；
//! 2. 追加「无补丁」层不改变当前值；追加补丁层只改补丁里那个键；
//! 3. `append_layer_checked` 的乐观并发：链头对 ⇒ 通过；链头错 ⇒ `AppendConflict`；
//! 4. `compact` 之后值不变、链长回到 1；
//! 5. 任意字节喂 `verify` / `value_at` 都不许 panic（错误一律分类）。
mod common;

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use amberice::{Delegation, HashAlgo, KeyPair, Map, SignError, Value};

/// 固定密钥：层里存的是「谁签的」，所以 fuzz 必须能重现同一把钥匙。
fn key() -> &'static KeyPair {
    static KEY: OnceLock<KeyPair> = OnceLock::new();
    KEY.get_or_init(|| KeyPair::from_seed(&[0x5Au8; 32]))
}

/// 固定补丁：`port: 9090`（有序对象才能进层体）。
fn patch() -> Value {
    let mut m = Map::ordered();
    m.insert("port".to_string(), Value::Int(9090));
    Value::Object(m)
}

/// `verify` 的错误分类：`SignError` 的 `Display` 不许为空（没有 `kind()`，只能这样兜）。
fn assert_sign_error(e: &SignError) {
    assert!(!e.to_string().is_empty(), "签名错误文案为空：{e:?}");
}

fuzz_target!(|data: &[u8]| {
    if data.len() > common::MAX_INPUT {
        return;
    }
    let len = data.len();
    let quota = common::decompression_allowance(data);
    let kp = key();

    // —— 任意字节：只读入口不许 panic ——
    let base = common::check_call("load", len, quota, || amberice::load(data));

    let Ok(base_value) = base else {
        // 读不出来的文件：写侧只剩「拒绝」这一条路，仍然要分类。
        // （读不出来 ≠ 签不了：缺能力位/缺密钥是**读**的拒绝，不该替写侧立规矩。）
        if let Err(e) = amberice::append_layer(data, kp, None, None, HashAlgo::Sha256) {
            assert_sign_error(&e);
        }
        let _ = amberice::verify(data, HashAlgo::Sha256);
        let _ = amberice::layer::value_at(data, &Value::Null, 0);
        return;
    };

    // 追加后整链该不该通过，取决于**已有历史**：
    // 末层作者不是我时，D16 会（正确地）判「换了作者但没有 delegation」⇒ `ok = false`。
    // 所以「自己签的必须自己验得过」这条无条件判据挂在 `compact` 上（见第 5 步），
    // 这里只判「链长 +1」，以及在前提成立时判整链通过。
    let prior = amberice::layer::layers(data).unwrap_or_default();
    let prior_len = prior.len();
    let same_author = prior
        .last()
        .map(|l| l.author == kp.public_bytes())
        .unwrap_or(true);
    let chain_should_ok = same_author
        && amberice::verify(data, HashAlgo::Sha256)
            .map(|r| r.ok)
            .unwrap_or(false);

    let (_, _, bytes) = common::count_allocs(|| {
        // —— 无补丁层：值必须一字不变 ——
        let signed = match amberice::append_layer(data, kp, None, None, HashAlgo::Sha256) {
            Ok(v) => v,
            Err(e) => {
                assert_sign_error(&e);
                return;
            }
        };

        // 1. 追加后链长必须 +1；前提成立时整链必须通过。
        match amberice::verify(&signed, HashAlgo::Sha256) {
            Ok(r) => {
                assert_eq!(r.layers, prior_len + 1, "追加一层后链长必须是 {}，实得 {:?}", prior_len + 1, r.lines);
                if !same_author {
                    // D16：换作者必须有 delegation。我用自己的钥匙接在别人后面，
                    // 又没有授权 ⇒ 必须判不通过（这条是链安全的核心否定判据）。
                    assert!(
                        !r.ok,
                        "换作者却没有 delegation，居然验过了：{:?}",
                        r.lines
                    );
                } else if chain_should_ok {
                    assert!(r.ok, "链本可通过且作者未变，追加后却未通过：{:?}", r.lines);
                }
            }
            Err(e) => panic!("自己签出的文件 verify 直接报错：{e}"),
        }

        // 2. 无补丁 ⇒ 当前值不变。
        match amberice::load(&signed) {
            Ok(v) => assert!(v.eq_ordered(&base_value), "无补丁层改变了当前值"),
            Err(e) => panic!("签完名反而读不出来了：{e}"),
        }

        // 3. 乐观并发：带对链头通过、带错链头冲突。
        let head = amberice::layer::chain_head(&signed, HashAlgo::Sha256)
            .expect("读链头失败")
            .expect("签过名就一定有链头");
        match amberice::append_layer_checked(&signed, kp, None, None, HashAlgo::Sha256, &head) {
            Ok(v) => {
                let r = amberice::verify(&v, HashAlgo::Sha256).expect("verify 报错");
                assert_eq!(r.layers, prior_len + 2, "再追加一层后链长必须是 {}：{:?}", prior_len + 2, r.lines);
                if !same_author {
                    assert!(!r.ok, "上游已因换作者而断链，再追加一层不可能变通过：{:?}", r.lines);
                } else if chain_should_ok {
                    assert!(r.ok, "两层链验不过：{:?}", r.lines);
                }
            }
            Err(e) => panic!("链头对得上却拒绝追加：{e}"),
        }
        let mut stale = head;
        stale[0] ^= 0xFF;
        match amberice::append_layer_checked(&signed, kp, None, None, HashAlgo::Sha256, &stale) {
            Err(SignError::AppendConflict { .. }) => {}
            other => panic!("链头不符应报 AppendConflict，实得 {other:?}"),
        }

        // 4. 补丁层：只有补丁里那个键变。
        match amberice::append_layer(data, kp, Some(patch()), None, HashAlgo::Sha256) {
            Ok(patched) => {
                match amberice::load(&patched) {
                    Ok(v) => {
                        let mut expect = base_value.clone();
                        if let Value::Object(m) = &mut expect {
                            m.insert("port".to_string(), Value::Int(9090));
                        }
                        assert!(v.eq_ordered(&expect), "补丁层的当前值与预期不符");
                        // 历史回放：从**基文档**重放到我这一层（索引 `prior_len`）必须等于当前值。
                        // 注意不能用 `base_value`（那已经是叠加了历史补丁的当前值）——输入本身
                        // 可能就带着别作者的层。
                        match amberice::base_value(data) {
                            Ok(base_doc) => match amberice::layer::value_at(&patched, &base_doc, prior_len) {
                                Ok(v0) => assert!(
                                    v0.eq_ordered(&v),
                                    "历史回放到最后一层必须等于当前值"
                                ),
                                Err(e) => panic!("历史回放失败：{e}"),
                            },
                            Err(e) => common::assert_container_error(&e),
                        }
                    }
                    Err(e) => panic!("补丁层读不出来：{e}"),
                }
            }
            Err(e) => assert_sign_error(&e),
        }

        // 5. compact：丢弃历史、重签一层，值不变。
        match amberice::compact(&signed, kp, HashAlgo::Sha256) {
            Ok(c) => {
                match amberice::load(&c) {
                    Ok(v) => assert!(v.eq_ordered(&base_value), "compact 改变了当前值"),
                    Err(e) => panic!("compact 产物读不出来：{e}"),
                }
                let r = amberice::verify(&c, HashAlgo::Sha256).expect("verify 报错");
                assert!(r.ok && r.layers == 1, "compact 后应是 1 层：{:?}", r.lines);
            }
            Err(e) => assert_sign_error(&e),
        }

        // 6. 伪造的换钥授权：写进去可以不拒，但**不许 panic**。
        // 注意这里不能断言 `ok = false`：作者没变时 delegation 根本不参与判定，
        // 真正该被拒的是上面那条「换作者却无授权」。
        let bogus = Delegation {
            from: [0u8; 32],
            max_index: 1_000,
            sig: vec![0u8; 64],
        };
        if let Err(e) = amberice::append_layer(&signed, kp, None, Some(bogus), HashAlgo::Sha256) {
            assert_sign_error(&e);
        }
    });

    common::assert_within_budget("amberice_layer", len, bytes);
});
