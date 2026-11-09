// SPDX-License-Identifier: MulanPSL-2.0
//! 敌意层链：**伪造的 Trailer** 不许把读者打崩 / 卡死 / 吃光内存。
//!
//! 这三条都是 fuzz 撞出来的（`rust/fuzz/fuzz_targets/amberice_load.rs`），修在
//! `layer::layers` / `layer::base_prefix_len`：
//!
//! 1. `last_layer_off = u64::MAX` —— 旧代码 `off + LAYER_HEADER_LEN` 在 debug 档 panic
//!    （release 档静默回绕，等于把边界检查绕过去）；
//! 2. `layer_count = u32::MAX` —— 旧代码按它 `Vec::with_capacity`，当场要 4G 项的分配；
//! 3. `prev_off` 成环 —— 旧代码是 `loop`，没有任何步数上限（死循环）。
//!
//! 本文件**不**门控 `sign`：读层链不需要密码学，坏 trailer 的拒绝也应当是读侧义务。
use amberice::layer::{self, Trailer};
use amberice::{Map, Value};

fn doc() -> Value {
    let mut m = Map::ordered();
    m.insert("port".to_string(), Value::Int(8080));
    Value::Object(m)
}

/// 在文件尾追加一个 Trailer（`.ambr` 的 Trailer 就在 EOF，读者只看最后 64 B）。
fn with_trailer(file: &[u8], t: Trailer) -> Vec<u8> {
    let mut out = file.to_vec();
    out.extend_from_slice(&t.to_bytes());
    out
}

#[test]
fn wild_last_layer_off_is_an_error_not_a_panic() {
    let f = amberice::dump(&amberice::CanonicalValue::try_from(&doc()).unwrap()).unwrap();
    let bad = with_trailer(
        &f,
        Trailer {
            layer_count: 1,
            last_layer_off: u64::MAX,
            head_hash: [0; 32],
        },
    );
    // debug 档下旧实现会在 `u64::MAX + 16` 处 panic（debug-assertions 开着时是加法溢出）。
    assert!(layer::layers(&bad).is_err());
    assert!(layer::base_prefix_len(&bad).is_err());
    // 整条读路径也不许崩：`load` 只是把层链当补丁来源，坏链要报错。
    assert!(amberice::load(&bad).is_err());
}

#[test]
fn absurd_layer_count_does_not_preallocate() {
    let f = amberice::dump(&amberice::CanonicalValue::try_from(&doc()).unwrap()).unwrap();
    let bad = with_trailer(
        &f,
        Trailer {
            layer_count: u32::MAX,
            last_layer_off: 0,
            head_hash: [0; 32],
        },
    );
    // 判据是「快速返回 Err」：旧实现先按 4G 项预分配，这里会直接 OOM/abort。
    let t0 = std::time::Instant::now();
    assert!(layer::layers(&bad).is_err());
    assert!(
        t0.elapsed().as_secs() < 5,
        "坏 layer_count 不该让读者长时间分配/遍历"
    );
}

#[test]
fn cyclic_prev_off_terminates() {
    // 自指环：最后一条记录的 `prev_off` 指向它自己。
    // 只验证「有上限、会终止」——具体错在哪一类不是本测试的契约。
    let f = amberice::dump(&amberice::CanonicalValue::try_from(&doc()).unwrap()).unwrap();
    let off = f.len() as u64;
    let mut out = f.clone();
    // 记录头：len:u32 ‖ prev_off:u64 ‖ reserved:u32，body 为空
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&off.to_le_bytes()); // 自指
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&Trailer {
        layer_count: 1,
        last_layer_off: off,
        head_hash: [0; 32],
    }
    .to_bytes());

    let t0 = std::time::Instant::now();
    // 空 body 不是合法层 ⇒ 立刻报 Canonical；关键是**不许死循环**。
    assert!(layer::layers(&out).is_err());
    assert!(layer::base_prefix_len(&out).is_err());
    assert!(t0.elapsed().as_secs() < 5, "自指 prev_off 导致未终止");
}
