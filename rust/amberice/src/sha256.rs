// SPDX-License-Identifier: MulanPSL-2.0
//! SHA-256 (FIPS 180-4) — zero dependency, ~120 lines.
//! SHA-256（FIPS 180-4）—— 零依赖，约 120 行。
//!
//! Why hand-rolled / 为什么自己写：`.ambr` 的零依赖铁律（D3 / §20.1 第 2 条）——
//! 段头里那 32 B 摘要的算法由 manifest 的 `hash_id` 选（`0x30 SHA-256` / `0x31 SM3` /
//! `0x32 BLAKE3`），而 v1 只需要 SHA-256。为它拖一个 `sha2` 进来，等于让「读一个文件不必
//! 拖进解析器」的承诺变成「还得拖一条依赖树」。
//!
//! ⚠️ 这不是通用密码学库：**只实现 SHA-256，只做摘要**，没有 HMAC、没有流式接口、
//! 没有清零（S8 的 `zeroize` 是 T5 的可选 feature）。段校验用它，签名的哈希用法等 T4。
//!
//! # 常量时间说明
//!
//! 段摘要是**公开数据**之间的比较（文件里的 32 B vs 重算的 32 B），不是密钥材料，
//! 故 [`verify`] 用普通比较即可（§24.1 S7 针对的是 tag / 签名的比较）。
//! ｜ Segment digests are public data, so a plain comparison is fine here (S7 is about tags/signatures).

/// Round constants: the first 32 bits of the fractional parts of the cube roots of the first
/// 64 primes. ｜ 64 个轮常量（前 64 个质数立方根小数部分的前 32 位）。
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Initial hash value: the first 32 bits of the fractional parts of the square roots of the
/// first 8 primes. ｜ 初始哈希值（前 8 个质数平方根小数部分的前 32 位）。
const H0: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// Digest length in bytes (fixed 32 ⇒ the segment header never changes length, §20.1).
/// 摘要长度固定 32 B（⇒ 段头长度不变，§20.1 第 2 条）。
pub const DIGEST_LEN: usize = 32;

/// Compute the SHA-256 digest of `data`. ｜ 计算 `data` 的 SHA-256 摘要。
pub fn digest(data: &[u8]) -> [u8; DIGEST_LEN] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize()
}

/// Incremental SHA-256 — required by the streaming writer: the segment header's digest
/// covers bytes that are never all in memory at once (§19.2「写入留占位、段尾回填」).
/// 增量 SHA-256 —— 流式写的必要件：段头摘要覆盖的字节不会同时在内存里。
///
/// 与 [`digest`] 必须给出**同一个结果**（`sha256.rs` 的用例逐块喂入比对）。
#[derive(Clone)]
pub struct Sha256 {
    h: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    /// Total bytes fed so far (the padding encodes the **bit** length).
    /// 已喂入的字节数（padding 里写的是**比特**长度）。
    total: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    /// Fresh state. ｜ 新建。
    pub fn new() -> Self {
        Sha256 {
            h: H0,
            buf: [0u8; 64],
            buf_len: 0,
            total: 0,
        }
    }

    /// Feed more bytes. Any chunking of the same input yields the same digest.
    /// 继续喂字节；同一输入无论怎么切块，结果相同。
    pub fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);

        // 先把上次剩下的半块填满
        if self.buf_len > 0 {
            let need = 64 - self.buf_len;
            let take = need.min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let mut w = [0u8; 64];
                w.copy_from_slice(&self.buf);
                compress(&mut self.h, &w);
                self.buf_len = 0;
            }
        }

        let mut chunks = data.chunks_exact(64);
        for block in &mut chunks {
            let mut w = [0u8; 64];
            w.copy_from_slice(block);
            compress(&mut self.h, &w);
        }

        let rem = chunks.remainder();
        if !rem.is_empty() {
            self.buf[..rem.len()].copy_from_slice(rem);
            self.buf_len = rem.len();
        }
    }

    /// Finish and return the digest. Consumes the state (no accidental reuse).
    /// 收尾并给出摘要；消耗状态，防止误重用。
    pub fn finalize(mut self) -> [u8; DIGEST_LEN] {
        let bits = self.total.wrapping_mul(8);
        let mut tail = [0u8; 128];
        tail[..self.buf_len].copy_from_slice(&self.buf[..self.buf_len]);
        tail[self.buf_len] = 0x80;
        let tail_len = if self.buf_len < 56 { 64 } else { 128 };
        tail[tail_len - 8..tail_len].copy_from_slice(&bits.to_be_bytes());
        for i in 0..tail_len / 64 {
            let mut w = [0u8; 64];
            w.copy_from_slice(&tail[i * 64..(i + 1) * 64]);
            compress(&mut self.h, &w);
        }

        let mut out = [0u8; DIGEST_LEN];
        for (i, word) in self.h.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

/// Constant-ish helper: does `data` hash to `expected`? ｜ `data` 的摘要是否等于 `expected`？
pub fn verify(data: &[u8], expected: &[u8; DIGEST_LEN]) -> bool {
    digest(data) == *expected
}

fn compress(h: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let mut a = h[0];
    let mut b = h[1];
    let mut c = h[2];
    let mut d = h[3];
    let mut e = h[4];
    let mut f = h[5];
    let mut g = h[6];
    let mut hh = h[7];

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let t1 = hh
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);

        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }

    h[0] = h[0].wrapping_add(a);
    h[1] = h[1].wrapping_add(b);
    h[2] = h[2].wrapping_add(c);
    h[3] = h[3].wrapping_add(d);
    h[4] = h[4].wrapping_add(e);
    h[5] = h[5].wrapping_add(f);
    h[6] = h[6].wrapping_add(g);
    h[7] = h[7].wrapping_add(hh);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(d: &[u8; 32]) -> String {
        d.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// FIPS 180-4 / NIST CAVP vectors — these are the *only* reason to trust a hand-rolled hash.
    /// 标准向量 —— 手写哈希唯一的可信依据。
    #[test]
    fn nist_vectors() {
        assert_eq!(
            hex(&digest(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // 56 B：正好卡在「余数 ≥ 56 ⇒ 补两块」那条边界上
        assert_eq!(
            hex(&digest(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million_a = vec![b'a'; 1_000_000];
        assert_eq!(
            hex(&digest(&million_a)),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    /// Padding boundaries, cross-checked against CPython's `hashlib`.
    /// 填充边界（与 CPython `hashlib` 交叉核对过）。
    #[test]
    fn padding_boundaries_match_reference() {
        for (n, want) in [
            (55, "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"),
            (56, "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"),
            (63, "7d3e74a05d7db15bce4ad9ec0658ea98e3f06eeecf16b4c6fff2da457ddc2f34"),
            (64, "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"),
            (65, "635361c48bb9eab14198e76ea8ab7f1a41685d6ad62aa9146d301d4f17eb0ae0"),
        ] {
            let data = vec![b'a'; n];
            assert_eq!(hex(&digest(&data)), want, "len={n}");
        }
    }

    /// Any single-byte change must change the digest (and be caught by `verify`).
    /// 任一单字节改动都会改摘要（并被 `verify` 抓到）。
    #[test]
    fn single_byte_change_is_caught() {
        let mut data = b"the quick brown fox".to_vec();
        let good = digest(&data);
        assert!(verify(&data, &good));
        for i in 0..data.len() {
            let old = data[i];
            data[i] = old ^ 0x01;
            assert!(!verify(&data, &good), "第 {i} 字节改动未被检出");
            data[i] = old;
        }
    }
}

#[cfg(test)]
mod incremental_tests {
    use super::*;

    /// 同一个输入按**任意**切法喂进去，摘要必须与一次性调用逐字节相同。
    /// 这是流式写正确性的根：段头摘要不再依赖「字节是否同时在内存里」。
    #[test]
    fn any_chunking_matches_one_shot() {
        let data: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let want = digest(&data);

        for step in [1usize, 3, 63, 64, 65, 127, 128, 129, 500, 1000] {
            let mut h = Sha256::new();
            for chunk in data.chunks(step) {
                h.update(chunk);
            }
            assert_eq!(h.finalize(), want, "step={step}");
        }

        // 空输入与逐字节喂入
        let mut h = Sha256::new();
        h.update(&[]);
        assert_eq!(h.finalize(), digest(&[]));
    }

    /// 边界长度：55/56/63/64/65 是 padding 分支的临界点。
    #[test]
    fn padding_edges() {
        for n in [0usize, 1, 55, 56, 63, 64, 65, 119, 120, 128] {
            let data = vec![0xA5u8; n];
            let mut h = Sha256::new();
            h.update(&data[..n / 2]);
            h.update(&data[n / 2..]);
            assert_eq!(h.finalize(), digest(&data), "n={n}");
        }
    }
}
