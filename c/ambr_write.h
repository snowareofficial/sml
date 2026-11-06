/*
** SPDX-License-Identifier: MulanPSL-2.0
** ambr_write.h — opt-in `.ambr` **writer** for the C binding.
** ambr_write.h —— C 端可选启用的 `.ambr` **写入器**。
**
** How to choose / 怎么选:
**   The read-only decoder (`ambr.c` + `ambr.h`) has zero dependencies and stays that way.
**   This file is a **separate translation unit**: add `ambr_write.c` to your build and link
**   `amberice_ffi` (`cargo build -p amberice-ffi --features sml`), and writes become
**   available. Do nothing, and nothing changes — no macros, no `#ifdef`, no dead code.
**   ｜ 只读解码器零依赖且保持不变；本文件是**独立编译单元**：把 `ambr_write.c` 加进构建、
**   并链接带 `sml` feature 的 `amberice_ffi`，写入能力就有了。不加则一切照旧 ——
**   没有宏、没有 `#ifdef`、没有死代码。
**
**   The linker is the mode switch. / **链接器就是模式开关。**
**
** Capabilities / 能力位:
**   `sml_ambr_caps()` reports what the linked cdylib supports. Branch on it when a build
**   might be read-only instead of assuming the symbol exists.
**   ｜ 用 `sml_ambr_caps()` 查询，而不是假定符号存在。
**
** Memory / 内存: bytes returned by `sml_ambr_pack_text` are owned by the caller —
**   release with `sml_ambr_free_bytes(p, out_len)`, not `free()`.
**   ｜ 返回的字节归调用方，用 `sml_ambr_free_bytes` 释放，不能用 `free()`。
*/
#ifndef SML_AMBR_WRITE_H
#define SML_AMBR_WRITE_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Capability bits — same values as `ambr_caps()` in amberice-ffi. ｜ 与 FFI 逐位一致。 */
#define SML_AMBR_CAP_READ 1u
#define SML_AMBR_CAP_WRITE 2u

/* Capabilities of the linked cdylib. ｜ 已链接 cdylib 的能力位。 */
unsigned sml_ambr_caps(void);

/* Pack SML source text into `.ambr` bytes.
** Objects keep source order (canonical requires it); a lone top-level scalar round-trips too.
** Returns NULL on failure and writes an `AMBR: ...` message into `err` (may be NULL).
** ｜ 把 SML 源文本打成 `.ambr`；对象保源序；顶层标量也可往返。
** 失败返回 NULL，err 里写 `AMBR: ...`（err 可为 NULL）。 */
unsigned char *sml_ambr_pack_text(const char *text, size_t len, size_t *out_len,
                                  char *err, size_t errsz);

/* Release bytes returned by `sml_ambr_pack_text`. `len` must be the `out_len` it wrote.
** ｜ 释放 `sml_ambr_pack_text` 返回的字节；len 必须是它写出的 out_len。 */
void sml_ambr_free_bytes(unsigned char *p, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* SML_AMBR_WRITE_H */
