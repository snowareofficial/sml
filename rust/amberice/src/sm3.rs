// SPDX-License-Identifier: MulanPSL-2.0
//! SM3 (GB/T 32905-2016) — zero dependency.
//! SM3（GB/T 32905-2016）—— 零依赖。
//!
//! Why it is here / 为什么有它：§20 的哈希区间 `0x30 SHA-256 · 0x31 SM3 · 0x32 BLAKE3` 三个都是
//! 32 B 摘要，所以段头那 32 B 不用变长；**选哪个由 manifest 的 `hash_id` 定**。用户 2026-11-04
//! 定「默认 SHA-256，可选 SM3 / BLAKE3」，故 SM3 必须真能算 —— 否则「可选」是空话。
//! 国密这一路对国内合规场景是刚需，标准文档（GB/T 32905）公开、实现短。
//!
//! ⚠️ BLAKE3 只登记编号（`0x32`）、**未实现**：它是树哈希（Bao 树 + 分块 + 可并行），代码量
//! 比 SHA-256/SM3 大一个量级，且没有像 GB/T 那样一页能读完的标准向量表可供自验。
//! 按 §20「未知 ID 明确拒绝」的口径，声明 `0x32` 的文件会被明确拒绝，不会静默拿错算法算。
//! ｜ BLAKE3 is registered but not implemented (tree hash); declaring it is rejected explicitly.
//!
//! # 结构差异（读代码时别按 SHA-256 的直觉套）
//!
//! | | SHA-256 | SM3 |
//! |---|---|---|
//! | 消息扩展 | 64 个 W | **68 个 W + 64 个 W′**（`W′[j] = W[j] ^ W[j+4]`） |
//! | 布尔函数 | Ch / Maj | **FF / GG，前 16 轮与后 48 轮不同** |
//! | 常量 | 64 个 K | **一个 T 的两段**（`0x79cc4519` / `0x7a879d8a`），每轮再 `rotl(T, j)` |
//! | 状态更新 | 常规 | 多一个 `P0` 作用在 `TT2` 上 |

/// IV：GB/T 32905-2016 §5.3 的初始值。｜ IV from the standard.
const IV: [u32; 8] = [
    0x7380_166f, 0x4914_b2b9, 0x1724_42d7, 0xda8a_0600, 0xa96f_30bc, 0x1631_38aa, 0xe38d_ee4d,
    0xb0fb_0e4e,
];

/// `T_j`：前 16 轮一个值，之后另一个（标准里的 `T_j = T_j <<< (j mod 32)` 由 [`compress`] 现场算）。
const T0: u32 = 0x79cc_4519;
const T1: u32 = 0x7a87_9d8a;

/// Digest length (32 B, same as the other two algorithms ⇒ §20.1 rule 2 holds).
/// 摘要长度 32 B（与另两个算法一致 ⇒ §20.1 第 2 条成立）。
pub const DIGEST_LEN: usize = 32;

/// Compute the SM3 digest of `data`. ｜ 计算 `data` 的 SM3 摘要。
pub fn digest(data: &[u8]) -> [u8; DIGEST_LEN] {
    let mut v = IV;

    let (blocks, rem) = data.as_chunks::<64>();
    for block in blocks {
        let mut w = [0u8; 64];
        w.copy_from_slice(block.as_slice());
        compress(&mut v, &w);
    }

    // 与 SHA-256 同一套 Merkle–Damgård 填充：0x80 + 0 + 64 位大端比特长度。
    let mut tail = [0u8; 128];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;
    let tail_len = if rem.len() < 56 { 64 } else { 128 };
    let bits = (data.len() as u64).wrapping_mul(8);
    tail[tail_len - 8..tail_len].copy_from_slice(&bits.to_be_bytes());
    for i in 0..tail_len / 64 {
        let mut w = [0u8; 64];
        w.copy_from_slice(&tail[i * 64..(i + 1) * 64]);
        compress(&mut v, &w);
    }

    let mut out = [0u8; DIGEST_LEN];
    for (i, word) in v.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[inline]
fn ff(j: usize, x: u32, y: u32, z: u32) -> u32 {
    if j < 16 {
        x ^ y ^ z
    } else {
        (x & y) | (x & z) | (y & z)
    }
}

#[inline]
fn gg(j: usize, x: u32, y: u32, z: u32) -> u32 {
    if j < 16 {
        x ^ y ^ z
    } else {
        (x & y) | ((!x) & z)
    }
}

#[inline]
fn p0(x: u32) -> u32 {
    x ^ x.rotate_left(9) ^ x.rotate_left(17)
}

#[inline]
fn p1(x: u32) -> u32 {
    x ^ x.rotate_left(15) ^ x.rotate_left(23)
}

fn compress(v: &mut [u32; 8], block: &[u8; 64]) {
    // 消息扩展：W[0..68]、W′[0..64]
    let mut w = [0u32; 68];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for j in 16..68 {
        w[j] = p1(w[j - 16] ^ w[j - 9] ^ w[j - 3].rotate_left(15))
            ^ w[j - 13].rotate_left(7)
            ^ w[j - 6];
    }
    let mut wp = [0u32; 64];
    for j in 0..64 {
        wp[j] = w[j] ^ w[j + 4];
    }

    let (mut a, mut b, mut c, mut d) = (v[0], v[1], v[2], v[3]);
    let (mut e, mut f, mut g, mut h) = (v[4], v[5], v[6], v[7]);

    for j in 0..64 {
        let tj = if j < 16 { T0 } else { T1 };
        let a12 = a.rotate_left(12);
        let ss1 = a12
            .wrapping_add(e)
            .wrapping_add(tj.rotate_left((j % 32) as u32))
            .rotate_left(7);
        let ss2 = ss1 ^ a12;
        let tt1 = ff(j, a, b, c)
            .wrapping_add(d)
            .wrapping_add(ss2)
            .wrapping_add(wp[j]);
        let tt2 = gg(j, e, f, g)
            .wrapping_add(h)
            .wrapping_add(ss1)
            .wrapping_add(w[j]);

        d = c;
        c = b.rotate_left(9);
        b = a;
        a = tt1;
        h = g;
        g = f.rotate_left(19);
        f = e;
        e = p0(tt2);
    }

    v[0] ^= a;
    v[1] ^= b;
    v[2] ^= c;
    v[3] ^= d;
    v[4] ^= e;
    v[5] ^= f;
    v[6] ^= g;
    v[7] ^= h;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(d: &[u8; 32]) -> String {
        d.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Standard vectors (GB/T 32905-2016 附录 A) + CPython `hashlib` cross-check.
    /// 标准向量 + 与 CPython `hashlib`（OpenSSL 的 SM3）交叉核对。
    #[test]
    fn standard_and_boundary_vectors() {
        assert_eq!(
            hex(&digest(b"abc")),
            "66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0"
        );
        // 标准里的第二组：`abcd` 重复 16 次（64 B，正好一整块）
        let abcd16: Vec<u8> = b"abcd".iter().cycle().take(64).copied().collect();
        assert_eq!(
            hex(&digest(&abcd16)),
            "debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732"
        );
        // 填充边界（与 hashlib 核对）
        for (n, want) in [
            (0usize, "1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b"),
            (55, "288337eef51eec62e7544d7270424c8dbe656254c99852870a73b2453a6a7fb1"),
            (56, "ba00ebedaab54065a5fd4f9f56326016203166bcee3eed44ea868d59d67aa3c8"),
            (64, "616ec433c359e7c2b19f360e2b8f2a1b6e9ed76b8dc1a7d207b31a5341c611e9"),
            (65, "3d1d94afa238ec3e2bbc20ad504702b24c16f2889c94973f2f8da3526c44e4bc"),
        ] {
            assert_eq!(hex(&digest(&vec![b'a'; n])), want, "len={n}");
        }
        let million = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&digest(&million)),
            "c8aaf89429554029e231941a2acc0ad61ff2a5acd8fadd25847a3a732b3b02c3"
        );
    }

    /// SM3 and SHA-256 must not collide on the same input — the whole point of `hash_id`.
    /// 同一输入的两种摘要必须不同 —— `hash_id` 的意义就在这里。
    #[test]
    fn differs_from_sha256() {
        let a = super::super::sha256::digest(b"abc");
        let b = digest(b"abc");
        assert_ne!(a, b);
    }
}
