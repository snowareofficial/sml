// SPDX-License-Identifier: MulanPSL-2.0
// ambr.hpp — read-only `.ambr` decoder for the C++ implementation (T7 / N03).
// ambr.hpp —— C++ 端 `.ambr` 只读解码器（T7 / N03）。
//
// Scope / 范围: 40 B header + 41 B table + VALUE/STRTAB; every other segment skipped by len.
// No compression / encryption / typed semantics — such VALUE bodies are refused, not guessed.
// ｜ 只解 VALUE/STRTAB；压缩或加密的 VALUE 明确报错，其余段按 len 跳过。
//
// Errors / 错误: not in `errors/codes.sml` yet (E-AMBR-* lands with T7) ⇒ message starts
// with `AMBR:` and carries no code. ｜ 错误暂不带码，前缀 `AMBR:`。
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
