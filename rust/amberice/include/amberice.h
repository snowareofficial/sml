/*
** SPDX-License-Identifier: MulanPSL-2.0
** amberice.h —— `.ambr` 的 C ABI（`amberice-ffi` crate）。
** ｜ C ABI for `.ambr` (crate `amberice-ffi`).
**
** 构建：`cargo build -p amberice-ffi`，产出 `amberice_ffi.dll` /
** `libamberice_ffi.so` / `libamberice_ffi.dylib`。独立 crate，`amberice` 保持纯 rlib；
** 本仓没有 `ffi` feature。
** ｜ Build with `cargo build -p amberice-ffi`; standalone crate, `amberice` stays a pure
**   rlib, and there is no `ffi` feature in this repo.
**
** 符号：读 `ambr_caps` / `ambr_text` / `ambr_free_str`，缓冲 `ambr_alloc` /
** `ambr_dealloc`，写 `ambr_pack_text` / `ambr_free_bytes`（后者需 `sml` feature）。
** 能力位用 `ambr_caps` 查询，不要按名字猜符号是否存在。
** ｜ Symbols: read, host-buffer and write groups above; `ambr_pack_text` needs `sml`.
**   Query `ambr_caps` for capability bits; do not assume a symbol is present.
**
** 错误约定：失败时 `ambr_text` / `ambr_pack_text` 返回 NULL，把 `AMBR: …` 写进 `err`；
** NUL 结尾、按 `errsz` 截断、不越界；`err` 可为 NULL 或 `errsz == 0`。
** ｜ On failure they return NULL and write `AMBR: …` into `err`: NUL-terminated,
**   truncated to `errsz`, never overrun; `err` may be NULL or `errsz == 0`.
*/
#ifndef AMBERICE_H
#define AMBERICE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* 查询本构建的能力位。 ｜ Capability bits of this build. */
uint32_t ambr_caps(void);

/* 把 `.ambr` 字节解成 canonical 文本，结果用 `ambr_free_str` 释放。
   ｜ Decode `.ambr` bytes into canonical text; free with `ambr_free_str`. */
char *ambr_text(const unsigned char *buf, size_t len, char *err, size_t errsz);

/* 释放 `ambr_text` 返回的字符串；NULL 是空操作。
   ｜ Release a string from `ambr_text`; NULL is a no-op. */
void ambr_free_str(char *p);

/* 宿主侧输入缓冲，给没有 malloc 的运行时（如 wasm）用；用 `ambr_dealloc` 释放。
   ｜ Host-side input buffer for runtimes without malloc (e.g. wasm); free with `ambr_dealloc`. */
unsigned char *ambr_alloc(size_t len);
void ambr_dealloc(unsigned char *p, size_t len);

/* 把 SML 源文本打包成 `.ambr` 字节；仅 `sml` feature 下导出。
   ｜ Pack SML source text into `.ambr`; exported only with the `sml` feature. */
unsigned char *ambr_pack_text(const char *text, size_t len, size_t *out_len,
                              char *err, size_t errsz);

/* 释放 `ambr_pack_text` 返回的字节；NULL 或 len == 0 是空操作。
   ｜ Release bytes from `ambr_pack_text`; NULL / len == 0 are no-ops. */
void ambr_free_bytes(unsigned char *p, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* AMBERICE_H */
