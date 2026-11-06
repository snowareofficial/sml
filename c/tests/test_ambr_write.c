/*
** SPDX-License-Identifier: MulanPSL-2.0
** test_ambr_write.c — optional `.ambr` writer smoke test (C binding).
** test_ambr_write.c —— C 端可选写入器的冒烟测试。
**
** Scope / 范围:
**   Proves the link-time opt-in works: pack → load → values come back, a lone top-level
**   scalar round-trips, and failures carry an `AMBR:` message. Byte-for-byte canonical
**   parity is asserted by the ends that have a canonical emitter (Rust / JS / the FFI
**   bindings) — pure C only reads values, so it checks semantics here.
**   ｜ 证明「链接期选择」可用：打包 → 读回 → 值正确；顶层标量可往返；失败带 AMBR: 消息。
**   逐字符 canonical 对拍由有发射器的端负责；纯 C 只读值，所以这里只验语义。
**
** Build / 构建:
**   cargo build -p amberice-ffi --features sml
**   gcc -std=c99 -I c c/tests/test_ambr_write.c c/ambr.c c/ambr_write.c \
**       -L<dir> -lamberice_ffi -o test_ambr_write
*/
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ambr.h"
#include "ambr_write.h"

static int failures = 0;

static void ok(const char *what) {
    printf("  PASS  %s\n", what);
}

static void fail(const char *what, const char *detail) {
    printf("  FAIL  %s  %s\n", what, detail ? detail : "");
    failures++;
}

static void t_caps(void) {
    unsigned caps = sml_ambr_caps();
    if (caps & SML_AMBR_CAP_READ) ok("caps: READ");
    else fail("caps: READ", "missing");

    if (caps & SML_AMBR_CAP_WRITE) ok("caps: WRITE");
    else fail("caps: WRITE", "missing — build amberice-ffi with --features sml");
}

static void t_container(void) {
    const char *src = "server {\n  host: \"example.com\"\n  port: 8080\n}\n";
    char err[256] = {0};
    size_t n = 0;

    unsigned char *bytes = sml_ambr_pack_text(src, strlen(src), &n, err, sizeof err);
    if (bytes == NULL) {
        fail("pack container", err);
        return;
    }
    ok("pack container");

    sml_value *v = sml_ambr_load(bytes, n, err, sizeof err);
    sml_ambr_free_bytes(bytes, n);
    if (v == NULL) {
        fail("load container", err);
        return;
    }
    ok("load container");

    sml_value *host = sml_get_path(v, "server.host");
    if (host != NULL && host->type == SML_STR && strcmp(host->u.s, "example.com") == 0) {
        ok("container server.host == \"example.com\"");
    } else {
        fail("container server.host", "wrong value");
    }

    sml_value *port = sml_get_path(v, "server.port");
    if (port != NULL && port->type == SML_INT && port->u.i == 8080) {
        ok("container server.port == 8080");
    } else {
        fail("container server.port", "wrong value");
    }

    sml_free(v);
}

static void t_top_scalar(void) {
    char err[256] = {0};
    size_t n = 0;

    unsigned char *bytes = sml_ambr_pack_text("42", 2, &n, err, sizeof err);
    if (bytes == NULL) {
        fail("pack top-level scalar", err);
        return;
    }

    sml_value *v = sml_ambr_load(bytes, n, err, sizeof err);
    sml_ambr_free_bytes(bytes, n);
    if (v != NULL && v->type == SML_INT && v->u.i == 42) {
        ok("top-level scalar 42 round-trips");
    } else {
        fail("top-level scalar 42", v ? "wrong value" : err);
    }
    sml_free(v);
}

static void t_error(void) {
    char err[256] = {0};
    size_t n = 0;

    unsigned char *p = sml_ambr_pack_text("bad {", 5, &n, err, sizeof err);
    if (p == NULL && strncmp(err, "AMBR:", 5) == 0) {
        ok("error path returns NULL + AMBR: message");
    } else {
        fail("error path", p ? "unexpected success" : err);
        if (p) sml_ambr_free_bytes(p, n);
    }
}

int main(void) {
    printf("[ambr-write] optional writer smoke test / 可选写入器冒烟\n");
    t_caps();
    t_container();
    t_top_scalar();
    t_error();

    printf("\n");
    if (failures == 0) {
        printf("ALL OK\n");
        return 0;
    }
    printf("SOME FAILED (%d)\n", failures);
    return 1;
}
