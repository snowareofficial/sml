// SPDX-License-Identifier: MulanPSL-2.0
//! `.ambr` read-only decoder for JS (T7 / N03).
//! `.ambr` 只读解码器（JS 端，T7 / N03）。
//!
//! Scope ｜ 范围:
//! - parses the 40 B header, walks the 41 B segment table, reads `VALUE` (0x01) and
//!   `STRTAB` (0x02), and **skips every other segment** (table entries carry `len`);
//! - does **not** interpret `TYPED` (0x70) / `DIR` (0x03) / `MANIFEST` (0x04) semantics —
//!   that is the whole point of "unknown segments are skippable" (spec §23.1);
//! - refuses compressed (`comp != 0`) or encrypted (`enc != 0`) segments instead of guessing.
//! ｜ 只解 VALUE / STRTAB，其余段按 `len` 跳过；不解释 TYPED/DIR/MANIFEST 语义。
//!
//! Value mapping ｜ 值映射 (same as `js/sml.mjs`, so both paths compare directly):
//! `Int` within ±2^53-1 → `Number`, beyond → decimal `String`; `Float` → `Number`;
//! objects keep **source order** (plain object insertion order).
//! ｜ 整数超出 ±2^53-1 时转字符串（与解析器一致），对象保留源序。
//!
//! Not implemented yet ｜ 尚未做: digest verification (needs SHA-256; do it in the harness
//! before trusting a file) and write-back (a read-only port is conformant by design).

import { sha256, digestEqual } from "./sha256.mjs";

const MAGIC = [0x41, 0x4d, 0x42, 0x52]; // "AMBR"
const SEG_VALUE = 0x01;
const SEG_STRTAB = 0x02;
const ALIGN_MIN = 8;
const ALIGN_BLOCK = 4096;
const NODE_HEADER = 6; // tag:u8 | flags:u8 | subtree_len:u32
const SEG_HEADER = 41; // id/comp/enc/flags/len/sha256 (spec §19.1)

const INT_MAX = 9007199254740991n; // Number.MAX_SAFE_INTEGER
const INT_MIN = -9007199254740991n;

/// Decode error with a byte offset. ｜ 带字节偏移的解码错误。
export class AmbrError extends Error {
  constructor(message, at) {
    super(at === undefined ? message : `${message} (at byte ${at})`);
    this.name = "AmbrError";
    this.at = at;
  }
}

function alignUp(x, granule) {
  return Math.ceil(x / granule) * granule;
}

function readU16(b, o) {
  return b[o] | (b[o + 1] << 8);
}

function readU32(b, o) {
  return (b[o] | (b[o + 1] << 8) | (b[o + 2] << 16) | (b[o + 3] << 24)) >>> 0;
}

// u64 as Number: `.ambr` files are far below 2^53 bytes; a larger value is a corrupt header.
// u64 转 Number：文件远小于 2^53，超出即头部损坏。
function readU64(b, o) {
  const lo = readU32(b, o);
  const hi = readU32(b, o + 4);
  if (hi > 0x1fffff) throw new AmbrError("u64 超出安全整数范围", o);
  return hi * 4294967296 + lo;
}

class Reader {
  constructor(bytes, pos = 0, end = bytes.length) {
    this.b = bytes;
    this.pos = pos;
    this.end = end;
  }

  eof() {
    return this.pos >= this.end;
  }

  u8() {
    if (this.pos >= this.end) throw new AmbrError("读取越界", this.pos);
    return this.b[this.pos++];
  }

  u32() {
    if (this.pos + 4 > this.end) throw new AmbrError("读取越界", this.pos);
    const v = readU32(this.b, this.pos);
    this.pos += 4;
    return v;
  }

  f64() {
    if (this.pos + 8 > this.end) throw new AmbrError("读取越界", this.pos);
    const v = new DataView(this.b.buffer, this.b.byteOffset + this.pos, 8).getFloat64(0, true);
    this.pos += 8;
    return v;
  }

  /// Unsigned LEB128, max 10 bytes (u64). ｜ 无符号 LEB128，最多 10 字节。
  varint() {
    let shift = 0n;
    let out = 0n;
    for (let i = 0; i < 10; i++) {
      const byte = this.u8();
      out |= BigInt(byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) return out;
      shift += 7n;
    }
    throw new AmbrError("LEB128 超过 10 字节", this.pos);
  }
}

function intFromZigzag(z) {
  const n = (z >> 1n) ^ -(z & 1n);
  return n > INT_MAX || n < INT_MIN ? n.toString() : Number(n);
}

/// Decode one node; returns the value and advances `r`.
/// 解一个节点，返回其值并推进 `r`。
function decodeNode(r, strtab, depth, maxDepth) {
  if (depth > maxDepth) throw new AmbrError("嵌套超过上限", r.pos);
  const at = r.pos;
  const tag = r.u8();
  r.u8(); // flags: bit 0 = "this object has __type" — an optimisation hint only
  const subtreeLen = r.u32();
  const payloadStart = r.pos;
  let value;

  switch (tag) {
    case 0: // Null
      value = null;
      break;
    case 1: // False
      value = false;
      break;
    case 2: // True
      value = true;
      break;
    case 3: // Int (zigzag)
      value = intFromZigzag(r.varint());
      break;
    case 4: // Float (f64 LE)
      value = r.f64();
      break;
    case 5: {
      // Str: STRTAB id
      const id = Number(r.varint());
      if (!Number.isInteger(id) || id < 0 || id >= strtab.length) {
        throw new AmbrError(`字符串 id ${id} 越界`, at);
      }
      value = strtab[id];
      break;
    }
    case 6: {
      // Arr: count + nodes
      const count = Number(r.varint());
      const arr = new Array(count);
      for (let i = 0; i < count; i++) arr[i] = decodeNode(r, strtab, depth + 1, maxDepth);
      value = arr;
      break;
    }
    case 7: {
      // Obj: count + (key_id, node)* — source order
      const count = Number(r.varint());
      const obj = {};
      for (let i = 0; i < count; i++) {
        const keyId = Number(r.varint());
        if (!Number.isInteger(keyId) || keyId < 0 || keyId >= strtab.length) {
          throw new AmbrError(`键 id ${keyId} 越界`, at);
        }
        const key = strtab[keyId];
        if (Object.prototype.hasOwnProperty.call(obj, key)) {
          throw new AmbrError(`对象里出现重复键 ${JSON.stringify(key)}`, at);
        }
        obj[key] = decodeNode(r, strtab, depth + 1, maxDepth);
      }
      value = obj;
      break;
    }
    default:
      throw new AmbrError(`未定义的节点 tag ${tag}`, at);
  }

  const consumed = r.pos - payloadStart;
  if (consumed !== subtreeLen) {
    throw new AmbrError(
      `subtree_len 声明 ${subtreeLen}，实际消费 ${consumed}`,
      at
    );
  }
  return value;
}

/// Decode a `STRTAB` segment body: `(len:varint ‖ utf8)*` — **no** count prefix.
/// 解 `STRTAB` 段体：`(len:varint ‖ utf8)*`，**不带**表项个数。
function decodeStrtab(bytes) {
  const r = new Reader(bytes);
  const dec = new TextDecoder("utf-8", { fatal: true });
  const out = [];
  while (!r.eof()) {
    const start = r.pos;
    const len = Number(r.varint());
    if (!Number.isInteger(len) || len < 0 || r.pos + len > r.end) {
      throw new AmbrError(`STRTAB 第 ${out.length} 项长度越界`, start);
    }
    try {
      out.push(dec.decode(bytes.subarray(r.pos, r.pos + len)));
    } catch {
      throw new AmbrError(`STRTAB 第 ${out.length} 项不是合法 UTF-8`, start);
    }
    r.pos += len;
  }
  return out;
}

/// Decode a whole `.ambr` file into a plain JS value.
/// 把整份 `.ambr` 解成普通 JS 值。
///
/// @param {Uint8Array|ArrayBuffer} input
/// @param {{maxDepth?: number}} [options]
export function load(input, options = {}) {
  const b = input instanceof Uint8Array ? input : new Uint8Array(input);
  const maxDepth = options.maxDepth ?? 128;
  if (b.length < 40) throw new AmbrError("文件短于 40 B 头");

  for (let i = 0; i < 4; i++) {
    if (b[i] !== MAGIC[i]) throw new AmbrError("不是 .ambr 文件（magic 不是 AMBR）", i);
  }
  const fmt = b[6];
  const hdrLen = b[7];
  const tableOff = readU64(b, 8);
  const count = readU32(b, 16);
  const flags = readU16(b, 20);
  if (fmt !== 0) throw new AmbrError(`不支持的字节布局版本 fmt=${fmt}`, 6);
  if (hdrLen < 40) throw new AmbrError(`头部长度 ${hdrLen} 小于 40`, 7);
  if (tableOff < hdrLen) throw new AmbrError(`段表偏移 ${tableOff} 落在头内`, 8);

  const align = (flags & 1) !== 0 ? ALIGN_BLOCK : ALIGN_MIN;
  let bodyAt = alignUp(tableOff + count * SEG_HEADER, align);

  let strtabBytes = null;
  let valueBytes = null;

  for (let i = 0; i < count; i++) {
    const o = tableOff + i * SEG_HEADER;
    if (o + SEG_HEADER > b.length) throw new AmbrError("段表越界", o);
    const id = b[o];
    const comp = b[o + 1];
    const enc = b[o + 2];
    // 41 B entry: id:u8 | comp:u8 | enc:u8 | flags:u16 | len:u32 | sha256:32
    // 表项布局：`len` 在偏移 5（不是 4，`flags` 占 2 B）。
    const len = readU32(b, o + 5);
    if (bodyAt + len > b.length) throw new AmbrError(`段 0x${id.toString(16)} 长度越界`, o);
    if (id === SEG_VALUE || id === SEG_STRTAB) {
      if (comp !== 0) throw new AmbrError(`段 0x${id.toString(16)} 压缩过（comp=${comp}），本解码器不解压`, o);
      if (enc !== 0) throw new AmbrError(`段 0x${id.toString(16)} 加密过（enc=${enc}），本解码器不解密`, o);
      const body = b.subarray(bodyAt, bodyAt + len);
      // Digest check (T7 criterion: "read the header, skip, **verify the digest**").
      // ｜ T7 判据里的「验摘要」：段头 32 B 就是对段体做的 SHA-256。
      if (options.verify !== false) {
        const want = b.subarray(o + 9, o + 41);
        if (!digestEqual(sha256(body), want)) {
          throw new AmbrError(`段 0x${id.toString(16)} 摘要不符（段体被改动？）`, bodyAt);
        }
      }
      if (id === SEG_VALUE) valueBytes = body;
      else strtabBytes = body;
    }
    bodyAt = alignUp(bodyAt + len, align);
  }

  if (!valueBytes) throw new AmbrError("找不到 VALUE 段（0x01）");
  const strtab = strtabBytes ? decodeStrtab(strtabBytes) : [];
  const r = new Reader(valueBytes);
  const value = decodeNode(r, strtab, 0, maxDepth);
  if (!r.eof()) throw new AmbrError(`VALUE 段读完仍有 ${r.end - r.pos} B 剩余`, r.pos);
  return value;
}

export default { load, AmbrError };
