// SPDX-License-Identifier: MulanPSL-2.0
// sm3.mjs — dependency-free SM3 (GB/T 32905-2016) for the `.ambr` decoder.
// sm3.mjs —— 零依赖 SM3，供 `.ambr` 解码器按 manifest 的 `hash_id = 0x31` 校验段摘要。
//
// 只实现 32 B 摘要：段头固定 32 B（§19.1），不需要其他长度。
// ｜ Only the 32-byte digest is implemented, which is all a segment header can hold.

const IV = [0x7380166f, 0x4914b2b9, 0x172442d7, 0xda8a0600,
            0xa96f30bc, 0x163138aa, 0xe38dee4d, 0xb0fb0e4e];

const rotl = (x, n) => ((x << n) | (x >>> (32 - n))) >>> 0;
const p0 = (x) => (x ^ rotl(x, 9) ^ rotl(x, 17)) >>> 0;
const p1 = (x) => (x ^ rotl(x, 15) ^ rotl(x, 23)) >>> 0;

function compress(state, block, offset) {
  const w = new Array(68);
  for (let j = 0; j < 16; j++) {
    const at = offset + j * 4;
    w[j] = ((block[at] << 24) | (block[at + 1] << 16) | (block[at + 2] << 8) | block[at + 3]) >>> 0;
  }
  for (let j = 16; j < 68; j++) {
    w[j] = (p1((w[j - 16] ^ w[j - 9] ^ rotl(w[j - 3], 15)) >>> 0) ^ rotl(w[j - 13], 7) ^ w[j - 6]) >>> 0;
  }
  const w1 = new Array(64);
  for (let j = 0; j < 64; j++) w1[j] = (w[j] ^ w[j + 4]) >>> 0;

  let [a, b, c, d, e, f, g, h] = state;
  for (let j = 0; j < 64; j++) {
    const tj = j < 16 ? 0x79cc4519 : 0x7a879d8a;
    const ss1 = rotl((rotl(a, 12) + e + rotl(tj, j % 32)) >>> 0, 7);
    const ss2 = (ss1 ^ rotl(a, 12)) >>> 0;
    const ff = j < 16 ? (a ^ b ^ c) : ((a & b) | (a & c) | (b & c));
    const gg = j < 16 ? (e ^ f ^ g) : ((e & f) | (~e & g));
    const tt1 = (((ff >>> 0) + d + ss2 + w1[j]) >>> 0);
    const tt2 = (((gg >>> 0) + h + ss1 + w[j]) >>> 0);
    d = c;
    c = rotl(b, 9);
    b = a;
    a = tt1;
    h = g;
    g = rotl(f, 19);
    f = e;
    e = p0(tt2);
  }
  return [(state[0] ^ a) >>> 0, (state[1] ^ b) >>> 0, (state[2] ^ c) >>> 0, (state[3] ^ d) >>> 0,
          (state[4] ^ e) >>> 0, (state[5] ^ f) >>> 0, (state[6] ^ g) >>> 0, (state[7] ^ h) >>> 0];
}

/// SM3 of a byte array → 32-byte Uint8Array. ｜ 对字节数组求 SM3。
/// @param {Uint8Array} input
/// @returns {Uint8Array}
export function sm3(input) {
  const bitLen = input.length * 8;
  // Padding: 0x80, zeros, then 8-byte big-endian bit length.
  const withPad = Math.ceil((input.length + 9) / 64) * 64;
  const buf = new Uint8Array(withPad);
  buf.set(input);
  buf[input.length] = 0x80;
  const dv = new DataView(buf.buffer);
  // bitLen may exceed 2^32 only for absurd inputs; write hi/lo properly anyway.
  dv.setUint32(withPad - 8, Math.floor(bitLen / 4294967296));
  dv.setUint32(withPad - 4, bitLen >>> 0);

  let state = IV.slice();
  for (let offset = 0; offset < withPad; offset += 64) {
    state = compress(state, buf, offset);
  }
  const out = new Uint8Array(32);
  const odv = new DataView(out.buffer);
  for (let i = 0; i < 8; i++) odv.setUint32(i * 4, state[i]);
  return out;
}

export default sm3;
