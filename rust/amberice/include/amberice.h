/*
** SPDX-License-Identifier: MulanPSL-2.0
** amberice.h — minimal C ABI for `.ambr` (feature `ffi`).
** amberice.h —— `.ambr` 的最小 C ABI（feature `ffi`）。
**
** Build / 构建: cargo build -p amberice-ffi
**   → Windows `amberice_ffi.dll` / Linux `libamberice_ffi.so` / macOS `libamberice_ffi.dylib`
**   （独立 crate：`amberice` 本体保持纯 rlib）
**
** Why only two symbols / 为什么只有两个符号:
**   各 FFI 端本来就有 SML 解析器；返回 canonical 文本即可物化值树，且能与跨端向量逐字节比对。
**   It keeps the ABI stable and the lifetime rules trivial: you own one string.
**
** Error convention / 错误约定:
**   `ambr_text` returns NULL and writes `AMBR: …` into `err` (NUL-terminated, truncated to
**   `errsz`, never overrun; `err` may be NULL or `errsz == 0`).
*/
#ifndef AMBERICE_H
#define AMBERICE_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Decode `.ambr` bytes into canonical text. Free the result with ambr_free_str. */
char *ambr_text(const unsigned char *buf, size_t len, char *err, size_t errsz);

/* Release a string returned by ambr_text. NULL is a no-op. */
void ambr_free_str(char *p);

#ifdef __cplusplus
}
#endif

#endif /* AMBERICE_H */
