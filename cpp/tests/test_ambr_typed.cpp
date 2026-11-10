// SPDX-License-Identifier: MulanPSL-2.0
// test_ambr_typed.cpp — C++ 端 SCHEMA/TYPED 动态渲染（D45 / §25.8）对拍。
//
// Build / 构建:
//   g++ -std=c++17 -I cpp cpp/ambr.cpp cpp/tests/test_ambr_typed.cpp -o t_ambr_typed
//
// Fixture 与 JS / C 端共用（hex 由 Rust Writer 生成），避免三端各写一份字节。

#include "ambr.hpp"

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

#include "../../c/tests/typed_fixture.h"

using namespace sml;

static bool hex2bin(const char* hex, std::vector<unsigned char>* out) {
    const size_t n = std::strlen(hex);
    if (n % 2 != 0) return false;
    out->resize(n / 2);
    for (size_t i = 0; i < n / 2; i++) {
        auto nib = [](char c) -> int {
            if (c >= '0' && c <= '9') return c - '0';
            if (c >= 'a' && c <= 'f') return c - 'a' + 10;
            return -1;
        };
        const int hi = nib(hex[i * 2]);
        const int lo = nib(hex[i * 2 + 1]);
        if (hi < 0 || lo < 0) return false;
        (*out)[i] = (unsigned char)((hi << 4) | lo);
    }
    return true;
}

static int check(bool cond, const char* what) {
    if (!cond) std::printf("FAIL  %s\n", what);
    return cond ? 0 : 1;
}

int main() {
    std::vector<unsigned char> bytes;
    if (!hex2bin(TYPED_FIXTURE_HEX, &bytes)) {
        std::printf("FAIL  fixture hex\n");
        return 1;
    }

    std::string err;
    auto vars = ambr_load_typed(bytes.data(), bytes.size(), &err);
    if (!vars) {
        std::printf("FAIL  load_typed: %s\n", err.c_str());
        return 1;
    }

    int bad = 0;
    bad += check((*vars)->tag == Value::Tag::Arr && (*vars)->arr.size() == 1, "one variable");
    ValuePtr item = (*vars)->arr.empty() ? nullptr : (*vars)->arr[0];
    ValuePtr id = item ? item->get("id") : nullptr;
    ValuePtr name = item ? item->get("name") : nullptr;
    ValuePtr val = item ? item->get("value") : nullptr;
    bad += check(id && id->tag == Value::Tag::Int && id->i == 1, "var id = 1");
    bad += check(name && name->tag == Value::Tag::Str && name->s == "User", "type name = User");
    bad += check(val && val->tag == Value::Tag::Obj, "value is object");

    ValuePtr uid = val ? val->get("id") : nullptr;
    ValuePtr uname = val ? val->get("name") : nullptr;
    ValuePtr tags = val ? val->get("tags") : nullptr;
    ValuePtr note = val ? val->get("note") : nullptr;
    bad += check(uid && uid->tag == Value::Tag::Int && uid->i == 42, "field id = 42");
    bad += check(uname && uname->tag == Value::Tag::Str && uname->s == "ada", "field name = ada");
    bad += check(tags && tags->tag == Value::Tag::Arr && tags->arr.size() == 2, "tags has 2 items");
    if (tags && tags->tag == Value::Tag::Arr && tags->arr.size() == 2) {
        bad += check(tags->arr[0]->tag == Value::Tag::Str && tags->arr[0]->s == "a", "tags[0] = a");
        bad += check(tags->arr[1]->tag == Value::Tag::Str && tags->arr[1]->s == "b", "tags[1] = b");
    }
    bad += check(note && note->tag == Value::Tag::Null, "note = null");

    if (bad) return 1;
    std::printf("ok  1/1 typed render\n");
    return 0;
}
