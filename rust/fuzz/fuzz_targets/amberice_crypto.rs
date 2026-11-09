#![no_main]
//! L2 加密路径模糊：**固定密钥**下把任意字节喂给 `load_encrypted`，并对合法明文做加密往返。
//!
//! 为什么单独一个 target（HANDOFF §4.3 第 7 位）：AAD / nonce / 分块边界是「错了也不崩、
//! 只是解出垃圾或验不过」的那类 bug，只靠 `amberice_load`（不解密）永远碰不到。
//!
//! 断言：
//! 1. 任意字节进 `load_encrypted` / `get_encrypted` / `read_file_keys` 都必须**分类报错或成功**；
//! 2. 自己 `dump_encrypted` 的文件必须解得回原值，且单路径取值与整树一致；
//! 3. 密文改一个字节 / 换一把钥匙 ⇒ **必须**失败（否则 AAD 或摘要没覆盖到）；
//! 4. 口令路径不传 KDF 参数 ⇒ `KdfRequired`（明确拒绝，不是写出一个打不开的文件）。
mod common;

use libfuzzer_sys::fuzz_target;
use amberice::{CanonicalValue, ContainerError, DumpOptions, KeySource, KdfParams};

/// 固定主密钥：`KeySource::Bytes` 是「外部秘密（KMS/HSM）」那条路，最容易重现。
const KEY: [u8; 32] = [0x9Cu8; 32];

/// 只做单路径取值对照的两条路径（固定文档里 `k0`/`a` 常出现；不存在也只是报错）。
const PATHS: &[&str] = &["a", "k0"];

fuzz_target!(|data: &[u8]| {
    if data.len() > common::MAX_INPUT {
        return;
    }
    let len = data.len();
    let quota = common::decompression_allowance(data);
    let ks = KeySource::Bytes(&KEY);

    // —— 1. 任意字节：三条解密入口都不许 panic ——
    match common::check_call("load_encrypted", len, quota, || amberice::load_encrypted(data, &ks)) {
        Ok(_) => {}
        Err(e) => common::assert_container_error(&e),
    }
    for p in PATHS {
        if let Err(e) = amberice::get_encrypted(data, p, &ks) {
            common::assert_container_error(&e);
        }
    }
    if let Err(e) = amberice::container::read_file_keys(data, &ks) {
        common::assert_container_error(&e);
    }

    // —— 2. 明文文件：加密往返 + 篡改必须被拒 ——
    let Ok(plain) = amberice::load(data) else {
        return;
    };
    let Ok(cv) = CanonicalValue::try_from(&plain) else {
        return;
    };

    let (_, _, bytes) = common::count_allocs(|| {
        let enc = match amberice::dump_encrypted(&cv, &DumpOptions::default(), &ks) {
            Ok(v) => v,
            Err(e) => {
                common::assert_container_error(&e);
                return;
            }
        };

        let back = match amberice::load_encrypted(&enc, &ks) {
            Ok(v) => v,
            Err(e) => panic!("自己加密的文件解不回来：{e}"),
        };
        assert!(back.eq_ordered(&plain), "加密往返后值/键序变了");

        // 单路径：与整树解码必须同答。
        for p in PATHS {
            let one = amberice::get_encrypted(&enc, p, &ks);
            let full = sml_value::Path::parse(p).and_then(|path| back.get_path(&path).cloned());
            match (one, full) {
                (Ok(a), Ok(b)) => assert!(a.eq_ordered(&b), "路径 {p:?} 加密取值的两套实现不一致"),
                (Ok(_), Err(_)) => panic!("路径 {p:?} 整树取值失败、加密取值却成功"),
                (Err(e), Ok(_)) => common::assert_container_error(&e),
                (Err(_), Err(_)) => {}
            }
        }

        // 篡改一个字节：必须报错（AEAD 或段摘要兜底），不许静默给出错值。
        //
        // 必须改在**段体内部**：`block_align` 的 4 KiB 对齐填充不在摘要/认证范围内
        // （§19.7 丙），改填充字节本来就无害 —— 拿它当判据会得到假阳性（fuzz 撞过）。
        let info = amberice::info(&enc).expect("自己加密的文件 info 必须通过");
        if let Some(seg) = info
            .segments
            .iter()
            .find(|s| s.header.enc != 0 && s.header.len > 0)
        {
            let at = seg.offset as usize + seg.header.len as usize / 2;
            let mut bad = enc.clone();
            bad[at] ^= 0xFF;
            match amberice::load_encrypted(&bad, &ks) {
                Err(e) => common::assert_container_error(&e),
                Ok(_) => panic!(
                    "加密段体在偏移 {at} 改了一个字节，却被当成合法文件解开（段 {}）",
                    seg.header.seg_id
                ),
            }
        }

        // 换一把钥匙：必须 AuthFailed / 失败，不许「恰好也解得开」。
        const OTHER: [u8; 32] = [0x11u8; 32];
        if amberice::load_encrypted(&enc, &KeySource::Bytes(&OTHER)).is_ok() {
            panic!("换了一把钥匙却解开了密文");
        }

        // 口令路径：写入时必须带 KDF 参数。
        let no_kdf = KeySource::Passphrase {
            secret: b"correct horse",
            kdf: None,
        };
        match amberice::dump_encrypted(&cv, &DumpOptions::default(), &no_kdf) {
            Err(ContainerError::KdfRequired) => {}
            other => panic!("口令写入缺 KDF 参数应报 KdfRequired，实得 {other:?}"),
        }

        // 口令路径：带最便宜的合法参数走一遍完整往返（8 KiB 是 `KdfParams::validate` 的下限）。
        let cheap = KeySource::Passphrase {
            secret: b"correct horse",
            kdf: Some(KdfParams {
                m_cost: 8,
                t_cost: 1,
                p_cost: 1,
            }),
        };
        match amberice::dump_encrypted(&cv, &DumpOptions::default(), &cheap) {
            Ok(enc2) => {
                let read_key = KeySource::Passphrase {
                    secret: b"correct horse",
                    kdf: None,
                };
                match amberice::load_encrypted(&enc2, &read_key) {
                    Ok(v) => assert!(v.eq_ordered(&plain), "口令往返后值变了"),
                    Err(e) => panic!("自己用口令加密的文件解不回来：{e}"),
                }
            }
            Err(e) => common::assert_container_error(&e),
        }
    });

    common::assert_within_budget("amberice_crypto", len, bytes);
});
