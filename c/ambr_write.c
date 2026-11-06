/*
** SPDX-License-Identifier: MulanPSL-2.0
** ambr_write.c — `.ambr` writer for the C binding (thin forwarding to amberice-ffi).
** ambr_write.c —— C 端 `.ambr` 写入器（转发到 amberice-ffi）。
**
** This is deliberately a **forwarder**, not a pure-C encoder: implementing canonical
** encoding (STRTAB + value blocks + segment table + manifest + per-segment SHA-256)
** in C would be a second implementation with its own drift risk. The read-only pure-C
** decoder stays as the format-portability proof (T7 / N03); writing is a capability you
** opt into by linking the cdylib.
** ｜ 这里刻意只做**转发**，不在 C 里重写 canonical 编码器：那是第二套实现、会漂。
** 纯 C 只读解码器继续作为格式可移植性的证明；写入则是「链接 cdylib 才获得」的能力。
**
** Build / 构建:
**   cargo build -p amberice-ffi --features sml
**   gcc ... ambr.c ambr_write.c -L<dir> -lamberice_ffi
**   (Windows + MinGW: generate `libamberice_ffi.dll.a` with gendef/dlltool first.)
*/
#include "ambr_write.h"

/* Imported from amberice-ffi (`rust/amberice-ffi/src/lib.rs`). */
/* 来自 amberice-ffi 的符号。 */
extern unsigned ambr_caps(void);
extern unsigned char *ambr_pack_text(const char *text, size_t len, size_t *out_len,
                                     char *err, size_t errsz);
extern void ambr_free_bytes(unsigned char *p, size_t len);

unsigned sml_ambr_caps(void) {
    return ambr_caps();
}

unsigned char *sml_ambr_pack_text(const char *text, size_t len, size_t *out_len,
                                  char *err, size_t errsz) {
    return ambr_pack_text(text, len, out_len, err, errsz);
}

void sml_ambr_free_bytes(unsigned char *p, size_t len) {
    ambr_free_bytes(p, len);
}
