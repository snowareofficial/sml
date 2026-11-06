// SPDX-License-Identifier: MulanPSL-2.0
// test_ambr.cpp — `.ambr` 解码器对拍容器向量（T7 / N03，C++ 端）。
//
// 判据：fixture 里 21 个 `ambr_hex` 全部解码成功 + 若干形状逐值正确。
// 运行：./t_ambr.exe [fixture.json]（默认 ../tests/vectors/amberice-container.v1.json）
#include <cstdio>
#include <fstream>
#include <sstream>
#include <string>

#include "ambr.hpp"

static int failures = 0;

static void ok(const std::string& what) { std::printf("  ok: %s\n", what.c_str()); }

static void fail(const std::string& what, const std::string& detail) {
    std::printf("FAIL: %s — %s\n", what.c_str(), detail.c_str());
    failures++;
}

static std::vector<unsigned char> hex_to_bytes(const std::string& hex) {
    std::vector<unsigned char> out;
    if (hex.size() % 2 != 0) return out;
    out.reserve(hex.size() / 2);
    auto nib = [](char c) -> int {
        if (c >= '0' && c <= '9') return c - '0';
        if (c >= 'a' && c <= 'f') return c - 'a' + 10;
        return -1;
    };
    for (size_t i = 0; i + 1 < hex.size(); i += 2) {
        int hi = nib(hex[i]), lo = nib(hex[i + 1]);
        if (hi < 0 || lo < 0) return {};
        out.push_back((unsigned char)((hi << 4) | lo));
    }
    return out;
}

static bool take_quoted(const std::string& text, size_t from, const std::string& needle,
                        std::string* dst, size_t* end_at) {
    size_t at = text.find(needle, from);
    if (at == std::string::npos) return false;
    size_t s = at + needle.size();
    size_t e = text.find('"', s);
    if (e == std::string::npos) return false;
    *dst = text.substr(s, e - s);
    *end_at = e + 1;
    return true;
}

static void check_case(const std::string& id, const std::vector<unsigned char>& bytes) {
    std::string err;
    auto v = sml::ambr_load(bytes.data(), bytes.size(), &err);
    if (!v) {
        fail(id, "解码失败 " + err);
        return;
    }
    const auto& val = **v;
    if (id == "scalar-int") {
        if (val.tag == sml::Value::Tag::Int && val.i == 1) ok(id);
        else fail(id, "期望整数 1");
    } else if (id == "flat-array") {
        if (val.tag == sml::Value::Tag::Arr && val.arr.size() == 3 && val.arr[0]->i == 1 &&
            val.arr[2]->i == 3) ok(id);
        else fail(id, "期望 [1 2 3]");
    } else if (id == "strtab-dedup") {
        auto k = val.get("k");
        auto other = val.get("other");
        if (k && other && k->tag == sml::Value::Tag::Str && k->s == "k" && other->s == "k") ok(id);
        else fail(id, "期望 k/other 都是 \"k\"");
    } else if (id == "nested-containers") {
        auto cfg = val.get("config");
        auto tags = val.get("tags");
        if (cfg && cfg->get("port") && cfg->get("port")->i == 8080 && tags &&
            tags->arr.size() == 1 && tags->arr[0]->s == "x") ok(id);
        else fail(id, "期望 config.port=8080 / tags=[x]");
    } else if (id == "typed-block") {
        auto name = val.get("__name");
        auto action = val.get("action");
        if (name && name->s == "ssh" && action && action->s == "allow") ok(id);
        else fail(id, "期望 __name=ssh / action=allow");
    }
}

int main(int argc, char** argv) {
    const std::string path = argc > 1 ? argv[1] : "../tests/vectors/amberice-container.v1.json";
    std::ifstream in(path);
    if (!in) {
        std::printf("[!] 打不开 %s（在 cpp/ 下运行，或把 fixture 路径作为第一个参数）\n", path.c_str());
        return 2;
    }
    std::stringstream ss;
    ss << in.rdbuf();
    const std::string json = ss.str();

    std::printf("[ambr] 解码器对拍（C++）\n");
    int cases = 0;
    size_t pos = 0;
    for (;;) {
        std::string id, hex;
        size_t id_end = 0;
        if (!take_quoted(json, pos, "\"id\": \"", &id, &id_end)) break;
        size_t hex_end = 0;
        if (!take_quoted(json, id_end, "\"ambr_hex\": \"", &hex, &hex_end)) break;
        auto bytes = hex_to_bytes(hex);
        if (bytes.empty()) {
            fail(id, "hex 非法");
        } else {
            check_case(id, bytes);
        }
        cases++;
        pos = hex_end;
    }

    std::printf("[ambr] %d 例，%d 个失败\n", cases, failures);
    if (cases != 21) {
        std::printf("FAIL: 期望 21 例，实得 %d\n", cases);
        failures++;
    }
    std::printf("%s\n", failures ? "[ambr] FAILED" : "[ambr] ALL OK");
    return failures ? 1 : 0;
}
