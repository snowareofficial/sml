// SPDX-License-Identifier: MulanPSL-2.0
// blake3.mjs — dependency-free BLAKE3 (unkeyed 32-byte digest) for the `.ambr` decoder.
// blake3.mjs —— 零依赖 BLAKE3（无密钥 32 B 摘要），供 `hash_id = 0x32` 校验段摘要。
//
// 与 `rust/amberice/src/blake3.rs` 同构：1 KiB 分块、chunk 链、父节点合并、`ROOT` 收尾。
// 不做 keyed / derive_key / XOF —— 段头只要 32 B。
// ｜ Mirrors the Rust scalar implementation; no keyed mode, no XOF.

const OUT_LEN = 32;
const BLOCK_LEN = 64;
const CHUNK_LEN = 1024;
const MAX_DEPTH = 54;

const CHUNK_START = 1 << 0;
const CHUNK_END = 1 << 1;
const PARENT = 1 << 2;
const ROOT = 1 << 3;

const IV = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
            0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];

const MSG_PERMUTATION = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

const rotr = (x, n) => ((x >>> n) | (x << (32 - n))) >>> 0;

function g(s, a, b, c, d, mx, my) {
  s[a] = (s[a] + s[b] + mx) >>> 0;
  s[d] = rotr(s[d] ^ s[a], 16);
  s[c] = (s[c] + s[d]) >>> 0;
  s[b] = rotr(s[b] ^ s[c], 12);
  s[a] = (s[a] + s[b] + my) >>> 0;
  s[d] = rotr(s[d] ^ s[a], 8);
  s[c] = (s[c] + s[d]) >>> 0;
  s[b] = rotr(s[b] ^ s[c], 7);
}

function round(s, m) {
  g(s, 0, 4, 8, 12, m[0], m[1]);
  g(s, 1, 5, 9, 13, m[2], m[3]);
  g(s, 2, 6, 10, 14, m[4], m[5]);
  g(s, 3, 7, 11, 15, m[6], m[7]);
  g(s, 0, 5, 10, 15, m[8], m[9]);
  g(s, 1, 6, 11, 12, m[10], m[11]);
  g(s, 2, 7, 8, 13, m[12], m[13]);
  g(s, 3, 4, 9, 14, m[14], m[15]);
}

function permute(m) {
  const old = m.slice();
  for (let i = 0; i < 16; i++) m[i] = old[MSG_PERMUTATION[i]];
}

function compress(cv, blockWords, counter, blockLen, flags) {
  const s = [
    cv[0], cv[1], cv[2], cv[3], cv[4], cv[5], cv[6], cv[7],
    IV[0], IV[1], IV[2], IV[3],
    counter >>> 0, Math.floor(counter / 4294967296), blockLen >>> 0, flags >>> 0,
  ];
  const m = blockWords.slice();
  for (let i = 0; i < 7; i++) {
    round(s, m);
    permute(m);
  }
  for (let i = 0; i < 8; i++) {
    s[i] = (s[i] ^ s[i + 8]) >>> 0;
    s[i + 8] = (s[i + 8] ^ cv[i]) >>> 0;
  }
  return s;
}

function wordsFromLeBytes(buf, len) {
  const words = new Array(16).fill(0);
  const full = Math.floor(len / 4);
  // DataView 覆盖整个缓冲区：末尾不足 4 B 时不能按 u32 读。
  const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  for (let i = 0; i < full; i++) words[i] = dv.getUint32(i * 4, true);
  const rem = len % 4;
  if (rem) {
    let w = 0;
    for (let i = 0; i < rem; i++) w |= buf[full * 4 + i] << (8 * i);
    words[full] = w >>> 0;
  }
  return words;
}

class ChunkState {
  constructor(counter) {
    this.cv = IV.slice();
    this.counter = counter;
    this.buf = new Uint8Array(BLOCK_LEN);
    this.bufLen = 0;
    this.blocksCompressed = 0;
  }

  get len() {
    return BLOCK_LEN * this.blocksCompressed + this.bufLen;
  }

  startFlag() {
    return this.blocksCompressed === 0 ? CHUNK_START : 0;
  }

  update(input) {
    let at = 0;
    while (at < input.length) {
      if (this.bufLen === BLOCK_LEN) {
        const words = wordsFromLeBytes(this.buf, BLOCK_LEN);
        const state = compress(this.cv, words, this.counter, BLOCK_LEN, this.startFlag());
        this.cv = state.slice(0, 8);
        this.blocksCompressed++;
        this.buf = new Uint8Array(BLOCK_LEN);
        this.bufLen = 0;
      }
      const take = Math.min(BLOCK_LEN - this.bufLen, input.length - at);
      this.buf.set(input.subarray(at, at + take), this.bufLen);
      this.bufLen += take;
      at += take;
    }
  }

  output() {
    return {
      inputCv: this.cv,
      blockWords: wordsFromLeBytes(this.buf, this.bufLen),
      counter: this.counter,
      blockLen: this.bufLen,
      flags: this.startFlag() | CHUNK_END,
    };
  }
}

function chainingValue(o) {
  return compress(o.inputCv, o.blockWords, o.counter, o.blockLen, o.flags).slice(0, 8);
}

function parentOutput(left, right) {
  const words = new Array(16);
  for (let i = 0; i < 8; i++) {
    words[i] = left[i];
    words[i + 8] = right[i];
  }
  return { inputCv: IV.slice(), blockWords: words, counter: 0, blockLen: BLOCK_LEN, flags: PARENT };
}

function parentCv(left, right) {
  return chainingValue(parentOutput(left, right));
}

/// BLAKE3 of a byte array → 32-byte Uint8Array. ｜ 对字节数组求 BLAKE3（32 B）。
/// @param {Uint8Array} input
/// @returns {Uint8Array}
export function blake3(input) {
  let chunk = new ChunkState(0);
  const stack = [];
  let at = 0;

  while (at < input.length) {
    if (chunk.len === CHUNK_LEN) {
      const cv = chainingValue(chunk.output());
      let total = chunk.counter + 1;
      stack.push(cv);
      // 按「total 为偶数就合并」收敛成二叉树。
      while (total % 2 === 0 && stack.length >= 2) {
        const right = stack.pop();
        const left = stack.pop();
        stack.push(parentCv(left, right));
        total /= 2;
      }
      chunk = new ChunkState(chunk.counter + 1);
    }
    const take = Math.min(CHUNK_LEN - chunk.len, input.length - at);
    chunk.update(input.subarray(at, at + take));
    at += take;
  }

  let output = chunk.output();
  for (let i = stack.length - 1; i >= 0; i--) {
    output = parentOutput(stack[i], chainingValue(output));
  }

  // 根输出：计数器固定 0，并带 ROOT 标志。
  const state = compress(output.inputCv, output.blockWords, 0, output.blockLen, output.flags | ROOT);
  const out = new Uint8Array(OUT_LEN);
  const dv = new DataView(out.buffer);
  for (let i = 0; i < 8; i++) dv.setUint32(i * 4, state[i], true);
  return out;
}

export default blake3;
