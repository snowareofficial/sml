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
    /* ⚠️ 这里没有「嵌套数组」的用例，原因不是漏写：
       C 的 parse_array **不递归**（元素只处理块/字符串/裸词，遇到 '[' 直接跳过并丢弃），
       所以嵌套数组在 C 里既不会递归、也不会报错 —— 它是被静默丢掉的，
       属**数据正确性**问题（与 JS 早先修掉的「嵌套数组静默截断」同类），
       已单独登记；在它修好之前，把「嵌套数组应当报深度错」写成期望只会误导人。 */

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
    /* 只断言「不崩」：值对不对是上面说的那个待办，不在这里判。 */
    CHECK(v != NULL, "100 层数组嵌套应当解析成功（内容正确性见文件头注释）");
    if (!v) printf("      err: %s\n", err);
    sml_free(v);
    free(t);

    /* 2) 远超上限：必须报错返回，不许崩（这条在修之前会打穿栈） */
    CASE("10 万层块嵌套");
    t = nest_blocks(100000); expect_error("10 万层块嵌套", t);  free(t);

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
