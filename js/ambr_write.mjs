// SPDX-License-Identifier: MulanPSL-2.0
//! `.ambr` writer for JS — the same bytes the Rust encoder produces (wasm, no reimplementation).
//! `.ambr` 写入器（JS 端）—— 直接用 Rust 编码器出的 wasm，不重写编码逻辑。
//!
//! Scope ｜ 范围:
//! - `pack(text)` → `.ambr` bytes, via `ambr_pack_text` (SML 源文本 → canonical 字节);
//! - `canonicalText(bytes)` → canonical text, via `ambr_text` (写出来的能自己读回去);
//! - `caps()` → capability bits (`1` = READ, `2` = WRITE, same values as `ambr_caps()`).
//! ｜ 写入走 `ambr_pack_text`，回读走 `ambr_text`；能力位与 C ABI 逐位一致。
//!
//! Why wasm and not a JS encoder / 为什么用 wasm 而不是 JS 重写编码器:
//!   canonical 字节是**跨端契约**（`tests/vectors/amberice-container.v1.json`）。JS 端
//!   再写一遍 strtab 去重、段对齐、SHA-256、对象源序，等于多一份会漂移的实现；
//!   而 Rust 编码器已经在 9 端之间对拍过。这里只搬字节，不搬规则。
//!   ｜ Canonical bytes are a cross-end contract; a second encoder in JS would drift.
//!
//! Loading ｜ 加载:
//!   `amberice-ffi` 是纯 C-ABI 的 cdylib（无 `wasm-bindgen`），所以这里用裸
//!   `WebAssembly.instantiate` —— 与 `site/static/sml-rs.js` 同一套做法。
//!   同一份文件在浏览器（fetch）与 Node（fs）都能跑，不分支实现。
//!
//! ```js
//! import { pack, loadAmbrWriter } from "./ambr_write.mjs";
//! const bytes = await pack("a: 1\nb: [ x y ]\n");   // 首次调用自动加载 js/build/amberice_ffi.wasm
//! const w = await loadAmbrWriter();                  // 或自己持有实例，避免重复实例化
//! w.pack("a: 1\n");                                  // 同步版本
//! ```
//!
//! 产物缺失时：`python js/build_ambr_wasm.py`。
//!
//! Not implemented ｜ 尚未做: 压缩 / 签名段（`comp != 0` / `sign` feature）—— 编码器默认不产，
//! 需要时由 Rust 侧开 feature，本模块不加开关。

import { AmbrError } from "./ambr.mjs";

export { AmbrError };

/// Capability bits — same values as `ambr_caps()` in amberice-ffi.
/// 能力位 —— 与 FFI 的 `ambr_caps()` 逐位一致。
export const CAP_READ = 1 << 0;
export const CAP_WRITE = 1 << 1;

const ERR_SIZE = 256;
const DEFAULT_WASM = new URL("./build/amberice_ffi.wasm", import.meta.url);
const enc = new TextEncoder();
const dec = new TextDecoder("utf-8", { fatal: true });

const isNode =
  typeof process !== "undefined" && !!process.versions && !!process.versions.node;

/// Pointer returns are i32 on wasm32; normalise to unsigned, 0 means NULL.
/// ｜ wasm32 的指针是 i32，统一转无符号；0 视为 NULL。
function ptr(p) {
  return p >>> 0;
}

function readCStr(ex, p) {
  if (!p) return "";
  const mem = new Uint8Array(ex.memory.buffer);
  let end = p;
  while (end < mem.length && mem[end] !== 0) end++;
  return dec.decode(mem.subarray(p, end));
}

/// Wrap an instantiated wasm module (or its exports) into a writer.
/// 把已实例化的 wasm 模块（或其 exports）包成写入器。
export function createAmbrWriter(instance) {
  const ex = instance?.exports ?? instance;
  for (const name of ["ambr_pack_text", "ambr_alloc", "ambr_dealloc", "ambr_caps"]) {
    if (typeof ex?.[name] !== "function") {
      throw new AmbrError(
        `wasm 模块缺符号 ${name} —— 请用 --features sml 重新构建 amberice-ffi / ` +
          `missing export ${name}, rebuild amberice-ffi with --features sml`
      );
    }
  }

  const caps = () => ex.ambr_caps() >>> 0;
  const writable = () => (caps() & CAP_WRITE) !== 0;

  /// Copy `text` into linear memory. Returns [ptr, byteLength, capacity].
  /// 把文本写进线性内存，返回 [指针, 字节数, 申请长度]（不写 NUL：长度是显式参数）。
  function putText(text) {
    const bytes = enc.encode(text);
    const cap = Math.max(bytes.length, 1);
    const p = ptr(ex.ambr_alloc(cap));
    if (!p) throw new AmbrError("AMBR: [E-AMBR-007] ambr_alloc 返回 NULL（内存不足？）");
    new Uint8Array(ex.memory.buffer, p, bytes.length).set(bytes);
    return [p, bytes.length, cap];
  }

  function throwFromErr(pErr, fallback) {
    const msg = readCStr(ex, pErr);
    throw new AmbrError(msg || fallback);
  }

  /// Pack SML source text into `.ambr` bytes. Throws `AmbrError` on failure.
  /// 把 SML 源文本打成 `.ambr` 字节；失败抛 `AmbrError`。
  function pack(text) {
    if (typeof text !== "string") {
      throw new TypeError("pack(text): text 必须是 String / must be a string");
    }
    if (text.length === 0) throw new AmbrError("AMBR: [E-AMBR-001] empty input");
    if (!writable()) {
      throw new AmbrError(
        "AMBR: [E-AMBR-003] this build is read-only — rebuild amberice-ffi with --features sml / " +
          "本次构建只读：请用 --features sml 重新构建 amberice-ffi"
      );
    }

    const [pIn, len, inCap] = putText(text);
    const pLen = ptr(ex.ambr_alloc(4));
    const pErr = ptr(ex.ambr_alloc(ERR_SIZE));
    try {
      const pOut = ptr(ex.ambr_pack_text(pIn, len, pLen, pErr, ERR_SIZE));
      if (!pOut) throwFromErr(pErr, "AMBR: [E-AMBR-008] encode failed (no message)");
      const n = new DataView(ex.memory.buffer).getUint32(pLen, true);
      if (n === 0) {
        ex.ambr_free_bytes(pOut, 0);
        throw new AmbrError("AMBR: [E-AMBR-008] encoder returned 0 bytes");
      }
      // Copy out: the returned slice lives in linear memory, which may grow later.
      // ｜ 先拷出来：返回的切片在线性内存里，后续分配可能让它失效。
      const out = new Uint8Array(ex.memory.buffer, pOut, n).slice();
      ex.ambr_free_bytes(pOut, n);
      return out;
    } finally {
      ex.ambr_dealloc(pIn, inCap);
      ex.ambr_dealloc(pLen, 4);
      ex.ambr_dealloc(pErr, ERR_SIZE);
    }
  }

  /// Decode `.ambr` bytes into canonical text (the same ABI the other bindings use).
  /// 把 `.ambr` 字节解成 canonical 文本（与其余各端同一符号）。
  function canonicalText(input) {
    const b = input instanceof Uint8Array ? input : new Uint8Array(input);
    if (b.length < 40) throw new AmbrError("AMBR: [E-AMBR-002] 文件短于 40 B 头");
    const cap = Math.max(b.length, 1);
    const p = ptr(ex.ambr_alloc(cap));
    if (!p) throw new AmbrError("AMBR: [E-AMBR-007] ambr_alloc 返回 NULL（内存不足？）");
    const pErr = ptr(ex.ambr_alloc(ERR_SIZE));
    new Uint8Array(ex.memory.buffer, p, b.length).set(b);
    try {
      const pOut = ptr(ex.ambr_text(p, b.length, pErr, ERR_SIZE));
      if (!pOut) throwFromErr(pErr, "AMBR: [E-AMBR-008] decode failed (no message)");
      try {
        return readCStr(ex, pOut);
      } finally {
        ex.ambr_free_str(pOut);
      }
    } finally {
      ex.ambr_dealloc(p, cap);
      ex.ambr_dealloc(pErr, ERR_SIZE);
    }
  }

  return { caps, writable, pack, canonicalText, exports: ex };
}

/// Read a wasm source into bytes. Accepts ArrayBuffer / TypedArray / Module / URL / path.
/// ｜ 把 wasm 源读成字节：接受 ArrayBuffer / TypedArray / Module / URL / 路径。
async function readSource(source) {
  if (source instanceof WebAssembly.Module) return { module: source };
  if (source instanceof ArrayBuffer) return { bytes: new Uint8Array(source) };
  if (ArrayBuffer.isView(source)) {
    return { bytes: new Uint8Array(source.buffer, source.byteOffset, source.byteLength) };
  }

  const looksBare = typeof source === "string" && !/^[a-z][a-z0-9+.-]*:/i.test(source);
  const isFileUrl =
    (source instanceof URL && source.protocol === "file:") ||
    (typeof source === "string" && source.startsWith("file:"));
  if (isNode && (looksBare || isFileUrl)) {
    const { readFile } = await import("node:fs/promises");
    const { fileURLToPath } = await import("node:url");
    const path = source instanceof URL ? fileURLToPath(source)
      : looksBare ? source
        : fileURLToPath(new URL(source));
    return { bytes: new Uint8Array(await readFile(path)) };
  }

  const res = await fetch(source);
  if (!res.ok) {
    throw new AmbrError(`加载 wasm 失败：HTTP ${res.status} (${String(source)})`);
  }
  return { bytes: new Uint8Array(await res.arrayBuffer()) };
}

/// Load the writer from a wasm source (default: `js/build/amberice_ffi.wasm`).
/// ｜ 从 wasm 源加载写入器（默认 `js/build/amberice_ffi.wasm`）。
export async function loadAmbrWriter(source = DEFAULT_WASM, options = {}) {
  let mod;
  try {
    const src = await readSource(source);
    mod = src.module ?? new WebAssembly.Module(src.bytes);
  } catch (e) {
    if (e instanceof AmbrError) throw e;
    throw new AmbrError(
      `加载 wasm 失败：${e.message} —— 先跑 python js/build_ambr_wasm.py / ` +
        `run python js/build_ambr_wasm.py first`
    );
  }
  const instance = await WebAssembly.instantiate(mod, options.imports ?? {});
  return createAmbrWriter(instance.instance ?? instance);
}

let defaultPending = null;

/// Lazily instantiated default writer (one wasm instance per process).
/// ｜ 默认写入器：整个进程共用一份 wasm 实例，按需初始化。
export function defaultWriter(source = DEFAULT_WASM) {
  if (!defaultPending) {
    defaultPending = loadAmbrWriter(source).catch((e) => {
      defaultPending = null; // 允许修好产物后重试 ｜ let a later attempt retry
      throw e;
    });
  }
  return defaultPending;
}

/// Convenience: `await pack(text)` — initialises the default writer on first call.
/// ｜ 便捷入口：首次调用自动初始化默认写入器。
export async function pack(text, source = DEFAULT_WASM) {
  return (await defaultWriter(source)).pack(text);
}

export default { pack, loadAmbrWriter, createAmbrWriter, defaultWriter, AmbrError,
  CAP_READ, CAP_WRITE };
