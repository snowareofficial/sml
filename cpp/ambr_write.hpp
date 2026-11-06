// SPDX-License-Identifier: MulanPSL-2.0
// ambr_write.hpp — opt-in `.ambr` **writer** for the C++ binding.
// ambr_write.hpp —— C++ 端可选启用的 `.ambr` **写入器**。
//
// How to choose / 怎么选:
//   The read-only decoder (`ambr.cpp` + `ambr.hpp`) has zero dependencies and stays that way.
//   This is a **separate translation unit**: add `ambr_write.cpp` to your build and link
//   `amberice_ffi` (`cargo build -p amberice-ffi --features sml`), and writes become
//   available. Do nothing, and nothing changes — no macros, no `#ifdef`.
//   ｜ 只读解码器零依赖且保持不变；本文件是**独立编译单元**：加进构建并链接带 `sml`
//   feature 的 cdylib，写入能力就有了。不加则一切照旧 —— 没有宏、没有 `#ifdef`。
//
//   The linker is the mode switch. / **链接器就是模式开关。**
//
// Memory / 内存: the returned vector is owned by the caller (a normal `std::vector`).
//   ｜ 返回的 vector 归调用方所有。

#ifndef SML_AMBR_WRITE_HPP
#define SML_AMBR_WRITE_HPP

#include <cstddef>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace sml {

// Capability bits — same values as `ambr_caps()` in amberice-ffi.
// 能力位 —— 与 FFI 逐位一致。
inline constexpr unsigned AmbrCapRead = 1u << 0;
inline constexpr unsigned AmbrCapWrite = 1u << 1;

// Capabilities of the linked cdylib. ｜ 已链接 cdylib 的能力位。
unsigned ambr_caps();

// True when this build can encode. ｜ 本构建能否编码。
inline bool ambr_can_write() { return (ambr_caps() & AmbrCapWrite) != 0; }

// Pack SML source text into `.ambr` bytes.
// Objects keep source order (canonical requires it); a lone top-level scalar round-trips too.
// `err` (optional) receives an `AMBR: ...` message on failure.
// ｜ 把 SML 源文本打成 `.ambr`；对象保源序；顶层标量也可往返。
// 失败时 err（可空）收到 `AMBR: ...`。
std::optional<std::vector<unsigned char>> ambr_pack_text(std::string_view text, std::string* err);

} // namespace sml

#endif // SML_AMBR_WRITE_HPP
