// SPDX-License-Identifier: MulanPSL-2.0
//! 加密：原始密钥（也就是外部 KMS/secret manager 那条路）与口令派生。
//! Encryption: a raw 32 B key (the external-secret path) or a passphrase.
//!
//! `cargo run -p smlb --features crypto --example encrypted`
//! （只有 `crypto` 是必需的；键序走 `Map::ordered()`，与 feature 无关）

#[cfg(not(feature = "crypto"))]
fn main() {
    eprintln!("本例需要 `crypto`：");
    eprintln!("  cargo run -p smlb --features crypto --example encrypted");
}

#[cfg(feature = "crypto")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use smlb::{
        check, dump_encrypted, get_encrypted, info, load, load_encrypted, CanonicalValue,
        DumpOptions, KdfParams, KeySource, Map, Value,
    };

    let mut cfg = Map::ordered();
    cfg.insert("token".to_string(), Value::Str("s3cr3t".into()));
    let mut root = Map::ordered();
    root.insert("config".to_string(), Value::Object(cfg));
    let doc = Value::Object(root);

    // 1) 原始密钥：调用方从哪弄到这 32 B，格式一概不管（KMS / HSM / 环境变量都行）
    let master = [0x42u8; 32];
    let cv = CanonicalValue::try_from(doc.clone())?;
    let f = dump_encrypted(&cv, &DumpOptions::default(), &KeySource::Bytes(&master))?;

    // 加密文件强制 4 KiB 对齐；`MANIFEST` 保持明文（KDF 参数在里面）
    let meta = info(&f)?;
    println!("{} 段，对齐 {} B", meta.segments.len(), meta.align);

    // `check` 不需要钥匙：结构、摘要、算法、块数都能读
    assert!(check(&f)?.ok);

    // 对钥读得回来；错钥是**认证失败**，不是「文件坏了」
    assert!(load_encrypted(&f, &KeySource::Bytes(&master))?.eq_ordered(&doc));
    let e = load_encrypted(&f, &KeySource::Bytes(&[0u8; 32])).unwrap_err();
    assert_eq!(e.kind(), "auth-failed");
    println!("错钥：{e}");

    // 取单个字段也走带钥路径（按块解密，只解要用的那些块）
    assert_eq!(
        get_encrypted(&f, "config.token", &KeySource::Bytes(&master))?,
        Value::Str("s3cr3t".into())
    );

    // 不给钥匙 ⇒ key-required
    assert_eq!(load(&f).unwrap_err().kind(), "key-required");

    // 2) 口令：写时选参数，读时**听文件的**（不给参数）
    let pass: &[u8] = b"correct horse battery staple";
    let f2 = dump_encrypted(
        &cv,
        &DumpOptions::default(),
        &KeySource::Passphrase {
            secret: pass,
            kdf: Some(KdfParams::argon2id_default()),
        },
    )?;
    let back = load_encrypted(
        &f2,
        &KeySource::Passphrase {
            secret: pass,
            kdf: None,
        },
    )?;
    assert!(back.eq_ordered(&doc));

    println!("OK");
    Ok(())
}
