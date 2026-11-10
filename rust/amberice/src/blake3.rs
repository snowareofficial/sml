// SPDX-License-Identifier: MulanPSL-2.0
//! BLAKE3（`0x32`）的零依赖标量实现（D44）。
//! A dependency-free scalar BLAKE3 implementation (D44).
//!
//! 只实现**无密钥 32 B 摘要**：这正是段头摘要需要的形态（§19.1 固定 32 B）。
//! 不做 keyed hash / derive_key / XOF 扩展输出 —— 那些没有消费方，多写只会多一份要维护的
//! 代码。树形结构按官方参考实现：1 KiB 分块、chunk 链、父节点合并，最后以 `ROOT` 标志收尾。
//! ｜ Only the unkeyed 32-byte digest: no keyed mode, no KDF, no XOF.

const OUT_LEN: usize = 32;
const BLOCK_LEN: usize = 64;
const CHUNK_LEN: usize = 1024;
const MAX_DEPTH: usize = 54;

const CHUNK_START: u32 = 1 << 0;
const CHUNK_END: u32 = 1 << 1;
const PARENT: u32 = 1 << 2;
const ROOT: u32 = 1 << 3;

/// BLAKE3 IV —— 与 BLAKE2s 相同。 ｜ Same IV as BLAKE2s.
const IV: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

/// 消息字置换（7 轮里每轮过后重排）。 ｜ Message permutation applied between rounds.
const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

fn round(state: &mut [u32; 16], m: &[u32; 16]) {
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);
    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

fn permute(m: &mut [u32; 16]) {
    let old = *m;
    for (i, &at) in MSG_PERMUTATION.iter().enumerate() {
        m[i] = old[at];
    }
}

/// 压缩函数：返回 16 个字（前 8 个是 CV，全 16 个供 XOF；这里只用前 8）。
/// The compression function; only the first 8 words are used as a chaining value.
fn compress(
    cv: &[u32; 8],
    block_words: &[u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    let mut state = [
        cv[0],
        cv[1],
        cv[2],
        cv[3],
        cv[4],
        cv[5],
        cv[6],
        cv[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter as u32,
        (counter >> 32) as u32,
        block_len,
        flags,
    ];
    let mut m = *block_words;
    for _ in 0..7 {
        round(&mut state, &m);
        permute(&mut m);
    }
    for i in 0..8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= cv[i];
    }
    state
}

fn words_from_le_bytes(block: &[u8; BLOCK_LEN]) -> [u32; 16] {
    let mut words = [0u32; 16];
    for (i, w) in words.iter_mut().enumerate() {
        let at = i * 4;
        *w = u32::from_le_bytes([block[at], block[at + 1], block[at + 2], block[at + 3]]);
    }
    words
}

/// 一次待输出的压缩：CV、块字、计数器、块长、标志。 ｜ One pending compression.
#[derive(Clone)]
struct Output {
    input_cv: [u32; 8],
    block_words: [u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
}

impl Output {
    fn chaining_value(&self) -> [u32; 8] {
        let state = compress(
            &self.input_cv,
            &self.block_words,
            self.counter,
            self.block_len,
            self.flags,
        );
        let mut cv = [0u32; 8];
        cv.copy_from_slice(&state[..8]);
        cv
    }

    /// 根输出：输出块计数器固定为 0，并带上 `ROOT` 标志。
    /// Root output: the output block counter is 0 and `ROOT` is set.
    fn root_hash(&self) -> [u8; OUT_LEN] {
        let state = compress(
            &self.input_cv,
            &self.block_words,
            0,
            self.block_len,
            self.flags | ROOT,
        );
        let mut out = [0u8; OUT_LEN];
        for (i, w) in state.iter().take(8).enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        out
    }
}

fn parent_output(left: &[u32; 8], right: &[u32; 8]) -> Output {
    let mut block_words = [0u32; 16];
    block_words[..8].copy_from_slice(left);
    block_words[8..].copy_from_slice(right);
    Output {
        input_cv: IV,
        block_words,
        counter: 0,
        block_len: BLOCK_LEN as u32,
        flags: PARENT,
    }
}

fn parent_cv(left: &[u32; 8], right: &[u32; 8]) -> [u32; 8] {
    parent_output(left, right).chaining_value()
}

/// 单个 chunk（≤ 1 KiB）的状态。 ｜ State for one chunk of at most 1 KiB.
#[derive(Clone)]
struct ChunkState {
    cv: [u32; 8],
    chunk_counter: u64,
    buf: [u8; BLOCK_LEN],
    buf_len: usize,
    blocks_compressed: usize,
}

impl ChunkState {
    fn new(chunk_counter: u64) -> Self {
        Self {
            cv: IV,
            chunk_counter,
            buf: [0u8; BLOCK_LEN],
            buf_len: 0,
            blocks_compressed: 0,
        }
    }

    fn len(&self) -> usize {
        BLOCK_LEN * self.blocks_compressed + self.buf_len
    }

    fn start_flag(&self) -> u32 {
        if self.blocks_compressed == 0 {
            CHUNK_START
        } else {
            0
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            // 缓冲满一块：不是最后一块 ⇒ 只带 CHUNK_START（若是首块），不带 CHUNK_END。
            if self.buf_len == BLOCK_LEN {
                let words = words_from_le_bytes(&self.buf);
                let state = compress(
                    &self.cv,
                    &words,
                    self.chunk_counter,
                    BLOCK_LEN as u32,
                    self.start_flag(),
                );
                self.cv.copy_from_slice(&state[..8]);
                self.blocks_compressed += 1;
                self.buf = [0u8; BLOCK_LEN];
                self.buf_len = 0;
            }
            let take = (BLOCK_LEN - self.buf_len).min(input.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&input[..take]);
            self.buf_len += take;
            input = &input[take..];
        }
    }

    fn output(&self) -> Output {
        Output {
            input_cv: self.cv,
            block_words: words_from_le_bytes(&self.buf),
            counter: self.chunk_counter,
            block_len: self.buf_len as u32,
            flags: self.start_flag() | CHUNK_END,
        }
    }
}

/// 流式 BLAKE3 哈希器：`update` 多次后 `finalize`。 ｜ Streaming BLAKE3 hasher.
#[derive(Clone)]
pub struct Blake3 {
    chunk_state: ChunkState,
    cv_stack: [[u32; 8]; MAX_DEPTH],
    cv_stack_len: usize,
}

impl Default for Blake3 {
    fn default() -> Self {
        Self::new()
    }
}

impl Blake3 {
    /// 新建一个空哈希器。 ｜ Create an empty hasher.
    pub fn new() -> Self {
        Self {
            chunk_state: ChunkState::new(0),
            cv_stack: [[0u32; 8]; MAX_DEPTH],
            cv_stack_len: 0,
        }
    }

    /// 追加输入；可多次调用。 ｜ Feed bytes; may be called repeatedly.
    pub fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            if self.chunk_state.len() == CHUNK_LEN {
                let cv = self.chunk_state.output().chaining_value();
                let total_chunks = self.chunk_state.chunk_counter + 1;
                self.add_chunk_cv(cv, total_chunks);
                self.chunk_state = ChunkState::new(total_chunks);
            }
            let take = (CHUNK_LEN - self.chunk_state.len()).min(input.len());
            self.chunk_state.update(&input[..take]);
            input = &input[take..];
        }
    }

    /// 把已完成的 chunk CV 压栈，并按「总 chunk 数为偶数就合并」的规则收敛成二叉树。
    /// Push a chunk CV and merge while the total chunk count stays even.
    fn add_chunk_cv(&mut self, mut new_cv: [u32; 8], mut total_chunks: u64) {
        while total_chunks & 1 == 0 {
            let left = self.cv_stack[self.cv_stack_len - 1];
            self.cv_stack_len -= 1;
            new_cv = parent_cv(&left, &new_cv);
            total_chunks >>= 1;
        }
        self.cv_stack[self.cv_stack_len] = new_cv;
        self.cv_stack_len += 1;
    }

    /// 收尾，返回 32 B 摘要。 ｜ Finish and return the 32-byte digest.
    pub fn finalize(&self) -> [u8; OUT_LEN] {
        let mut output = self.chunk_state.output();
        for at in (0..self.cv_stack_len).rev() {
            let right = output.chaining_value();
            output = parent_output(&self.cv_stack[at], &right);
        }
        output.root_hash()
    }
}

/// 一次性算 32 B 摘要。 ｜ One-shot 32-byte digest.
pub fn digest(data: &[u8]) -> [u8; OUT_LEN] {
    let mut h = Blake3::new();
    h.update(data);
    h.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 官方向量的输入模式：`input[i] = i % 251`。 ｜ Official test input pattern.
    fn vector_input(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// 官方向量取 32 B 摘要（即 64 B XOF 前缀的前一半）。
    /// Official vectors, truncated to the 32-byte digest.
    #[test]
    fn official_vectors() {
        let cases: &[(usize, &str)] = &[
            (0, "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"),
            (1, "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"),
            (1024, "42214739f095a406f3fc83deb889744ac00df831c10daa55189b5d121c855af7"),
            (1025, "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444"),
            (2049, "5f4d72f40d7a5f82b15ca2b2e44b1de3c2ef86c426c95c1af0b6879522563030"),
            (3072, "b98cb0ff3623be03326b373de6b9095218513e64f1ee2edd2525c7ad1e5cffd2"),
        ];
        for &(len, want) in cases {
            assert_eq!(hex(&digest(&vector_input(len))), want, "len={len}");
        }
    }

    /// 流式分片必须与一次性结果相同（分片边界故意选在 64 / 1024 附近）。
    /// Streaming with awkward split points must equal the one-shot digest.
    #[test]
    fn streaming_matches_one_shot() {
        let data = vector_input(4097);
        let want = digest(&data);
        for split in [1usize, 63, 64, 65, 1023, 1024, 1025, 2048] {
            let mut h = Blake3::new();
            for part in data.chunks(split) {
                h.update(part);
            }
            assert_eq!(h.finalize(), want, "split={split}");
        }
    }

    #[test]
    fn empty_is_the_known_digest() {
        assert_eq!(
            hex(&digest(b"")),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }
}
