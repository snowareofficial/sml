// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
// eclog-rs 的渲染对拍 oracle：解析一份 .ecl/.json，按 scheme 渲染，字节写到 stdout。
// Render conformance oracle for eclog-rs: parse an input, render with a scheme, write
// the bytes to stdout.
//
// 依赖上游 Eclog-CPP 的 amalgamation（MIT，Copyright (c) Vallest Systems LLC）：
//   g++ -std=c++11 -O1 -I <eclog-cpp>/Compact oracle.cpp <eclog-cpp>/Compact/Eclog.cpp -o oracle
// 驱动脚本：`python tools/regen_render_golden.py`（见同目录）。

#include <Eclog.h>

#include <cstdio>
#include <cstring>
#include <string>

#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#endif

using namespace vallest;

static bool loadFile(const char* path, std::string& out)
{
    FILE* f = fopen(path, "rb");
    if (!f) {
        return false;
    }

    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);

    out.resize((size_t)(size > 0 ? size : 0));
    if (size > 0 && fread(&out[0], 1, (size_t)size, f) != (size_t)size) {
        fclose(f);
        return false;
    }

    fclose(f);
    return true;
}

int main(int argc, char** argv)
{
    if (argc < 3) {
        fprintf(stderr, "usage: oracle <input> <typical|json>\n");
        return 64;
    }

#if defined(_WIN32)
    _setmode(_fileno(stdout), _O_BINARY);
#endif

    std::string text;
    if (!loadFile(argv[1], text)) {
        fprintf(stderr, "cannot read %s\n", argv[1]);
        return 66;
    }

    eclog::Document doc;
    eclog::ErrorCode ec;
    doc.parse(eclog::cstring(text.data(), text.size()), ec);
    if (ec) {
        fprintf(stderr, "parse error: %s\n", ec.message());
        return 2;
    }

    eclog::RendererConfig rc(strcmp(argv[2], "json") == 0
        ? eclog::RendererConfig::scheme_json_compatible
        : eclog::RendererConfig::scheme_typical);

    eclog::MemoryOutputStream os;
    doc.render(os, rc, ec);
    if (ec) {
        fprintf(stderr, "render error: %s\n", ec.message());
        return 3;
    }

    fwrite(os.data(), 1, os.size(), stdout);
    return 0;
}
