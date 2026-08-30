/*
 * example_rs.c — 演示通过 Rust cdylib 后端使用 SML v3 能力。
 * 编译 (Windows / mingw, sml.dll 在 PATH 或同目录):
 *   gcc example_rs.c -L<E:/snoware-target>/release -lsml -o example_rs
 *   (导入库为 libsml.dll.a；若用 MSVC 则 link sml.lib)
 */
#include "sml_rs.h"
#include <stdio.h>
#include <string.h>

static void demo(const char *title, char *out) {
    if (out) {
        printf("=== %s ===\n%s\n\n", title, out);
        sml_free(out);
    } else {
        printf("=== %s ===\n(NULL / 解析失败)\n\n", title);
    }
}

int main(void) {
    /* 1. 基础解析 */
    const char *doc =
        "server {\n"
        "  host: web.example\n"
        "  port: 8080\n"
        "  env: $env.APP_ENV\n"
        "}\n";
    demo("基础 sml_parse", sml_parse(doc));

    /* 2. v3: 注入 env + 限制版本 */
    const char *opts = "{\"env\":{\"APP_ENV\":\"production\"},\"allow\":[\"v1\",\"v2\",\"v3\"]}";
    demo("v3 sml_parse_ex (env 注入)", sml_parse_ex(doc, opts));

    /* 3. 列出支持的特性 */
    demo("sml_features", sml_features());

    /* 4. 版本 */
    char *v = sml_version();
    printf("version: %s\n", v ? v : "(null)");
    if (v) sml_free(v);

    return 0;
}
