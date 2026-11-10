/*
** SPDX-License-Identifier: MulanPSL-2.0
** ambr.h — read-only `.ambr` decoder for the C implementation (T7 / N03).
** ambr.h —— C 端 `.ambr` 只读解码器（T7 / N03）。
**
** Scope / 范围:
**   - 40 B header + 41 B segment table + `VALUE` (0x01) + `STRTAB` (0x02);
**     every other segment (MANIFEST / DIR / FRAMES / …) is skipped by its `len`.
**   - `sml_ambr_load_typed()` additionally reads `TYPED` (0x70) + `SCHEMA` (0x71) and
**     renders the typed variables (D45); comp/enc forms of those two are ignored by the
**     plain loader and refused by the typed loader.
**   - No compression, no encryption: a compressed/encrypted VALUE is reported instead of
**     guessed.
**   ｜ 主路径只解 VALUE/STRTAB；`sml_ambr_load_typed` 另解 TYPED/SCHEMA；压缩/加密的 VALUE 报错。
**
** 错误信息：解码错误不占 `errors/codes.sml` 五端码位，当前不带码，消息前缀是 `AMBR:`。
** 落 `E-AMBR-*` 须改该表并重生成五端产物，不在本端手写码。
** ｜ Error messages: decoder errors are outside the five-end code table and carry no code;
**    the `AMBR:` prefix is current. Any `E-AMBR-*` code must be added via `errors/codes.sml`
**    with all five ends regenerated, never hand-written in this binding.
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

/* Render `TYPED` (0x70) via the embedded `SCHEMA` (0x71) — D45 / §25.8.
** Returns an array of `{ id, name, value }`; an empty array when the file has no TYPED
** segment. NULL + `AMBR: …` on error (TYPED without SCHEMA, malformed schema, …).
** ｜ 用内嵌 SCHEMA 渲染 TYPED，返回 [{ id, name, value }]；无 TYPED 时返回空数组。 */
sml_value *sml_ambr_load_typed(const unsigned char *buf, size_t len, char *err, size_t errsz);

/* Decode a file. Same error convention. ｜ 解一个文件，错误约定同上。 */
sml_value *sml_ambr_load_file(const char *path, char *err, size_t errsz);

#ifdef __cplusplus
}
#endif

#endif /* SML_AMBR_H */
