// SPDX-License-Identifier: MulanPSL-2.0
// test_ambr_write.cpp — optional `.ambr` writer smoke test (C++ binding).
// test_ambr_write.cpp —— C++ 端可选写入器的冒烟测试。
//
// Scope / 范围: same as the C test — the link-time opt-in works, values come back, a lone
// top-level scalar round-trips, failures carry `AMBR:`. Byte-for-byte canonical parity is
// asserted by the ends that own a canonical emitter.
// ｜ 与 C 测试同范围：链接期选择可用、值能读回、顶层标量可往返、失败带 AMBR:。
//
// Build / 构建:
//   cargo build -p amberice-ffi --features sml
//   g++ -std=c++17 -I cpp cpp/tests/test_ambr_write.cpp cpp/ambr.cpp cpp/ambr_write.cpp \
//       -L<dir> -lamberice_ffi -o t_ambr_write

#include <cstdio>
#include <cstring>
#include <string>

#include "ambr.hpp"
#include "ambr_write.hpp"

static int failures = 0;

static void ok(const char* what) { std::printf("  PASS  %s\n", what); }

static void fail(const char* what, const std::string& detail) {
    std::printf("  FAIL  %s  %s\n", what, detail.c_str());
    failures++;
}

static void t_caps() {
    unsigned caps = sml::ambr_caps();
    if (caps & sml::AmbrCapRead) ok("caps: READ");
    else fail("caps: READ", "missing");

    if (sml::ambr_can_write()) ok("caps: WRITE");
    else fail("caps: WRITE", "missing — build amberice-ffi with --features sml");
}

static void t_container() {
    const std::string src = "server {\n  host: \"example.com\"\n  port: 8080\n}\n";
    std::string err;

    auto bytes = sml::ambr_pack_text(src, &err);
    if (!bytes) {
        fail("pack container", err);
        return;
    }
    ok("pack container");

    std::string load_err;
    auto v = sml::ambr_load(bytes->data(), bytes->size(), &load_err);
    if (!v) {
        fail("load container", load_err);
        return;
    }
    ok("load container");

    auto server = (*v)->get("server");
    auto host = server ? server->get("host") : nullptr;
    if (host && host->tag == sml::Value::Tag::Str && host->s == "example.com") {
        ok("container server.host");
    } else {
        fail("container server.host", "wrong value");
    }

    auto port = server ? server->get("port") : nullptr;
    if (port && port->tag == sml::Value::Tag::Int && port->i == 8080) {
        ok("container server.port == 8080");
    } else {
        fail("container server.port", "wrong value");
    }
}

static void t_top_scalar() {
    std::string err;
    auto bytes = sml::ambr_pack_text("42", &err);
    if (!bytes) {
        fail("pack top-level scalar", err);
        return;
    }

    std::string load_err;
    auto v = sml::ambr_load(bytes->data(), bytes->size(), &load_err);
    if (v && (*v)->tag == sml::Value::Tag::Int && (*v)->i == 42) {
        ok("top-level scalar 42 round-trips");
    } else {
        fail("top-level scalar 42", v ? "wrong value" : load_err);
    }
}

static void t_error() {
    std::string err;
    auto bytes = sml::ambr_pack_text("bad {", &err);
    if (!bytes && err.rfind("AMBR:", 0) == 0) {
        ok("error path returns nullopt + AMBR: message");
    } else {
        fail("error path", bytes ? "unexpected success" : err);
    }
}

int main() {
    std::printf("[ambr-write] optional writer smoke test / 可选写入器冒烟\n");
    t_caps();
    t_container();
    t_top_scalar();
    t_error();

    std::printf("\n");
    if (failures == 0) {
        std::printf("ALL OK\n");
        return 0;
    }
    std::printf("SOME FAILED (%d)\n", failures);
    return 1;
}
