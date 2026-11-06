// SPDX-License-Identifier: MulanPSL-2.0
//! Segment encryption (T5, §19.13): per-block AEAD over 4 KiB blocks.
//! 段加密（T5，§19.13）：按 4 KiB 块做 AEAD。
//!
//! # 密钥怎么进来（用户 2026-11-05 裁决「丙」）
//!
//! [`KeySource`] 一个 enum 管两条路：**原始 32 字节**（外部 KMS/HSM/secret manager 取回的就是它）
//! 与**口令派生**。文件里只声明「用什么算法、KDF 参数是什么」，**永远不写密钥存在哪**
//! —— 那是部署问题，写进文件等于把内网拓扑一起发出去。
//!
//! # 块布局（§19.13 的草案，已实现）
//!
//! ```text
//! 加密段体 = plain_len:u32le ‖ block_count:u32le ‖ block…
//! block    = ciphertext(≤4096 B) ‖ tag(16 B)          ← 最后一块可以短
//! nonce    = layer_id:u32le ‖ block_index:u32le ‖ file_salt[0..4]
//! AAD      = seg_id:u8 ‖ comp:u8 ‖ block_index:u32le ‖ block_count:u32le ‖ plain_len:u32le
//! ```
//!
//! **AAD 把段头字段绑进 tag**：改 `comp` / `enc` / 块序 ⇒ 解密当场失败，不必等签名。
//! 这正是 §19.8 里「改段头 `flags` 无人察觉」那个缺口在加密文件上的补丁。
//! ｜ The AAD binds header fields into the tag: flipping `comp` breaks authentication.
//!
//! `MANIFEST` 段**永不加密**（与「永不压缩」同一条理由）：`kdf` 参数就在它里面，
//! 加密它就成了「要先解密才能知道怎么解密」。
//!
//! # 确定性
//!
//! 加密文件**不是字节确定的**（`file_salt` 每条文件随机，D41 允许它进一次性标识）。
//! 所以只有明文 `dump` 才承诺 C12 的「同值同字节」；加密件承诺的是「同一把钥匙能读回来」。
//! ｜ Encrypted files are intentionally not byte-deterministic (random per-file salt).

#[cfg(feature = "crypto")]
use aes_gcm::aead::{Aead, Payload};
#[cfg(feature = "crypto")]
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};

/// §20 `0x10`: AES-256-GCM — the only cipher this build implements.
/// §20 的 `0x10`：AES-256-GCM —— 本构建唯一实现的对称加密。
pub const ENC_AES256_GCM: u8 = 0x10;

/// §20 `0x50`: no KDF — the caller hands over a raw 32 B key.
/// §20 的 `0x50`：不派生 —— 调用方直接给 32 B 密钥。
pub const KDF_NONE: u8 = 0x00;
/// §20 `0x51`: Argon2id — passphrase input.
/// §20 的 `0x51`：Argon2id —— 口令输入。
pub const KDF_ARGON2ID: u8 = 0x51;

/// Encryption block size (§21 / D28). ｜ 加密块大小。
pub const BLOCK: usize = 4096;
/// AEAD tag length (GCM). ｜ 认证标签长度。
pub const TAG: usize = 16;
/// Segment-level crypto header: `plain_len ‖ block_count`. ｜ 加密段体头。
pub const CRYPTO_HEADER: usize = 8;

/// Argon2id parameters, written into the manifest so any implementation can reopen the file.
/// Argon2id 参数，写进 manifest —— 换任何实现都能打开同一份文件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost in KiB ｜ 内存开销（KiB）。
    pub m_cost: u32,
    /// Iterations ｜ 迭代次数。
    pub t_cost: u32,
    /// Parallelism ｜ 并行度。
    pub p_cost: u32,
}

impl KdfParams {
    /// 64 MiB / 3 passes / 1 lane — OWASP's floor for Argon2id, cheap enough for a CLI.
    /// 64 MiB / 3 轮 / 1 并发 —— OWASP 的底线，命令行场景也扛得住。
    pub fn argon2id_default() -> KdfParams {
        KdfParams {
            m_cost: 64 * 1024,
            t_cost: 3,
            p_cost: 1,
        }
    }

    /// Sanity bounds: refuse absurd parameters that would hang or exhaust memory.
    /// 合理性边界：拒绝会卡死或吃光内存的参数。
    pub fn validate(&self) -> Result<(), String> {
        if self.m_cost < 8 || self.m_cost > 4 * 1024 * 1024 {
            return Err(format!("m_cost {} 不在 8..4MiB", self.m_cost));
        }
        if self.t_cost == 0 || self.t_cost > 64 {
            return Err(format!("t_cost {} 不在 1..64", self.t_cost));
        }
        if self.p_cost == 0 || self.p_cost > 64 {
            return Err(format!("p_cost {} 不在 1..64", self.p_cost));
        }
        Ok(())
    }
}

/// How to obtain the 32-byte master key. ｜ 怎么拿到那 32 字节主密钥。
///
/// One enum, two directions — and the direction is enforced, not documented:
/// 写要选 KDF 参数（`Some`），读要听文件的（`None`）；选错方向会**报错**而不是静默走偏。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource<'a> {
    /// Raw 32 B — this is also the external-secret path (KMS / HSM / secret manager).
    /// 32 字节原始密钥 —— 外部秘密那条路就是它。
    Bytes(&'a [u8; 32]),
    /// Passphrase. Write: `kdf: Some(params)`; read: `kdf: None` (the file decides).
    /// 口令。写：`Some(参数)`；读：`None`（文件说了算）。
    Passphrase {
        /// The passphrase bytes ｜ 口令字节。
        secret: &'a [u8],
        /// KDF parameters (write-only; `None` on read) ｜ KDF 参数（只在写时给）。
        kdf: Option<KdfParams>,
    },
}

/// Derived material for one file. ｜ 一个文件的派生结果。
pub struct FileKeys {
    /// AEAD key for `ENC_AES256_GCM` ｜ 对称加密密钥。
    pub aead: [u8; 32],
    /// Per-file salt, stored in the manifest, also seeds the nonce ｜ 每文件一个盐。
    pub file_salt: [u8; 16],
}

/// What a writer needs (params chosen by the caller). ｜ 写者需要的东西。
pub(crate) struct WriteKeys {
    /// Derived material ｜ 派生材料。
    pub keys: FileKeys,
    /// KDF id written into the manifest ｜ 写进 manifest 的 KDF 编号。
    pub kdf_kind: u8,
    /// KDF params written into the manifest ｜ 写进 manifest 的参数。
    pub kdf: Option<KdfParams>,
}

#[cfg(feature = "crypto")]
fn hkdf32(salt: &[u8], ikm: &[u8], info: &[u8]) -> Result<[u8; 32], ()> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (salt, ikm, info);
        Err(())
    }
    #[cfg(feature = "crypto")]
    {
        use hkdf::Hkdf;
        use sha2::Sha256;
        let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
        let mut out = [0u8; 32];
        hk.expand(info, &mut out).map_err(|_| ())?;
        Ok(out)
    }
}

/// Derive the per-file AEAD key from the caller's key material.
/// 从调用方的材料派生本文件的 AEAD 密钥。
///
/// `kdf_from_file` is what the manifest says (read path); `chosen` is what the writer wants.
/// 读路径传 manifest 里的参数，写路径传调用方选的参数 —— 两者不一致就报错，不猜。
#[cfg(feature = "crypto")]
pub(crate) fn derive_write_keys(
    src: &KeySource<'_>,
    file_salt: [u8; 16],
) -> Result<WriteKeys, String> {
    match src {
        KeySource::Bytes(master) => Ok(WriteKeys {
            keys: FileKeys {
                aead: hkdf32(&file_salt, *master, b"amberice:0x10").map_err(|_| "HKDF 失败".to_string())?,
                file_salt,
            },
            kdf_kind: KDF_NONE,
            kdf: None,
        }),
        KeySource::Passphrase { secret, kdf } => {
            let Some(params) = kdf else {
                return Err("写口令文件必须给 KdfParams（kdf: Some(..)）".to_string());
            };
            params.validate()?;
            let master = argon2_master(secret, &file_salt, *params)?;
            Ok(WriteKeys {
                keys: FileKeys {
                    aead: hkdf32(&file_salt, &master, b"amberice:0x10")
                        .map_err(|_| "HKDF 失败".to_string())?,
                    file_salt,
                },
                kdf_kind: KDF_ARGON2ID,
                kdf: Some(*params),
            })
        }
    }
}

/// Derive the per-file AEAD key on the read path.
/// 读路径：按文件声明的 `kdf` 派生。
#[cfg(feature = "crypto")]
pub(crate) fn derive_read_keys(
    src: &KeySource<'_>,
    kdf_kind: u8,
    kdf_params: Option<KdfParams>,
    file_salt: [u8; 16],
) -> Result<FileKeys, String> {
    let master: [u8; 32] = match (kdf_kind, src) {
        (KDF_NONE, KeySource::Bytes(m)) => **m,
        (KDF_NONE, KeySource::Passphrase { .. }) => {
            return Err("文件用的是原始密钥（kdf=0x00），却给了口令".to_string())
        }
        (KDF_ARGON2ID, KeySource::Bytes(m)) => {
            let _ = m;
            return Err("文件用的是口令派生（kdf=0x51），却给了原始密钥".to_string());
        }
        (KDF_ARGON2ID, KeySource::Passphrase { secret, kdf }) => {
            let params = match (kdf, kdf_params) {
                // 读时给了参数：必须与文件一致，否则不假装没看见
                (Some(given), Some(file)) if *given != file => {
                    return Err(format!(
                        "给的 KDF 参数 {given:?} 与文件里的 {file:?} 不符"
                    ))
                }
                (Some(given), _) => *given,
                (None, Some(file)) => file,
                (None, None) => return Err("文件没写 KDF 参数，无法派生".to_string()),
            };
            params.validate()?;
            argon2_master(secret, &file_salt, params)?
        }
        (other, _) => return Err(format!("不支持的 KDF 编号 0x{other:02x}")),
    };
    Ok(FileKeys {
        aead: hkdf32(&file_salt, &master, b"amberice:0x10").map_err(|_| "HKDF 失败".to_string())?,
        file_salt,
    })
}

#[cfg(feature = "crypto")]
fn argon2_master(secret: &[u8], salt: &[u8; 16], params: KdfParams) -> Result<[u8; 32], String> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (secret, salt, params);
        Err("本构建未启用 `crypto` feature".to_string())
    }
    #[cfg(feature = "crypto")]
    {
        use argon2::{Algorithm, Argon2, Params, Version};
        let p = Params::new(params.m_cost, params.t_cost, params.p_cost, Some(32))
            .map_err(|e| format!("Argon2 参数无效：{e}"))?;
        let a = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
        let mut out = [0u8; 32];
        a.hash_password_into(secret, salt, &mut out)
            .map_err(|e| format!("Argon2 派生失败：{e}"))?;
        Ok(out)
    }
}

/// Random 16 B per-file salt (D41: 熵只进一次性标识).
/// 每文件 16 B 随机盐（D41：熵只进一次性标识）。
#[cfg(feature = "crypto")]
pub(crate) fn random_salt() -> Result<[u8; 16], String> {
    #[cfg(not(feature = "crypto"))]
    {
        Err("本构建未启用 `crypto` feature".to_string())
    }
    #[cfg(feature = "crypto")]
    {
        let mut salt = [0u8; 16];
        getrandom::fill(&mut salt).map_err(|e| format!("取不到 OS 熵：{e}"))?;
        Ok(salt)
    }
}

#[cfg(feature = "crypto")]
fn nonce_bytes(block_index: u32, file_salt: &[u8; 16]) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[0..4].copy_from_slice(&0u32.to_le_bytes()); // layer_id：明文段恒 0（签名层在 T4）
    n[4..8].copy_from_slice(&block_index.to_le_bytes());
    n[8..12].copy_from_slice(&file_salt[0..4]);
    n
}

#[cfg(feature = "crypto")]
fn aad(seg_id: u8, comp: u8, block_index: u32, block_count: u32, plain_len: u32) -> [u8; 15] {
    let mut a = [0u8; 15];
    a[0] = seg_id;
    a[1] = comp;
    a[2..6].copy_from_slice(&block_index.to_le_bytes());
    a[6..10].copy_from_slice(&block_count.to_le_bytes());
    a[10..14].copy_from_slice(&plain_len.to_le_bytes());
    a[14] = 0; // 版本/保留：留给将来的算法参数，现在恒 0 且进 AAD
    a
}

/// Encrypt one segment body. ｜ 加密一个段体。
pub(crate) fn seal_segment(
    seg_id: u8,
    comp: u8,
    plain: &[u8],
    keys: &FileKeys,
) -> Result<Vec<u8>, String> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (seg_id, comp, plain, keys);
        Err("本构建未启用 `crypto` feature".to_string())
    }
    #[cfg(feature = "crypto")]
    {
        let plain_len = u32::try_from(plain.len()).map_err(|_| "段过大".to_string())?;
        let block_count = plain.len().div_ceil(BLOCK).max(1) as u32;
        let cipher = Aes256Gcm::new_from_slice(&keys.aead).map_err(|_| "密钥长度不对".to_string())?;

        let mut out = Vec::with_capacity(plain.len() + block_count as usize * TAG + CRYPTO_HEADER);
        out.extend_from_slice(&plain_len.to_le_bytes());
        out.extend_from_slice(&block_count.to_le_bytes());

        for i in 0..block_count {
            let start = (i as usize) * BLOCK;
            let end = (start + BLOCK).min(plain.len());
            let chunk = if start <= plain.len() { &plain[start..end] } else { &[][..] };
            let nonce = nonce_bytes(i, &keys.file_salt);
            let sealed = cipher
                .encrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: chunk,
                        aad: &aad(seg_id, comp, i, block_count, plain_len),
                    },
                )
                .map_err(|_| "加密失败".to_string())?;
            out.extend_from_slice(&sealed);
        }
        Ok(out)
    }
}

/// Why a segment could not be decrypted. ｜ 段解不开的原因。
///
/// `block` 有值表示「某一块认证/越界失败」—— 这是**认证失败**，与「格式不对」要分开报，
/// 否则用户没法判断是钥匙错还是文件坏了。
pub(crate) struct OpenError {
    /// Which block failed (None = structural problem) ｜ 出错块号（None = 结构问题）。
    pub block: Option<u32>,
    /// Human-readable reason ｜ 原因。
    pub reason: String,
}

fn structural(reason: impl Into<String>) -> OpenError {
    OpenError {
        block: None,
        reason: reason.into(),
    }
}

/// Decrypt one segment body. ｜ 解密一个段体。
pub(crate) fn open_segment(
    seg_id: u8,
    comp: u8,
    stored: &[u8],
    keys: &FileKeys,
    budget: usize,
) -> Result<Vec<u8>, OpenError> {
    #[cfg(not(feature = "crypto"))]
    {
        let _ = (seg_id, comp, stored, keys, budget);
        Err(structural("本构建未启用 `crypto` feature"))
    }
    #[cfg(feature = "crypto")]
    {
        if stored.len() < CRYPTO_HEADER {
            return Err(structural("加密段体不足 8 B"));
        }
        let plain_len =
            u32::from_le_bytes([stored[0], stored[1], stored[2], stored[3]]) as usize;
        let block_count =
            u32::from_le_bytes([stored[4], stored[5], stored[6], stored[7]]);
        if block_count == 0 {
            return Err(structural("块数为 0"));
        }
        if plain_len > budget {
            return Err(structural(format!(
                "解出的明文 {plain_len} B 超过上限 {budget} B"
            )));
        }
        let expect_blocks = plain_len.div_ceil(BLOCK).max(1) as u32;
        if block_count != expect_blocks {
            return Err(structural(format!(
                "块数 {block_count} 与明文长度 {plain_len} 对不上（应为 {expect_blocks}）"
            )));
        }

        let cipher = Aes256Gcm::new_from_slice(&keys.aead).map_err(|_| structural("密钥长度不对"))?;
        let mut out = Vec::with_capacity(plain_len);
        let mut at = CRYPTO_HEADER;
        for i in 0..block_count {
            let remaining = plain_len - out.len();
            let take = remaining.min(BLOCK);
            let want = take + TAG;
            if at + want > stored.len() {
                return Err(OpenError {
                    block: Some(i),
                    reason: "块越出段体".to_string(),
                });
            }
            let nonce = nonce_bytes(i, &keys.file_salt);
            let opened = cipher
                .decrypt(
                    Nonce::from_slice(&nonce),
                    Payload {
                        msg: &stored[at..at + want],
                        aad: &aad(seg_id, comp, i, block_count, plain_len as u32),
                    },
                )
                .map_err(|_| OpenError {
                    block: Some(i),
                    reason: "钥匙错、密文被改，或段头被改".to_string(),
                })?;
            out.extend_from_slice(&opened);
            at += want;
        }
        if at != stored.len() {
            return Err(structural(format!(
                "密文尾部多出 {} B",
                stored.len() - at
            )));
        }
        Ok(out)
    }
}
