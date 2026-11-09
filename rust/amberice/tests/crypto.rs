// SPDX-License-Identifier: MulanPSL-2.0
//! T5 encryption tests (§19.13) / T5 加密验收。
#![cfg(all(feature = "preserve_order", feature = "crypto"))]

#[allow(unused_imports)]
#[allow(unused_imports)]
use amberice::{ContainerError, DumpOptions, KdfParams, KeySource, Map, Store, Value};

fn doc() -> Value {
    let mut cfg = Map::ordered();
    cfg.insert("port".to_string(), Value::Int(8080));
    cfg.insert("name".to_string(), Value::Str("demo".into()));
    let mut m = Map::ordered();
    m.insert("config".to_string(), Value::Object(cfg));
    Value::Object(m)
}

fn master(seed: u8) -> [u8; 32] {
    [seed; 32]
}

/// 加密往返：同一把钥读得回来，`check` 不需要钥就能判结构。
#[test]
fn encrypted_round_trip() {
    let key = master(1);
    let f = amberice::dump_encrypted( &cv(&doc()) ,
        &DumpOptions::default(),
        &KeySource::Bytes(&key),
    )
    .unwrap();

    // 明文段不该出现：VALUE/STRTAB 都声明了 enc
    let info = amberice::info(&f).unwrap();
    let value_seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == amberice::container::SEG_VALUE)
        .unwrap();
    assert_eq!(value_seg.header.enc, amberice::crypto::ENC_AES256_GCM);
    // 加密文件强制 4 KiB 对齐
    assert_eq!(info.align, 4096);

    assert!(
        amberice::load_encrypted(&f, &KeySource::Bytes(&key))
            .unwrap()
            .eq_ordered(&doc())
    );

    // 不带钥 ⇒ 明确要钥，而不是读出错值
    assert_eq!(
        amberice::load(&f).unwrap_err().kind(),
        "key-required",
        "没给钥必须报 key-required"
    );

    let report = amberice::check(&f).unwrap();
    assert!(report.ok, "{:?}", report.lines);
    let text = report.lines.join("\n");
    assert!(text.contains("加密"), "{text}");
    assert!(text.contains("未解密"), "{text}");
}

/// 错钥：认证失败（而不是「文件坏了」）。
#[test]
fn wrong_key_is_auth_failed() {
    let key = master(1);
    let f = amberice::dump_encrypted( &cv(&doc()) , &DumpOptions::default(), &KeySource::Bytes(&key)).unwrap();

    let e = amberice::load_encrypted(&f, &KeySource::Bytes(&master(2))).unwrap_err();
    assert_eq!(e.kind(), "auth-failed", "{e}");
    assert!(e.to_string().contains("第 0 块"), "{e}");
}

/// 改密文一个字节 ⇒ 认证失败。
#[test]
fn tampered_ciphertext_is_auth_failed() {
    let key = master(3);
    let mut f = amberice::dump_encrypted( &cv(&doc()) , &DumpOptions::default(), &KeySource::Bytes(&key)).unwrap();
    let info = amberice::info(&f).unwrap();
    let seg = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == amberice::container::SEG_VALUE)
        .unwrap();
    let at = seg.offset as usize + amberice::crypto::CRYPTO_HEADER + 5;
    f[at] ^= 0x01;

    // 摘要先响（段体被改），也接受 auth-failed —— 两者都证明「改不动」
    match amberice::load_encrypted(&f, &KeySource::Bytes(&key)) {
        Err(e) => assert!(
            matches!(e.kind(), "digest-mismatch" | "auth-failed"),
            "kind={} {e}",
            e.kind()
        ),
        Ok(_) => panic!("改了一个字节竟然还能读出来"),
    }
}

/// **改段头也读不出来** —— 段头字段进了 AAD，这是 §19.8 缺口在加密文件上的补丁。
#[test]
fn tampered_segment_header_is_caught() {
    let key = master(4);
    let mut f = amberice::dump_encrypted( &cv(&doc()) , &DumpOptions::default(), &KeySource::Bytes(&key)).unwrap();
    let info = amberice::info(&f).unwrap();
    let idx = info
        .segments
        .iter()
        .position(|s| s.header.seg_id == amberice::container::SEG_VALUE)
        .unwrap();
    // 段头第 2 字节是 `comp`：0 → 1（zstd）。摘要不覆盖段头，但 AAD 覆盖。
    let at = info.table_off as usize + idx * amberice::container::SEG_HEADER_LEN + 1;
    f[at] = 1;

    let e = amberice::load_encrypted(&f, &KeySource::Bytes(&key)).unwrap_err();
    assert_eq!(e.kind(), "auth-failed", "改段头必须认证失败：{e}");
}

/// 口令：写要参数、读听文件的。
#[test]
fn passphrase_round_trip() {
    let pass: &[u8] = b"correct horse battery staple";
    let f = amberice::dump_encrypted( &cv(&doc()) ,
        &DumpOptions::default(),
        &KeySource::Passphrase {
            secret: pass,
            kdf: Some(KdfParams::argon2id_default()),
        },
    )
    .unwrap();

    // 读：不给参数，全部取自 manifest
    let v = amberice::load_encrypted(
        &f,
        &KeySource::Passphrase {
            secret: pass,
            kdf: None,
        },
    )
    .unwrap();
    assert!(v.eq_ordered(&doc()));

    // 口令错 ⇒ 认证失败
    let e = amberice::load_encrypted(
        &f,
        &KeySource::Passphrase {
            secret: b"wrong",
            kdf: None,
        },
    )
    .unwrap_err();
    assert_eq!(e.kind(), "auth-failed");
}

/// 写口令但没给参数 ⇒ `kdf-required`；读时参数不符 ⇒ `kdf-mismatch`。
#[test]
fn kdf_parameter_mistakes_are_explicit() {
    let pass: &[u8] = b"pw";
    let e = amberice::dump_encrypted( &cv(&doc()) ,
        &DumpOptions::default(),
        &KeySource::Passphrase {
            secret: pass,
            kdf: None,
        },
    )
    .unwrap_err();
    assert_eq!(e.kind(), "kdf-required", "{e}");

    let f = amberice::dump_encrypted( &cv(&doc()) ,
        &DumpOptions::default(),
        &KeySource::Passphrase {
            secret: pass,
            kdf: Some(KdfParams {
                m_cost: 8,
                t_cost: 1,
                p_cost: 1,
            }),
        },
    )
    .unwrap();

    let e = amberice::load_encrypted(
        &f,
        &KeySource::Passphrase {
            secret: pass,
            kdf: Some(KdfParams::argon2id_default()),
        },
    )
    .unwrap_err();
    assert_eq!(e.kind(), "kdf-mismatch", "{e}");
}

/// 零拷贝随机访问对加密文件明说不行；`get_encrypted` 走整段解密。
#[test]
fn store_refuses_encrypted_files() {
    let key = master(5);
    let f = amberice::dump_encrypted( &cv(&doc()) , &DumpOptions::default(), &KeySource::Bytes(&key)).unwrap();

    let e = Store::open(&f).expect_err("加密文件不能零拷贝");
    assert_eq!(e.kind(), "encrypted-not-zero-copy", "{e}");

    let port = amberice::get_encrypted(&f, "config.port", &KeySource::Bytes(&key)).unwrap();
    assert_eq!(port, Value::Int(8080));
}

/// 两种钥混用要报错，不能悄悄算出一堆垃圾。
#[test]
fn mixing_key_kinds_is_refused() {
    let key = master(6);
    let plain_key_file =
        amberice::dump_encrypted( &cv(&doc()) , &DumpOptions::default(), &KeySource::Bytes(&key)).unwrap();

    let e = amberice::load_encrypted(
        &plain_key_file,
        &KeySource::Passphrase {
            secret: b"pw",
            kdf: None,
        },
    )
    .unwrap_err();
    assert!(e.to_string().contains("原始密钥"), "{e}");

    let pass_file = amberice::dump_encrypted( &cv(&doc()) ,
        &DumpOptions::default(),
        &KeySource::Passphrase {
            secret: b"pw",
            kdf: Some(KdfParams {
                m_cost: 8,
                t_cost: 1,
                p_cost: 1,
            }),
        },
    )
    .unwrap();
    let e = amberice::load_encrypted(&pass_file, &KeySource::Bytes(&key)).unwrap_err();
    assert!(e.to_string().contains("口令派生"), "{e}");
}

/// 加密文件仍报 `PrimaryNotValue` 之外的正常错误面：manifest 永远明文。
#[test]
fn manifest_stays_plaintext() {
    let key = master(7);
    let f = amberice::dump_encrypted( &cv(&doc()) , &DumpOptions::default(), &KeySource::Bytes(&key)).unwrap();
    let info = amberice::info(&f).unwrap();
    let manifest = info
        .segments
        .iter()
        .find(|s| s.header.seg_id == amberice::container::SEG_MANIFEST)
        .unwrap();
    assert_eq!(manifest.header.enc, 0, "manifest 必须明文（KDF 参数在里面）");
    // 不带钥也能读出 manifest 的字段
    let m = amberice::container::manifest_of(&f).unwrap();
    assert_eq!(m.crypto_enc, amberice::crypto::ENC_AES256_GCM);
    assert_eq!(m.kdf_kind, amberice::crypto::KDF_NONE);
    assert!(m.file_salt.is_some());
}

/// 没开 `crypto` 档的构建必须明确拒绝，而不是写出读不回来的文件。
#[cfg(not(feature = "crypto"))]
#[test]
fn missing_feature_is_reported() {
    let key = master(8);
    let e: ContainerError =
        amberice::dump_encrypted( &cv(&doc()) , &DumpOptions::default(), &KeySource::Bytes(&key)).unwrap_err();
    assert_eq!(e.kind(), "crypto");
}

/// 测试用：把值断言成 `CanonicalValue`（无序在测试里就该 panic，而不是静默通过）。
fn cv(v: &amberice::Value) -> amberice::CanonicalValue {
    amberice::CanonicalValue::try_from(v).expect("测试值必须是有序构造的")
}
