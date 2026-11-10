// SPDX-License-Identifier: MulanPSL-2.0
//! `.ambr` read-only decoder for JS (T7 / N03).
//! `.ambr` 只读解码器（JS 端，T7 / N03）。
//!
//! Scope ｜ 范围:
//! - parses the 40 B header, walks the 41 B segment table, reads `VALUE` (0x01) and
//!   `STRTAB` (0x02);
//! - `loadTyped()` additionally reads `TYPED` (0x70) and `SCHEMA` (0x71) and renders the
//!   typed variables (D45); `DIR` / `MANIFEST` / other segments stay skippable (§23.1);
//! - refuses compressed (`comp != 0`) or encrypted (`enc != 0`) segments instead of guessing.
//! ｜ 主路径解 VALUE / STRTAB；`loadTyped` 另解 TYPED + SCHEMA，其余段仍按 `len` 跳过。
//!
//! Value mapping ｜ 值映射 (same as `js/sml.mjs`, so both paths compare directly):
//! `Int` within ±2^53-1 → `Number`, beyond → decimal `String`; `Float` → `Number`;
//! objects keep **source order** (plain object insertion order).
//! ｜ 整数超出 ±2^53-1 时转字符串（与解析器一致），对象保留源序。
//!
//! Not implemented here ｜ 本文件不做: digest verification on write and the write path itself —
//! hashing is in `sha256.mjs`, and encoding lives in [`./ambr_write.mjs`] (Rust encoder via wasm,
//! so canonical bytes cannot drift). ｜ 摘要与编码器不在本文件：写入见 `ambr_write.mjs`。

import { sha256, digestEqual } from "./sha256.mjs";

const MAGIC = [0x41, 0x4d, 0x42, 0x52]; // "AMBR"
const SEG_VALUE = 0x01;
const SEG_STRTAB = 0x02;
const SEG_TYPED = 0x70;
const SEG_SCHEMA = 0x71;
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
/// Walk the container and return the segment bodies we care about.
/// 走一遍容器，返回关心的段体。
function walk(input, options = {}) {
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
  const parts = {
    valueBytes: null,
    strtabBytes: null,
    typedBytes: null,
    schemaBytes: null,
    maxDepth,
  };

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
    const wanted =
      id === SEG_VALUE || id === SEG_STRTAB || id === SEG_TYPED || id === SEG_SCHEMA;
    if (wanted) {
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
      if (id === SEG_VALUE) parts.valueBytes = body;
      else if (id === SEG_STRTAB) parts.strtabBytes = body;
      else if (id === SEG_TYPED) parts.typedBytes = body;
      else parts.schemaBytes = body;
    }
    bodyAt = alignUp(bodyAt + len, align);
  }
  return parts;
}

/// Decode a whole `.ambr` file into a plain JS value.
/// 把整份 `.ambr` 解成普通 JS 值。
///
/// @param {Uint8Array|ArrayBuffer} input
/// @param {{maxDepth?: number}} [options]
export function load(input, options = {}) {
  const p = walk(input, options);
  if (!p.valueBytes) throw new AmbrError("找不到 VALUE 段（0x01）");
  const strtab = p.strtabBytes ? decodeStrtab(p.strtabBytes) : [];
  const r = new Reader(p.valueBytes);
  const value = decodeNode(r, strtab, 0, p.maxDepth);
  if (!r.eof()) throw new AmbrError(`VALUE 段读完仍有 ${r.end - r.pos} B 剩余`, r.pos);
  return value;
}

// ---------------------------------------------------------------------------
// SCHEMA（0x71）+ TYPED（0x70）动态渲染（D45 / §25.8）
// ---------------------------------------------------------------------------

/// Decode self-contained L0 bytes: `count:varint ‖ strings ‖ node tree`.
/// 解自包含 L0：`count ‖ 字符串表 ‖ 节点树`。
function decodeL0(bytes, maxDepth) {
  const r = new Reader(bytes);
  const count = Number(r.varint());
  if (!Number.isInteger(count) || count < 0 || count > 1_000_000) {
    throw new AmbrError("L0 字符串个数越界", 0);
  }
  const dec = new TextDecoder("utf-8", { fatal: true });
  const strtab = [];
  for (let i = 0; i < count; i++) {
    const at = r.pos;
    const len = Number(r.varint());
    if (!Number.isInteger(len) || len < 0 || r.pos + len > r.end) {
      throw new AmbrError(`L0 字符串 ${i} 长度越界`, at);
    }
    try {
      strtab.push(dec.decode(bytes.subarray(r.pos, r.pos + len)));
    } catch {
      throw new AmbrError(`L0 字符串 ${i} 不是合法 UTF-8`, at);
    }
    r.pos += len;
  }
  const value = decodeNode(r, strtab, 0, maxDepth);
  if (!r.eof()) throw new AmbrError(`L0 读完仍有 ${r.end - r.pos} B 剩余`, r.pos);
  return value;
}

const PRIMS = new Set([
  "bool", "u8", "u16", "u32", "u64", "i8", "i16", "i32", "i64",
  "f32", "f64", "str", "bytes",
]);

function parseTy(ty) {
  if (typeof ty === "string") {
    if (PRIMS.has(ty)) return { kind: ty };
    throw new AmbrError(`未知 ty ${JSON.stringify(ty)}`);
  }
  if (ty && typeof ty === "object" && !Array.isArray(ty)) {
    if ("vec" in ty) return { kind: "vec", inner: parseTy(ty.vec) };
    if ("opt" in ty) return { kind: "opt", inner: parseTy(ty.opt) };
    if ("type" in ty) {
      if (typeof ty.type !== "string" || !/^[0-9a-f]{16}$/.test(ty.type)) {
        throw new AmbrError("type 引用必须是 16 位小写十六进制");
      }
      return { kind: "named", id: BigInt("0x" + ty.type) };
    }
  }
  throw new AmbrError("ty 必须是字符串或 {vec|opt|type}");
}

function parseSchema(schema) {
  if (!schema || typeof schema !== "object" || Array.isArray(schema)) {
    throw new AmbrError("SCHEMA 必须是对象");
  }
  if (schema.schema_version !== 1) throw new AmbrError("schema_version 只支持 1");
  if (!Array.isArray(schema.types)) throw new AmbrError("SCHEMA 缺 types");
  const out = new Map();
  for (const t of schema.types) {
    if (!t || typeof t !== "object" || Array.isArray(t)) throw new AmbrError("types[] 必须是对象");
    if (typeof t.id !== "string" || !/^[0-9a-f]{16}$/.test(t.id)) {
      throw new AmbrError("type id 必须是 16 位小写十六进制");
    }
    const id = BigInt("0x" + t.id);
    if ((t.kind ?? "struct") !== "struct") throw new AmbrError("v1 只支持 kind: struct");
    if (!Array.isArray(t.fields)) throw new AmbrError(`type ${t.id} 缺 fields`);
    const fields = [];
    let last = 0;
    for (const f of t.fields) {
      if (!f || !Number.isInteger(f.id) || f.id <= last) {
        throw new AmbrError("fields 必须按 id 严格升序");
      }
      last = f.id;
      if (typeof f.name !== "string") throw new AmbrError("field 缺 name");
      fields.push({ name: f.name, ty: parseTy(f.ty) });
    }
    if (out.has(id)) throw new AmbrError(`type id ${t.id} 重复`);
    out.set(id, { name: String(t.name), fields });
  }
  return out;
}

function typedDirectory(body) {
  const r = new Reader(body);
  const count = Number(r.varint());
  if (!Number.isInteger(count) || count < 0 || count > 1_000_000) {
    throw new AmbrError("TYPED 条目数越界");
  }
  const entries = [];
  for (let i = 0; i < count; i++) {
    const id = Number(r.varint());
    if (!Number.isInteger(id) || id <= 0) throw new AmbrError(`var_id ${id} 非法`);
    const lo = r.u32();
    const hi = r.u32();
    const typeId = (BigInt(hi) << 32n) | BigInt(lo);
    const off = r.u32();
    const len = r.u32();
    entries.push({ id, typeId, off, len });
  }
  return { entries, base: r.pos };
}

function readF32(r) {
  if (r.pos + 4 > r.end) throw new AmbrError("读取越界", r.pos);
  const v = new DataView(r.b.buffer, r.b.byteOffset + r.pos, 4).getFloat32(0, true);
  r.pos += 4;
  return v;
}

function zigzag(z) {
  return (z >> 1n) ^ -(z & 1n);
}

function intValue(n) {
  return n > INT_MAX || n < INT_MIN ? n.toString() : Number(n);
}

function decodeStruct(r, t, types, depth) {
  if (depth > 128) throw new AmbrError("TYPED 嵌套超过上限", r.pos);
  const obj = {};
  for (const f of t.fields) obj[f.name] = decodeTyped(r, f.ty, types, depth + 1);
  return obj;
}

function decodeTyped(r, ty, types, depth) {
  switch (ty.kind) {
    case "bool": {
      const v = r.varint();
      if (v === 0n) return false;
      if (v === 1n) return true;
      throw new AmbrError(`布尔的 tag 只能是 0/1，读到 ${v}`);
    }
    case "u8":
    case "u16":
    case "u32": {
      const max = { u8: 255n, u16: 65535n, u32: 4294967295n }[ty.kind];
      const v = r.varint();
      if (v > max) throw new AmbrError(`整数超出 ${ty.kind} 的范围`);
      return Number(v);
    }
    case "u64":
      return intValue(r.varint());
    case "i8":
    case "i16":
    case "i32":
    case "i64": {
      const n = zigzag(r.varint());
      const max = {
        i8: 127n, i16: 32767n, i32: 2147483647n, i64: 9223372036854775807n,
      }[ty.kind];
      if (n > max || n < -max - 1n) throw new AmbrError(`整数超出 ${ty.kind} 的范围`);
      return intValue(n);
    }
    case "f32":
      return readF32(r);
    case "f64":
      return r.f64();
    case "str": {
      const at = r.pos;
      const len = Number(r.varint());
      if (!Number.isInteger(len) || len < 0 || r.pos + len > r.end) {
        throw new AmbrError("字符串长度越界", at);
      }
      const bytes = r.b.subarray(r.pos, r.pos + len);
      r.pos += len;
      try {
        return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
      } catch {
        throw new AmbrError("字符串不是合法 UTF-8", at);
      }
    }
    case "bytes": {
      const at = r.pos;
      const len = Number(r.varint());
      if (!Number.isInteger(len) || len < 0 || r.pos + len > r.end) {
        throw new AmbrError("字节长度越界", at);
      }
      let hex = "";
      for (let i = 0; i < len; i++) hex += r.b[r.pos + i].toString(16).padStart(2, "0");
      r.pos += len;
      return hex;
    }
    case "vec": {
      const count = Number(r.varint());
      if (!Number.isInteger(count) || count < 0 || count > 1_000_000) {
        throw new AmbrError("数组元素数越界", r.pos);
      }
      const out = new Array(count);
      for (let i = 0; i < count; i++) out[i] = decodeTyped(r, ty.inner, types, depth + 1);
      return out;
    }
    case "opt": {
      const tag = r.varint();
      if (tag === 0n) return null;
      if (tag === 1n) return decodeTyped(r, ty.inner, types, depth + 1);
      throw new AmbrError(`Option 的 tag 只能是 0/1，读到 ${tag}`);
    }
    case "named": {
      const t = types.get(ty.id);
      if (!t) throw new AmbrError(`SCHEMA 里没有 type_id ${ty.id.toString(16).padStart(16, "0")}`);
      return decodeStruct(r, t, types, depth + 1);
    }
    default:
      throw new AmbrError(`未知 ty ${ty.kind}`);
  }
}

/// Render `TYPED` via an embedded `SCHEMA` (D45): `[{ id, name, value }]`.
/// 用内嵌 `SCHEMA` 渲染 `TYPED`：`[{ id, name, value }]`。
export function loadTyped(input, options = {}) {
  const p = walk(input, options);
  if (!p.typedBytes) return [];
  if (!p.schemaBytes) throw new AmbrError("没有 SCHEMA 段（0x71），无法按名渲染 TYPED");
  const types = parseSchema(decodeL0(p.schemaBytes, p.maxDepth));
  const { entries, base } = typedDirectory(p.typedBytes);
  const out = [];
  for (const e of entries) {
    const t = types.get(e.typeId);
    if (!t) throw new AmbrError(`SCHEMA 里没有 type_id ${e.typeId.toString(16).padStart(16, "0")}`);
    const start = base + e.off;
    const end = start + e.len;
    if (end > p.typedBytes.length) throw new AmbrError("TYPED 载荷越界", start);
    const r = new Reader(p.typedBytes.subarray(start, end));
    const value = decodeStruct(r, t, types, 0);
    if (!r.eof()) throw new AmbrError("TYPED 载荷解完仍有剩余字节", r.pos);
    out.push({ id: e.id, name: t.name, value });
  }
  return out;
}

export default { load, loadTyped, AmbrError };
