// SPDX-License-Identifier: MulanPSL-2.0
// ambr.hpp — read-only `.ambr` decoder for the C++ implementation (T7 / N03).
// ambr.hpp —— C++ 端 `.ambr` 只读解码器（T7 / N03）。
//
// Scope / 范围: 40 B header + 41 B table + VALUE/STRTAB; every other segment skipped by len.
// `ambr_load_typed()` additionally reads TYPED (0x70) + SCHEMA (0x71) and renders the typed
// variables (D45); comp/enc forms of those two are ignored by the plain loader and refused
// by the typed loader. No compression / encryption — such VALUE bodies are refused.
// ｜ 主路径只解 VALUE/STRTAB；`ambr_load_typed` 另解 TYPED/SCHEMA；压缩或加密的 VALUE 明确报错。
//
// 错误：`errors/codes.sml` 的独立 `AMBR` 域（`E-AMBR-001`…`008`）；FFI 串形如
// `AMBR: [E-AMBR-002] <detail>`，宿主按码分支。码来自 `amberice::AmbrError`。
// ｜ Errors: standalone `AMBR` domain (`E-AMBR-001`..`008`); FFI strings carry
//    `AMBR: [E-AMBR-002] <detail>` so hosts can branch on the code.
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

// Render `TYPED` (0x70) via the embedded `SCHEMA` (0x71) — D45 / §25.8.
// Returns an array of `{ id, name, value }`; empty array when there is no TYPED.
// 用内嵌 SCHEMA 渲染 TYPED，返回 [{ id, name, value }]；无 TYPED 时返回空数组。
std::optional<ValuePtr> ambr_load_typed(const unsigned char* buf, size_t len, std::string* err);

// Decode a file. ｜ 解一个文件。
std::optional<ValuePtr> ambr_load_file(const std::string& path, std::string* err);

} // namespace sml

#endif // SML_AMBR_HPP
