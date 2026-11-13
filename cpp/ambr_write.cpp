// SPDX-License-Identifier: MulanPSL-2.0
// ambr_write.cpp — `.ambr` writer for the C++ binding (thin forwarding to amberice-ffi).
// ambr_write.cpp —— C++ 端 `.ambr` 写入器（转发到 amberice-ffi）。
//
// Why a forwarder / 为什么只转发:
//   A pure-C++ canonical encoder (STRTAB + value blocks + segment table + manifest +
//   per-segment SHA-256) would be a second implementation with its own drift risk. The
//   read-only pure-C++ decoder stays as the format-portability proof (T7 / N03); writing
//   is a capability you opt into by linking the cdylib.
//   ｜ 纯 C++ 重写 canonical 编码器是第二套实现、会漂。纯 C++ 只读解码器继续作为
//   格式可移植性的证明；写入是「链接 cdylib 才获得」的能力。
//
// Build / 构建:
//   cargo build -p amberice-ffi --features sml
//   g++ ... ambr.cpp ambr_write.cpp -L<dir> -lamberice_ffi

#include "ambr_write.hpp"

extern "C" {
unsigned ambr_caps(void);
unsigned char *ambr_pack_text(const char *text, size_t len, size_t *out_len,
                              char *err, size_t errsz);
void ambr_free_bytes(unsigned char *p, size_t len);
}

namespace sml {

unsigned ambr_caps() { return ::ambr_caps(); }

std::optional<std::vector<unsigned char>> ambr_pack_text(std::string_view text, std::string* err) {
    if (!ambr_can_write()) {
        if (err) {
            *err = "AMBR: [E-AMBR-003] this build is read-only — rebuild amberice-ffi with --features sml / "
                   "本次构建只读：请用 --features sml 重新构建 amberice-ffi";
        }
        return std::nullopt;
    }

    char ebuf[256] = {0};
    size_t n = 0;
    unsigned char* p = ::ambr_pack_text(text.data(), text.size(), &n, ebuf, sizeof ebuf);
    if (p == nullptr) {
        if (err) *err = ebuf;
        return std::nullopt;
    }

    std::vector<unsigned char> out(p, p + n);
    ::ambr_free_bytes(p, n);
    return out;
}

} // namespace sml
