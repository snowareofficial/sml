/* test_codes.c — 「触发条件 → 期望码」回归（W10，C 侧）
 *
 * 职责不是「C 报了错」，而是钉住**报的是哪个码**：码是跨端稳定契约，文案不是
 * （见 errors/README.md 与 errors/codes.sml）。期望值一律用 sml_codes.h 的宏，
 * 不手打字符串 —— 顺带保证「宏真的能被 C 测试 include 到」。
 *
 * 覆盖范围：C 侧**真的会报**的码。C 的 LEX 层不报错（未闭合字符串/块注释、
 * 未知转义一律静默接受，见 README 清点），故这里没有 LEX 用例。
 *
 * 另有**不产生错误码**的保真用例，同属跨端契约，故一并放在本文件：
 *   - `test_numeric_fidelity`：超 i64 的纯整数形态必须原样保留为字符串，不能变成被夹住的
 *     Int（Rust 侧 B10）。
 *   - `test_nested_arrays`（W17）：嵌套数组的**形状**必须与 Rust/JS 逐字一致。修之前
 *     `[` 被兜底 else 静默丢掉，`m: [ 1, [2, 3], 4 ]` 会得到 `{"m":[1,2,3],"4":4}` ——
 *     不报错，但数据错、还**凭空多一个键**（比"少一层"更坏，因为假键会被下游当真实数据）。
 *
 * 三个码**无法在本文件构造**，只在此说明、不写用例（写了会误导人）：
 *   - E-PARSE-012（解析失败兜底）：当前所有失败路径都会先写入带码的消息，
 *     故 `err[0] == '\0'` 的兜底分支实际不可达；它只是防御性保留。
 *   - E-INCLUDE-010（基准目录不可解析）：能打开入口文件时其所在目录必然可
 *     规范化，该分支要构造「目录存在却 realpath/_fullpath 失败」才有意义。
 *   - E-LIMIT-010（内存分配失败）：只有 malloc 真的失败才触发，无法用输入
 *     确定性构造（要靠注入式分配器或超大数据量，不值当）。码本身由
 *     `sml: oom` 时代的 3 处站点统一改用，见 sml.c 的 resolve_includes /
 *     sml_parse_file。
 *
 * 编译运行：
 *     gcc -std=c99 -Wall -Wextra sml.c test_codes.c -o test_codes && ./test_codes
 * 或走 `python build_check.py --run` / `make -C c test`。
 *
 * 临时文件一律以 `_tc_` 开头（被 .gitignore 的「下划线开头」通配规则忽略，即便中途
 * 崩溃残留也不会污染 git status）。
 *
 * SPDX-License-Identifier: MulanPSL-2.0
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>   /* isinf：钉住 1e400 -> Float(inf) 不回退 */

#include "sml.h"
#include "sml_codes.h"

static int failures = 0;

/* err 以码开头、码后接一个空格（或字符串结束）。码内不含空格。
   不用 strncmp == 0 单独判定：那样 "E-CONTRACT-0011" 也会误命中 "E-CONTRACT-001"。 */
static int code_is(const char *err, const char *want) {
    size_t n = strlen(want);
    if (strncmp(err, want, n) != 0) return 0;
    return err[n] == ' ' || err[n] == '\0';
}

static void report(const char *tag, const char *want, const char *err, int ok) {
    if (ok) {
        printf("  ok: %-30s -> %s\n", tag, want);
    } else {
        printf("FAIL: %s 期望码 %s, 实得错误 \"%s\"\n", tag, want, err);
        failures++;
    }
}

/* 内存文本：失败（返回 NULL）且码相符 */
static void expect_code(const char *tag, const char *src, const char *want) {
    char err[256];
    sml_value *v;
    memset(err, 0, sizeof(err));
    v = sml_parse(src, err, sizeof(err));
    if (v) {
        printf("FAIL: %s 应当失败却解析成功\n", tag);
        failures++;
        sml_free(v);
        return;
    }
    report(tag, want, err, code_is(err, want));
}

/* ---- 文件型用例（include）的辅助 ---- */

static int write_file(const char *name, const char *content) {
    FILE *f = fopen(name, "wb");
    if (!f) return -1;
    fputs(content, f);
    fclose(f);
    return 0;
}

static void rm_file(const char *name) { remove(name); }

static void expect_code_file(const char *tag, const char *path, const char *want) {
    char err[256];
    sml_value *v;
    memset(err, 0, sizeof(err));
    v = sml_parse_file(path, err, sizeof(err));
    if (v) {
        printf("FAIL: %s 应当失败却解析成功\n", tag);
        failures++;
        sml_free(v);
        return;
    }
    report(tag, want, err, code_is(err, want));
}

/* a { a { ... 200 层（超上限 128） */
static char *nest_blocks(int n) {
    size_t cap = (size_t)n * 6 + 1;
    char *s = (char *)malloc(cap);
    char *p = s;
    int i;
    if (!s) return NULL;
    for (i = 0; i < n; i++) { memcpy(p, "a { ", 4); p += 4; }
    for (i = 0; i < n; i++) { memcpy(p, "} ", 2); p += 2; }
    *p = '\0';
    return s;
}

/* ------------------------------------------------------------------ */
/* 契约：E-CONTRACT-*                                                  */
/* ------------------------------------------------------------------ */
static void test_contract_codes(void) {
    printf("[CONTRACT]\n");

    /* 引用了未定义的契约（块级 @is） */
    expect_code("未定义契约（@is）",
                "server { @is Nope }\n",
                SML_E_CONTRACT_001);

    /* 引用了未定义的契约（字段类型是契约引用，嵌套情形） */
    expect_code("未定义契约（字段引用）",
                "@contract S { inner: Missing }\n"
                "server {\n  @is S\n  inner: { y: 1 }\n}\n",
                SML_E_CONTRACT_001);

    /* 字段类型不符 */
    expect_code("字段类型不符",
                "@contract S { port: int }\n"
                "server {\n  @is S\n  port: oops\n}\n",
                SML_E_CONTRACT_002);

    /* 必填字段缺失 */
    expect_code("必填字段缺失",
                "@contract S { port: int }\n"
                "server { @is S }\n",
                SML_E_CONTRACT_003);

    /* 未声明字段（严格模式） */
    expect_code("未声明字段（严格）",
                "@contract S { port: int }\n"
                "server {\n  @is S\n  port: 1\n  extra: 2\n}\n",
                SML_E_CONTRACT_004);

    /* 数值越界 */
    expect_code("数值越界",
                "@contract S { ratio: num min 0 max 1 }\n"
                "server {\n  @is S\n  ratio: 2\n}\n",
                SML_E_CONTRACT_005);

    /* 枚举取值不在列表内 —— 与「类型不符」是两条码，不能混用 */
    expect_code("枚举取值非法",
                "@contract S { st: enum [ a b ] }\n"
                "server {\n  @is S\n  st: c\n}\n",
                SML_E_CONTRACT_006);

    /* 组合字段应为块，实际给了标量 */
    expect_code("组合字段非块",
                "@contract I { x: int }\n"
                "@contract O { inner: I }\n"
                "server {\n  @is O\n  inner: 5\n}\n",
                SML_E_CONTRACT_008);
}

/* ------------------------------------------------------------------ */
/* 语法与上限：E-PARSE-012（不可达，只声明）、E-LIMIT-001              */
/* ------------------------------------------------------------------ */
static void test_parse_and_limit(void) {
    char *src;
    printf("[PARSE / LIMIT]\n");

    /* E-PARSE-012 见文件头注释：兜底分支不可达，故不写用例。 */

    src = nest_blocks(200);
    if (!src) { printf("FAIL: 构造深嵌套输入失败（OOM）\n"); failures++; return; }
    expect_code("嵌套过深（200 层）", src, SML_E_LIMIT_001);
    free(src);
}

/* ------------------------------------------------------------------ */
/* 特性与版本：E-FEATURE-004 / E-FEATURE-005                          */
/* ------------------------------------------------------------------ */
static void test_feature_codes(void) {
    printf("[FEATURE]\n");

    expect_code("未知版本声明",
                "@version v9\nk: 1\n",
                SML_E_FEATURE_004);

    /* v2/v3 下裸词必须加引号（@version 生效后再解析值） */
    expect_code("裸词必须加引号（v2）",
                "@version v2\nk: hello\n",
                SML_E_FEATURE_005);
}

/* ------------------------------------------------------------------ */
/* include：E-INCLUDE-001 / 002 / 003 / 004，E-LIMIT-003               */
/* ------------------------------------------------------------------ */
static void test_include_codes(void) {
    char name[64], content[128];
    char *buf;
    char *p;
    int i;

    printf("[INCLUDE]\n");

    /* 1) include 目标不存在 */
    if (write_file("_tc_inc_root.sml", "include \"_tc_no_such_inc.sml\"\n") != 0) {
        printf("FAIL: 无法写临时文件 _tc_inc_root.sml\n"); failures++;
    } else {
        expect_code_file("include 目标缺失", "_tc_inc_root.sml", SML_E_INCLUDE_001);
    }
    rm_file("_tc_inc_root.sml");

    /* 2) 循环引用：a 含 b，b 含 a */
    if (write_file("_tc_cyc_a.sml", "include \"_tc_cyc_b.sml\"\n") != 0 ||
        write_file("_tc_cyc_b.sml", "include \"_tc_cyc_a.sml\"\n") != 0) {
        printf("FAIL: 无法写循环引用临时文件\n"); failures++;
    } else {
        expect_code_file("include 循环引用", "_tc_cyc_a.sml", SML_E_INCLUDE_002);
    }
    rm_file("_tc_cyc_a.sml");
    rm_file("_tc_cyc_b.sml");

    /* 3) 越界：include 到基准目录之外（c/ 的上一级） */
    if (write_file("_tc_esc.sml", "include \"../README.md\"\n") != 0) {
        printf("FAIL: 无法写临时文件 _tc_esc.sml\n"); failures++;
    } else {
        expect_code_file("include 越出基准目录", "_tc_esc.sml", SML_E_INCLUDE_003);
    }
    rm_file("_tc_esc.sml");

    /* 4) 嵌套层数超过 MAX_INC_DEPTH(32)：d0 -> d1 -> ... -> d33 */
    for (i = 0; i <= 33; i++) {
        snprintf(name, sizeof(name), "_tc_d%d.sml", i);
        if (i < 33)
            snprintf(content, sizeof(content), "include \"_tc_d%d.sml\"\n", i + 1);
        else
            snprintf(content, sizeof(content), "k: 1\n");
        if (write_file(name, content) != 0) {
            printf("FAIL: 无法写临时文件 %s\n", name); failures++;
            break;
        }
    }
    if (i > 33) {
        expect_code_file("include 嵌套超限", "_tc_d0.sml", SML_E_INCLUDE_004);
    }
    for (i = 0; i <= 33; i++) {
        snprintf(name, sizeof(name), "_tc_d%d.sml", i);
        rm_file(name);
    }

    /* 5) 全局展开次数超过 MAX_INC_EXPANSIONS(256)：同一叶子 include 300 次。
          每次都在 depth=0 处理，故不会被循环检测误报为 E-INCLUDE-002。 */
    buf = (char *)malloc(300u * 40u + 1u);
    if (!buf) {
        printf("FAIL: 构造展开输入失败（OOM）\n"); failures++;
    } else {
        p = buf;
        for (i = 0; i < 300; i++) p += sprintf(p, "include \"_tc_exp_leaf.sml\"\n");
        *p = '\0';
        if (write_file("_tc_exp_leaf.sml", "k: 1\n") != 0 ||
            write_file("_tc_exp_root.sml", buf) != 0) {
            printf("FAIL: 无法写展开次数临时文件\n"); failures++;
        } else {
            expect_code_file("include 展开次数超限", "_tc_exp_root.sml", SML_E_LIMIT_003);
        }
        free(buf);
    }
    rm_file("_tc_exp_leaf.sml");
    rm_file("_tc_exp_root.sml");
}

/* ------------------------------------------------------------------ */
/* 宿主绑定层：E-IO-001、E-INTERNAL-001（空指针入参）                  */
/* ------------------------------------------------------------------ */
static void test_io_and_internal(void) {
    char err[256];
    sml_value *v;

    printf("[IO / INTERNAL]\n");

    memset(err, 0, sizeof(err));
    v = sml_parse_file("_tc_no_such_entry.sml", err, sizeof(err));
    if (v) { printf("FAIL: 入口文件不存在应当失败\n"); failures++; sml_free(v); }
    else report("入口文件读取失败", SML_E_IO_001, err, code_is(err, SML_E_IO_001));

    memset(err, 0, sizeof(err));
    v = sml_parse(NULL, err, sizeof(err));
    if (v) { printf("FAIL: sml_parse(NULL) 应当失败\n"); failures++; sml_free(v); }
    else report("sml_parse(NULL)", SML_E_INTERNAL_001, err, code_is(err, SML_E_INTERNAL_001));

    memset(err, 0, sizeof(err));
    v = sml_parse_file(NULL, err, sizeof(err));
    if (v) { printf("FAIL: sml_parse_file(NULL) 应当失败\n"); failures++; sml_free(v); }
    else report("sml_parse_file(NULL)", SML_E_INTERNAL_001, err,
                code_is(err, SML_E_INTERNAL_001));
}

/* err 允许为 NULL / errsz 为 0：一个字节都不许写（与 test_limits.c 同一条性质，
   这里只针对**新增了码前缀**的写入点再确认一次）。 */
static void test_err_null_safe(void) {
    char guard[8];
    size_t i;
    sml_value *v;
    printf("[err 空缓冲]\n");
    v = sml_parse("@version v9\nk: 1\n", NULL, 256);
    if (v) { printf("FAIL: 未知版本应当失败\n"); failures++; sml_free(v); }
    memset(guard, 0x7f, sizeof(guard));
    v = sml_parse("@version v9\nk: 1\n", guard, 0);
    if (v) { failures++; sml_free(v); }
    for (i = 0; i < sizeof(guard); i++) {
        if (guard[i] != 0x7f) {
            printf("FAIL: errsz=0 时往缓冲区写了东西\n"); failures++; break;
        }
    }
    printf("  ok: %-30s -> %s\n", "err=NULL / errsz=0 不写", "E-FEATURE-004 路径");
}

/* ------------------------------------------------------------------ */
/* 未闭合的块 / 数组 / 契约体：E-PARSE-001                             */
/* ------------------------------------------------------------------ */
static void test_unclosed_codes(void) {
    printf("[PARSE：未闭合]\n");

    expect_code("未闭合块（一层）", "a {\n", SML_E_PARSE_001);
    expect_code("未闭合块（嵌套）", "a { a {\n", SML_E_PARSE_001);
    expect_code("未闭合数组", "a: [1, 2\n", SML_E_PARSE_001);
    expect_code("顶层未闭合数组", "[1, 2\n", SML_E_PARSE_001);
    expect_code("未闭合契约体", "@contract C { a: int\n", SML_E_PARSE_001);
}

/* ------------------------------------------------------------------ */
/* 契约 min/max 边界取值：E-PARSE-022 / E-CONTRACT-010                 */
/* ------------------------------------------------------------------ */
static void test_bound_codes(void) {
    printf("[CONTRACT：min/max 边界]\n");

    expect_code("min 非数字",
                "@contract S { r: num min abc }\nserver { @is S\n r: 1\n}\n",
                SML_E_PARSE_022);
    /* 修前这条报的是 E-CONTRACT-005 —— 「非法边界当 0 后把合法值判成越界」，
       是**错码**（比静默更坏：会被当成已处理）。必须是 E-PARSE-022。 */
    expect_code("max 非数字（曾报错码）",
                "@contract S { r: num max abc }\nserver { @is S\n r: 1\n}\n",
                SML_E_PARSE_022);
    /* strtod 认 "0x10"（十六进制浮点 = 16.0），Rust 的 f64::from_str 不认 —— 同码拒绝 */
    expect_code("min 十六进制写法",
                "@contract S { r: num min 0x10 }\nserver { @is S\n r: 1\n}\n",
                SML_E_PARSE_022);
    expect_code("min 缺取值",
                "@contract S { r: num min }\nserver { @is S\n r: 1\n}\n",
                SML_E_PARSE_022);
    expect_code("min nan（非有限）",
                "@contract S { r: num min nan }\nserver { @is S\n r: 1\n}\n",
                SML_E_CONTRACT_010);
    expect_code("min 1e400（浮点溢出成 inf）",
                "@contract S { r: num min 1e400 }\nserver { @is S\n r: 1\n}\n",
                SML_E_CONTRACT_010);

    /* 合法边界不能被误伤：min 0.5 必须照常生效（1 >= 0.5 通过） */
    {
        char err[256];
        sml_value *v;
        memset(err, 0, sizeof(err));
        v = sml_parse("@contract S { r: num min 0.5 }\nserver { @is S\n r: 1\n}\n",
                      err, sizeof(err));
        if (!v) { printf("FAIL: min 0.5 应解析成功，实得 \"%s\"\n", err); failures++; }
        else { printf("  ok: %-30s -> 成功（小数边界生效）\n", "min 0.5"); sml_free(v); }
    }
}

/* ------------------------------------------------------------------ */
/* 数值保真：超 i64 的纯整数形态 -> Str（无错误码，但同样是跨端契约）   */
/* ------------------------------------------------------------------ */
static sml_value *parse_k(const char *tag, const char *src, char *err, size_t errsz) {
    sml_value *v;
    memset(err, 0, errsz);
    v = sml_parse(src, err, errsz);
    if (!v) { printf("FAIL: %s 应当成功却失败（err=\"%s\"）\n", tag, err); failures++; }
    return v;
}

static void expect_str_value(const char *tag, const char *src, const char *want) {
    char err[256];
    sml_value *v = parse_k(tag, src, err, sizeof(err)), *k;
    if (!v) return;
    k = sml_obj_get(v, "k");
    if (!k || k->type != SML_STR || strcmp(k->u.s, want) != 0) {
        printf("FAIL: %s 期望 STR \"%s\"（原样保留），实得 %s\n", tag, want,
               !k ? "无字段 k" : (k->type == SML_STR ? k->u.s : "非字符串"));
        failures++;
    } else {
        printf("  ok: %-30s -> STR 原样保留\n", tag);
    }
    sml_free(v);
}

static void expect_int_value(const char *tag, const char *src, long long want) {
    char err[256];
    sml_value *v = parse_k(tag, src, err, sizeof(err)), *k;
    if (!v) return;
    k = sml_obj_get(v, "k");
    if (!k || k->type != SML_INT || k->u.i != want) {
        printf("FAIL: %s 期望 INT %lld，实得 %s\n", tag, want,
               !k ? "无字段 k" : (k->type == SML_INT ? "别的整数" : "非整数"));
        failures++;
    } else {
        printf("  ok: %-30s -> INT %lld\n", tag, want);
    }
    sml_free(v);
}

static void test_numeric_fidelity(void) {
    char err[256];
    sml_value *v, *k;

    printf("[数值：超 i64 整数的保真]\n");

    /* i64 边界内：仍是 Int（守卫不能误伤） */
    expect_int_value("i64 上界", "k: 9223372036854775807\n", 9223372036854775807LL);
    expect_int_value("i64 下界", "k: -9223372036854775808\n",
                     (-9223372036854775807LL - 1));

    /* 超出 i64：必须**原样保留为字符串**（对齐 Rust B10）。
       修前会夹成 LLONG_MAX/MIN 的错值 —— 形如正常数字、能通过 `int` 契约校验，
       属「静默数据损坏」而非「静默通过」。 */
    expect_str_value("i64 上界 +1", "k: 9223372036854775808\n", "9223372036854775808");
    expect_str_value("20 个 9", "k: 99999999999999999999\n", "99999999999999999999");
    expect_str_value("负溢出", "k: -99999999999999999999\n", "-99999999999999999999");
    expect_str_value("带正号的溢出", "k: +99999999999999999999\n", "+99999999999999999999");

    /* 防回退：strtod 那支**不动** —— 1e400 仍走 Float(inf)，与 Rust 一致 */
    v = parse_k("1e400", "k: 1e400\n", err, sizeof(err));
    if (v) {
        k = sml_obj_get(v, "k");
        if (!k || k->type != SML_FLOAT || !isinf(k->u.f)) {
            printf("FAIL: 1e400 应仍为 FLOAT inf（strtod 支不得改动）\n");
            failures++;
        } else {
            printf("  ok: %-30s -> FLOAT inf（对齐 Rust，保持不变）\n", "1e400");
        }
        sml_free(v);
    }
}

/* ------------------------------------------------------------------ */
/* 嵌套数组（W17）：值层面的回归，期望值与 Rust/JS **逐字一致**          */
/* ------------------------------------------------------------------ */
/* 用 `sml_parse_json` 比对整体形状，而不是逐层 `sml_arr_get`：嵌套数组要钉的是
   「整棵子树的形状」，逐层取下标既长又容易写错；而 JSON 是各端共有的口径，
   这里的期望值全部是从 Rust/JS **实跑抄来的**（不是读代码推的）。 */
static void expect_json(const char *tag, const char *src, const char *want) {
    char *j = sml_parse_json(src);
    if (j && strcmp(j, want) == 0) {
        printf("  ok: %-34s -> %s\n", tag, want);
    } else {
        printf("FAIL: %s\n      期望 %s\n      实得 %s\n", tag, want,
               j ? j : "(解析失败/无输出)");
        failures++;
    }
    sml_free_cstr(j);
}

static void test_nested_arrays(void) {
    printf("[嵌套数组：值层面（W17）]\n");

    /* ⚠️ 修前实测（改之前下面这些**全部**是错的，而且一个错都不报）：
         m: [ [ a ] ]        → {"m":["a"]}           （少一层）
         m: [ 1, [2, 3], 4 ] → {"m":[1,2,3],"4":4}   （**凭空多一个键 "4"**）
         m: [ [a], [b] ]     → {"m":["a"]}           （`[b]` 整个丢掉）
         a: + 100 层 `[..]`  → {"a":[]}              （吞成空数组）
       根因：`[` 落到 `parse_array` 的兜底 else 被 `next` 丢掉 —— 内层的 `]` 于是被外层
       当成结束符，剩下的 token 交给外层块解析、被当成键名（假键 `"4"` 就是这么来的）。 */
    expect_json("一层嵌套", "m: [ [ a ] ]\n", "{\"m\":[[\"a\"]]}");
    expect_json("夹在标量之间", "m: [ 1, [2, 3], 4 ]\n", "{\"m\":[1,[2,3],4]}");
    expect_json("双层嵌套", "m: [[1]]\n", "{\"m\":[[1]]}");
    expect_json("顶层嵌套数组", "[[a]]\n", "[[\"a\"]]");
    expect_json("块与数组混排", "m: [ { x: 1 } [ y ] ]\n", "{\"m\":[{\"x\":1},[\"y\"]]}");
    expect_json("两个子数组", "m: [ [a], [b] ]\n", "{\"m\":[[\"a\"],[\"b\"]]}");
    expect_json("深三层", "m: [[[z]]]\n", "{\"m\":[[[\"z\"]]]}");
}

int main(void) {
    test_contract_codes();
    test_parse_and_limit();
    test_unclosed_codes();
    test_bound_codes();
    test_numeric_fidelity();
    test_nested_arrays();
    test_feature_codes();
    test_include_codes();
    test_io_and_internal();
    test_err_null_safe();

    if (failures == 0) {
        printf("ALL CODE TESTS PASSED\n");
        return 0;
    }
    printf("%d FAILURES\n", failures);
    return 1;
}
