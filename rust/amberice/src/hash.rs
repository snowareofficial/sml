// SPDX-License-Identifier: MulanPSL-2.0
//! Hash dispatch by §20 algorithm id — **SHA-256 default, SM3 and BLAKE3 implemented**.
//! 按 §20 的算法编号分派摘要 —— **默认 SHA-256，SM3 与 BLAKE3 均已实现**。
//!
//! Why a dispatcher at all / 为什么要这一层：段头里那 32 B 摘要**不带算法标记**（§19.1 固定
//! 32 B，§20.1 第 2 条：三个算法的摘要都是 32 B）⇒ 算法只能由**文件自己声明**（manifest 的
//! `hash_id`），读者照它算。所以「算出摘要」这一步必须能按 id 走，而不是硬写 SHA-256 ——
//! 否则「可选 SM3」就只是一句空话。
//!
//! 三条不许越界 / three lines not to cross：
//!
//! 1. **未知 id 明确拒绝**（§20「编号一经发布永不复用」+ 未知算法跳过等于拿错算法验签）；
//! 2. **登记 ≠ 实现**：表里有号但本构建算不出 ⇒ [`HashError::NotImplemented`]，不是一个
//!    「大概也行」的默认（`0x32` 曾长期处于这个状态，实现见 [`crate::blake3`]）；
//! 3. **默认只影响写、不影响读**：写文件时默认 SHA-256；读文件一律以 manifest 的 `hash_id` 为准
//!    （`hash_id` 缺失时也退回默认，那是 T1 时代没有 manifest 的文件）。

/// Algorithms from §20's hash range (all produce 32-byte digests).
/// §20 哈希区间的算法（都产出 32 B 摘要）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HashAlgo {
    /// `0x30` — SHA-256 (FIPS 180-4). ｜ `0x30` ✓ 已实现。
    Sha256,
    /// `0x31` — SM3 (GB/T 32905-2016). ｜ `0x31` ✓ 已实现。
    Sm3,
    /// `0x32` — BLAKE3 (unkeyed 32 B digest; see [`crate::blake3`]).
    /// `0x32` ✓ 已实现（无密钥 32 B 摘要，见 [`crate::blake3`]）。
    Blake3,
}

/// What this build writes when the caller does not choose. ｜ 调用方不指定时的写入口径。
pub const DEFAULT: HashAlgo = HashAlgo::Sha256;

impl HashAlgo {
    /// §20 algorithm id. ｜ §20 的算法编号。
    pub const fn id(self) -> u8 {
        match self {
            HashAlgo::Sha256 => 0x30,
            HashAlgo::Sm3 => 0x31,
            HashAlgo::Blake3 => 0x32,
        }
    }

    /// Map a §20 id back; `None` = unknown id (must be rejected, never guessed).
    /// 反查编号；`None` = 未知编号（必须拒绝，绝不猜）。
    pub const fn from_id(id: u8) -> Option<HashAlgo> {
        match id {
            0x30 => Some(HashAlgo::Sha256),
            0x31 => Some(HashAlgo::Sm3),
            0x32 => Some(HashAlgo::Blake3),
            _ => None,
        }
    }

    /// Human-readable name for tool output. ｜ 给工具看的名字。
    pub const fn name(self) -> &'static str {
        match self {
            HashAlgo::Sha256 => "SHA-256",
            HashAlgo::Sm3 => "SM3",
            HashAlgo::Blake3 => "BLAKE3",
        }
    }

    /// Does this build actually compute it? ｜ 本构建真能算吗？
    pub const fn is_implemented(self) -> bool {
        matches!(self, HashAlgo::Sha256 | HashAlgo::Sm3 | HashAlgo::Blake3)
    }
}

/// Why a digest could not be produced. ｜ 算不出摘要的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HashError {
    /// The id is not in §20 at all ｜ 编号不在 §20 表里。
    Unknown(u8),
    /// The id is in §20 but this build does not implement it ｜ 表里有号但本构建没实现。
    NotImplemented(u8),
}

impl std::fmt::Display for HashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashError::Unknown(id) => write!(f, "未知哈希算法 id 0x{id:02x}（§20 表里没有）"),
            HashError::NotImplemented(id) => write!(
                f,
                "哈希算法 id 0x{id:02x}（{}）已登记但本构建未实现，拒绝猜一个算法去验",
                HashAlgo::from_id(*id).map(|a| a.name()).unwrap_or("?")
            ),
        }
    }
}

impl std::error::Error for HashError {}

/// Compute a 32-byte digest with the requested algorithm.
/// 用指定算法算 32 B 摘要。
pub fn digest(algo: HashAlgo, data: &[u8]) -> Result<[u8; 32], HashError> {
    match algo {
        HashAlgo::Sha256 => Ok(crate::sha256::digest(data)),
        HashAlgo::Sm3 => Ok(crate::sm3::digest(data)),
        HashAlgo::Blake3 => Ok(crate::blake3::digest(data)),
    }
}

/// Same, but from a raw §20 id — the form a file gives us.
/// 同上，但输入是文件里那个 §20 编号。
pub fn digest_by_id(id: u8, data: &[u8]) -> Result<[u8; 32], HashError> {
    let algo = HashAlgo::from_id(id).ok_or(HashError::Unknown(id))?;
    digest(algo, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        for algo in [HashAlgo::Sha256, HashAlgo::Sm3, HashAlgo::Blake3] {
            assert_eq!(HashAlgo::from_id(algo.id()), Some(algo));
        }
        assert_eq!(HashAlgo::from_id(0x33), None);
    }

    #[test]
    fn unknown_id_is_an_error_not_a_guess() {
        assert_eq!(digest_by_id(0x99, b"x"), Err(HashError::Unknown(0x99)));
    }

    #[test]
    fn three_implemented_algorithms_differ() {
        let d = [
            digest(HashAlgo::Sha256, b"abc").unwrap(),
            digest(HashAlgo::Sm3, b"abc").unwrap(),
            digest(HashAlgo::Blake3, b"abc").unwrap(),
        ];
        assert_ne!(d[0], d[1]);
        assert_ne!(d[0], d[2]);
        assert_ne!(d[1], d[2]);
    }
}
