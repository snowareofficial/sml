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
** 错误信息：`errors/codes.sml` 的独立 `AMBR` 域（`E-AMBR-001`…`008`）；
** FFI 错误串形如 `AMBR: [E-AMBR-002] <detail>`，宿主按方括号里的码分支，不解析文案。
** 码由 `rust/amberice/src/error.rs` 的 `AmbrError` 统一给出，本端不手抄码表。
** ｜ Error messages: the standalone `AMBR` domain in `errors/codes.sml`
**    (`E-AMBR-001`..`008`). FFI errors read `AMBR: [E-AMBR-002] <detail>`; hosts branch on
**    the bracketed code, never on prose. Source: `amberice::AmbrError`.
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

/* 解码时的可观测信息：manifest 声明的摘要算法，以及**是否真的验过**。
** `digest_skipped = 1` 表示 hash_id 是预留号或多级元组，本端算不了、已明确跳过。
** ｜ Observability: the declared §20 `hash_id` and whether digests were actually verified. */
typedef struct sml_ambr_info {
    int hash_id;        /* §20 编号；无 manifest 时按默认 0x30 */
    int digest_skipped; /* 1 = 预留号 / 多级元组 ⇒ 未校验 */
} sml_ambr_info;

/* Decode a whole `.ambr` image. Returns NULL on failure and writes `AMBR: …` into err.
** 解一整份 `.ambr`；失败返回 NULL，err 里写 `AMBR: …`（err 可空）。 */
sml_value *sml_ambr_load(const unsigned char *buf, size_t len, char *err, size_t errsz);

/* Same, plus the observability struct (may be NULL when the caller does not care).
** 同上，另交出可观测信息（`info` 可为 NULL）。 */
sml_value *sml_ambr_load_ex(const unsigned char *buf, size_t len, sml_ambr_info *info,
                            char *err, size_t errsz);

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
