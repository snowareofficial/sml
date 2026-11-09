// SPDX-License-Identifier: MulanPSL-2.0
// ambr.hpp — read-only `.ambr` decoder for the C++ implementation (T7 / N03).
// ambr.hpp —— C++ 端 `.ambr` 只读解码器（T7 / N03）。
//
// Scope / 范围: 40 B header + 41 B table + VALUE/STRTAB; every other segment skipped by len.
// No compression / encryption / typed semantics — such VALUE bodies are refused, not guessed.
// ｜ 只解 VALUE/STRTAB；压缩或加密的 VALUE 明确报错，其余段按 len 跳过。
//
// 错误：容器错误不占 `errors/codes.sml` 五端码位，当前不带码、前缀 `AMBR:`；
// 落 `E-AMBR-*` 须改该表并重生成五端产物，不在本端手写码。
// ｜ Errors: outside the five-end code table; no code, `AMBR:` prefix. Any `E-AMBR-*`
//    code must be added via `errors/codes.sml` with all ends regenerated.
#ifndef SML_AMBR_HPP
#define SML_AMBR_HPP

#include <cstddef>
#include <optional>
#include <string>

#include "sml.hpp"

namespace sml {

// Decode a whole `.ambr` image. ｜ 解一整份 `.ambr`。
// `err` (optional) receives an `AMBR: …` message on failure.
std::optional<ValuePtr> ambr_load(const unsigned char* buf, size_t len, std::string* err);

// Decode a file. ｜ 解一个文件。
std::optional<ValuePtr> ambr_load_file(const std::string& path, std::string* err);

} // namespace sml

#endif // SML_AMBR_HPP
