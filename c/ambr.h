/*
** SPDX-License-Identifier: MulanPSL-2.0
** ambr.h — read-only `.ambr` decoder for the C implementation (T7 / N03).
** ambr.h —— C 端 `.ambr` 只读解码器（T7 / N03）。
**
** Scope / 范围:
**   - 40 B header + 41 B segment table + `VALUE` (0x01) + `STRTAB` (0x02);
**     every other segment (MANIFEST / DIR / TYPED / FRAMES / …) is skipped by its `len`.
**   - No compression, no encryption, no typed semantics: such segments are skipped, and a
**     compressed/encrypted VALUE is reported instead of guessed.
**   ｜ 只解 VALUE/STRTAB；其余段按 len 跳过；压缩/加密的 VALUE 明确报错。
**
** Error messages / 错误信息:
**   Decoder errors are **not** in `errors/codes.sml` yet (the `E-AMBR-*` table lands with T7),
**   so they carry no code — the message starts with `AMBR:` instead. Once the table exists,
**   replace the prefix with the assigned code.
**   ｜ 解码错误暂未进码表，消息前缀是 `AMBR:`；码表落地后换成 `E-AMBR-*`。
**
** Memory / 内存: the returned tree is owned by the caller (`sml_free`).
** 编译: gcc sml.c ambr.c test_ambr.c -o test_ambr
*/
#ifndef SML_AMBR_H
#define SML_AMBR_H

#include <stddef.h>
#include "sml.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Decode a whole `.ambr` image. Returns NULL on failure and writes `AMBR: …` into err.
** 解一整份 `.ambr`；失败返回 NULL，err 里写 `AMBR: …`（err 可空）。 */
sml_value *sml_ambr_load(const unsigned char *buf, size_t len, char *err, size_t errsz);

/* Decode a file. Same error convention. ｜ 解一个文件，错误约定同上。 */
sml_value *sml_ambr_load_file(const char *path, char *err, size_t errsz);

#ifdef __cplusplus
}
#endif

#endif /* SML_AMBR_H */
