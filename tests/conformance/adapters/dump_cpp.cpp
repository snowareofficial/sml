/* W5：把 .sml 用 **C++ 侧** 解析后 dump 成 SML 文本，供跨实现一致性套件比对。
 *
 * 形态照抄 tools/dump_c.c（W4 的 C 侧 dumper）—— 只把解析器换成 cpp/sml.cpp。
 *
 * 用法（仓库根）：
 *   g++ -std=c++17 -Icpp -o <tmp>/dump_cpp.exe tests/conformance/adapters/dump_cpp.cpp cpp/sml.cpp
 *   dump_cpp.exe <file.sml> ...
 *
 * 输出协议（与 tools/dump_c.c 同一口径，驱动脚本按这个解析）：
 *   "DUMP <路径>\n" + dump 原文       解析成功
 *   "ERR  <路径> <错误信息>\n"         解析失败（信息以错误码开头）
 *
 * 注：C++ 侧**没有**裸 `include` 展开（只认 `@include`，见 cpp/sml.hpp 的注释），
 *     故这里是**有意**不传 include_dir —— 套件要钉的正是这个现状，而不是绕过它。
 */
#include <cstdio>
#include <string>

#include "sml.hpp"

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#endif

static bool read_all(const char *path, std::string &out) {
    FILE *f = fopen(path, "rb");
    if (!f) return false;
    char buf[65536];
    size_t n;
    while ((n = fread(buf, 1, sizeof buf, f)) > 0) out.append(buf, n);
    fclose(f);
    return true;
}

int main(int argc, char **argv) {
#ifdef _WIN32
    /* 与 tools/dump_c.c 同一坑：stdout 默认文本模式会把 '\n' 翻成 "\r\n"，
       归一化后虽然看不出来，但 dump 进临时文件再被 Rust 回读时没必要引入这个噪声。 */
    _setmode(_fileno(stdout), _O_BINARY);
#endif
    for (int i = 1; i < argc; i++) {
        std::string text;
        if (!read_all(argv[i], text)) {
            std::printf("ERR %s E-IO-001 读取失败\n", argv[i]);
            continue;
        }
        std::string err;
        sml::ValuePtr v = sml::Parser::parse(text, &err);
        if (!v) {
            if (err.empty()) err = "E-INTERNAL-001 sml: 解析失败但没有给出原因";
            std::printf("ERR %s %s\n", argv[i], err.c_str());
            continue;
        }
        std::string out = sml::Parser::to_sml(v);
        std::printf("DUMP %s\n", argv[i]);
        std::fwrite(out.data(), 1, out.size(), stdout);
        std::fputc('\n', stdout);
    }
    return 0;
}
