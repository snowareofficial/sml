// SPDX-License-Identifier: MulanPSL-2.0
//! Minimal C ABI for `.ambr` — text in / bytes out, no value handles.
//! `.ambr` 的最小 C ABI：文本进、字节出，不暴露值句柄。
//!
//! Independent crate / 独立 crate: `amberice` stays a pure rlib; this cdylib is the only
//! artifact that FFI consumers link. ｜ `amberice` 保持纯 rlib，FFI 消费方只链接这个 cdylib。
//!
//! Why text and not a tree / 为什么返回文本而不是值树:
//!   Every FFI consumer already has an SML parser (Python/C#/Go/Ruby all wrap `swsml`).
//!   Handing back **canonical text** keeps the ABI to two symbols (`ambr_text` + free) and
//!   makes the result verifiable byte-for-byte against the cross-end fixture. A tree ABI
//!   would need dozens of accessors and its own lifetime rules for no extra reach.
//!   ｜ 各 FFI 端本来就有 SML 解析器；返回 canonical 文本只需两个符号，且能与向量逐字节比对。
//!
//! Symbols / 符号: `ambr_text` + `ambr_free_str` (read), `ambr_pack_text` +
//! `ambr_free_bytes` (write, `sml` feature), `ambr_caps` (capability bits), and
//! `ambr_alloc` + `ambr_dealloc` (input buffers for hosts without a allocator, e.g. wasm).
//! ｜ 读两个、写两个、能力位一个、输入缓冲两个。
//!
//! Errors / 错误: message starts with `AMBR: `; the buffer is NUL-terminated and truncated,
//! never overrun (`errsz == 0` is allowed). ｜ 错误写进调用方缓冲，绝不越界。

use std::os::raw::{c_char, c_uchar};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Capability bit: decode (`ambr_text`).
/// 能力位：解码（`ambr_text`）。
pub const AMBR_CAP_READ: u32 = 1 << 0;
/// Capability bit: encode (`ambr_pack_text`, requires the `sml` feature).
/// 能力位：编码（`ambr_pack_text`，需要 `sml` feature）。
pub const AMBR_CAP_WRITE: u32 = 1 << 1;

/// Capabilities of **this** build. Callers branch on it instead of guessing whether a
/// symbol exists; that keeps one source for every language and makes "read-only build"
/// an explicit, queryable fact rather than a link error.
/// ｜ 本次构建的能力位。调用方查它，而不是猜符号在不在 —— 同一份源码适配两种构建，
/// 「只读构建」变成可查询的事实，而不是链接错误。
#[no_mangle]
pub extern "C" fn ambr_caps() -> u32 {
    AMBR_CAP_READ | if cfg!(feature = "sml") { AMBR_CAP_WRITE } else { 0 }
}

/// Copy `msg` into the caller's buffer (NUL-terminated, truncated, never overrun).
/// 把错误写进调用方缓冲（NUL 结尾、可截断、不越界）。
unsafe fn write_err(err: *mut c_char, errsz: usize, msg: &str) {
    if err.is_null() || errsz == 0 {
        return;
    }
    let bytes = msg.as_bytes();
    let n = bytes.len().min(errsz - 1);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), err as *mut u8, n);
    *err.add(n) = 0;
}

/// Decode `.ambr` bytes into **canonical text**. Returns NULL on failure.
/// 把 `.ambr` 解成 **canonical 文本**；失败返回 NULL。
///
/// The returned string is owned by the caller: release with [`ambr_free_str`].
/// ｜ 返回的字符串归调用方，用 [`ambr_free_str`] 释放。
#[no_mangle]
pub unsafe extern "C" fn ambr_text(
    buf: *const c_uchar,
    len: usize,
    err: *mut c_char,
    errsz: usize,
) -> *mut c_char {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if buf.is_null() {
            return Err("AMBR: null pointer (空指针)".to_string());
        }
        let bytes = std::slice::from_raw_parts(buf, len);
        let value = amberice::load(bytes).map_err(|e| format!("AMBR: {e}"))?;
        value
            .canonical_text()
            .map_err(|e| format!("AMBR: {e}"))
    }));

    match outcome {
        Ok(Ok(text)) => match std::ffi::CString::new(text) {
            Ok(c) => c.into_raw(),
            Err(_) => {
                write_err(err, errsz, "AMBR: text contains NUL (文本含 NUL)");
                std::ptr::null_mut()
            }
        },
        Ok(Err(msg)) => {
            write_err(err, errsz, &msg);
            std::ptr::null_mut()
        }
        Err(_) => {
            write_err(err, errsz, "AMBR: decoder panic (解码器 panic)");
            std::ptr::null_mut()
        }
    }
}

/// Allocate a caller-owned buffer to hold input bytes. Free with [`ambr_dealloc`].
/// 分配一块调用方自有的输入缓冲；用 [`ambr_dealloc`] 释放。
///
/// Why this symbol exists / 为什么需要它: a wasm host has no `malloc` — without an
/// allocator on this side, `ambr_pack_text` and `ambr_text` are unreachable from
/// `WebAssembly.instantiate` (the JS binding writes the source text into linear
/// memory first). Native callers pass their own pointers and never need it.
/// ｜ wasm 宿主没有 `malloc`；没有这个符号，裸 `WebAssembly.instantiate` 无法把
/// 源文本写进线性内存。原生调用方自带指针，用不到它。
#[no_mangle]
pub extern "C" fn ambr_alloc(len: usize) -> *mut c_uchar {
    let mut buf: Vec<u8> = vec![0u8; len];
    let p = buf.as_mut_ptr();
    std::mem::forget(buf); // ownership moves to the host ｜ 所有权移交宿主
    p
}

/// Release a buffer from [`ambr_alloc`]. `len` must be the length it was asked for.
/// NULL and `len == 0` are no-ops. ｜ 释放 [`ambr_alloc`] 的缓冲；`len` 必须与申请时一致。
#[no_mangle]
pub unsafe extern "C" fn ambr_dealloc(p: *mut c_uchar, len: usize) {
    if p.is_null() || len == 0 {
        return;
    }
    // len = 0, cap = len: recycle the allocation without dropping uninitialised bytes.
    // ｜ 不构造元素，只回收分配。
    let _ = Vec::<u8>::from_raw_parts(p, 0, len);
}

/// Release a string returned by [`ambr_text`]. NULL is a no-op.
/// 释放 [`ambr_text`] 返回的字符串；NULL 是空操作。
#[no_mangle]
pub unsafe extern "C" fn ambr_free_str(p: *mut c_char) {
    if !p.is_null() {
        drop(std::ffi::CString::from_raw(p));
    }
}

/// Pack SML source text into `.ambr` bytes. Requires the `sml` feature.
/// 把 SML 源文本打成 `.ambr` 字节；需要 `sml` feature。
///
/// The parse lives here, not in `amberice`: the container layer must stay usable
/// without a parser (spec §13.1). Objects keep source order via `parse_ordered`,
/// which is what canonical bytes require.
/// ｜ 解析放在这里而不是 `amberice`：容器层必须在没有解析器时也能用。
/// 用 `parse_ordered` 保源序 —— canonical 字节的前提。
///
/// Returns NULL on failure; `*out_len` is 0 then. Free with [`ambr_free_bytes`].
/// 失败返回 NULL（此时 `*out_len` 为 0）；用 [`ambr_free_bytes`] 释放。
#[cfg(feature = "sml")]
#[no_mangle]
pub unsafe extern "C" fn ambr_pack_text(
    text: *const c_char,
    len: usize,
    out_len: *mut usize,
    err: *mut c_char,
    errsz: usize,
) -> *mut c_uchar {
    if !out_len.is_null() {
        *out_len = 0;
    }

    /// Move metadata keys to the end of every object.
    ///
    /// `canonical_text` puts `__type` / `__name` / `__featags` **first** (C10, for
    /// readability), but the byte side stores **source order**, where the parser appends
    /// them last. Parsing canonical text and dumping it straight would reorder the object
    /// and change the bytes (conformance: typed-block / featags-nested).
    /// ｜ 文本侧元数据在前（C10），字节侧在末尾（源序）；直接 dump 会换序。
    fn metadata_last(v: sml::Value) -> sml::Value {
        // `Value` implements `Drop`, so match by reference and clone the children.
        // ｜ `Value` 实现了 `Drop`，按引用匹配、克隆子节点。
        if let sml::Value::Object(m) = &v {
            let mut plain: Vec<(String, sml::Value)> = Vec::new();
            let mut meta: Vec<(String, sml::Value)> = Vec::new();
            for (k, val) in m {
                let val = metadata_last(val.clone());
                if k == "__type" || k == "__name" || k == "__featags" {
                    meta.push((k.to_string(), val));
                } else {
                    plain.push((k.to_string(), val));
                }
            }
            plain.extend(meta);
            let mut out = sml::Map::ordered();
            for (k, val) in plain {
                out.insert(k, val);
            }
            return sml::Value::Object(out);
        }
        if let sml::Value::Array(a) = &v {
            return sml::Value::Array(a.iter().map(|x| metadata_last(x.clone())).collect());
        }
        v
    }

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if text.is_null() {
            return Err("AMBR: null pointer (空指针)".to_string());
        }
        let raw = std::slice::from_raw_parts(text as *const u8, len);
        let src = std::str::from_utf8(raw)
            .map_err(|e| format!("AMBR: invalid UTF-8 / 无效 UTF-8: {e}"))?;
        // `allow_top_scalar` 是必需的：`canonical_text` 会输出 `42\n` / `null\n`，
        // 默认入口按 W16 报 E-PARSE-008；写路径必须能读回自己写出的东西。
        // ｜ Must opt in: canonical_text can emit a lone scalar, and this path reads it back.
        let opts = sml::ParseOptions::new().ordered().allow_top_scalar();
        let out = sml::parse_with(src, opts).map_err(|e| format!("AMBR: {e}"))?;
        let canonical = amberice::CanonicalValue::try_from(metadata_last(out.value))
            .map_err(|e| format!("AMBR: not canonical / 非 canonical: {e}"))?;
        amberice::dump(&canonical).map_err(|e| format!("AMBR: {e}"))
    }));

    match outcome {
        Ok(Ok(bytes)) => {
            let mut boxed = bytes.into_boxed_slice();
            let n = boxed.len();
            let p = boxed.as_mut_ptr();
            std::mem::forget(boxed);
            if !out_len.is_null() {
                *out_len = n;
            }
            p
        }
        Ok(Err(msg)) => {
            write_err(err, errsz, &msg);
            std::ptr::null_mut()
        }
        Err(_) => {
            write_err(err, errsz, "AMBR: encoder panic (编码器 panic)");
            std::ptr::null_mut()
        }
    }
}

/// Release bytes returned by [`ambr_pack_text`]. `len` must be the `out_len` it wrote.
/// 释放 [`ambr_pack_text`] 返回的字节；`len` 必须是它写出的 `out_len`。
#[no_mangle]
pub unsafe extern "C" fn ambr_free_bytes(p: *mut c_uchar, len: usize) {
    if !p.is_null() && len > 0 {
        drop(Box::from_raw(std::slice::from_raw_parts_mut(p, len) as *mut [u8]));
    }
}
