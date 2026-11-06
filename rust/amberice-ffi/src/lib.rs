// SPDX-License-Identifier: MulanPSL-2.0
//! Minimal C ABI for `.ambr` — one function, no value handles.
//! `.ambr` 的最小 C ABI：只有一个函数，不暴露值句柄。
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
//! Errors / 错误: message starts with `AMBR: `; the buffer is NUL-terminated and truncated,
//! never overrun (`errsz == 0` is allowed). ｜ 错误写进调用方缓冲，绝不越界。

use std::os::raw::{c_char, c_uchar};
use std::panic::{catch_unwind, AssertUnwindSafe};

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

/// Release a string returned by [`ambr_text`]. NULL is a no-op.
/// 释放 [`ambr_text`] 返回的字符串；NULL 是空操作。
#[no_mangle]
pub unsafe extern "C" fn ambr_free_str(p: *mut c_char) {
    if !p.is_null() {
        drop(std::ffi::CString::from_raw(p));
    }
}
