// SPDX-License-Identifier: MulanPSL-2.0
//! Signing and verification (T4 / §21.1) — only compiled with the `sign` feature.
//! 签名与验签（T4 / §21.1）—— 只在 `sign` feature 下编译。
//!
//! # 为什么这两个 crate 可以进来（用户 2026-11-04 批准「可以引入」，但要求避开热点）
//!
//! 台账（RustSec 实测，2026-11-05 查）：
//!
//! | crate | 公告 | 处置 |
//! |---|---|---|
//! | `ed25519-dalek` | RUSTSEC-2022-0093（双公钥签名预言机），1.0.1 起修复 | 用 **3.0**，历史只有这一条 |
//! | `curve25519-dalek`（间接） | RUSTSEC-2024-0344（标量减法时序可变），**>= 4.1.3** 修复 | 随 ed25519-dalek 3.0 带 5.x，已越过修复点 |
//! | `getrandom` | 无公告页 | 仅 keygen 取熵 |
//!
//! 被**排除**的（同期更热）：`ring`（RUSTSEC-2025-0010 直接标 unmaintained）、
//! `ruzstd`（2026-10-13 HIGH 解压炸弹）、`zstd` 绑定（2026-10-26 三条新公告）——
//! 后者属 T5，等冷却再定。
//! ｜ Chosen: ed25519-dalek 3.0 + getrandom. Excluded: ring (unmaintained), ruzstd/zstd (fresh advisories).
//!
//! # 签名对象
//!
//! `canonical_text(layer 去掉 sig)`（见 [`crate::layer`] 的模块头）。
//! 用 `verify_strict`：拒绝非规范编码与小阶公钥 —— 宁可拒一个合法签名的边界情况，
//! 也不放过一个可塑性（malleability）问题。

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::hash::HashAlgo;
use crate::layer::{self, Delegation, Layer, LayerError, SigEntry};
use sml_value::Value;

/// §20 algorithm id for Ed25519 — the only signature algorithm this build implements.
/// §20 的 Ed25519 编号 —— 本构建唯一实现的签名算法。
pub const ED25519: u8 = 0x40;

/// A signing key (Ed25519 seed + derived public key). ｜ 一把签名密钥。
pub struct KeyPair {
    signing: SigningKey,
}

impl KeyPair {
    /// From a 32-byte seed given as 64 hex chars. ｜ 从 64 位十六进制的 32 B 种子构造。
    pub fn from_seed_hex(seed_hex: &str) -> Result<KeyPair, SignError> {
        let seed = from_hex(seed_hex).ok_or(SignError::BadSeed)?;
        if seed.len() != 32 {
            return Err(SignError::BadSeed);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&seed);
        Ok(KeyPair {
            signing: SigningKey::from_bytes(&arr),
        })
    }

    /// From 32 raw seed bytes. ｜ 从 32 B 种子构造。
    pub fn from_seed(seed: &[u8; 32]) -> KeyPair {
        KeyPair {
            signing: SigningKey::from_bytes(seed),
        }
    }

    /// Generate a fresh key using OS entropy (D41: 熵只进一次性标识).
    /// 用 OS 熵生成新密钥（D41：熵只进一次性标识）。
    pub fn generate() -> Result<KeyPair, SignError> {
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).map_err(|e| SignError::NoEntropy(format!("{e}")))?;
        Ok(KeyPair::from_seed(&seed))
    }

    /// Seed as 64 hex chars (writing it out is the caller's risk to take).
    /// 种子（64 位十六进制）；把它写出去的风险由调用方承担。
    pub fn seed_hex(&self) -> String {
        hex(&self.signing.to_bytes())
    }

    /// Public key as 64 hex chars. ｜ 公钥（64 位十六进制）。
    pub fn public_hex(&self) -> String {
        hex(&self.signing.verifying_key().to_bytes())
    }

    /// Raw public key bytes. ｜ 原始公钥字节。
    pub fn public_bytes(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    /// Sign the delegation text (D16: 由**前一个作者**签).
    /// 签一份换钥授权文本（D16：由前一个作者签）。
    pub fn delegate(&self, to: &[u8; 32], max_index: u32) -> Delegation {
        let text = layer::delegation_text(&self.public_bytes(), to, max_index);
        let sig = self.signing.sign(text.as_bytes());
        Delegation {
            from: self.public_bytes(),
            max_index,
            sig: sig.to_bytes().to_vec(),
        }
    }
}

/// Append one signed layer to `bytes` (the whole append path: layer + trailer).
/// 追加一个已签名的层（层记录 + 新 Trailer）。
///
/// **不改旧字节**：新层与 Trailer 都写在原文件末尾之后；旧 Trailer 变成中间字节，
/// 读者只认 EOF 往前 64 B 的那个。
/// Append-only: new layer + trailer go after EOF; the old trailer becomes interior bytes.
pub fn append_layer(
    bytes: &[u8],
    key: &KeyPair,
    patch: Option<Value>,
    delegation: Option<Delegation>,
    algo: HashAlgo,
) -> Result<Vec<u8>, SignError> {
    let existing = layer::layers(bytes).map_err(SignError::Layer)?;
    let old_trailer = layer::Trailer::parse(bytes);

    let prev_hash = match existing.last() {
        Some(l) => head_hash(l, algo)?,
        None => [0u8; 32],
    };
    let index = existing.len() as u32;
    let mut l = Layer::new(index, prev_hash, key.public_bytes());
    l.patch = patch;
    l.delegation = delegation;

    let text = l.signed_text()?;
    let sig = key.signing.sign(text.as_bytes());
    l.sigs.push(SigEntry {
        algo: ED25519,
        bytes: sig.to_bytes().to_vec(),
    });

    let body = l.canonical_bytes()?;
    let prev_off = old_trailer.map(|t| t.last_layer_off).unwrap_or(0);
    let new_off = bytes.len() as u64;

    let mut out = Vec::with_capacity(bytes.len() + body.len() + layer::LAYER_HEADER_LEN + layer::TRAILER_LEN);
    out.extend_from_slice(bytes);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&prev_off.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // reserved
    out.extend_from_slice(&body);

    let trailer = layer::Trailer {
        layer_count: index + 1,
        last_layer_off: new_off,
        head_hash: hash_of(&body, algo)?,
    };
    out.extend_from_slice(&trailer.to_bytes());
    Ok(out)
}

/// `H(whole layer, with sigs)` — what the next layer's `prev_hash` must equal.
/// 整层（含签名）的哈希 —— 下一层 `prev_hash` 必须等于它。
fn head_hash(l: &Layer, algo: HashAlgo) -> Result<[u8; 32], SignError> {
    let body = l.canonical_bytes()?;
    hash_of(&body, algo)
}

fn hash_of(body: &[u8], algo: HashAlgo) -> Result<[u8; 32], SignError> {
    crate::hash::digest(algo, body).map_err(SignError::Hash)
}

/// Verify **and** pin the chain head against an external anchor (D18: 信任根交给调用者).
/// 验签并把链头钉在外部锚点上（D18：信任根交给调用者）。
///
/// 为什么需要它：整条层链可以被**完整剥离**（删掉 Trailer 与所有层记录），文件就退回
/// 「未签名」，而 `verify` 会告诉你「通过（0 层）」。只有记住过一个期望的 head，才能把
/// 「本来就没签」与「签了又被剥掉」分开。这就是 `--trust` 的全部意义。
/// ｜ Without an anchor, “unsigned” and “stripped” look identical — this function is the anchor.
pub fn verify_trusted(
    bytes: &[u8],
    algo: HashAlgo,
    trust: &[u8; 32],
) -> Result<VerifyReport, SignError> {
    let mut report = verify(bytes, algo)?;
    match crate::layer::chain_head(bytes, algo) {
        Ok(Some(head)) if head == *trust => {
            report
                .lines
                .push("锚点     head 与期望一致（外部锚点，D18）".to_string());
        }
        Ok(Some(head)) => {
            report.ok = false;
            let got: String = head[..8].iter().map(|b| format!("{b:02x}")).collect();
            let want: String = trust[..8].iter().map(|b| format!("{b:02x}")).collect();
            report.lines.push(format!(
                "锚点     head 不符：文件是 {got}…，期望 {want}… —— 层链被改过或被替换"
            ));
        }
        Ok(None) => {
            report.ok = false;
            report.lines.push(
                "锚点     文件里**一层都没有**，而 --trust 给了期望值 —— 层链可能被整个剥离（这条只能靠外部锚点发现）"
                    .to_string(),
            );
        }
        Err(e) => {
            report.ok = false;
            report.lines.push(format!("锚点     算不出链头：{e}"));
        }
    }
    Ok(report)
}

/// Verification report: one line per check, `ok` only when everything passed.
/// 验签报告：一条检查一行，全过才是 `ok`。
#[derive(Debug, Clone)]
pub struct VerifyReport {
    /// Human-readable lines ｜ 逐条结论。
    pub lines: Vec<String>,
    /// `true` only when every check passed ｜ 全过才为 `true`。
    pub ok: bool,
    /// Number of layers ｜ 层数。
    pub layers: usize,
}

/// Verify the whole chain: indexes, `prev_hash` links, trailer head, every signature (D36),
/// and the delegation chain (D16).
/// 验整条链：序号、`prev_hash`、Trailer head、每条签名（D36）、换钥授权链（D16）。
///
/// ⚠️ 时间不进判定（D17）：层里根本没存时间 —— 存了也只展示。
/// Time is not part of the decision (D17); layers carry none.
pub fn verify(bytes: &[u8], algo: HashAlgo) -> Result<VerifyReport, SignError> {
    let all = match layer::layers(bytes) {
        Ok(v) => v,
        Err(e) => {
            return Ok(VerifyReport {
                lines: vec![format!("层链   读不出来：{e}")],
                ok: false,
                layers: 0,
            })
        }
    };
    let mut lines = Vec::new();
    let mut ok = true;

    if all.is_empty() {
        lines.push("层链   没有层（未签名文件）".to_string());
        return Ok(VerifyReport {
            lines,
            ok: true,
            layers: 0,
        });
    }

    // 1) 序号连续 + prev_hash 链接 + 首层 prev_hash 为全 0
    let mut prev_head = [0u8; 32];
    for (i, l) in all.iter().enumerate() {
        if l.index as usize != i {
            ok = false;
            lines.push(format!("层#{i}   序号是 {}，应为 {i}", l.index));
        }
        if l.prev_hash != prev_head {
            ok = false;
            lines.push(format!("层#{i}   prev_hash 与上一层不符（链断了）"));
        }
        prev_head = match head_hash(l, algo) {
            Ok(h) => h,
            Err(e) => {
                ok = false;
                lines.push(format!("层#{i}   算不了哈希：{e}"));
                [0u8; 32]
            }
        };
    }

    // 2) Trailer 的 head_hash 必须等于最后一层
    if let Some(t) = layer::Trailer::parse(bytes) {
        if t.head_hash != prev_head {
            ok = false;
            lines.push("Trailer head_hash 与最后一层不符".to_string());
        }
    }

    // 3) 每条签名都要过（D36：全部通过，不是任一）
    for (i, l) in all.iter().enumerate() {
        let text = match l.signed_text() {
            Ok(t) => t,
            Err(e) => {
                ok = false;
                lines.push(format!("层#{i}   可签文本算不出来：{e}"));
                continue;
            }
        };
        let vk = match VerifyingKey::from_bytes(&l.author) {
            Ok(vk) => vk,
            Err(_) => {
                ok = false;
                lines.push(format!("层#{i}   作者公钥非法"));
                continue;
            }
        };
        if l.sigs.is_empty() {
            ok = false;
            lines.push(format!("层#{i}   没有签名"));
            continue;
        }
        for (j, s) in l.sigs.iter().enumerate() {
            if s.algo != ED25519 {
                ok = false;
                lines.push(format!(
                    "层#{i}   签名#{j} 用了未实现的算法 0x{:02x}",
                    s.algo
                ));
                continue;
            }
            let arr: [u8; 64] = match s.bytes.as_slice().try_into() {
                Ok(a) => a,
                Err(_) => {
                    ok = false;
                    lines.push(format!("层#{i}   签名#{j} 长度不是 64 B"));
                    continue;
                }
            };
            let sig = Signature::from_bytes(&arr);
            match vk.verify_strict(text.as_bytes(), &sig) {
                Ok(()) => lines.push(format!(
                    "层#{i}   签名#{j} OK（作者 {}…）",
                    &hex(&l.author)[..8]
                )),
                Err(_) => {
                    ok = false;
                    lines.push(format!("层#{i}   签名#{j} **验不过**"));
                }
            }
        }
    }

    // 4) 换钥授权链（D16）：作者变了必须有前一位作者的授权，且 j ≤ max_index
    for i in 1..all.len() {
        let prev_author = all[i - 1].author;
        let cur = &all[i];
        if cur.author == prev_author {
            continue;
        }
        match &cur.delegation {
            None => {
                ok = false;
                lines.push(format!(
                    "层#{i}   换了作者（{}… → {}…）但没有 delegation",
                    &hex(&prev_author)[..8],
                    &hex(&cur.author)[..8]
                ));
            }
            Some(d) => {
                if d.from != prev_author {
                    ok = false;
                    lines.push(format!("层#{i}   delegation 的 from 不是上一位作者"));
                }
                if i as u32 > d.max_index {
                    ok = false;
                    lines.push(format!(
                        "层#{i}   超出 delegation 的 max_index {}",
                        d.max_index
                    ));
                }
                let text = layer::delegation_text(&d.from, &cur.author, d.max_index);
                let verifier = match VerifyingKey::from_bytes(&d.from) {
                    Ok(vk) => vk,
                    Err(_) => {
                        ok = false;
                        lines.push(format!("层#{i}   delegation.from 公钥非法"));
                        continue;
                    }
                };
                let arr: [u8; 64] = match d.sig.as_slice().try_into() {
                    Ok(a) => a,
                    Err(_) => {
                        ok = false;
                        lines.push(format!("层#{i}   delegation 签名长度不是 64 B"));
                        continue;
                    }
                };
                if verifier
                    .verify_strict(text.as_bytes(), &Signature::from_bytes(&arr))
                    .is_ok()
                {
                    lines.push(format!("层#{i}   delegation 有效（≤ max_index {}）", d.max_index));
                } else {
                    ok = false;
                    lines.push(format!("层#{i}   delegation 签名验不过"));
                }
            }
        }
    }

    lines.push(format!(
        "结论     {}（{} 层）",
        if ok { "全部通过" } else { "未通过" },
        all.len()
    ));
    Ok(VerifyReport {
        lines,
        ok,
        layers: all.len(),
    })
}

/// Why signing or verification could not proceed. ｜ 签名/验签进行不下去的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SignError {
    /// The seed is not 32 bytes of hex ｜ 种子不是 32 B 的十六进制。
    BadSeed,
    /// The OS entropy source failed ｜ OS 熵源失败。
    NoEntropy(String),
    /// Canonical encoding failed (C2/C13) ｜ canonical 编码失败。
    Canonical(crate::CanonicalError),
    /// The layer chain is unreadable ｜ 层链读不出来。
    Layer(LayerError),
    /// The digest algorithm is unavailable ｜ 摘要算法不可用。
    Hash(crate::hash::HashError),
}

impl From<crate::CanonicalError> for SignError {
    fn from(e: crate::CanonicalError) -> Self {
        SignError::Canonical(e)
    }
}

impl std::fmt::Display for SignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignError::BadSeed => write!(f, "种子必须是 32 字节（64 位十六进制）"),
            SignError::NoEntropy(e) => write!(f, "取不到 OS 熵：{e}"),
            SignError::Canonical(e) => write!(f, "{e}"),
            SignError::Layer(e) => write!(f, "{e}"),
            SignError::Hash(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SignError {}

fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        out.push(u8::from_str_radix(&s[i..i + 2], 16).ok()?);
    }
    Some(out)
}
