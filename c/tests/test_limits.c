/* test_limits.c — 深度上限 +「err 允许为 NULL」的回归用例
 *
 * 两条都来自审计，且都属于「用户能给一个输入、进程就死」的那一类：
 *
 *  1. 深嵌套必须**报错返回 NULL**，绝不能崩。栈溢出在 C 里是段错误，接不住 ——
 *     所以用例本身就是断言：能跑到下一行，就说明没崩。
 *  2. err 允许为 NULL（sml.h 写的是「若非 NULL」）。修之前有两条路径写成
 *         snprintf(errbuf ? errbuf : (char[1]){0}, errsz, ...)
 *     本意是「没有缓冲就丢弃」，但 snprintf 按 errsz 写 —— 传 NULL + errsz=256
 *     就会往 1 字节的临时缓冲写 255 字节，直接踩栈；另有一批点干脆直接
 *     snprintf(errbuf, ...)，errbuf 为 NULL 时空指针写。
 *     两类现在都必须消失：缓冲区为空时**一个字节都不许写**。
 *
 * 编译运行：
 *     gcc -std=c99 -Wall -Wextra sml.c test_limits.c -o test_limits && ./test_limits
 * 或走 `python build_check.py --run` / `make test`。
 *
 * SPDX-License-Identifier: MulanPSL-2.0
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "sml.h"

static int failures = 0;
#define CHECK(c, m) do { if (!(c)) { printf("FAIL: %s\n", m); failures++; } } while (0)

/* 崩溃会被系统直接收走，stdout 缓冲里的东西**全部丢失** —— 所以进入每个可能致死的
   用例前先打标记并 fflush：崩了以后看最后一行就知道死在哪个 case。
   （仓库里 _audit_c_probe.c 用的是同一招。） */
#define CASE(tag) do { printf("  case: %s\n", tag); fflush(stdout); } while (0)

/* a { a { ... } }：纯块嵌套（审计里被绕过的正是这条路径） */
static char *nest_blocks(int n) {
    size_t cap = (size_t)n * 6 + 1;
    char *s = (char *)malloc(cap);
    char *p = s;
    for (int i = 0; i < n; i++) { memcpy(p, "a { ", 4); p += 4; }
    for (int i = 0; i < n; i++) { memcpy(p, "} ", 2); p += 2; }
    *p = '\0';
    return s;
}

/* a: [[[...]]]：数组嵌套 */
static char *nest_arrays(int n) {
    size_t cap = 4 + (size_t)n * 2 + 1;
    char *s = (char *)malloc(cap);
    char *p = s;
    memcpy(p, "a: ", 3); p += 3;
    memset(p, '[', (size_t)n); p += (size_t)n;
    memset(p, ']', (size_t)n); p += (size_t)n;
    *p = '\0';
    return s;
}

/* err 非空：必须失败、必须写错误、必须返回 NULL */
static void expect_error(const char *tag, const char *text) {
    char err[256];
    sml_value *v;
    memset(err, 0, sizeof(err));
    v = sml_parse(text, err, sizeof(err));
    if (v) { printf("FAIL: %s 应当失败却解析成功了\n", tag); failures++; sml_free(v); return; }
    if (!err[0]) { printf("FAIL: %s 失败但没有写错误信息\n", tag); failures++; }
}

/* err 为 NULL 或 errsz 为 0：一个字节都不许写。
   这几条用例的价值就是「跑得到下一行」—— 崩了就没有任何输出。 */
static void expect_error_no_buf(const char *tag, const char *text) {
    char guard[8];
    size_t i;
    sml_value *v;

    v = sml_parse(text, NULL, 256);
    if (v) { printf("FAIL: %s（err=NULL）应当失败却成功了\n", tag); failures++; sml_free(v); }

    memset(guard, 0x7f, sizeof(guard));
    v = sml_parse(text, guard, 0);
    if (v) { failures++; sml_free(v); }
    for (i = 0; i < sizeof(guard); i++) {
        if (guard[i] != 0x7f) {
            printf("FAIL: %s（errsz=0）往缓冲区里写了东西\n", tag);
            failures++;
            break;
        }
    }
}

int main(void) {
    char *t;
    char err[256];
    sml_value *v;
    /* 嵌套数组（W17 已修）：`nest_arrays()` 现在有对应的深度入口了。
       修之前 C 的 parse_array **不递归**（`[` 落到兜底 else 被 `next` 丢掉），所以深数组
       既不会递归、也不会报错 —— 它是被**静默吞空**的（`a: ` + 100 层 `[..]` → `{"a":[]}`），
       这正是当时不在这里写期望的原因（写「应当报深度错」只会误导人）。
       **值层面的正确性在 test_codes.c 的 `expect_json` 里钉**（与跨端探针同口径），
       本文件只钉深度边界与「不许崩」。 */

    /* 1) 正常深度：守卫不能误伤（上限 128 层，100 层应当照常解析） */
    t = nest_blocks(100);
    memset(err, 0, sizeof(err));
    v = sml_parse(t, err, sizeof(err));
    CHECK(v != NULL, "100 层块嵌套应当解析成功");
    if (!v) printf("      err: %s\n", err);
    sml_free(v);
    free(t);

    t = nest_arrays(100);
    memset(err, 0, sizeof(err));
    v = sml_parse(t, err, sizeof(err));
    /* 只断言「不崩」：值对不对由 test_codes.c 的 expect_json 钉（见文件头注释）。 */
    CHECK(v != NULL, "100 层数组嵌套应当解析成功（内容正确性见 test_codes.c）");
    if (!v) printf("      err: %s\n", err);
    sml_free(v);
    free(t);

    /* 2) 远超上限：必须报错返回，不许崩（这条在修之前会打穿栈） */
    CASE("10 万层块嵌套");
    t = nest_blocks(100000); expect_error("10 万层块嵌套", t);  free(t);

    /* 2.5) 边界要**逐格**钉住：128 层放行、第 129 层报此码。
       只测 100 与 100000 是**测不出「差一格」**的 —— 这两个数在两种口径下结果一样，
       而"差一格"正是实际发生的 bug（C 原先用 `>=`，128 层就报，而文案写「超过 128 层」）。
       口径来源：五端**闭合**嵌套实测（改前 Rust 127 / C 127 / C++ 128 / JS 128 / Lua 128，
       现统一为 128 / 129）。
       W17 之后**数组嵌套也有深度入口了**（parse_array 与 parse_block 共用 ps->depth），
       故下面块与数组**各钉一组**，口径同为 128 放行 / 129 报。 */
    t = nest_blocks(128);
    memset(err, 0, sizeof(err));
    v = sml_parse(t, err, sizeof(err));
    CHECK(v != NULL, "128 层块嵌套应当放行（各端口径）");
    if (!v) printf("      err: %s\n", err);
    sml_free(v);
    free(t);

    t = nest_blocks(129);
    expect_error("129 层块嵌套", t);
    free(t);

    /* 数组嵌套的同一组边界（W17）。
       ⚠️ 这两格在修之前**测不了**：那时 `[` 被静默丢掉，129 层也会"成功"返回
       `{"a":[]}`，把它期望成 E-LIMIT-001 只会误导人。
       ⚠️ 两者的**判别力不同**，别混：128 层那格是「守卫不能误伤」，改前改后**都通过**
       （改前它也是"成功"的，只是内容被吞空）；真正能抓 W17 的是**129 层那格**
       （改前静默成功 → `expect_error` 红）与 test_codes.c 的 7 条形状断言。 */
    t = nest_arrays(128);
    memset(err, 0, sizeof(err));
    v = sml_parse(t, err, sizeof(err));
    CHECK(v != NULL, "128 层数组嵌套应当放行（与块嵌套同口径）");
    if (!v) printf("      err: %s\n", err);
    sml_free(v);
    free(t);

    t = nest_arrays(129);
    expect_error("129 层数组嵌套", t);
    free(t);

    /* 3) err 为空：越界写与空指针写的回归（深嵌套、版本、契约、空指针入参四条路径） */
    CASE("err=NULL：深嵌套");
    t = nest_blocks(100000); expect_error_no_buf("10 万层块嵌套", t); free(t);
    CASE("err=NULL：未知版本");
    expect_error_no_buf("未知版本", "@version v9\n");
    CASE("err=NULL：未定义契约");
    expect_error_no_buf("未定义契约", "@is Missing\nx { }\n");
    CASE("err=NULL：空指针入参");
    expect_error_no_buf("空指针入参", NULL);

    /* 4) err=NULL 不影响正常文档 */
    t = nest_blocks(100);
    v = sml_parse(t, NULL, 256);
    CHECK(v != NULL, "正常深度 + err=NULL 仍应成功");
    sml_free(v);
    free(t);

    if (failures == 0) {
        printf("ALL LIMIT TESTS PASSED\n");
        return 0;
    }
    printf("%d FAILURES\n", failures);
    return 1;
}
