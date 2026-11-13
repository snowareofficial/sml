/*
** SPDX-License-Identifier: MulanPSL-2.0
** sml.c — SML (SNOWARE Markup Language) 纯 C 实现
**
** 零依赖 C99 单文件实现。API 见 sml.h。
**
** 实现结构:
**   1. 值构造/释放/容器操作
**   2. 词法 (token 流)  — 含 \u{XXXX}/\uXXXX 字符串转义
**   3. 递归下降解析 (块/数组/标量/片段) + 契约系统(@contract/@is)
**   4. 序列化 (round-trip)
**   5. JSON 桥 (C-ABI 兼容)
**   6. include / @include 文本内联 (sml_parse_file)
**
** Comments: line comments use '#' '--' or '//'; block comments use slash-star
**   and star-slash, or underscore-star and star-underscore (aligned with Rust/JS/Lua).
** Contracts: '@contract Name [loose] { ... }' defines a contract; '@is Name'
**   applies it (block-level or field-level). Aligned with Rust sml-rs: strict mode,
**   defaults, required, enum, min/max bounds, and nested ContractRef composition.
*/

/* `strdup` / `strtok_r` / `realpath` 是 POSIX / XSI，不是 ISO C99：严格 `-std=c99`
   下 glibc 不声明，编译器会把它们当成返回 int，指针直接被截断。
   - `_XOPEN_SOURCE 700` 覆盖 realpath（XSI），并隐含 POSIX.1-2008；
   - `_POSIX_C_SOURCE 200809L` 明确覆盖 strdup / strtok_r 与 realpath(path, NULL)。
   feature test macro 必须在任何系统头文件之前定义；MSVC 走自己的别名，不需要。*/
#if !defined(_WIN32)
#ifndef _XOPEN_SOURCE
#define _XOPEN_SOURCE 700
#endif
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#endif

#include "sml.h"
#include "sml_codes.h"   /* 错误码宏；唯一事实来源 errors/codes.sml（见 errors/README.md）*/

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <math.h>
#include <stdarg.h>
#include <errno.h>   /* strtoll 溢出判定（ERANGE）：见 coerce_word 的 B10 分支 */

/* =====================================================================
** 0. 错误信息写入
** ===================================================================== */

/* 统一的错误写入。**缓冲区允许为 NULL** —— sml.h 写的是 err「若非 NULL」，
   即调用方可以不关心文案；缓冲区为空（或大小为 0）时这里一个字节都不写，
   失败与否由调用方自行置 failed 标志。

   为什么必须统一走这里：审计发现两条路径写成
       snprintf(errbuf ? errbuf : (char[1]){0}, errsz, ...)
   本意是「没有缓冲就丢弃」，但 snprintf **按 errsz 写** —— 传进来的 errsz 可能是
   256，于是那 1 字节的临时缓冲被写到至多 255 字节，直接踩栈；调用方写成
   `sml_parse(text, NULL, 256)` 就能触发。同理，所有直接 `snprintf(errbuf, ...)`
   的点在 errbuf 为 NULL 时是空指针写（同样只需传 NULL 即崩）。
   两类问题在这里一起消失：只要缓冲区为空，就一个字节都不写。

   **错误码约定（W10）**：sml.h 的报错接口没有放码的槽位，故把**错误码写在消息最前**，
   码与文案之间用一个空格分隔，形如 `E-LIMIT-001 嵌套过深（超过 128 层）...`。
   码取自 `sml_codes.h` 的宏（唯一事实来源 errors/codes.sml），**不手打字符串**。
   码内不含空格，调用方取第一个空格之前的部分即可。取码方式见 sml.h 的「解析」一节。

   本文件**每一处**报错都带码，没有例外：连内存分配失败也有 —— 纯 C/C++ 要自己管内存，
   malloc 失败是可报的错误条件，码表为它单列了 E-LIMIT-010（`SML_E_LIMIT_010`）。
   新增报错点时同时补码与用例（码表是唯一事实来源 errors/codes.sml）。 */
static void set_err(char *buf, size_t sz, const char *fmt, ...) {
    va_list ap;
    if (!buf || sz == 0) return;
    va_start(ap, fmt);
    vsnprintf(buf, sz, fmt, ap);
    va_end(ap);
}

/* =====================================================================
** 1. 值构造 / 释放 / 容器
** ===================================================================== */

sml_value *sml_new_null(void) {
    sml_value *v = (sml_value *)calloc(1, sizeof(sml_value));
    if (v) v->type = SML_NULL;
    return v;
}

sml_value *sml_new_bool(int b) {
    sml_value *v = sml_new_null();
    if (v) { v->type = SML_BOOL; v->u.boolean = b ? 1 : 0; }
    return v;
}

sml_value *sml_new_int(long long i) {
    sml_value *v = sml_new_null();
    if (v) { v->type = SML_INT; v->u.i = i; }
    return v;
}

sml_value *sml_new_float(double f) {
    sml_value *v = sml_new_null();
    if (v) { v->type = SML_FLOAT; v->u.f = f; }
    return v;
}

sml_value *sml_new_strn(const char *s, size_t n) {
    sml_value *v = sml_new_null();
    if (!v) return NULL;
    v->type = SML_STR;
    v->u.s = (char *)malloc(n + 1);
    if (!v->u.s) { free(v); return NULL; }
    memcpy(v->u.s, s, n);
    v->u.s[n] = '\0';
    return v;
}

sml_value *sml_new_str(const char *s) {
    return sml_new_strn(s ? s : "", s ? strlen(s) : 0);
}

sml_value *sml_new_array(void) {
    sml_value *v = sml_new_null();
    if (v) {
        v->type = SML_ARRAY;
        v->u.arr.items = NULL;
        v->u.arr.len = v->u.arr.cap = 0;
    }
    return v;
}

sml_value *sml_new_object(void) {
    sml_value *v = sml_new_null();
    if (v) {
        v->type = SML_OBJECT;
        v->u.obj.head = v->u.obj.tail = NULL;
        v->u.obj.len = 0;
    }
    return v;
}

void sml_free(sml_value *v) {
    if (!v) return;
    switch (v->type) {
        case SML_STR:
            free(v->u.s);
            break;
        case SML_ARRAY: {
            size_t i;
            for (i = 0; i < v->u.arr.len; i++) sml_free(v->u.arr.items[i]);
            free(v->u.arr.items);
            break;
        }
        case SML_OBJECT: {
            sml_field *f = v->u.obj.head;
            while (f) {
                sml_field *nx = f->next;
                free(f->key);
                sml_free(f->value);
                free(f);
                f = nx;
            }
            break;
        }
        default:
            break;
    }
    free(v);
}

/* 深拷贝 (用于契约默认值回填, 避免共享引用被修改) */
static sml_value *sml_clone(const sml_value *v) {
    if (!v) return NULL;
    switch (v->type) {
        case SML_NULL:  return sml_new_null();
        case SML_BOOL:  return sml_new_bool(v->u.boolean);
        case SML_INT:   return sml_new_int(v->u.i);
        case SML_FLOAT: return sml_new_float(v->u.f);
        case SML_STR:   return sml_new_str(v->u.s);
        case SML_ARRAY: {
            sml_value *a = sml_new_array();
            size_t i;
            for (i = 0; i < v->u.arr.len; i++) sml_arr_push(a, sml_clone(v->u.arr.items[i]));
            return a;
        }
        case SML_OBJECT: {
            sml_value *o = sml_new_object();
            sml_field *f;
            for (f = v->u.obj.head; f; f = f->next) sml_obj_set(o, f->key, sml_clone(f->value));
            return o;
        }
    }
    return sml_new_null();
}

void sml_obj_set(sml_value *obj, const char *key, sml_value *val) {
    if (!obj || obj->type != SML_OBJECT || !key || !val) return;
    /* 替换已存在的键 */
    sml_field *f;
    for (f = obj->u.obj.head; f; f = f->next) {
        if (strcmp(f->key, key) == 0) {
            sml_free(f->value);
            f->value = val;
            return;
        }
    }
    /* 追加 */
    f = (sml_field *)calloc(1, sizeof(sml_field));
    if (!f) return;
    f->key = strdup(key);
    f->value = val;
    if (obj->u.obj.tail) obj->u.obj.tail->next = f;
    else obj->u.obj.head = f;
    obj->u.obj.tail = f;
    obj->u.obj.len++;
}

sml_value *sml_obj_get(const sml_value *obj, const char *key) {
    if (!obj || obj->type != SML_OBJECT || !key) return NULL;
    sml_field *f;
    for (f = obj->u.obj.head; f; f = f->next) {
        if (strcmp(f->key, key) == 0) return f->value;
    }
    return NULL;
}

sml_value *sml_get_path(const sml_value *v, const char *path) {
    if (!v || !path) return NULL;
    const sml_value *cur = v;
    char buf[256];
    size_t plen = strlen(path);
    if (plen >= sizeof(buf)) return NULL;
    memcpy(buf, path, plen + 1);
    char *save = NULL;
    char *tok = strtok_r(buf, ".", &save);
    while (tok) {
        if (!cur || cur->type != SML_OBJECT) return NULL;
        cur = sml_obj_get(cur, tok);
        if (!cur) return NULL;
        tok = strtok_r(NULL, ".", &save);
    }
    return (sml_value *)cur;
}

void sml_arr_push(sml_value *arr, sml_value *val) {
    if (!arr || arr->type != SML_ARRAY || !val) return;
    if (arr->u.arr.len >= arr->u.arr.cap) {
        size_t ncap = arr->u.arr.cap ? arr->u.arr.cap * 2 : 8;
        sml_value **ni = (sml_value **)realloc(arr->u.arr.items, ncap * sizeof(sml_value *));
        if (!ni) return;
        arr->u.arr.items = ni;
        arr->u.arr.cap = ncap;
    }
    arr->u.arr.items[arr->u.arr.len++] = val;
}

sml_value *sml_arr_get(const sml_value *arr, size_t i) {
    if (!arr || arr->type != SML_ARRAY || i >= arr->u.arr.len) return NULL;
    return arr->u.arr.items[i];
}

size_t sml_arr_len(const sml_value *arr) {
    return (arr && arr->type == SML_ARRAY) ? arr->u.arr.len : 0;
}

void sml_free_str(char *s) { free(s); }
void sml_free_cstr(char *p) { free(p); }

const char *sml_version(void) {
    return "sml 0.2.0 (SNOWARE Markup Language, pure C)";
}

/* 版本字面量 → 主版本号（`docs/swsml/spec/version.md` §1）。
   | Version literal -> major number.
   返回 1..4 = 本实现支持；-1 = 5.0 族（Dev，未启用）；0 = 无法识别。
   接受四类写法（与 Rust `Edition::parse` 同口径）：
     `vN` 前缀 / `N` / `N.0` 点号 / `N0` 连写；另接受代号 `camelia`（等价 50）。
   注意 `N0` 与 `N.0` 是同一件事：`10` 读作 `1.0`，`40` 读作 `4.0`。 */
static int version_lit_major(const char *s) {
    char buf[32];
    if (!s) return 0;
    if (s[0] == '"') { /* 只写代号时是引号串 */
        size_t len = strlen(s);
        if (len < 3 || s[len - 1] != '"' || len - 2 >= sizeof buf) return 0;
        memcpy(buf, s + 1, len - 2);
        buf[len - 2] = '\0';
    } else {
        size_t n = strlen(s);
        if (n >= sizeof buf) return 0;
        memcpy(buf, s, n + 1);
    }
    if (strcmp(buf, "camelia") == 0) return -1;
    const char *p = buf;
    if (*p == 'v' || *p == 'V') p++;
    if (p[0] < '1' || p[0] > '9') return 0;
    const int major = p[0] - '0';
    const int ok = (p[1] == '\0')
                   || (p[1] == '.' && p[2] == '0' && p[3] == '\0')
                   || (p[1] == '0' && p[2] == '\0');
    if (!ok) return 0;
    return major <= 4 ? major : -1;
}

/* =====================================================================
** 1b. 字符 / 编码辅助
** ===================================================================== */

static int hexdigit(int c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

/* 把 Unicode 码点编码为 UTF-8, 写入 sb (slen 更新) */
static void put_utf8(char *sb, size_t *slen, unsigned long cp) {
    if (cp <= 0x7F) {
        sb[(*slen)++] = (char)cp;
    } else if (cp <= 0x7FF) {
        sb[(*slen)++] = (char)(0xC0 | (cp >> 6));
        sb[(*slen)++] = (char)(0x80 | (cp & 0x3F));
    } else if (cp <= 0xFFFF) {
        sb[(*slen)++] = (char)(0xE0 | (cp >> 12));
        sb[(*slen)++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        sb[(*slen)++] = (char)(0x80 | (cp & 0x3F));
    } else {
        sb[(*slen)++] = (char)(0xF0 | (cp >> 18));
        sb[(*slen)++] = (char)(0x80 | ((cp >> 12) & 0x3F));
        sb[(*slen)++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        sb[(*slen)++] = (char)(0x80 | (cp & 0x3F));
    }
}

/* =====================================================================
** 2. 词法
** ===================================================================== */

typedef enum {
    T_LBRACE, T_RBRACE, T_LBRACK, T_RBRACK, T_COMMA, T_COLON, T_AT, T_BAREAT,
    T_QMARK, T_EQ,   /* 契约修饰符 ? 与默认值 = (独立 token, 确保 str? / = 不并入裸词) */
    T_STR,   /* 引号串 (已解码, 含 \u 转义) */
    T_WORD,  /* 裸词 */
    T_EOF,
} tok_type;

typedef struct {
    tok_type t;
    char *v;   /* T_STR / T_WORD 的内容 */
    int line;  /* 该 token 起始行（1 起）；任务 D 的 @is 同行判据用 */
} token;

typedef struct {
    token *toks;
    size_t n, cap, pos;
    int cur_line;    /* 当前行（lex_run 维护；token.line 的来源） */
    char *errbuf;
    size_t errsz;
    int failed;      /* 词法错误已记（只记第一条）；sml_parse 见到它立即返回 NULL */
    unsigned features;  /* 预处理扫出的特性集（供引号串 `$env.` 内插等处判定） */
} lexer;

/* 词法错误写入：**只记第一条**（与 Rust 的 `tokenize()` 返回 Err 即短路同义），
   码写在消息最前（口径同 set_err）。为什么必须短路：词法与语法共用一个 err 缓冲，
   若带着词法错误继续解析，解析器写的后续错误会**覆盖**它 —— 用户拿到的码就不是
   第一个错的码了（W16 之前 C 的 LEX 层根本不报，这个"谁覆盖谁"的问题因此没暴露）。

   复用的 `errbuf`/`errsz` 与 set_err 同一约定：缓冲区为空或大小为 0 时一个字节都不写。 */
static void lex_err(lexer *lx, const char *fmt, ...) {
    va_list ap;
    if (lx->failed) return;
    lx->failed = 1;
    if (!lx->errbuf || lx->errsz == 0) return;
    va_start(ap, fmt);
    vsnprintf(lx->errbuf, lx->errsz, fmt, ap);
    va_end(ap);
}

static void lex_push(lexer *lx, tok_type t, char *v) {
    if (lx->n >= lx->cap) {
        size_t ncap = lx->cap ? lx->cap * 2 : 64;
        token *nt = (token *)realloc(lx->toks, ncap * sizeof(token));
        if (!nt) return;
        lx->toks = nt;
        lx->cap = ncap;
    }
    lx->toks[lx->n].t = t;
    lx->toks[lx->n].v = v;
    lx->toks[lx->n].line = lx->cur_line;
    lx->n++;
}

static void lex_free(lexer *lx) {
    size_t i;
    for (i = 0; i < lx->n; i++) free(lx->toks[i].v);
    free(lx->toks);
    lx->toks = NULL;
    lx->n = lx->cap = 0;
}

static void lex_run(lexer *lx, const char *text) {
    const char *p = text;
    char buf[1024];
    size_t blen = 0;
    lx->cur_line = 1;
#define FLUSH() do { if (blen) { buf[blen] = '\0'; lex_push(lx, T_WORD, strdup(buf)); blen = 0; } } while (0)
    while (*p) {
        char c = *p;
        if (c == '#') {
            /* 单行注释到行尾 */
            while (*p && *p != '\n') p++;
        } else if (c == '-' && p[1] == '-') {
            /* `--` 单行注释到行尾 */
            while (*p && *p != '\n') p++;
        } else if (c == '/' && p[1] == '/') {
            /* 斜杠斜杠 单行注释到行尾 */
            while (*p && *p != '\n') p++;
        } else if (c == '/' && p[1] == '*') {
            /* 斜杠星 多行注释，直到 星斜杠。**EOF 未闭合 ⇒ E-LEX-002**
               （W16：改前静默吃到文件结尾 —— 后面整篇内容凭空消失，一个错都不报）。 */
            p += 2;
            int closed = 0;
            while (*p) {
                if (*p == '\n') lx->cur_line++;
                if (*p == '*' && p[1] == '/') { p += 2; closed = 1; break; }
                p++;
            }
            if (!closed)
                lex_err(lx, SML_E_LEX_002 " 未闭合的块注释 /* ... */（遇到文件结尾）");
        } else if (c == '_' && p[1] == '*') {
            /* `_*` 多行注释，直到 `*_`。**EOF 未闭合 ⇒ E-LEX-003**（同上）。 */
            p += 2;
            int closed = 0;
            while (*p) {
                if (*p == '\n') lx->cur_line++;
                if (*p == '*' && p[1] == '_') { p += 2; closed = 1; break; }
                p++;
            }
            if (!closed)
                lex_err(lx, SML_E_LEX_003 " 未闭合的块注释 _* ... *_（遇到文件结尾）");
        } else if (c == '"') {
            FLUSH();
            p++;
            char sb[4096];
            size_t slen = 0;
            int closed = 0;   /* 见过结束引号；否则 EOF ⇒ E-LEX-001 */
            while (*p) {
                if (*p == '"') { p++; closed = 1; break; }
                /* 转义符后**直接**遇文件结尾 ⇒ E-LEX-006（与 Rust 词法层同码）。
                   语义上不是「少了结束引号」，而是「少了一个被转义的字符」——
                   修复动作不同（补字符 vs 补引号），故不能并进 001。
                   T21 裁决（2026-11-06）：五端统一到 006。 */
                if (*p == '\\' && !p[1]) {
                    lex_err(lx, SML_E_LEX_006 " 字符串中的转义符后遇到文件结束");
                    break;
                }
                if (*p == '\\' && p[1]) {
                    p++;
                    char e = *p;
                    switch (e) {
                        case 'n': sb[slen++] = '\n'; break;
                        case 't': sb[slen++] = '\t'; break;
                        case 'r': sb[slen++] = '\r'; break;
                        case '0': sb[slen++] = '\0'; break;
                        case '"': sb[slen++] = '"'; break;
                        case '\\': sb[slen++] = '\\'; break;
                        case 'u': {
                            /* \uXXXX（定长四位）或 \u{...}。两类失败都报 E-LEX-005：
                               ① 定长形式**不足 4 位**（改前照收：`"\u12"` 静默变成控制字符
                                  U+0012，Rust/JS 都报 005，C 是唯一的异类）；
                               ② 非十六进制、花括号未闭合、空 hex；
                               ③ 非法码点（代理区 D800-DFFF / 超出 U+10FFFF）——
                                  put_utf8 会把它们编成非法 UTF-8，正是 Rust
                                  `char::from_u32` 拒绝的那两类。 */
                            int has_brace = 0;
                            if (*(p + 1) == '{') { has_brace = 1; p++; }
                            unsigned long cp = 0;
                            int cnt = 0, bad = 0;
                            while (1) {
                                if (has_brace) {
                                    if (*(p + 1) == '}') { p++; break; }
                                    if (!*(p + 1)) { bad = 1; break; }
                                } else {
                                    if (cnt >= 4) break;
                                }
                                int h = hexdigit((unsigned char)*(p + 1));
                                if (h < 0) { bad = 1; break; }
                                cp = cp * 16 + (unsigned long)h;
                                cnt++;
                                p++;
                            }
                            if (!bad && !has_brace && cnt != 4) bad = 1;
                            if (!bad && cnt == 0) bad = 1;
                            if (!bad && (cp > 0x10FFFFUL ||
                                         (cp >= 0xD800UL && cp <= 0xDFFFUL))) bad = 1;
                            if (bad)
                                lex_err(lx, SML_E_LEX_005
                                        " Unicode 转义非法（位数不足、非十六进制、或非法码点）");
                            else
                                put_utf8(sb, &slen, cp);
                            break;
                        }
                        default:
                            /* 未知转义 ⇒ E-LEX-004。改前 default 只把该字符原样收下、
                               **反斜杠直接丢掉**：`"C:\Users"` 静默变成 `C:Users`
                               （路径/正则被悄悄改坏）。Rust 与 C++ 同为严格策略。 */
                            lex_err(lx, SML_E_LEX_004
                                    " 字符串含未知转义符 \\%c（仅支持 \\n \\t \\r \\0 \\\" \\\\ \\uXXXX）", e);
                            break;
                    }
                    p++;
                } else {
                    sb[slen++] = *p++;
                }
                if (slen >= sizeof(sb) - 5) break;
            }
            sb[slen] = '\0';
            /* 未闭合字符串 ⇒ E-LEX-001（改前把余下全文当串内容，静默吞掉整篇）。
               注意只在**真的遇到文件结尾**时报：上面那个缓冲区保护 break 不属于此条件。 */
            if (!closed && *p == '\0')
                lex_err(lx, SML_E_LEX_001 " 字符串未闭合（缺少结束引号）");
            /* 内插 `$env.NAME`（与 Rust `interp_str` 同语义）：NAME 取 [字母数字_ .]，
               取到第一个其它字符为止；无 `$env.` 或名字为空时原样保留。
               原先只做「整串恰为 `$env.NAME`」，内嵌形式不替换 —— 那是 conformance 26
               的分歧；四端统一为内插后该用例转 AGREE。
               ｜ Inline `$env.NAME`, same semantics as the Rust reference. */
            /* env 门控：引号串含 `$env.` 且未开 env ⇒ E-FEATURE-002（与 Rust 同判据）。
               `lx->features` 由预处理扫出，词法阶段可用。 */
            if (strstr(sb, "$env.") != NULL && !(lx->features & (1u << 2))) {
                lex_err(lx, SML_E_FEATURE_002
                        " 字符串 `\"%s\"` 无法解析：未启用特性 `env`", sb);
                break;
            }
            {
                char tmp[sizeof(sb)];
                const char *src = sb;
                size_t ti = 0;
                int changed = 0;
                while (*src && ti + 1 < sizeof(tmp)) {
                    if (strncmp(src, "$env.", 5) == 0) {
                        const char *name = src + 5;
                        const char *q = name;
                        while (*q && ((*q >= 'a' && *q <= 'z') || (*q >= 'A' && *q <= 'Z')
                                      || (*q >= '0' && *q <= '9') || *q == '_' || *q == '.')) q++;
                        if (q > name) {
                            char var[256];
                            size_t nl = (size_t)(q - name);
                            if (nl >= sizeof(var)) nl = sizeof(var) - 1;
                            memcpy(var, name, nl);
                            var[nl] = '\0';
                            const char *ev = getenv(var);
                            const char *val = ev ? ev : "";
                            size_t vl = strlen(val);
                            if (ti + vl + 1 >= sizeof(tmp)) vl = sizeof(tmp) - ti - 2;
                            memcpy(tmp + ti, val, vl);
                            ti += vl;
                            src = q;
                            changed = 1;
                            continue;
                        }
                    }
                    tmp[ti++] = *src++;
                }
                tmp[ti] = '\0';
                if (changed) memcpy(sb, tmp, ti + 1);
            }
            lex_push(lx, T_STR, strdup(sb));
        } else if (c == '{') { FLUSH(); lex_push(lx, T_LBRACE, NULL); p++; }
        else if (c == '}') { FLUSH(); lex_push(lx, T_RBRACE, NULL); p++; }
        else if (c == '[') { FLUSH(); lex_push(lx, T_LBRACK, NULL); p++; }
        else if (c == ']') { FLUSH(); lex_push(lx, T_RBRACK, NULL); p++; }
        else if (c == ',') { FLUSH(); lex_push(lx, T_COMMA, NULL); p++; }
        else if (c == ':') { FLUSH(); lex_push(lx, T_COLON, NULL); p++; }
        else if (c == '?') { FLUSH(); lex_push(lx, T_QMARK, NULL); p++; }
        else if (c == '=') { FLUSH(); lex_push(lx, T_EQ, NULL); p++; }
        else if (c == '$' && p[1] == '{') {
            /* `${var}` 整体作为一个 WORD（`@for` 插值）：否则 `{` / `}` 会被当结构记号切开，
               键名位置也认不出 `${`（`${h}_port` 会散成 `${h`、`}`、`_port`）。
               Keep `${var}` as one word so it survives structural splitting. */
            const char *close = strchr(p + 2, '}');
            if (close) {
                size_t len = (size_t)(close - p) + 1;
                if (blen + len < sizeof(buf)) {
                    memcpy(buf + blen, p, len);
                    blen += len;
                }
                p = close + 1;
            } else {
                if (blen < sizeof(buf) - 1) buf[blen++] = c;
                p++;
            }
        }
        /* `@` 仅当位于**词首**时才是片段定义标记（`@base { ... }`）。
        ** 出现在词中间时（典型如邮箱 `a@b.c`）必须作为普通字符保留：
        ** 否则 `a@b.c` 会被切成 WORD("a") + AT + WORD("b.c")，
        ** 后半段在解析时被丢弃，导致邮箱静默损坏为 `a`。 */
        else if (c == '@') {
            if (blen == 0) {
                /* 孤立 `@`：其后紧跟空白 / 行尾 ⇒ 没有指令名或片段名。
                   与 Rust 的 `Tok::BareAt` 同判据（sml-lex/src/lib.rs 的 `'@'` 分支）。
                   不区分的话，`@` + 下一行的 `k` 会被拼成 `@k` 未注册指令（E-PARSE-005）。 */
                char n = p[1];
                FLUSH();
                lex_push(lx, (n == '\0' || n == ' ' || n == '\t' || n == '\n' || n == '\r')
                             ? T_BAREAT : T_AT, NULL);
            }
            else { if (blen < sizeof(buf) - 1) buf[blen++] = c; }
            p++;
        }
        else if (c == ' ' || c == '\t' || c == '\n' || c == '\r') { FLUSH(); if (c == '\n') lx->cur_line++; p++; }
        else {
            if (blen < sizeof(buf) - 1) buf[blen++] = c;
            p++;
        }
    }
    FLUSH();
    lex_push(lx, T_EOF, NULL);
#undef FLUSH
}

/* =====================================================================
** 3. 契约系统 (与 Rust sml-rs 对齐)
** ===================================================================== */

typedef enum {
    CT_ANY, CT_STR, CT_INT, CT_NUM, CT_BOOL, CT_ENUM, CT_ARRAY, CT_CONTRACTREF
} cty;

typedef struct cfield {
    char *name;
    cty ty;
    int required;            /* 默认 1; '?' 修饰符置 0 */
    sml_value *def;          /* 默认值 (own) 或 NULL */
    int min_set, max_set;
    double min, max;
    char **enum_vals; size_t enum_n;   /* CT_ENUM */
    char *ref_name;          /* CT_CONTRACTREF */
    cty arr_inner;           /* CT_ARRAY 元素类型 */
    struct cfield *next;
} cfield;

typedef struct ccontract {
    char *name;
    int allow_extra;         /* loose */
    cfield *fields;          /* 头插链表 */
    struct ccontract *next;
} ccontract;

/* 内置契约表：宿主注入，供 `@weakbind` 软校验（与文档内 `@contract` 互不相干）。 */
struct sml_builtin {
    ccontract *contracts;
};

static cty type_from_name(const char *t) {
    if (!t) return CT_ANY;
    if (strcmp(t, "str") == 0) return CT_STR;
    if (strcmp(t, "int") == 0) return CT_INT;
    if (strcmp(t, "num") == 0) return CT_NUM;
    if (strcmp(t, "bool") == 0) return CT_BOOL;
    return CT_ANY;
}

sml_builtin *sml_builtin_new(void) {
    sml_builtin *b = (sml_builtin *)calloc(1, sizeof(sml_builtin));
    return b;
}

void sml_builtin_add_field(sml_builtin *b, const char *contract,
                           const char *field, const char *type, int required) {
    ccontract *c;
    cfield *f;
    if (!b || !contract || !field) return;
    for (c = b->contracts; c; c = c->next) {
        if (strcmp(c->name, contract) == 0) break;
    }
    if (!c) {
        c = (ccontract *)calloc(1, sizeof(ccontract));
        if (!c) return;
        c->name = strdup(contract);
        c->next = b->contracts;
        b->contracts = c;
    }
    f = (cfield *)calloc(1, sizeof(cfield));
    if (!f) return;
    f->name = strdup(field);
    f->ty = type_from_name(type);
    f->required = required ? 1 : 0;
    f->next = c->fields;
    c->fields = f;
}

static cfield *builtin_find_field(sml_builtin *b, const char *contract, const char *field) {
    ccontract *c;
    cfield *f;
    if (!b || !contract || !field) return NULL;
    for (c = b->contracts; c; c = c->next) {
        if (strcmp(c->name, contract) == 0) break;
    }
    if (!c) return NULL;
    for (f = c->fields; f; f = f->next) {
        if (strcmp(f->name, field) == 0) return f;
    }
    return NULL;
}

void sml_builtin_set_range(sml_builtin *b, const char *contract, const char *field,
                           int has_min, double min, int has_max, double max) {
    cfield *f = builtin_find_field(b, contract, field);
    if (!f) return;
    f->min_set = has_min ? 1 : 0;
    f->min = min;
    f->max_set = has_max ? 1 : 0;
    f->max = max;
}

void sml_builtin_add_enum(sml_builtin *b, const char *contract, const char *field,
                          const char *value) {
    cfield *f = builtin_find_field(b, contract, field);
    char **nv;
    if (!f || !value) return;
    nv = (char **)realloc(f->enum_vals, (f->enum_n + 1) * sizeof(char *));
    if (!nv) return;
    f->enum_vals = nv;
    f->enum_vals[f->enum_n++] = strdup(value);
    f->ty = CT_ENUM;
}

void sml_builtin_free(sml_builtin *b) {
    ccontract *c;
    if (!b) return;
    c = b->contracts;
    while (c) {
        ccontract *cn = c->next;
        cfield *f = c->fields;
        while (f) {
            cfield *fn = f->next;
            free(f->name);
            free(f->ref_name);
            if (f->enum_vals) {
                size_t k;
                for (k = 0; k < f->enum_n; k++) free(f->enum_vals[k]);
                free(f->enum_vals);
            }
            free(f);
            f = fn;
        }
        free(c->name);
        free(c);
        c = cn;
    }
    free(b);
}

static const char *kind_name(const sml_value *v) {
    switch (v->type) {
        case SML_NULL:   return "null";
        case SML_BOOL:   return "bool";
        case SML_INT:    return "int";
        case SML_FLOAT:  return "float";
        case SML_STR:    return "str";
        case SML_ARRAY:  return "array";
        case SML_OBJECT: return "object";
    }
    return "?";
}

static const char *type_name(cty t) {
    switch (t) {
        case CT_ANY:         return "any";
        case CT_STR:         return "str";
        case CT_INT:         return "int";
        case CT_NUM:         return "num";
        case CT_BOOL:        return "bool";
        case CT_ENUM:        return "enum";
        case CT_ARRAY:       return "array";
        case CT_CONTRACTREF: return "contract-ref";
    }
    return "?";
}

static int value_eq_str(const sml_value *v, const char *s) {
    if (!v) return 0;
    if (v->type == SML_STR) return strcmp(v->u.s, s) == 0;
    if (v->type == SML_INT) {
        char buf[32];
        snprintf(buf, sizeof buf, "%lld", v->u.i);
        return strcmp(buf, s) == 0;
    }
    return 0;
}

/* =====================================================================
** 解析 (递归下降)
** ===================================================================== */

/* 片段表条目 */
struct frag {
    char *name;
    sml_value *val;
    struct frag *next;
};

typedef struct {
    lexer *lx;
    struct frag *frags;
    ccontract *contracts;   /* 全局契约表 (跨块可见) */
    int failed;             /* 契约校验失败标志 (不依赖 errbuf 内容) */
    int version;            /* 语法版本: 1=V1(裸词即字符串) 2=V2 3=V3(字符串须引号) */
    int depth;              /* 当前块/数组嵌套深度（栈溢出防护，见 parse_block） */
    unsigned features;      /* `@feature enable/disable` 记录的开关位；0 = 全关（C 端暂无版本基线） */
    /* `@when` 的挂起条件：0 = 无，1 = 保留下一项，2 = 丢弃下一项。
       只作用于紧邻的下一个字段/块，消费后立即清 0。
       Pending `@when`: 0 = none, 1 = keep next item, 2 = drop next item. */
    int pending_when;
    /* `@for` 的循环变量绑定（展开时临时设置，展开后恢复）。
       `@for` loop-variable binding: set during expansion, restored afterwards. */
    const char *loop_var;
    const char *loop_val;
    /* 诊断（warning）收集：只有 `sml_parse_ex` 传了出口才开启（默认零开销）。
       Diagnostics sink: enabled only when `sml_parse_ex` asks for it. */
    int want_diags;
    sml_diag *diags;
    size_t n_diags;
    size_t cap_diags;
    /* 弱绑定：宿主注入的内置契约表 + 「`@weakbind` 作用于下一个块」的挂起名。 */
    const sml_builtin *builtin;
    const char *pending_weak;
    /* 挂起项所在层（`ps->depth`）：块收尾时据此判断「是本块内的悬空项」还是
       「外层留给下一个兄弟块的」。Depth at which the pending bind was declared. */
    int pending_weak_depth;
    int pending_weak_line;
    /* 旁路表：`@weakbind` 的记录（不进主树）。Weak-binding side table. */
    sml_weakbind *binds;
    size_t n_binds, cap_binds;
    /* 当前块的路径栈（键名；数组元素段写成 `[n]`）。
       上层的 `enter_key` 由调用方在 parse_block 之前设置，进入时压栈、退出时弹栈。
       Path stack: callers set `enter_key` right before parse_block. */
    const char *enter_key;
    char **path;
    size_t path_n, path_cap;
    /* 当前字段**键 token** 的行号：重复键诊断要用它，不能用「下一个 token」
       （那个 token 已在下一行甚至越过块尾，报出来的行号是错的）。
       Line of the key token currently being parsed. */
    int key_line;
} parser;

/* 值嵌套深度上限：与 Rust 侧 MAX_VALUE_DEPTH 同口径。
   `a{a{a{ ... }}}` 这类输入会让递归下降一路压栈；栈溢出在 C 里是段错误，
   **无法被错误处理接住**，只能在递归入口主动限深。 */
#define SML_MAX_VALUE_DEPTH 128

/* 特性名表：与 Rust 的 `FEATURES` 同序同名（跨端契约，见
   `rust/sml-feature/src/lib.rs`）。C 端目前**只识别、不门控** ——
   位记录下来供后续 when / for / typed-block 使用，现有语法的门控随 P3 逐项接。 */
static const char *const SML_FEATURE_NAMES[] = {
    "bareword-string", "include", "env", "contract", "fragment",
    "top-level-array", "namespace", "implicit-ns", "multi-include",
    "glob-include", "regex-include", "ext-rewrite", "when", "for",
    "typed-block", "case-insensitive", "block-name", "type.featag",
    "lex.ident", "type.weak",
};

static int feature_index(const char *name) {
    size_t i;
    for (i = 0; i < sizeof(SML_FEATURE_NAMES) / sizeof(SML_FEATURE_NAMES[0]); i++) {
        if (strcmp(SML_FEATURE_NAMES[i], name) == 0) return (int)i;
    }
    return -1;
}

/* 版本基线：与 Rust `FeatureSet::baseline()` 一致的 8 位核心特性
   （bareword-string / include / env / contract / fragment / top-level-array /
   namespace / implicit-ns）。位序见 SML_FEATURE_NAMES。 */
#define SML_FEATURE_BASELINE \
    ((1u << 0) | (1u << 1) | (1u << 2) | (1u << 3) | \
     (1u << 4) | (1u << 5) | (1u << 6) | (1u << 7))

/* 预处理：扫行首的 `@feature enable|disable <名>[, <名>]`，算出初始特性集。
   必须在词法前调用 —— 引号串 `$env.` 内插、顶层数组形态判定都要看到最终特性集
   （与 Rust `sml-parse/src/scan.rs` 的 `strip_features` 同一定位）。
   解析主循环仍会再处理 `@feature`（幂等：重复置同一位结果不变）。
   简化：只看行首（前导空白后）；字符串内部的 `@feature` 会被误判，属已知边界。 */
static unsigned scan_features(const char *text) {
    unsigned feats = SML_FEATURE_BASELINE;
    const char *p = text;
    while (*p) {
        const char *line = p;
        const char *nl = strchr(p, '\n');
        size_t len = nl ? (size_t)(nl - p) : strlen(p);
        size_t i = 0;
        while (i < len && (line[i] == ' ' || line[i] == '\t')) i++;
        if (i + 8 <= len && strncmp(line + i, "@feature", 8) == 0
            && (i + 8 == len || line[i + 8] == ' ' || line[i + 8] == '\t')) {
            const char *q = line + i + 8;
            const char *end = line + len;
            int on = -1;
            while (q < end && (*q == ' ' || *q == '\t')) q++;
            if (end - q >= 6 && strncmp(q, "enable", 6) == 0) { on = 1; q += 6; }
            else if (end - q >= 7 && strncmp(q, "disable", 7) == 0) { on = 0; q += 7; }
            if (on >= 0) {
                for (;;) {
                    char name[64];
                    size_t nlen = 0;
                    int bit;
                    while (q < end && (*q == ' ' || *q == '\t' || *q == ',')) q++;
                    if (q >= end) break;
                    while (q < end && *q != ' ' && *q != '\t' && *q != ','
                           && nlen + 1 < sizeof(name)) {
                        name[nlen++] = *q++;
                    }
                    name[nlen] = '\0';
                    if (nlen == 0) break;
                    bit = feature_index(name);
                    if (bit < 0) break;   /* 未知名字留给解析层报 E-FEATURE-002 */
                    if (on) feats |= (1u << bit);
                    else    feats &= ~(1u << bit);
                }
            }
        }
        if (!nl) break;
        p = nl + 1;
    }
    return feats;
}

static sml_value *parse_block(parser *ps, tok_type closing);
static sml_value *parse_block_inner(parser *ps, tok_type closing);
static sml_value *parse_array(parser *ps);
/* 旁路表的路径栈（定义在 diag_free_all 之后，parse_block 先用到）。 */
static void path_push(parser *ps, const char *seg);
static void path_pop(parser *ps);
static sml_value *parse_array_inner(parser *ps);
/* 游标原语（定义在下方）：`parse_for` 需要提前可见。 */
static token *peek(parser *ps);
static token *peek_at(parser *ps, size_t off);
static token *next(parser *ps);

/* `@for var in a b c { ... }` —— 有界循环展开，返回数组。
   与 Rust `cond.rs::eval_for_header` + `Parser::parse_for` 同语义：
   只遍历有限枚举、变量只读、无 `while` / 无递归。
   Bounded loop only: finite enumeration, read-only variable, no recursion.
   ⚠️ 子 token 流通过**临时替换** lx->toks / n / pos 实现；所有出口都经 `done`
   统一恢复，避免失败路径把后续文档留在错的流上。 */
static sml_value *parse_for(parser *ps) {
    lexer *lx = ps->lx;
    token *saved_toks = lx->toks;
    size_t saved_n = lx->n, saved_pos = lx->pos;
    const char *saved_var = ps->loop_var, *saved_val = ps->loop_val;
    sml_value *arr = NULL;
    const char *var = NULL;
    const char **items = NULL;
    size_t n_items = 0, cap = 8, i, start = 0, end = 0, j;
    int depth_b = 1;

    next(ps); next(ps);   /* @ for */
    if (!peek(ps) || peek(ps)->t != T_WORD) {
        set_err(lx->errbuf, lx->errsz, SML_E_PARSE_018 " `@for` 缺少循环变量");
        ps->failed = 1;
        goto done;
    }
    var = peek(ps)->v;
    next(ps);
    if (!peek(ps) || peek(ps)->t != T_WORD || strcmp(peek(ps)->v, "in") != 0) {
        set_err(lx->errbuf, lx->errsz, SML_E_PARSE_018 " `@for` 缺少 `in`");
        ps->failed = 1;
        goto done;
    }
    next(ps);
    items = (const char **)malloc(cap * sizeof(char *));
    if (!items) { ps->failed = 1; goto done; }
    while (peek(ps) && peek(ps)->t == T_WORD) {
        if (n_items == cap) {
            const char **ni;
            cap *= 2;
            ni = (const char **)realloc(items, cap * sizeof(char *));
            if (!ni) { ps->failed = 1; goto done; }
            items = ni;
        }
        items[n_items++] = peek(ps)->v;
        next(ps);
    }
    if (n_items == 0) {
        set_err(lx->errbuf, lx->errsz,
                SML_E_PARSE_018 " `@for` 的 `in` 之后至少须有一个枚举项");
        ps->failed = 1;
        goto done;
    }
    if (!peek(ps) || peek(ps)->t != T_LBRACE) {
        set_err(lx->errbuf, lx->errsz, SML_E_PARSE_018 " `@for` 缺少循环体 `{ ... }`");
        ps->failed = 1;
        goto done;
    }
    start = lx->pos + 1;
    j = start;
    while (j < lx->n) {
        if (lx->toks[j].t == T_LBRACE) depth_b++;
        else if (lx->toks[j].t == T_RBRACE) { depth_b--; if (depth_b == 0) break; }
        j++;
    }
    if (depth_b != 0) {
        set_err(lx->errbuf, lx->errsz,
                SML_E_PARSE_001 " 未闭合的块（遇到文件结尾，缺少结束符号 }）");
        ps->failed = 1;
        goto done;
    }
    end = j;
    arr = sml_new_array();
    if (!arr) { ps->failed = 1; goto done; }
    for (i = 0; i < n_items && !ps->failed; i++) {
        size_t len = end - start;
        token *sub = (token *)calloc(len + 1, sizeof(token));
        sml_value *node;
        if (!sub) { ps->failed = 1; break; }
        memcpy(sub, lx->toks + start, len * sizeof(token));
        sub[len].t = T_EOF;
        sub[len].v = NULL;
        lx->toks = sub;
        lx->n = len + 1;
        lx->pos = 0;
        ps->loop_var = var;
        ps->loop_val = items[i];
        node = parse_block(ps, T_EOF);
        lx->toks = saved_toks;
        lx->n = saved_n;
        free(sub);
        if (ps->failed || !node) {
            if (node) sml_free(node);
            break;
        }
        sml_arr_push(arr, node);
    }

done:
    lx->toks = saved_toks;
    lx->n = saved_n;
    ps->loop_var = saved_var;
    ps->loop_val = saved_val;
    free(items);
    if (ps->failed) {
        if (arr) sml_free(arr);
        lx->pos = saved_pos;
        return NULL;
    }
    lx->pos = end + 1;
    return arr;
}

/* parse_block 的守卫 wrapper：真正的实现在 parse_block_inner。
   包一层而不是改每个 return 点，是为了不遗漏任何出口（错误路径同样要收尾）。
   真正让递归**收手**的不是这一层的返回值，而是 ps->failed + 各解析循环的 break。 */
static sml_value *parse_block(parser *ps, tok_type closing) {
    /* 口径（各端一致，**已实测钉住**）：`ps->depth` 是「进入本块**前**已进入的层数」
       （根块为 0），故允许 `depth == SML_MAX_VALUE_DEPTH` —— 第 128 层块放行、
       第 129 层报此码。
       ⚠️ 这里原先是 `>=`：128 层就被拒，而下面的文案写的是「**超过** 128 层」，
       行为与文案自相矛盾；且与 C++ / JS / Lua 实测边界（128 放行 / 129 报）差一格。
       改动前用闭合嵌套 `("a { "):rep(N) + ("} "):rep(N)` 实测量过五端：
       Rust 127 / C 127 / C++ 128 / JS 128 / Lua 128。
       本文件有**两处**同一守卫、共用 `ps->depth`：`parse_block` 与 `parse_array`
       （后者是 W17 补的 —— 嵌套数组递归落地时**必须同时**给闸，理由见那里的注释）。
       Rust 侧同样块与数组共用一个 `depth` 计数器，故两处口径一致。 */
    if (ps->depth > SML_MAX_VALUE_DEPTH) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_LIMIT_001 " 嵌套过深（超过 %d 层），疑似递归或恶意输入",
                SML_MAX_VALUE_DEPTH);
        ps->failed = 1;
        /* ⚠️ 这里**不能**只把 depth 复位为 0 就了事（最初就是这么写的，实测仍崩）：
           复位不会让已经压上去的栈帧退回来，外层循环随即又从 0 开始往下钻，于是
           「每 128 层一轮」反复压栈，10 万层块嵌套照样打穿栈。真正收手靠 ps->failed：
           parse_array / parse_block_inner 的循环见到它就 break，栈才会一层层退掉。 */
        return sml_new_null();
    }
    ps->depth++;
    /* 路径栈：调用方在调本函数前把本块的键名放进 `enter_key`（顶层为 NULL）。
       Path stack: the caller stores this block's key in `enter_key` first. */
    {
        const char *k = ps->enter_key;
        ps->enter_key = NULL;
        path_push(ps, k);
    }
    sml_value *v = parse_block_inner(ps, closing);
    path_pop(ps);
    if (ps->depth > 0) ps->depth--;
    return v;
}

/* 未闭合的块/数组/契约体（遇到文件结尾就收尾了）：E-PARSE-001。
   与 Rust 同码：rust/sml-parse/src/parser.rs 在「本层期望闭合符号（closing 非 None）
   却遇到文件结尾」时报 E_PARSE_001，契约体未闭合也归同一条。
   注：顶层（closing==T_EOF）遇到文件结尾是**正常**结束，不算未闭合，不报。 */
static void err_unclosed(parser *ps, const char *what) {
    if (ps->failed) return;   /* **第一条错误为准**：别覆盖已经写在 err 里的码 */
    set_err(ps->lx->errbuf, ps->lx->errsz,
            SML_E_PARSE_001 " 未闭合的%s（遇到文件结尾，缺少结束符号）", what);
    ps->failed = 1;
}

static token *peek(parser *ps) {
    return &ps->lx->toks[ps->lx->pos];
}

static token *peek_at(parser *ps, size_t off) {
    size_t idx = ps->lx->pos + off;
    if (idx >= ps->lx->n) return &ps->lx->toks[ps->lx->n - 1];
    return &ps->lx->toks[idx];
}

static token *next(parser *ps) {
    token *t = &ps->lx->toks[ps->lx->pos];
    if (t->t != T_EOF) ps->lx->pos++;
    return t;
}

/* 标量识别 (裸词) */
static sml_value *coerce_word(const char *w, parser *ps) {
    if (strcmp(w, "true") == 0) return sml_new_bool(1);
    if (strcmp(w, "false") == 0) return sml_new_bool(0);
    if (strcmp(w, "null") == 0) return sml_new_null();
    /* $env.VAR */
    if (strncmp(w, "$env.", 5) == 0) {
        const char *ev = getenv(w + 5);
        return sml_new_str(ev ? ev : "");
    }
    /* 片段引用 &name */
    if (w[0] == '&') {
        struct frag *f;
        for (f = ps->frags; f; f = f->next) {
            if (strcmp(f->name, w + 1) == 0) {
                /* 返回深拷贝 */
                return sml_clone(f->val);
            }
        }
        /* 未定义的片段引用 ⇒ E-INCLUDE-006（W16，与 Rust/JS 同码）。
           改前这里 `return sml_new_str(w)`：拼错的片段名**静默退化**成普通字符串
           `"&nosuchfrag"`，下游取值取不到还查不出原因 —— 用户已裁决这类必须报错
           （"这种不应该出现，堪比 void"）。
           返回 NULL 是安全的：sml_obj_set 见到 NULL 会跳过该键，且 ps->failed
           会让各解析循环立刻收手（与 v2 裸词报错那条路径同一处理方式）。 */
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_INCLUDE_006 " 未定义的片段引用 `%s`", w);
        ps->failed = 1;
        return NULL;
    }
    /* 数字 */
    char *end = NULL;
    errno = 0;   /* ERANGE 必须「先清零再检查」，否则会读到之前调用留下的旧值 */
    long long iv = strtoll(w, &end, 10);
    if (end && *end == '\0' && end != w) {
        /* B10（对齐 Rust sml-lex 的 coerce_word）：**纯整数形态但超出 i64** 时，
           保留为字符串 —— 不能返回被夹住的 Int。Rust 在 `w.parse::<i64>()` 失败且
           looks_int 时 `return Ok(Value::Str(w.to_string()))`（round-trip 安全、零精度
           损失），且该 return **早于** bareword-string 检查，故 v2/v3 下同样不报
           E-FEATURE-005 —— 这里保持同一分支顺序。
           修之前 `99999999999999999999` 会变成 Int 9223372036854775807（静默错值，
           且能通过 `int` 契约校验）。 */
        if (errno == ERANGE) return sml_new_str(w);
        return sml_new_int(iv);
    }
    /* 浮点 (含 .5 / 1e3 等 strtod 可解析) */
    if (strchr(w, '.') || strchr(w, 'e') || strchr(w, 'E')) {
        char *fend = NULL;
        double fv = strtod(w, &fend);
        if (fend && *fend == '\0' && fend != w) return sml_new_float(fv);
    }
    /* V2/V3 严格模式：自由字符串必须加引号 */
    if (ps->version >= 2) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_FEATURE_005 " v2/v3 字符串必须加引号，裸词 `%s` 应写作 \"%s\"", w, w);
        ps->failed = 1;
        return NULL;
    }
    /* `bareword-string` 门控：关闭后纯字符串裸词必须加引号（与 Rust 同码 005）。 */
    if (!(ps->features & (1u << 0))) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_FEATURE_005 " 裸词 `%s` 应写作 `\"%s\"`（bareword-string 已禁用）", w, w);
        ps->failed = 1;
        return NULL;
    }
    return sml_new_str(w);
}

/* T9：判断 token 是否为「值型标量」（数字 / 引号串 / bool / null）。
 * 裸词标识符（如 `server`、`a`，作块类型名 / 块名）**不算**值型 —— 这是区分
 * T9 误报（`42 a { }`）与合法带类型块（`server web { }`）的关键。与 Rust/C++ 同判据。 */
static int tok_is_scalar_value(const token *t) {
    if (!t) return 0;
    if (t->t == T_STR) return 1;                       /* 引号串 */
    if (t->t != T_WORD) return 0;                     /* @ 指令 / 结构记号 */
    const char *w = t->v;
    if (w[0] == '\0') return 0;
    if (strcmp(w, "true") == 0 || strcmp(w, "false") == 0
        || strcmp(w, "null") == 0)
        return 1;
    /* 数字：整串可解析为浮点（与 Rust 的 parse::<f64>() 同口径） */
    char *end = NULL;
    errno = 0;
    strtod(w, &end);
    return (end != w && *end == '\0');
}

static void frag_put(parser *ps, const char *name, sml_value *v) {
    struct frag *f = (struct frag *)calloc(1, sizeof(struct frag));
    if (!f) return;
    f->name = strdup(name);
    f->val = v;
    f->next = ps->frags;
    ps->frags = f;
}

/* 诊断（warning）收集与释放。Diagnostics: append / free all. */
static void diag_push_at(parser *ps, const char *code, const char *msg, int line) {
    sml_diag *nd;
    size_t nc;
    if (!ps->want_diags) return;
    if (ps->n_diags == ps->cap_diags) {
        nc = ps->cap_diags ? ps->cap_diags * 2 : 8;
        nd = (sml_diag *)realloc(ps->diags, nc * sizeof(sml_diag));
        if (!nd) return;
        ps->diags = nd;
        ps->cap_diags = nc;
    }
    ps->diags[ps->n_diags].code = strdup(code);
    ps->diags[ps->n_diags].message = strdup(msg);
    ps->diags[ps->n_diags].line = line;
    ps->n_diags++;
}

/* 行号取「下一个未消费 token」的行 —— 对**消费完才发现**的诊断（如重复键）
   这是下一行的行号，故那类调用点必须走 `diag_push_at` 显式给行。 */
static void diag_push(parser *ps, const char *code, const char *msg) {
    int line = (ps->lx && ps->lx->pos < ps->lx->n)
        ? (int)ps->lx->toks[ps->lx->pos].line : 0;
    diag_push_at(ps, code, msg, line);
}

/* 释放 `sml_parse_ex` 交出的诊断数组（含每条的 code/message）。 */
void sml_diag_free(sml_diag *diags, size_t n) {
    size_t i;
    if (!diags) return;
    for (i = 0; i < n; i++) { free(diags[i].code); free(diags[i].message); }
    free(diags);
}

static void diag_free_all(parser *ps) {
    sml_diag_free(ps->diags, ps->n_diags);
    ps->diags = NULL;
    ps->n_diags = 0;
    ps->cap_diags = 0;
}

/* ---- 弱绑定旁路表（`docs/swsml/spec/weakbind.md` §5）---- */

/* 释放旁路表（含每条的 path/name）。Free a weak-binding side table. */
void sml_weakbind_free(sml_weakbind *binds, size_t n) {
    size_t i;
    if (!binds) return;
    for (i = 0; i < n; i++) {
        free(binds[i].path);
        free(binds[i].name);
    }
    free(binds);
}

static void path_push(parser *ps, const char *seg) {
    if (!seg || !*seg) return;
    if (ps->path_n == ps->path_cap) {
        size_t nc = ps->path_cap ? ps->path_cap * 2 : 8;
        char **np = (char **)realloc(ps->path, nc * sizeof(char *));
        if (!np) return;
        ps->path = np;
        ps->path_cap = nc;
    }
    ps->path[ps->path_n++] = strdup(seg);
}

static void path_pop(parser *ps) {
    if (ps->path_n == 0) return;
    free(ps->path[--ps->path_n]);
}

static void path_free_all(parser *ps) {
    while (ps->path_n) path_pop(ps);
    free(ps->path);
    ps->path = NULL;
    ps->path_cap = 0;
}

/* 拼出「祖先路径 + 本级键」：键名用 `.` 连接，`[n]` 段不加点。
   Build the dotted path of the new leaf; `[n]` segments take no dot. */
static char *path_join(parser *ps, const char *leaf) {
    size_t i, len = 1;
    char *out;
    for (i = 0; i < ps->path_n; i++) len += strlen(ps->path[i]) + 2;
    if (leaf) len += strlen(leaf) + 2;
    out = (char *)calloc(1, len);
    if (!out) return NULL;
    for (i = 0; i < ps->path_n; i++) {
        if (i && ps->path[i][0] != '[') strcat(out, ".");
        strcat(out, ps->path[i]);
    }
    if (leaf && *leaf) {
        if (out[0] && leaf[0] != '[') strcat(out, ".");
        strcat(out, leaf);
    }
    return out;
}

/* 记一条弱绑定：主树不动，只进旁路表。Record one `@weakbind` (side table only). */
static void weak_bind_push(parser *ps, const char *name, int line, const char *leaf) {
    if (ps->n_binds == ps->cap_binds) {
        size_t nc = ps->cap_binds ? ps->cap_binds * 2 : 4;
        sml_weakbind *nb = (sml_weakbind *)realloc(ps->binds, nc * sizeof(sml_weakbind));
        if (!nb) return;
        ps->binds = nb;
        ps->cap_binds = nc;
    }
    ps->binds[ps->n_binds].path = path_join(ps, leaf);
    ps->binds[ps->n_binds].name = strdup(name ? name : "");
    ps->binds[ps->n_binds].line = line;
    ps->n_binds++;
}

static void weak_check_block(parser *ps, sml_value *node, const char *cname);

/* 对象内同名键冲突 -> 提升为数组 */
static void obj_set_dup(parser *ps, sml_value *obj, const char *key, sml_value *val) {
    sml_value *ex = sml_obj_get(obj, key);
    if (!ex) {
        sml_obj_set(obj, key, val);
        return;
    }
    /* 重复键：值树照旧提升为数组，同时记一条 `W-PARSE-001`（不改变值）。 */
    if (ps) diag_push_at(ps, "W-PARSE-001", "键重复出现，已提升为数组", ps->key_line);
    if (ex->type == SML_ARRAY) {
        sml_arr_push(ex, val);
    } else {
        sml_value *arr = sml_new_array();
        sml_arr_push(arr, ex);
        sml_arr_push(arr, val);
        /* 替换 (需要先删再设) */
        sml_field *f;
        for (f = obj->u.obj.head; f; f = f->next) {
            if (strcmp(f->key, key) == 0) {
                f->value = arr;
                return;
            }
        }
    }
}

/* `@when` 只作用于紧邻的下一个字段/块：条件为假就丢弃并释放该项。
   `@when` applies to the immediately following field/block only; a false
   condition drops the item and frees its value. */
static void set_field_maybe_skip(parser *ps, sml_value *obj, const char *key,
                                 sml_value *val) {
    if (ps->pending_when == 2) {
        ps->pending_when = 0;
        sml_free(val);
        return;
    }
    ps->pending_when = 0;
    /* 弱绑定：`@weakbind X` 之后的那个块，用内置表做**只读**软校验（不填默认值、
       不删改键），并把「哪个块绑了哪个契约」记进旁路表。
       Weak-binding soft check (read-only) + side-table record for the next block. */
    if (ps->pending_weak && val && val->type == SML_OBJECT) {
        const char *wname = ps->pending_weak;
        weak_check_block(ps, val, wname);
        weak_bind_push(ps, wname, ps->pending_weak_line, key);
        ps->pending_weak = NULL;
    }
    obj_set_dup(ps, obj, key, val);
}

/* `@when` 条件求值（闭集：`$env.NAME` [+ `==`/`!=`/`=` + 值]）。
   与 Rust `cond.rs::eval_when_cond` 同语义：无运算符时做真值测试
   （非空且非 "0" / "false"）。
   Closed set, no expression evaluator — mirrors Rust `cond.rs::eval_when_cond`. */
static int eval_when(parser *ps) {
    token *lhs = next(ps);
    const char *var;
    token *op;
    if (!lhs || (lhs->t != T_WORD && lhs->t != T_STR)) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_014 " `@when` 后须条件（如 `$env.ENV == \"prod\"`）");
        ps->failed = 1;
        return 0;
    }
    if (strncmp(lhs->v, "$env.", 5) != 0 || lhs->v[5] == '\0') {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_014 " `@when` 的条件左侧只支持 `$env.NAME`，得 `%s`", lhs->v);
        ps->failed = 1;
        return 0;
    }
    var = lhs->v + 5;
    if (!(ps->features & (1u << 2))) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_FEATURE_002 " `@when $env.%s` 需要特性 `env`", var);
        ps->failed = 1;
        return 0;
    }
    {
        const char *act = getenv(var);
        if (!act) act = "";
        op = peek(ps);
        {
            int is_eq = 0, is_ne = 0;
            if (op && op->t == T_EQ) {
                /* `=` 是独立 token；`==` 被词法切成两个 T_EQ
                   （Rust 侧是一个 Word("==")）。 */
                is_eq = 1;
                next(ps);
                if (peek(ps)->t == T_EQ) next(ps);
            } else if (op && op->t == T_WORD && strcmp(op->v, "==") == 0) {
                is_eq = 1;
                next(ps);
            } else if (op && op->t == T_WORD && strcmp(op->v, "!=") == 0) {
                is_ne = 1;
                next(ps);
            }
            if (!is_eq && !is_ne) {
                /* 真值测试：空串 / "0" / "false" 视为假 ｜ truthiness test */
                return (act[0] != '\0' && strcmp(act, "0") != 0
                        && strcmp(act, "false") != 0) ? 1 : 0;
            }
            {
                token *rhs = peek(ps);
                const char *b;
                char buf[512];
                int eq;
                const char *opname = is_ne ? "!=" : "==";
                if (!rhs || (rhs->t != T_WORD && rhs->t != T_STR)) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_015 " `@when` 的 `%s` 后须比较值", opname);
                    ps->failed = 1;
                    return 0;
                }
                /* 漏写比较值时，下一个字段名会被当比较值吃掉 —— 看 rhs 之后是不是 `:` */
                if (peek_at(ps, 1)->t == T_COLON) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_015 " `@when` 的 `%s` 后缺少比较值"
                            "（写成了运算符后直接换行）", opname);
                    ps->failed = 1;
                    return 0;
                }
                b = rhs->v;
                next(ps);
                if (b[0] == '"') {
                    size_t len = strlen(b);
                    size_t n = (len >= 2 && b[len - 1] == '"') ? len - 2 : len - 1;
                    if (n >= sizeof(buf)) n = sizeof(buf) - 1;
                    memcpy(buf, b + 1, n);
                    buf[n] = '\0';
                    b = buf;
                }
                eq = (strcmp(act, b) == 0);
                if (is_ne) return eq ? 0 : 1;
                return eq;
            }
        }
    }
}

/* `${var}` 只读插值：未绑定则原样保留。返回 malloc 的串（调用方 free）。
   Read-only loop-variable interpolation; unknown names stay verbatim.
   用固定缓冲 + 截断，避免 realloc 路径带来的泄漏面（SML 值本身也有长度上限）。 */
static char *interp_loop(parser *ps, const char *s) {
    static char buf[8192];
    const char *p = s;
    size_t len = 0;
    if (!ps->loop_var || !strstr(s, "${")) return strdup(s);
    while (*p && len + 1 < sizeof(buf)) {
        const char *at = strstr(p, "${");
        const char *close;
        size_t seg, nlen;
        if (!at) break;
        close = strchr(at + 2, '}');
        if (!close) break;
        seg = (size_t)(at - p);
        if (len + seg + 1 >= sizeof(buf)) seg = sizeof(buf) - len - 1;
        memcpy(buf + len, p, seg);
        len += seg;
        nlen = (size_t)(close - at - 2);
        if (nlen == strlen(ps->loop_var) && strncmp(at + 2, ps->loop_var, nlen) == 0) {
            size_t vlen = ps->loop_val ? strlen(ps->loop_val) : 0;
            if (len + vlen + 1 >= sizeof(buf)) vlen = sizeof(buf) - len - 1;
            if (vlen) { memcpy(buf + len, ps->loop_val, vlen); len += vlen; }
        } else {
            size_t keep = (size_t)(close - at) + 1;
            if (len + keep + 1 >= sizeof(buf)) keep = sizeof(buf) - len - 1;
            memcpy(buf + len, at, keep);
            len += keep;
        }
        p = close + 1;
    }
    {
        size_t rest = strlen(p);
        if (len + rest + 1 > sizeof(buf)) rest = sizeof(buf) - len - 1;
        memcpy(buf + len, p, rest);
        len += rest;
    }
    buf[len] = '\0';
    return strdup(buf);
}

/* 声明侧软校验：`@weakbind X` 的可选体内每个字段，与内置表比对（只读）。
   Declaration-side soft check for an optional `{ field: type }` body. */
static void weak_check_decl(parser *ps, const char *cname, const char *fname,
                            const char *tyname) {
    ccontract *c;
    cfield *f;
    char msg[256];
    if (!ps->builtin) return;
    for (c = ps->builtin->contracts; c; c = c->next) {
        if (strcmp(c->name, cname) == 0) break;
    }
    if (!c) return;   /* 契约名不在表里：已在声明处报过「未知契约」 */
    for (f = c->fields; f; f = f->next) {
        if (strcmp(f->name, fname) == 0) break;
    }
    if (!f) {
        snprintf(msg, sizeof(msg), "契约 `%s` 没有字段 `%s`", cname, fname);
        diag_push(ps, "W-CONTRACT-001", msg);
        return;
    }
    if (tyname && strcmp(type_name(f->ty), tyname) != 0) {
        snprintf(msg, sizeof(msg), "字段 `%s.%s` 的类型与内置契约不同", cname, fname);
        diag_push(ps, "W-CONTRACT-001", msg);
    }
}

/* 弱绑定软校验：只用内置表做**只读**检查（不填默认值、不删改键）。
   与 `@is` 的 apply_contract 是两条路径：那条会改树，这条只出 warning。
   Weak-binding soft check: read-only, never mutates the value. */
static void weak_check_block(parser *ps, sml_value *node, const char *cname) {
    ccontract *c;
    cfield *f;
    if (!ps->builtin) return;
    for (c = ps->builtin->contracts; c; c = c->next) {
        if (strcmp(c->name, cname) == 0) break;
    }
    if (!c) return;   /* 内置表里没有 ⇒ 声明侧已处理，块侧不重复报 */
    for (f = c->fields; f; f = f->next) {
        sml_value *v = sml_obj_get(node, f->name);
        char msg[256];
        if (!v) {
            if (f->required) {
                snprintf(msg, sizeof(msg), "块里缺必填字段 `%s.%s`", cname, f->name);
                diag_push(ps, "W-CONTRACT-001", msg);
            }
            continue;
        }
        /* 类型 / 枚举：只读比对，**不调用 check_type**（后者会 set_err、可能中止解析）。 */
        {
            int ok = 1;
            if (f->ty == CT_STR)  ok = (v->type == SML_STR);
            else if (f->ty == CT_INT) ok = (v->type == SML_INT);
            else if (f->ty == CT_NUM) ok = (v->type == SML_INT || v->type == SML_FLOAT);
            else if (f->ty == CT_BOOL) ok = (v->type == SML_BOOL);
            else if (f->ty == CT_ENUM) {
                if (v->type != SML_STR) ok = 0;
                else {
                    size_t k;
                    int found = 0;
                    for (k = 0; k < f->enum_n; k++) {
                        if (strcmp(f->enum_vals[k], v->u.s) == 0) { found = 1; break; }
                    }
                    ok = found;
                }
            }
            if (!ok) {
                snprintf(msg, sizeof(msg), "字段 `%s.%s` 与内置契约类型不符", cname, f->name);
                diag_push(ps, "W-CONTRACT-001", msg);
                continue;
            }
        }
        /* 数值区间：只对数值值做（字符串不在这里报）。 */
        if (f->min_set || f->max_set) {
            double n = 0;
            int isnum = 0;
            if (v->type == SML_INT) { n = (double)v->u.i; isnum = 1; }
            else if (v->type == SML_FLOAT) { n = v->u.f; isnum = 1; }
            if (isnum && ((f->min_set && n < f->min) || (f->max_set && n > f->max))) {
                snprintf(msg, sizeof(msg), "字段 `%s.%s` 超出内置契约的区间", cname, f->name);
                diag_push(ps, "W-CONTRACT-001", msg);
            }
        }
    }
}

/* ---- 契约: 查找/校验/应用 ---- */

static ccontract *contract_find(parser *ps, const char *name) {
    for (ccontract *c = ps->contracts; c; c = c->next)
        if (strcmp(c->name, name) == 0) return c;
    return NULL;
}

/* 前向声明 (check_type 中递归调用) */
static int apply_contract_rec(parser *ps, ccontract *c, sml_value *node,
                              char *err, size_t errsz);

/* 校验值是否符合字段规格 (含 ContractRef 递归)。出错写 err 返回 -1 */
static int check_type(parser *ps, const char *cname, const cfield *spec,
                      const sml_value *v, char *err, size_t errsz) {
    int ok = 0;
    switch (spec->ty) {
        case CT_ANY:  ok = 1; break;
        case CT_STR:  ok = (v->type == SML_STR); break;
        case CT_INT:  ok = (v->type == SML_INT); break;
        case CT_NUM:  ok = (v->type == SML_INT || v->type == SML_FLOAT); break;
        case CT_BOOL: ok = (v->type == SML_BOOL); break;
        case CT_ENUM:
            ok = 0;
            for (size_t i = 0; i < spec->enum_n; i++)
                if (value_eq_str(v, spec->enum_vals[i])) { ok = 1; break; }
            break;
        case CT_ARRAY: {
            if (v->type != SML_ARRAY) { ok = 0; break; }
            ok = 1;
            for (size_t i = 0; i < v->u.arr.len; i++) {
                sml_value *el = v->u.arr.items[i];
                int eok = 0;
                switch (spec->arr_inner) {
                    case CT_STR:  eok = (el->type == SML_STR); break;
                    case CT_INT:  eok = (el->type == SML_INT); break;
                    case CT_NUM:  eok = (el->type == SML_INT || el->type == SML_FLOAT); break;
                    case CT_BOOL: eok = (el->type == SML_BOOL); break;
                    case CT_ANY:  eok = 1; break;
                    default:      eok = 1; break;
                }
                if (!eok) { ok = 0; break; }
            }
            break;
        }
        case CT_CONTRACTREF: {
            if (v->type != SML_OBJECT) {
                /* 「组合字段应为块」是**独立条件**（E-CONTRACT-008），不能与「类型不符」
                   （E-CONTRACT-002）共用一个码 —— Rust / JS 侧同样区分，否则同一份文档
                   在 C 与它们之间会拿到不同的码，「同因同码」当场破功。 */
                set_err(err, errsz,
                         SML_E_CONTRACT_008 " 字段 `%s` 应为块并按契约 `%s` 校验，实际为 %s（契约 `%s`）",
                         spec->name, spec->ref_name, kind_name(v), cname);
                return -1;
            }
            ccontract *tgt = contract_find(ps, spec->ref_name);
            if (!tgt) {
                set_err(err, errsz,
                         SML_E_CONTRACT_001 " 字段 `%s` 引用了未定义的契约 `%s`（契约 `%s`）",
                         spec->name, spec->ref_name, cname);
                return -1;
            }
            if (apply_contract_rec(ps, tgt, (sml_value *)v, err, errsz) != 0) return -1;
            ok = 1;
            break;
        }
    }
    if (!ok) {
        /* 「取值不在枚举列表内」是**独立条件**（E-CONTRACT-006），不能与「类型不符」
           （E-CONTRACT-002）共用一个码 —— Rust / JS 侧同样区分（见 rust/sml-contract
           的 check_type：`if let TypeSpec::Enum(..)` 单独返回 E_CONTRACT_006）。 */
        if (spec->ty == CT_ENUM) {
            set_err(err, errsz,
                     SML_E_CONTRACT_006 " 字段 `%s` 类型应为 enum，实际取值不在列表内（契约 `%s`）",
                     spec->name, cname);
            return -1;
        }
        set_err(err, errsz,
                 SML_E_CONTRACT_002 " 字段 `%s` 类型应为 %s，实际为 %s（契约 `%s`）",
                 spec->name, type_name(spec->ty), kind_name(v), cname);
        return -1;
    }
    /* 数值区间 */
    if (spec->min_set || spec->max_set) {
        double n = 0;
        int isnum = 0;
        if (v->type == SML_INT)        { n = (double)v->u.i; isnum = 1; }
        else if (v->type == SML_FLOAT) { n = v->u.f;           isnum = 1; }
        if (isnum) {
            if (spec->min_set && n < spec->min) {
                set_err(err, errsz,
                         SML_E_CONTRACT_005 " 字段 `%s` 值 %g 小于下界 %g（契约 `%s`）",
                         spec->name, n, spec->min, cname);
                return -1;
            }
            if (spec->max_set && n > spec->max) {
                set_err(err, errsz,
                         SML_E_CONTRACT_005 " 字段 `%s` 值 %g 大于上界 %g（契约 `%s`）",
                         spec->name, n, spec->max, cname);
                return -1;
            }
        }
    }
    return 0;
}

/* 前向声明 (check_type 中用到) */
static int apply_contract_rec(parser *ps, ccontract *c, sml_value *node,
                              char *err, size_t errsz);

/* 对块应用契约: 严格性 + 默认值 + 逐字段校验/组合递归 */
static int apply_contract_rec(parser *ps, ccontract *c, sml_value *node,
                              char *err, size_t errsz) {
    if (node->type != SML_OBJECT) return 0;
    /* 1) 严格性: 未声明字段一律拒绝 (除非 loose) */
    if (!c->allow_extra) {
        sml_field *f;
        for (f = node->u.obj.head; f; f = f->next) {
            if (!strcmp(f->key, "__type") || !strcmp(f->key, "__name")) continue;
            int found = 0;
            for (cfield *cf = c->fields; cf; cf = cf->next)
                if (strcmp(cf->name, f->key) == 0) { found = 1; break; }
            if (!found) {
                set_err(err, errsz,
                         SML_E_CONTRACT_004 " 字段 `%s` 未在契约 `%s` 中声明（严格模式；如需允许额外字段请在契约名后写 `loose`）",
                         f->key, c->name);
                return -1;
            }
        }
    }
    /* 2) 逐字段: 填默认 + 类型/枚举/区间/组合校验 */
    for (cfield *cf = c->fields; cf; cf = cf->next) {
        sml_value *v = sml_obj_get(node, cf->name);
        if (!v) {
            if (cf->def) {
                sml_obj_set(node, cf->name, sml_clone(cf->def));
            } else if (cf->required) {
                set_err(err, errsz,
                         SML_E_CONTRACT_003 " 字段 `%s` 必填但缺失（契约 `%s`）",
                         cf->name, c->name);
                return -1;
            }
        } else {
            if (check_type(ps, c->name, cf, v, err, errsz) != 0) return -1;
        }
    }
    return 0;
}

static int apply_contract_name(parser *ps, sml_value *node, const char *name,
                               char *err, size_t errsz) {
    ccontract *c = contract_find(ps, name);
    if (!c) {
        set_err(err, errsz, SML_E_CONTRACT_001 " 引用了未定义的契约 `%s`", name);
        return -1;
    }
    return apply_contract_rec(ps, c, node, err, errsz);
}

/* 应用契约；失败则置 parser.failed 标志 (不依赖 errbuf 是否有遗留内容)。
   `ps->failed` 已经是 1 时**直接返回**：块级 `@is` 是在块解析**结束处**应用的，
   而块体里可能已经报过更具体的错（词法 / 语法 / 未定义片段引用）—— 不加这道
   闸，后应用的契约错会把它覆盖掉，用户拿到的码就不是第一个错。 */
static void apply_or_fail(parser *ps, sml_value *node, const char *name) {
    if (ps->failed) return;
    if (apply_contract_name(ps, node, name, ps->lx->errbuf, ps->lx->errsz) != 0)
        ps->failed = 1;
}

/* min/max 的边界取值：必须是十进制数字，且为**有限**值。
   非数字/非十进制写法 -> E-PARSE-022；非有限（nan / 1e400）-> E-CONTRACT-010。
   与 Rust 同码（rust/sml-parse/src/parser.rs 的 parse_spec_number：只收 Tok::Word 且
   f64 可解析；`!f.is_finite()` 时报 E_CONTRACT_010；其它一律 E_PARSE_022）。
   修之前这里是 `atof()` 且不看结果：`min abc` 静默变成下界 0，`max abc` 更糟 ——
   非法边界当 0 后把合法值判成 E-CONTRACT-005（**错码**）。 */
static int parse_bound_value(parser *ps, const char *kw, double *out) {
    token *nv = next(ps);
    char *end = NULL;
    double d;
    if (!nv || nv->t != T_WORD) {
        /* Rust 对 Tok::Str / 缺取值（EOF）同样报 E_PARSE_022（"期望数字"） */
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_022 " `%s` 的边界取值不是数字", kw);
        ps->failed = 1;
        return -1;
    }
    /* C99 的 strtod 还认 "0x10"（十六进制浮点，= 16.0），而 Rust 的 f64::from_str 不认。
       不收窄就会比 Rust 多接受一种写法；十进制浮点/科学计数/ inf / nan 都不含 x X p P。 */
    if (strpbrk(nv->v, "xXpP") != NULL) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_022 " `%s` 的边界取值 `%s` 不是数字", kw, nv->v);
        ps->failed = 1;
        return -1;
    }
    d = strtod(nv->v, &end);
    if (!end || end == nv->v || *end != '\0') {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_022 " `%s` 的边界取值 `%s` 不是数字", kw, nv->v);
        ps->failed = 1;
        return -1;
    }
    if (!isfinite(d)) {
        /* NaN 的所有比较均为 false，会令 min/max 校验被静默穿透（Rust 侧审计 #2 同因） */
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_CONTRACT_010 " `%s` 的边界取值 `%s` 非有限数，不能作为数值约束的取值", kw, nv->v);
        ps->failed = 1;
        return -1;
    }
    *out = d;
    return 0;
}

/* 解析契约体 (调用前已消费 '{'；负责消费 '}') */
static void parse_contract_body(parser *ps, ccontract *c) {
    if (peek(ps)->t != T_LBRACE) return;
    next(ps); /* consume { */
    while (1) {
        token *ft = peek(ps);
        if (ps->failed) break;   /* 出错后立刻收手：否则会继续吞 token 并可能覆盖已写的错误码 */
        /* —— 契约体里的 `@requires`：本端暂未实现，必须**响亮拒绝** ——
           参照 E-CONTRACT-017（契约叠加未实现）的既有先例。
           ⚠️ 改前 T_AT 落进下面「非 WORD/STR 就 next 丢弃」的兜底分支被**静默吞掉**，
           随后的 `requires` 反被当成字段名、`typed-block` 变成 CT_CONTRACTREF，
           最终 rc=0、契约里多出一个垃圾字段 —— 这正是要消灭的「静默给树」。 */
        if (ft->t == T_AT) {
            next(ps); /* 消费 '@' */
            token *dt = peek(ps);
            const char *dname = (dt->t == T_WORD || dt->t == T_STR) ? dt->v : "";
            if (strcmp(dname, "requires") == 0) {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_CONTRACT_018 " 本实现暂不支持契约的 `@requires` 指令（它要求先开启所列特性）");
            } else {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_005 " 契约体内不支持指令 `@%s`（契约体仅支持 `@requires`）", dname);
            }
            ps->failed = 1;
            break;
        }
        if (ft->t == T_RBRACE) { next(ps); break; }
        if (ft->t == T_EOF) { err_unclosed(ps, "契约体"); break; }
        if (ft->t != T_WORD && ft->t != T_STR) { next(ps); continue; }
        char *fname = next(ps)->v;
        if (peek(ps)->t == T_COLON) next(ps);
        cfield *cf = (cfield *)calloc(1, sizeof(cfield));
        cf->name = strdup(fname);
        cf->required = 1;
        cf->arr_inner = CT_ANY;
        /* 类型 */
        token *tt = peek(ps);
        if (tt->t == T_LBRACK) {
            /* 数组: [ inner ] */
            next(ps); /* consume [ */
            cf->ty = CT_ARRAY;
            token *it = peek(ps);
            if (it->t == T_WORD) {
                char *iw = next(ps)->v;
                if      (strcmp(iw, "str")  == 0) cf->arr_inner = CT_STR;
                else if (strcmp(iw, "int")  == 0) cf->arr_inner = CT_INT;
                else if (strcmp(iw, "num")  == 0) cf->arr_inner = CT_NUM;
                else if (strcmp(iw, "bool") == 0) cf->arr_inner = CT_BOOL;
                else if (strcmp(iw, "any")  == 0) cf->arr_inner = CT_ANY;
            }
            if (peek(ps)->t == T_RBRACK) next(ps);
        } else if (tt->t == T_WORD) {
            char *tw = next(ps)->v;
            if      (strcmp(tw, "str")  == 0) cf->ty = CT_STR;
            else if (strcmp(tw, "int")  == 0) cf->ty = CT_INT;
            else if (strcmp(tw, "num")  == 0) cf->ty = CT_NUM;
            else if (strcmp(tw, "bool") == 0) cf->ty = CT_BOOL;
            else if (strcmp(tw, "any")  == 0) cf->ty = CT_ANY;
            else if (strcmp(tw, "enum") == 0) {
                cf->ty = CT_ENUM;
                if (peek(ps)->t == T_LBRACK) {
                    next(ps);
                    while (peek(ps)->t != T_RBRACK && peek(ps)->t != T_EOF) {
                        token *et = next(ps);
                        if (et->t == T_WORD || et->t == T_STR) {
                            cf->enum_vals = (char **)realloc(cf->enum_vals,
                                            (cf->enum_n + 1) * sizeof(char *));
                            cf->enum_vals[cf->enum_n++] = strdup(et->v);
                        }
                    }
                    if (peek(ps)->t == T_RBRACK) next(ps);
                }
            } else {
                /* 契约名 (组合 / ContractRef) */
                cf->ty = CT_CONTRACTREF;
                cf->ref_name = strdup(tw);
            }
        } else {
            cf->ty = CT_ANY;
        }
        /* 修饰符：只保留单词形式 optional / default / min / max / loose。 */
        for (;;) {
            token *mt = peek(ps);
            if (mt->t == T_QMARK || mt->t == T_EQ) {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_006 " 契约字段修饰符只支持单词形式 `optional` / `default`");
                ps->failed = 1;
                break;
            }
            if (mt->t == T_WORD && strcmp(mt->v, "optional") == 0) {
                next(ps); cf->required = 0; continue;
            }
            if (mt->t == T_WORD && strcmp(mt->v, "default") == 0) {
                next(ps);
                token *dv = next(ps);
                if (dv) cf->def = (dv->t == T_STR) ? sml_new_str(dv->v)
                                                  : coerce_word(dv->v, ps);
                continue;
            }
            if (mt->t != T_WORD) break;
            if (strcmp(mt->v, "min") == 0) {
                next(ps);
                if (parse_bound_value(ps, "min", &cf->min) == 0) cf->min_set = 1;
                continue;
            }
            if (strcmp(mt->v, "max") == 0) {
                next(ps);
                if (parse_bound_value(ps, "max", &cf->max) == 0) cf->max_set = 1;
                continue;
            }
            if (strcmp(mt->v, "loose") == 0) {
                next(ps); c->allow_extra = 1; continue;
            }
            break;
        }
        cf->next = c->fields;
        c->fields = cf;
    }
}

/* parse_array 的守卫 wrapper：真正的实现在 parse_array_inner。
   与 parse_block **共用同一个 `ps->depth`**（Rust 侧同样块与数组共用一个 depth 计数器），
   故「128 层放行 / 第 129 层报 E-LIMIT-001」对块嵌套与数组嵌套是同一口径。
   ⚠️ 这层是 W17 补的：给 parse_array_inner 加嵌套数组递归时**必须同时**给闸 ——
   只加递归等于把「静默错解」换成「栈溢出」，而后者在 C 里是段错误，错误处理接不住。 */
static sml_value *parse_array(parser *ps) {
    if (ps->depth > SML_MAX_VALUE_DEPTH) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_LIMIT_001 " 嵌套过深（超过 %d 层），疑似递归或恶意输入",
                SML_MAX_VALUE_DEPTH);
        ps->failed = 1;
        return sml_new_null();
    }
    ps->depth++;
    sml_value *v = parse_array_inner(ps);
    if (ps->depth > 0) ps->depth--;
    return v;
}

/* 数组元素位置的裸块 `type [name…] { … }`（W4 的 B 类对齐点）。
   与 Rust 的 `bare_block_ahead()` + `parse_bare_block()` **逐字对应**：

   * 入口判据：当前 token 必须是 **T_WORD**。Rust 的 `Some(Tok::Str(_))` 分支直接当字符串
     元素、不试裸块 —— 所以 `[ "sec" { } ]` 在两端都**不是**块，别顺手给 T_STR 也开。
   * 预扫描：从当前位置起，连续的词/串之后紧跟 `{` 才算裸块；撞上别的 token（含 `]`、
     EOF）即「不是」。因此 `[ hello world ]` 仍是两个标量元素（Rust 同）。
   * 识别成功：消费掉「类型词 [参数…] { … }」并返回块对象，块内写
     `__type` = 类型词（**原词**，不经 coerce）、`__name` = 首个参数、多余参数进 `__args`
     —— 与 Rust `parse_bare_block` 一致（参数不再静默丢弃）。
   * 不识别：返回 NULL 且**一个 token 都不消费**，调用方按原来的标量路径处理。

   为什么参数要 clone 后存：`args` 这个临时数组下面就被 sml_free，直接借用其中的值会让
   `__name` / `__args` 变成悬垂指针（与 parse_block 里「带名块堆损坏」那条注释同因）。 */
static sml_value *try_parse_array_bare_block(parser *ps) {
    if (peek(ps)->t != T_WORD) return NULL;
    size_t probe = ps->lx->pos;
    int found = 0;
    while (probe < ps->lx->n) {
        tok_type pt = ps->lx->toks[probe].t;
        if (pt == T_WORD || pt == T_STR) probe++;
        else if (pt == T_LBRACE) { found = 1; break; }
        else break;
    }
    if (!found) return NULL;
    /* ⚠️ 先**消费类型名**再收参数（Rust：`self.next(); // 消费类型名` 然后
       `parse_bare_block(&w)`）。漏掉这一次消费，类型词就会被当成第一个参数 ⇒
       `section 情节 { }` 得到 `__name: section` + `__args: [情节]`（实测踩过）。 */
    token *first = next(ps);
    const char *type_word = first->v;   /* 原词（token 存活到 lex_free） */
    sml_value *args = sml_new_array();
    /* 任务 D：数组位置的块级多契约同样「响亮拒绝」（head 命中契约表且参数里出现
       第 2 个已注册契约词 ⇒ E-CONTRACT-017）。 */
    int key_is_contract = (contract_find(ps, type_word) != NULL);
    int arg_has_contract = 0;
    while (peek(ps)->t == T_WORD || peek(ps)->t == T_STR) {
        token *at = next(ps);
        if (key_is_contract && contract_find(ps, at->v)) arg_has_contract = 1;
        sml_arr_push(args, at->t == T_STR ? sml_new_str(at->v) : coerce_word(at->v, ps));
    }
    if (key_is_contract && arg_has_contract) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_CONTRACT_017 " 该实现暂不支持一个块应用多份契约（块级多契约写法）");
        ps->failed = 1;
        sml_free(args);
        return NULL;
    }
    next(ps);   /* `{`（预扫描已保证在此） */
    ps->enter_key = type_word;   /* 旁路表路径用块类型名当段 */
    sml_value *sub = parse_block(ps, T_RBRACE);
    sml_obj_set(sub, "__type", sml_new_str(type_word));
    if (sml_arr_len(args) > 0) {
        size_t i;
        sml_obj_set(sub, "__name", sml_clone(sml_arr_get(args, 0)));
        if (sml_arr_len(args) > 1) {
            sml_value *extra = sml_new_array();
            for (i = 1; i < sml_arr_len(args); i++)
                sml_arr_push(extra, sml_clone(sml_arr_get(args, i)));
            sml_obj_set(sub, "__args", extra);
        }
    }
    sml_free(args);
    return sub;
}

static sml_value *parse_array_inner(parser *ps) {
    sml_value *arr = sml_new_array();
    for (;;) {
        token *t = peek(ps);
        if (ps->failed) break;   /* 出错/超限后立刻收手，让栈退掉（见 parse_block） */
        if (t->t == T_RBRACK) { next(ps); break; }
        /* 数组以 `[` 开场（本函数只在消费掉 `[` 之后被调用），故遇 EOF 即未闭合 */
        if (t->t == T_EOF) { err_unclosed(ps, "数组"); break; }
        if (t->t == T_COMMA) { next(ps); continue; }
        if (t->t == T_LBRACE) {
            char seg[32];
            next(ps);
            /* 旁路表路径：数组元素写成 `[n]`（与 Rust 的 PathSeg::Index 同表达）。 */
            snprintf(seg, sizeof seg, "[%zu]", sml_arr_len(arr));
            ps->enter_key = seg;
            sml_arr_push(arr, parse_block(ps, T_RBRACE));
        } else if (t->t == T_LBRACK) {
            /* 嵌套数组 `m: [ [a] b ]` / `m: [[1]]`（W17）。
               此前**没有这一支**，`[` 落到最后的 else 被 `next` 丢掉；后果不是"少一层"，
               而是**静默错解 + 凭空造键**（改前实测，与 Rust/JS 都不一致）：
                 `m: [ [ a ] ]`         → `{"m":["a"]}`         （期望 `{"m":[["a"]]}`）
                 `m: [ 1, [2, 3], 4 ]`  → `{"m":[1,2,3],"4":4}` （内层 `]` 被外层当结束符，
                                                                    剩下的 `4` 变成了键）
                 `m: [ [a], [b] ]`      → `{"m":["a"]}`         （`[b]` 整个丢掉）
                 `a: ` + 100 层 `[..]`  → `{"a":[]}`            （全被吞成空数组）
               与 Rust（parser.rs 的 `Tok::LBrack` → `self.parse_array()`）和 JS
               （同源 bug 早先已修）对齐。
               ⚠️ **必须走 wrapper `parse_array`**（不是直接调 inner）：深度守卫只在
               wrapper 里，直接递归 inner 会让深数组绕过上限 —— 那就是栈溢出。 */
            next(ps);
            sml_arr_push(arr, parse_array(ps));
        } else if (t->t == T_STR) {
            sml_arr_push(arr, sml_new_str(next(ps)->v));
        } else if (t->t == T_WORD) {
            /* 数组位置的裸块 `type [name…] { … }`（W4 的 B 类差异所在）。
               改前这里只有「当标量」一条路 ⇒ `[ section 情节 { … } ]` 被拆成
               `section`、`情节`、`{ … }` **三个**元素，而 Rust 是一个块对象
               （`{ __type: section, __name: 情节, … }`）。判据与写法照抄 Rust，
               见 try_parse_array_bare_block。 */
            sml_value *blk = try_parse_array_bare_block(ps);
            if (blk) sml_arr_push(arr, blk);
            else     sml_arr_push(arr, coerce_word(next(ps)->v, ps));
        } else if (t->t == T_RBRACE) {
            /* 数组里多余的 `}` ⇒ E-PARSE-003（W16 落地；Rust 一直报它，JS 同批已改）。
               改前这一格落在下面的兜底 else 里**静默跳过**：`m: [ } ]` 得到 `{"m":[]}`，
               与输入形状不符却毫无提示 —— 属「静默改数据」，正是本批要堵的一类。 */
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_003 " 多余的结束符号 `}`，没有与之匹配的开始符号");
            ps->failed = 1;
            break;
        } else {
            /* 兜底：其余 token（孤立 `:` / `@` / `?` / `=` 等）仍**静默跳过**。
               ⚠️ 不在本批（W16 判定表）范围内，别顺手扩：判定表只把「数组里多余的 `}`」
               定为改成报错，其余留给后续逐条判定。 */
            next(ps);
        }
    }
    return arr;
}

/* 解析块/对象。closing=T_RBRACE 或 T_EOF(顶层) */
static sml_value *parse_block_inner(parser *ps, tok_type closing) {
    sml_value *obj = sml_new_object();
    char *block_is = NULL;  /* 块级 @is 契约名 (作用于本块) */
    for (;;) {
        token *t = peek(ps);
        if (ps->failed) break;   /* 出错/超限后立刻收手，让栈退掉（见 parse_block） */
        if (t->t == T_EOF) {
            /* 只在「本层期望 `}`」时才算未闭合；顶层（closing==T_EOF）遇 EOF 是正常结束 */
            if (closing == T_RBRACE) err_unclosed(ps, "块");
            break;
        }
        if (t->t == T_RBRACE || t->t == T_RBRACK) {
            if (closing == t->t) { next(ps); break; }
            if (closing == T_EOF) {
                /* 顶层（closing==T_EOF）遇到结束符号：没有与之匹配的开始符号 ⇒
                   E-PARSE-003（W16）。改前这里直接 break：`a: 1` + `}` 静默成功，
                   而 Rust / JS 都报 003。 */
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_003 " 多余的结束符号 `%s`，没有与之匹配的开始符号",
                        t->t == T_RBRACE ? "}" : "]");
                ps->failed = 1;
                break;
            }
            /* 本层期望 `}`，却遇到 `]` ⇒ 闭合符错配 E-PARSE-002（W16）。
               改前这一格落到下面的 key 分支，`kt->t` 既不是 WORD 也不是 STR 就
               直接 break —— `a { ] }` 静默得到 `{"a":{}}`（Rust 报 002，JS 同批已改）。 */
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_002 " 闭合符错配：期望 `}`，实得 `%s`",
                    t->t == T_RBRACE ? "}" : "]");
            ps->failed = 1;
            break;
        }
        if (t->t == T_COMMA) {
            /* 逗号只用于对象字段分隔：其后须是 `key:`；否则是裸词拆分
               （`a: x,y`）或孤立逗号 —— 报错而非静默吞掉（与 Rust 同判据）。 */
            token *c1 = peek_at(ps, 1);
            token *c2 = peek_at(ps, 2);
            if (!((c1->t == T_WORD || c1->t == T_STR) && c2->t == T_COLON)) {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_007 " 非预期的逗号：裸词中不可含逗号，请改用数组或引号包裹");
                ps->failed = 1;
                break;
            }
            next(ps); continue;
        }
        if (t->t == T_BAREAT) {
            /* 孤立 `@`（其后是空白 / 行尾）：报错，不把下一行的键当指令名，
               也不把紧随的块当片段体吞掉（Rust 的 Tok::BareAt 同判据）。 */
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_004 " 孤立的 @：其后没有指令名或片段名");
            ps->failed = 1;
            break;
        }
        if (t->t == T_AT) {
            token *nxt = peek_at(ps, 1);
            /* 默认忽略：未开 `type.weak` 时 `@weakbind` / `@weakdata` / `@weakdoc`
               不解释、不校验、不产出（`docs/swsml/spec/weakbind.md` §0.3）。
               片段定义形态（后跟 `{`）**不消费**，落回原有语义，不抢占名字；
               其余形态把「指令 + 同一行的参数」吞掉，值树零影响。
               Default-ignore: swallow weak* directives unless `type.weak` is on. */
            if (nxt && nxt->t == T_WORD
                && (strcmp(nxt->v, "weakbind") == 0
                    || strcmp(nxt->v, "weakdata") == 0
                    || strcmp(nxt->v, "weakdoc") == 0)
                && !(ps->features & (1u << 19))) {
                /* 2026-11-13 裁决：未开 `type.weak` 时**响亮拒绝**（与 Rust 同码 001），
                   不做「默认忽略」—— 少一个静默路径，也少一处四端与 Rust 的对照缺口。
                   Loudly reject weak* when `type.weak` is off (same code as Rust). */
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_FEATURE_001 " `%s`：当前文档已禁用该特性"
                        "（请先写 `@feature enable type.weak`）", nxt->v);
                ps->failed = 1;
                break;
            }
            /* 开了 `type.weak`：weak* 三条指令**登记但不解释**，值树零影响。
               With `type.weak` on: acknowledge the directives, never touch the value. */
            if (nxt && nxt->t == T_WORD
                && (strcmp(nxt->v, "weakbind") == 0
                    || strcmp(nxt->v, "weakdoc") == 0
                    || strcmp(nxt->v, "weakdata") == 0)) {
                int is_bind = (strcmp(nxt->v, "weakbind") == 0);
                int is_doc = (strcmp(nxt->v, "weakdoc") == 0);
                token *nm;
                next(ps);   /* @ */
                next(ps);   /* weak* */
                nm = peek(ps);
                if (!nm || (nm->t != T_WORD && nm->t != T_STR)) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_019 " `@%s` 后须名字", nxt->v);
                    ps->failed = 1;
                    break;
                }
                next(ps);   /* 契约名 / profile 名 */
                if (is_bind) {
                    /* 挂起到「紧邻的下一个块」；行号与所在层供收尾时判悬空。 */
                    ps->pending_weak = nm->v;
                    ps->pending_weak_line = nm->line;
                    ps->pending_weak_depth = ps->depth;
                }
                if (is_bind && ps->builtin) {
                    /* 声明侧：契约名在内置表里吗？ */
                    ccontract *bc;
                    for (bc = ps->builtin->contracts; bc; bc = bc->next) {
                        if (strcmp(bc->name, nm->v) == 0) break;
                    }
                    if (!bc) {
                        char dmsg[256];
                        snprintf(dmsg, sizeof(dmsg), "内置契约表里没有 `%s`", nm->v);
                        diag_push(ps, "W-CONTRACT-001", dmsg);
                    }
                }
                /* 可选体 `{ 字段: 类型 ... }`：判据必须看**同一行** ——
                   `@weakbind tls` 换行后的 `server { ... }` 是下一个块，不是体。 */
                if (is_bind && peek(ps)->t == T_LBRACE && peek(ps)->line == nm->line) {
                    token *fld;
                    next(ps);   /* { */
                    while ((fld = peek(ps)) && fld->t != T_RBRACE) {
                        token *ty;
                        if (fld->t != T_WORD && fld->t != T_STR) break;
                        next(ps);
                        if (peek(ps)->t == T_COLON) next(ps);
                        ty = peek(ps);
                        if (ty && (ty->t == T_WORD || ty->t == T_STR)) next(ps);
                        weak_check_decl(ps, nm->v, fld->v, ty ? ty->v : NULL);
                    }
                    if (peek(ps)->t == T_RBRACE) next(ps);
                }
                if (is_doc && peek(ps)->t == T_LBRACE) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_019
                            " `@weakdoc` 只写 profile 名，不接受载荷块"
                            "（要内联数据请用 `@weakdata`）");
                    ps->failed = 1;
                    break;
                }
                if (!is_bind && !is_doc && peek(ps)->t == T_LBRACE) {
                    /* `@weakdata` 的载荷块：跳过配对花括号，不解释。 */
                    int d = 1;
                    next(ps);
                    while (d > 0 && peek(ps)->t != T_EOF) {
                        if (peek(ps)->t == T_LBRACE) d++;
                        else if (peek(ps)->t == T_RBRACE) d--;
                        next(ps);
                    }
                    if (d != 0) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_PARSE_001 " 未闭合的块（遇到文件结尾，缺少结束符号 }）");
                        ps->failed = 1;
                        break;
                    }
                }
                continue;
            }
            /* `@version vN` 是版本声明指令，不是片段定义（与 Rust/Lua/JS 对齐）。 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "version") == 0) {
                next(ps); next(ps); /* @ version */
                token *lit = peek(ps);
                if (lit->t == T_WORD || lit->t == T_STR) {
                    const int ver = version_lit_major(lit->v);
                    if (ver < 0) {
                        /* 5.0 族：写法合法，但该代尚未定稿（Dev）⇒ 与 Rust 同码。 */
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_004
                                " 版本 `%s` 尚未启用（5.0 仍为 Dev；本实现支持 v1..v4）",
                                lit->v);
                        ps->failed = 1;
                    } else if (ver == 0) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_004
                                " 未知版本 `%s`；仅支持 v1..v4（v1–v4 / 1.0–4.0 / 10–40）",
                                lit->v);
                        ps->failed = 1;
                    } else {
                        ps->version = ver;
                    }
                    next(ps);
                }
                continue;
            }
            /* `@feature enable|disable <名>[, <名>]` —— 记录开关位。
               名字表与 Rust 的 FEATURES 同序同名；本端**只识别、不门控**（见
               SML_FEATURE_NAMES 的注释）。`mode` / `base` 子命令本端未实现，
               明确拒绝而不是静默忽略（登记见 crossend-sync.md 的 P3）。 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "feature") == 0) {
                token *sub;
                next(ps); /* @ */
                next(ps); /* feature */
                sub = peek(ps);
                if (!sub || sub->t != T_WORD || sub->line != t->line) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_007 " `@feature` 缺少参数（enable / disable）");
                    ps->failed = 1;
                    break;
                }
                if (strcmp(sub->v, "enable") == 0 || strcmp(sub->v, "disable") == 0) {
                    int on = (sub->v[0] == 'e');
                    int any = 0;
                    int sub_line = sub->line;
                    next(ps);
                    for (;;) {
                        token *nm = peek(ps);
                        int bit;
                        if (!nm || nm->t != T_WORD || nm->line != sub_line) break;
                        bit = feature_index(nm->v);
                        if (bit < 0) {
                            set_err(ps->lx->errbuf, ps->lx->errsz,
                                    SML_E_FEATURE_002 " 未知特性名 `%s`", nm->v);
                            ps->failed = 1;
                            break;
                        }
                        next(ps);
                        if (on) ps->features |= (1u << bit);
                        else    ps->features &= ~(1u << bit);
                        any = 1;
                        if (peek(ps)->t == T_COMMA) { next(ps); continue; }
                        break;
                    }
                    if (ps->failed) break;
                    if (!any) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_007 " `@feature %s` 缺少特性名", sub->v);
                        ps->failed = 1;
                        break;
                    }
                    continue;
                }
                /* mode / base：本端暂不支持（Rust 端已实现，登记为跨端差异） */
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_FEATURE_006 " `@feature %s`：本端暂不支持该子命令", sub->v);
                ps->failed = 1;
                break;
            }
            /* `@when <条件>`：条件裁剪，只作用于紧邻的下一个字段/块。
               `@when <cond>` — trims the immediately following field/block. */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "when") == 0) {
                if (!(ps->features & (1u << 12))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `when`：当前文档已禁用该特性");
                    ps->failed = 1;
                    break;
                }
                if (ps->pending_when != 0) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_016 " `@when` 连续出现："
                            "它只作用于紧邻的下一个字段或块");
                    ps->failed = 1;
                    break;
                }
                next(ps); /* @ */
                next(ps); /* when */
                ps->pending_when = eval_when(ps) ? 1 : 2;
                if (ps->failed) break;
                continue;
            }
            /* `@contract Name [loose] { ... }` 定义契约 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "contract") == 0) {
                /* 特性门控：`@feature disable contract` 后必须拒绝（与 Rust 的
                   `!features.has(Feature::Contract)` ⇒ E-FEATURE-001 同判据）。 */
                if (!(ps->features & (1u << 3))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `contract`：当前文档已禁用该特性");
                    ps->failed = 1;
                    break;
                }
                next(ps); next(ps); /* @ contract */
                char *cname = next(ps)->v;
                int loose = 0;
                if (peek(ps)->t == T_WORD && strcmp(peek(ps)->v, "loose") == 0) {
                    loose = 1; next(ps);
                } else if (peek(ps)->t == T_WORD && strcmp(peek(ps)->v, "strict") == 0) {
                    /* 显式严格（与默认等价，写出来只为可读性/团队规范）—— Rust 与 JS
                       都接受它（rust/sml-parse/src/parser.rs、js/sml.mjs）。
                       此前 C **只认 loose**：`@contract X strict { ... }` 会让
                       parse_contract_body 见不到 `{` 而直接返回 —— 契约字段为空、
                       紧跟的 `{ ... }` 变成名为 `strict` 的数据块（静默给错树）。
                       W16 全仓扫描抓到（`_gov_demo.sml`）。这里只做**对齐**，不改语义：
                       strict ≡ 默认严格。 */
                    next(ps);
                }
                ccontract *c = (ccontract *)calloc(1, sizeof(ccontract));
                c->name = strdup(cname);
                c->allow_extra = loose;
                parse_contract_body(ps, c);
                c->next = ps->contracts;
                ps->contracts = c;
                continue;
            }
            /* `@is Name` 块级契约应用 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "is") == 0) {
                /* 特性门控：与 `@contract` 同一判据（Rust 的 `@is` 也查 Contract 位）。 */
                if (!(ps->features & (1u << 3))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `contract`：当前文档已禁用该特性");
                    ps->failed = 1;
                    break;
                }
                next(ps); next(ps); /* @ is */
                token *cn = next(ps);
                if (cn && (cn->t == T_WORD || cn->t == T_STR)) {
                    char *cname_is = cn->v;
                    /* 任务 D：多契约「响亮拒绝」—— `@is` 后出现第 2 个裸词（同一行、
                       且该词不是「后跟 : 的键」）即报 E-CONTRACT-017，绝不静默给树。
                       与 Rust D1 同口径：遇 : / { / ] / 换行 / 逗号（可省）即停。 */
                    {
                        token *nx = peek(ps);
                        if (nx && nx->t == T_COMMA) nx = peek_at(ps, 1);  /* 跳过可省逗号 */
                        if (nx && (nx->t == T_WORD || nx->t == T_STR)) {
                            if (nx->line != cn->line) {
                                /* 换行即停：下一行的键/值不是契约名 */
                            } else if (peek_at(ps, (nx == peek(ps)) ? 1 : 2)
                                       && peek_at(ps, (nx == peek(ps)) ? 1 : 2)->t == T_COLON) {
                                /* 后跟 : 是键，非契约名 */
                            } else {
                                set_err(ps->lx->errbuf, ps->lx->errsz,
                                        SML_E_CONTRACT_017 " 该实现暂不支持一个块应用多份契约（@is 多契约写法）");
                                ps->failed = 1;
                            }
                        }
                    }
                    if (peek(ps)->t == T_LBRACE) {
                        /* `@is Name { ... }` 匿名块/当前块应用契约:
                        ** 解析块体、应用契约、合并字段回当前对象 */
                        next(ps);
                        ps->enter_key = NULL;   /* 匿名块：路径与所在块相同 */
                        sml_value *sub = parse_block(ps, T_RBRACE);
                        apply_or_fail(ps, sub, cname_is);
                        sml_field *f;
                        for (f = sub->u.obj.head; f; f = f->next)
                            sml_obj_set(obj, f->key, sml_clone(f->value));
                        sml_free(sub);
                    } else {
                        block_is = cname_is;
                    }
                }
                continue;
            }
            /* 片段定义: `@name { ... }`；参数只认**显式**写法 `type: X` / `name: Y`。
               ⚠️ **位置参数形式**（`@name X [Y] { ... }`）自 v4 起已废弃 —— 它与
               「拼错的指令」在 token 流上完全同形、无法判别，故一律报 E-PARSE-005
               （与 Rust 同判据），而不是猜。
               改前 C 的行为（两端都不报错）：`@foo bar { x: 1 }` 被**静默**当片段收下
               （拼错的指令名于是变成数据）；`@foo bar`（无体）**静默丢掉整行**。
               判据逐字对齐 Rust（`parser.rs` 的 is_param）：
                 · 只有 `type`/`name` **紧跟冒号**才算参数（`@type { .. }` 仍可定义）；
                 · 参数读完后既不是 `{` 也不是文件末尾 ⇒ 位置参数形式 ⇒ 005；
                 · 没有片段体（后面不是 `{`）⇒ 005。 */
            next(ps);                 /* @ */
            token *ft = next(ps);
            if (ft->t != T_WORD && ft->t != T_STR) {
                /* `@` 后不是名字（如孤立 `@`、`@ {`）：Rust 报 E-PARSE-011，
                   不属本批（W16 判定表）范围，保持既有的「停止解析本层」行为。 */
                break;
            }
            char *fname = ft->v;
            char *ftype = NULL, *farg = NULL;
            int dir_bad = 0;
            while (peek(ps)->t == T_WORD &&
                   (strcmp(peek(ps)->v, "type") == 0 || strcmp(peek(ps)->v, "name") == 0) &&
                   peek_at(ps, 1)->t == T_COLON) {
                int is_type = (strcmp(peek(ps)->v, "type") == 0);
                const char *kw = is_type ? "type" : "name";
                next(ps);   /* type / name */
                next(ps);   /* : */
                token *vt = peek(ps);
                if (vt->t != T_WORD && vt->t != T_STR) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_020 " 片段 `@%s` 的参数 `%s:` 后须值", fname, kw);
                    ps->failed = 1;
                    dir_bad = 1;
                    break;
                }
                if ((is_type ? ftype : farg) != NULL) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_020 " 片段 `@%s` 的 `%s:` 参数重复", fname, kw);
                    ps->failed = 1;
                    dir_bad = 1;
                    break;
                }
                if (is_type) ftype = next(ps)->v;
                else         farg  = next(ps)->v;
            }
            if (dir_bad) break;
            if (peek(ps)->t != T_LBRACE) {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_005 " `@%s` 不是合法指令且缺少片段体 { ... }；"
                        "若本意是「片段定义」，参数须显式写作 `type: X` 与 `name: Y`"
                        "（位置参数形式自 v4 起已废弃；不带参数时写作 `@%s { ... }`）；"
                        "若本意是「指令」，请检查拼写（合法指令：contract / is / version）",
                        fname, fname);
                ps->failed = 1;
                break;
            }
            next(ps);                 /* { */
            {
                ps->enter_key = NULL;   /* 片段体：不进主树，路径无意义 */
                sml_value *sub = parse_block(ps, T_RBRACE);
                if (ftype) {
                    sml_obj_set(sub, "__type", sml_new_str(ftype));
                    if (farg) sml_obj_set(sub, "__name", sml_new_str(farg));
                }
                if (!(ps->features & (1u << 4))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `fragment`：当前文档已禁用该特性");
                    ps->failed = 1;
                    sml_free(sub);
                    break;
                }
                frag_put(ps, fname, sub);
            }
            continue;
        }
        /* key */
        token *kt = next(ps);
        /* 键位置上的裸 `:`（P1 欠账，与 Rust / JS 对齐）：`::` 被词法切成两个 `:`，
           第二个 `:` 落到键位置。改前这里对「非 WORD/STR 的键」直接 break ⇒
           `X { a: B::C }` 静默得 {"X":{"a":"B"}}（第二个字段被丢、**还不报错**）。
           Rust / JS 均对同一输入报 E-PARSE-006，此处补齐同码；文案点明 `::` 与「加引号」。 */
        if (kt->t == T_COLON) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_006 " 键位置出现裸冒号 `:`（`::` 会被切成两个 `:`）；"
                    "含 `:` 的标量请加引号，如 a: \"B::C\"");
            ps->failed = 1;
            break;
        }
        if (kt->t != T_WORD && kt->t != T_STR) break;
        char *key = kt->v;
        ps->key_line = kt->line;   /* 重复键诊断要用**键所在行**，不是下一个 token */
        /* 键名位置不支持 `${...}` 插值（循环变量只可用于值位置）。
           No `$` interpolation in key position (E-PARSE-013). */
        if (strstr(key, "${")) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_013 " 键名位置不支持 `$` 插值：循环变量只可用于值位置");
            ps->failed = 1;
            break;
        }
        /* 行级 &frag 展开（splice），与 Rust / JS 对齐：块内独立一行、其后不是 `:` 的
           `&name` 若命中已定义片段，则把该片段的字段**合并进当前块**，而不是造一个
           `"&name"` 键。改前 C 让 `&name` 落到下面的 else 分支走 coerce_word，展开成
           片段值后存到字面键 `"&name"` 下 ⇒ `w { c: 3\n&base }` 得
           {"w":{"c":3,"&base":{...}}}，严格契约据此**误报** E-CONTRACT-004。
           语义与 JS 片段分支一致：按出现顺序合并、同名后者覆盖（sml_obj_set 覆盖）、
           **深拷贝**（片段在解析末尾单独释放，不能借指针）。C 无原型链，故无危险键概念；
           未命中的 `&name` 继续下落到 coerce_word，按 E-INCLUDE-006 报未定义片段引用。 */
        if (key[0] == '&' && !(peek(ps) && peek(ps)->t == T_COLON)) {
            struct frag *fr = NULL;
            for (struct frag *g = ps->frags; g; g = g->next)
                if (strcmp(g->name, key + 1) == 0) { fr = g; break; }
            if (fr && fr->val && fr->val->type == SML_OBJECT) {
                sml_field *ff;
                for (ff = fr->val->u.obj.head; ff; ff = ff->next)
                    sml_obj_set(obj, ff->key, sml_clone(ff->value));
                continue;   /* &name 已作为展开消费，不落入下面的键值处理 */
            }
        }
        /* 字段级 @is: key 后紧跟 @is Name */
        char *field_is = NULL;
        if (peek(ps)->t == T_AT) {
            token *nn = peek_at(ps, 1);
            if (nn && nn->t == T_WORD && strcmp(nn->v, "is") == 0) {
                next(ps); next(ps); /* @ is */
                token *cn = next(ps);
                if (cn && (cn->t == T_WORD || cn->t == T_STR)) {
                    field_is = cn->v;
                    /* 任务 D：字段级 @is 同样「响亮拒绝」第 2 个裸词（同一行、非键）。 */
                    {
                        token *nx = peek(ps);
                        if (nx && nx->t == T_COMMA) nx = peek_at(ps, 1);
                        if (nx && (nx->t == T_WORD || nx->t == T_STR)) {
                            if (nx->line != cn->line) {
                                /* 换行即停 */
                            } else if (peek_at(ps, (nx == peek(ps)) ? 1 : 2)
                                       && peek_at(ps, (nx == peek(ps)) ? 1 : 2)->t == T_COLON) {
                                /* 后跟 : 是键，非契约名 */
                            } else {
                                set_err(ps->lx->errbuf, ps->lx->errsz,
                                        SML_E_CONTRACT_017 " 该实现暂不支持一个块应用多份契约（@is 多契约写法）");
                                ps->failed = 1;
                            }
                        }
                    }
                }
            }
        }
        int colon = 0;
        if (peek(ps)->t == T_COLON) { colon = 1; next(ps); }
        token *nt = peek(ps);
        /* 裸块预扫描: 无冒号且后继是词, 可能 `type name { }` */
        if (!colon && nt->t == T_WORD) {
            size_t probe = ps->lx->pos;
            int found = 0;
            while (probe < ps->lx->n) {
                tok_type pt = ps->lx->toks[probe].t;
                if (pt == T_WORD || pt == T_STR) probe++;
                else if (pt == T_LBRACE) { found = 1; break; }
                else break;
            }
            if (found) {
                sml_value *args = sml_new_array();
                /* 任务 D：块级多契约「响亮拒绝」—— head 词命中契约表且 args 里出现第 2 个
                   已注册契约词 ⇒ E-CONTRACT-017（单契约 `server web prod` 的 web/prod 不是
                   契约，不受影响）。与 Rust D2 同口径：从首词起连续命中契约表的词才计数。 */
                int key_is_contract = (contract_find(ps, key) != NULL);
                int arg_has_contract = 0;
                while (peek(ps)->t == T_WORD || peek(ps)->t == T_STR) {
                    token *at = next(ps);
                    if (key_is_contract && contract_find(ps, at->v)) arg_has_contract = 1;
                    sml_arr_push(args, at->t == T_STR ? sml_new_str(at->v)
                                                      : coerce_word(at->v, ps));
                }
                if (key_is_contract && arg_has_contract) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_CONTRACT_017 " 该实现暂不支持一个块应用多份契约（块级多契约写法）");
                    ps->failed = 1;
                    sml_free(args);
                    continue;
                }
                if (peek(ps)->t == T_LBRACE) {
                    next(ps);
                    ps->enter_key = key;   /* 裸块：路径段用块类型名 */
                    sml_value *sub = parse_block(ps, T_RBRACE);
                    sml_obj_set(sub, "__type", sml_new_str(key));
                    /* 裸块参数**不再丢**：首个 ⇒ `__name`、其余 ⇒ `__args`，与 Rust
                       `parse_bare_block` 一致（数组位置是同一个写法，见
                       try_parse_array_bare_block；两处必须同步改，否则同一份输入在
                       键位置与数组位置会得到不同的树）。
                       改前只在「恰好一个参数」时写 `__name`，两个以上参数**直接丢弃**
                       —— 与 Rust 的 `server web prod {}`（⇒ `__name: web` +
                       `__args: [prod]`）不符，属静默改数据。
                       必须克隆再存：args 下一行就被 sml_free，直接借用 args[0] 会让
                       `__name` / `__args` 变成悬垂指针，根节点释放时二次释放 ——
                       即已知的「带名块堆损坏」。 */
                    if (sml_arr_len(args) > 0) {
                        size_t i;
                        sml_obj_set(sub, "__name", sml_clone(sml_arr_get(args, 0)));
                        if (sml_arr_len(args) > 1) {
                            sml_value *extra = sml_new_array();
                            for (i = 1; i < sml_arr_len(args); i++)
                                sml_arr_push(extra, sml_clone(sml_arr_get(args, i)));
                            sml_obj_set(sub, "__args", extra);
                        }
                    }
                    sml_free(args);
                    if (field_is)
                        apply_or_fail(ps, sub, field_is);
                    set_field_maybe_skip(ps, obj, key, sub);
                    continue;
                }
                sml_free(args);
            }
        }
        nt = peek(ps);
        if (nt->t == T_AT) {
            token *nx = peek_at(ps, 1);
            if (nx && nx->t == T_WORD && strcmp(nx->v, "for") == 0) {
                sml_value *arr;
                if (!(ps->features & (1u << 13))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `for`：当前文档已禁用该特性");
                    ps->failed = 1;
                    break;
                }
                arr = parse_for(ps);
                if (ps->failed) { if (arr) sml_free(arr); break; }
                if (field_is) apply_or_fail(ps, arr, field_is);
                set_field_maybe_skip(ps, obj, key, arr);
                continue;
            }
        }
        if (nt->t == T_LBRACE) {
            next(ps);
            ps->enter_key = key;   /* 旁路表路径：本块的键名 */
            sml_value *sub = parse_block(ps, T_RBRACE);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (nt->t == T_LBRACK) {
            next(ps);
            sml_value *sub = parse_array(ps);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (nt->t == T_STR) {
            char *iv = interp_loop(ps, next(ps)->v);
            sml_value *sub = sml_new_str(iv ? iv : "");
            free(iv);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (nt->t == T_WORD) {
            sml_value *sub = coerce_word(next(ps)->v, ps);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (colon) {
            sml_value *sub = sml_new_null();
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else {
            /* key 本身即值 (片段引用/裸词) */
            sml_value *sub = coerce_word(key, ps);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        }
    }
    /* `@when` 悬空：块结束时条件还没被任何字段/块消费。
       Dangling `@when`: the block ended before the condition consumed an item. */
    /* 只在没有更早的错误时补 017，避免把 016（连续出现）覆盖掉。 */
    if (!ps->failed && ps->pending_when != 0) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_017 " `@when` 后未跟随任何字段或块（悬空的条件）");
        ps->failed = 1;
    }
    /* 悬空的 `@weakbind`（本块内没等到块）：丢弃，不让它落到外层的下一个块上。
       软校验发生在 `set_field_maybe_skip` 消费挂起项的那一刻。
       A dangling `@weakbind` dies with its own block; the soft check happens where
       the pending bind is consumed (see set_field_maybe_skip). */
    if (ps->pending_weak && ps->pending_weak_depth >= ps->depth) {
        ps->pending_weak = NULL;
    }
    /* 块级 @is 应用 */
    if (block_is)
        apply_or_fail(ps, obj, block_is);
    return obj;
}

static sml_value *parse_core(const char *text, char *err, size_t errsz,
                             sml_diag **diags_out, size_t *n_diags_out,
                             const sml_builtin *builtin,
                             sml_weakbind **binds_out, size_t *n_binds_out) {
    if (diags_out) *diags_out = NULL;
    if (n_diags_out) *n_diags_out = 0;
    if (binds_out) *binds_out = NULL;
    if (n_binds_out) *n_binds_out = 0;
    if (!text) {
        if (err && errsz) set_err(err, errsz, SML_E_INTERNAL_001 " 空指针入参（text 为 NULL）");
        return NULL;
    }
    if (err && errsz) err[0] = '\0';   /* 成功时保持为空，避免误判 */
    /* 文件级规范化：跳过开头的 UTF-8 BOM（EF BB BF）。
       BOM **不是空白** ⇒ 会被词法器吞进第一个单词 ⇒ 第一个键名变成 "\xEF\xBB\xBFx"
       （最坏是**静默**改掉键名）。来源很常见：Windows 记事本「另存为 UTF-8」。
       与 Rust（sml-parse::scan::strip_version）、JS（parse）、C++/Lua 对齐 ——
       仓库里 smltools 的 YAML/XML 导入器早就各自去过 BOM，只有 native SML 漏了。 */
    if (text && strlen(text) >= 3 && (unsigned char)text[0] == 0xEF
        && (unsigned char)text[1] == 0xBB && (unsigned char)text[2] == 0xBF) {
        text += 3;
    }
    lexer lx;
    memset(&lx, 0, sizeof(lx));
    lx.errbuf = err;
    lx.errsz = errsz;
    /* 预处理：先扫 `@feature` 行定下特性集，词法与顶层形态判定才能看到它
       （与 Rust 的 strip_features 同一定位）。 */
    lx.features = scan_features(text);
    lex_run(&lx, text);
    /* 词法错误**先收手**：词法与语法共用一个 err 缓冲，若带着词法错误继续解析，
       解析器写的后续错误会覆盖它，用户拿到的码就不是**第一个**错的码。
       与 Rust 的 `tokenize()` 返回 Err 即短路同义。 */
    if (lx.failed) {
        lex_free(&lx);
        return NULL;
    }
    /* 顶层标量不可往返 ⇒ E-PARSE-008（与 Rust 同判据：**顶层第一个结构是值型标量、且其后
       紧跟块/数组/键值**，T9 收紧）。
       改前这里是**静默造键**：`42` 走 parse_block(T_EOF) 的「键即值」分支，被解析成
       `{"42": 42}`，重新序列化得到 `"42": 42` ≠ `42` —— 数据形状被悄悄改掉
       （与 W17 的 C 嵌套数组同族：不报错，但数据错）。
       判据与 Rust 逐字一致：
         - `42`                → 仅一个顶层结构（n == 2，含末尾 T_EOF）⇒ 报；
         - `42\na { b: 1 }`    → 值型标量 `42` 后跟块 `a` ⇒ 报（T9：42 被重解释成块类型名）；
         - `42\nb: 1` / `42\n[1,2]` → 后跟键值/数组 ⇒ 报；
         - `server web { }`    → `server` 是裸词标识符（块类型名）**非值型** ⇒ 不报（合法块）；
         - `hello world`       → 两 token 裸键对，值可往返 ⇒ 不报；
         - `@version v1` + `42` → 首结构是 @ 指令（非裸标量）⇒ 不报（保留正对照）；
         - `42 { a: 1 }`       → 名为 42 的块（紧跟 `{`）⇒ 不报。
       注：C 的 token 流末尾固定有一个 T_EOF，故「仅一个标量」= n == 2。 */
    {
        int scalar0 = (lx.toks[0].t == T_WORD || lx.toks[0].t == T_STR);
        int bare0 = scalar0 && lx.n >= 2
                    && lx.toks[1].t != T_LBRACE && lx.toks[1].t != T_COLON;
        if (bare0) {
            int only = (lx.n == 2);
            int followed = (lx.n >= 3 && lx.toks[1].t == T_LBRACK)
                || (lx.n >= 3
                    && (lx.toks[1].t == T_WORD || lx.toks[1].t == T_STR)
                    && (lx.toks[2].t == T_LBRACE || lx.toks[2].t == T_COLON));
            /* T9：仅当首个 token 是「值型标量」时，其后跟块/数组/键值才报；
               裸词标识符（块类型名/块名）不算值型，`server web { }` 合法。 */
            int first_is_val = tok_is_scalar_value(&lx.toks[0]);
            if (only || (first_is_val && followed)) {
                if (err && errsz)
                    set_err(err, errsz,
                            SML_E_PARSE_008 " 顶层须为容器（键值块、对象块或数组），单独的标量无法往返");
                lex_free(&lx);
                return NULL;
            }
        }
    }
    parser ps;
    memset(&ps, 0, sizeof(ps));
    ps.lx = &lx;
    ps.frags = NULL;
    ps.contracts = NULL;
    ps.features = lx.features;   /* 预处理算出的初始特性集（解析期 @feature 再增减，幂等） */
    ps.want_diags = (diags_out != NULL);
    ps.builtin = builtin;
    /* 旁路表恒收集（与 Rust 的 `weak_binds` 同为恒收集的 Vec），没要就最后释放。 */
    ps.enter_key = NULL;
    /* 顶层支持三种形态，与 sml_dump 的输出对称：
    **   - `[ ... ]` 数组
    **   - `{ ... }` 顶层对象块
    **   - 键值块（传统形态） */
    sml_value *v;
    /* 顶层数组门控：跳过开头的 `@version` / `@feature` 行，看**实质**第一个 token。
       `@feature` 行在前时 `peek()` 是 `@`，直接判 `first == T_LBRACK` 永远不成立，
       故必须先跳行（Rust 靠 strip_features 剥行，本端用只读跳过达到同一判定）。 */
    {
        size_t k = 0;
        while (k < lx.n && lx.toks[k].t == T_AT) {
            int li = lx.toks[k].line;
            k++;
            while (k < lx.n && lx.toks[k].line == li) k++;
        }
        if (k < lx.n && lx.toks[k].t == T_LBRACK && !(ps.features & (1u << 5))) {
            if (err && errsz)
                set_err(err, errsz,
                        SML_E_FEATURE_001 " `top-level-array`：当前文档已禁用该特性");
            diag_free_all(&ps);
            lex_free(&lx);
            return NULL;
        }
    }
    tok_type first = peek(&ps)->t;
    if (first == T_LBRACK) { next(&ps); v = parse_array(&ps); }
    else if (first == T_LBRACE) { next(&ps); v = parse_block(&ps, T_RBRACE); }
    else v = parse_block(&ps, T_EOF);
    /* 契约校验错误 -> 整体解析失败 (与 Rust 一致) */
    if (ps.failed) {
        sml_free(v);
        v = NULL;
    }
    /* 清理片段 (共享引用, 不 double free: 只释放链本身) */
    struct frag *f = ps.frags;
    while (f) { struct frag *nx = f->next; free(f->name); sml_free(f->val); f = nx; }
    lex_free(&lx);
    path_free_all(&ps);
    if (!v) {
        diag_free_all(&ps);
        sml_weakbind_free(ps.binds, ps.n_binds);
        if (err && errsz && err[0] == '\0')
            set_err(err, errsz, SML_E_PARSE_012 " 解析失败：未能给出更具体的原因");
        return NULL;
    }
    /* 成功：把诊断交给调用方（没开收集就 free 掉，避免泄漏）。 */
    if (diags_out) {
        *diags_out = ps.diags;
        if (n_diags_out) *n_diags_out = ps.n_diags;
    } else {
        diag_free_all(&ps);
    }
    /* 旁路表：只有调用方要才交出，否则释放（空表时两个出参为 NULL / 0）。 */
    if (binds_out) {
        *binds_out = ps.n_binds ? ps.binds : NULL;
        if (n_binds_out) *n_binds_out = ps.n_binds;
    } else {
        sml_weakbind_free(ps.binds, ps.n_binds);
    }
    return v;
}

/* 兼容入口：不收集诊断（与改动前逐字节一致）。 */
sml_value *sml_parse(const char *text, char *err, size_t errsz) {
    return parse_core(text, err, errsz, NULL, NULL, NULL, NULL, NULL);
}

sml_value *sml_parse_ex(const char *text, char *err, size_t errsz,
                        sml_diag **diags_out, size_t *n_diags_out) {
    return parse_core(text, err, errsz, diags_out, n_diags_out, NULL, NULL, NULL);
}

sml_value *sml_parse_with_builtins(const char *text, char *err, size_t errsz,
                                   const sml_builtin *b,
                                   sml_diag **diags, size_t *n_diags,
                                   sml_weakbind **binds, size_t *n_binds) {
    return parse_core(text, err, errsz, diags, n_diags, b, binds, n_binds);
}

/* =====================================================================
** 4. 序列化
** ===================================================================== */

typedef struct { char *buf; size_t len, cap; } sbuf;

static void sb_ensure(sbuf *b, size_t extra) {
    if (b->len + extra + 1 > b->cap) {
        size_t ncap = b->cap ? b->cap * 2 : 256;
        while (b->len + extra + 1 > ncap) ncap *= 2;
        char *nb = (char *)realloc(b->buf, ncap);
        if (nb) { b->buf = nb; b->cap = ncap; }
    }
}

static void sb_add(sbuf *b, const char *s) {
    size_t n = strlen(s);
    sb_ensure(b, n);
    memcpy(b->buf + b->len, s, n);
    b->len += n;
    b->buf[b->len] = '\0';
}

static void sb_addc(sbuf *b, char c) {
    sb_ensure(b, 1);
    b->buf[b->len++] = c;
    b->buf[b->len] = '\0';
}

static int needs_quote(const char *s) {
    if (!*s) return 1;
    for (; *s; s++) {
        if (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r' ||
            *s == ':' || *s == '#' || *s == '{' || *s == '}' ||
            *s == '"' || *s == '\\')
            return 1;
    }
    return 0;
}

static void dump_value(sbuf *b, const sml_value *v, int indent);
static void dump_inline(sbuf *b, const sml_value *v);
static void dump_element(sbuf *b, const sml_value *v, int indent);
static void dump_object_body(sbuf *b, const sml_value *v, int indent);
static void dump_array_body(sbuf *b, const sml_value *v, int indent);

/* 该对象是否为**非空**对象 —— 它决定两处「形态选择」，两处判据都必须与 Rust 一致：
   ① `key:` 之后**要不要**补那个空格（W4 ①）：Rust `starts_inline(v)` 的定义是
      「**空**对象才同行渲染」，非空对象会另起一行写 `{ … }` ⇒ 键后**不留**行尾空格。
   ② `dump_value` 里写 `{}` 还是 `\n{ … }`：Rust `dump_block` 同样是「空 ⇒ `{}`」。
   ⚠️ 原先这里**排除** `__type` / `__name`（把它们当内部标记键），那正是 A 类差异的根源：
   这两个键在 Rust 侧是**要往返的数据键**（原样序列化），于是「只有元数据的块」在 C 里被
   判成「空体」⇒ 输成 `{}`，**元数据被静默丢掉**。现在按「有没有键」判。 */
static int obj_has_body(const sml_value *v) {
    if (!v || v->type != SML_OBJECT) return 0;
    return v->u.obj.head != NULL;
}

/* 容器是否「扁平」：**直接子项全是标量**（不再嵌对象 / 数组）。
   与 Rust `dump.rs::is_flat` 同一判据，是「一行写完」还是「展开多行」的唯一开关：
     `{ type: home }`                      扁平 → `[ { type: home } ]` 仍是一行
     `[ { type: home } { type: office } ]` 扁平 → 一行（子项全是标量）
     `{ children: [ … ] }`                 非扁平 → `\n{ … }` 展开

   ⚠️ **只看一层，不递归**。递归版（「子孙全是标量」）是恒真判据 —— 任何对象的
   子孙最终都会落到标量，于是所有东西都被判成扁平、排版分毫不改。Rust 侧写这段时
   真踩过这个坑（编译与测试全绿，只是完全没生效），这里按同一规则实现，别改回递归。
   `__type` / `__name` 在 C 侧存为字符串，天然算标量，不影响判定（与 Rust 一致）。 */
static int is_flat(const sml_value *v) {
    if (!v) return 1;
    if (v->type == SML_OBJECT) {
        sml_field *f;
        for (f = v->u.obj.head; f; f = f->next) {
            const sml_value *fv = f->value;
            if (fv && (fv->type == SML_OBJECT || fv->type == SML_ARRAY)) return 0;
        }
        return 1;
    }
    if (v->type == SML_ARRAY) {
        size_t i;
        for (i = 0; i < v->u.arr.len; i++) {
            const sml_value *iv = v->u.arr.items[i];
            if (iv && (iv->type == SML_OBJECT || iv->type == SML_ARRAY)) return 0;
        }
        return 1;
    }
    return 1; /* 标量一律扁平 */
}

/* 写对象体：**不含**开头的 `{`（由调用方写），负责逐键换行与收尾 `}`。
   抽出来是为了让「键后面接的块」（先写 `\n{`，见 dump_value）与
   「数组元素里的对象」（`{` 跟在同一行，见 dump_element）共用同一套键渲染 ——
   与 Rust `dump.rs` 里 `dump_object_body` / `dump_block` / `dump_element` 的关系同构。
   ⚠️ **所有键都写**，包括 `__type` / `__name`：Rust `dump_object_body` 是
   `for (k, val) in m`（不筛键），因为裸块 `type [name] { … }` 解析后就长成这两个键，
   序列化时必须能写回去（否则元数据不往返）。这是 A 类差异的修复点。 */
static void dump_object_body(sbuf *b, const sml_value *v, int indent) {
    sml_field *f;
    int j;
    for (f = v->u.obj.head; f; f = f->next) {
        sb_add(b, "\n");
        for (j = 0; j < indent + 1; j++) sb_add(b, "  ");
        sb_add(b, f->key);
        /* W4 ①：「键: 后接块」**不留行尾空格**。
           对象体的 dump 是从 `\n{ … }` 起的一整块，这里若写 ": "，
           那个空格就落在行尾（Rust `to_sml` 早就不留了，见 CHANGELOG
           「to_sml 不再在『键: 后接块』时于行尾留空格」）。
           标量 / 空容器（同行渲染）照旧用 ": "。 */
        if (obj_has_body(f->value)) sb_add(b, ":");
        else                        sb_add(b, ": ");
        dump_value(b, f->value, indent + 1);
    }
    sb_add(b, "\n");
    for (j = 0; j < indent; j++) sb_add(b, "  ");
    sb_addc(b, '}');
}

/* 写数组体：**不含**开头的 `[`（由调用方写）。每个元素先换行 + 缩进，
   再由 dump_element 决定「一行写完」还是「就地展开」。 */
static void dump_array_body(sbuf *b, const sml_value *v, int indent) {
    size_t i;
    int j;
    sb_addc(b, '[');
    for (i = 0; i < v->u.arr.len; i++) {
        sb_add(b, "\n");
        for (j = 0; j < indent + 1; j++) sb_add(b, "  ");
        dump_element(b, v->u.arr.items[i], indent + 1);
    }
    sb_add(b, "\n");
    for (j = 0; j < indent; j++) sb_add(b, "  ");
    sb_addc(b, ']');
}

/* 浮点输出：整数值**必须保留小数点** —— `1.0` 若写成 `1`，回读就变成 Int，
   类型被静默改掉（Rust 侧 `format_float` 自 2026-10-28 起同一口径，见 TODO 雷 5 B）。
   NaN / inf 没有合法的 SML 字面量，写成**引号字符串**（与 Rust 一致），
   否则裸 `nan` 回读会变成 Str `nan`（看起来成功、其实类型变了）。 */
static void add_float(sbuf *b, double f) {
    char num[64];
    if (isnan(f) || isinf(f)) {
        snprintf(num, sizeof(num), "\"%g\"", f);
        sb_add(b, num);
        return;
    }
    snprintf(num, sizeof(num), "%g", f);
    sb_add(b, num);
    /* `%g` 对整数值吞掉小数点 ⇒ 补 `.0`；科学计数形式（含 e/E）本来就是浮点形态，不动。 */
    if (floor(f) == f && !strpbrk(num, "eE")) sb_add(b, ".0");
}

static void dump_value(sbuf *b, const sml_value *v, int indent) {
    char num[64];
    if (!v) { sb_add(b, "null"); return; }
    switch (v->type) {
        case SML_NULL: sb_add(b, "null"); break;
        case SML_BOOL: sb_add(b, v->u.boolean ? "true" : "false"); break;
        case SML_INT: snprintf(num, sizeof(num), "%lld", v->u.i); sb_add(b, num); break;
        case SML_FLOAT: add_float(b, v->u.f); break;
        case SML_STR:
            if (needs_quote(v->u.s)) {
                sb_addc(b, '"');
                for (const char *p = v->u.s; *p; p++) {
                    if (*p == '"') sb_add(b, "\\\"");
                    else if (*p == '\\') sb_add(b, "\\\\");
                    else sb_addc(b, *p);
                }
                sb_addc(b, '"');
            } else {
                sb_add(b, v->u.s);
            }
            break;
        case SML_ARRAY: {
            if (v->u.arr.len == 0) { sb_add(b, "[]"); break; }
            dump_array_body(b, v, indent);
            break;
        }
        case SML_OBJECT: {
            /* 空对象写 `{}`、非空写 `\n{ … }` —— 与 Rust `dump_block` 同判据。
               （A 类：这里原先排除 __type/__name，于是「只有元数据的块」被误判成空对象、
               元数据被静默丢掉。） */
            if (!obj_has_body(v)) { sb_add(b, "{}"); break; }
            /* `key:` 之后另起一行写 `{ … }`，与 Rust `dump_block` 同形。 */
            sb_add(b, "\n");
            int j;
            for (j = 0; j < indent; j++) sb_add(b, "  ");
            sb_addc(b, '{');
            dump_object_body(b, v, indent);
            break;
        }
    }
}

/* 把值压成**一行**写。契约：只对「扁平」值调用（见 is_flat）——
   因此它内部的递归永远不会撞上容器，压出来的行里不会再有换行。
   W4 ② 之前这里是**无条件**压行（数组项一律走它），于是整棵子树被塞进一行；
   现在由 dump_element 按 is_flat 决定是否走这里。 */
static void dump_inline(sbuf *b, const sml_value *v) {
    char num[64];
    if (!v) { sb_add(b, "null"); return; }
    switch (v->type) {
        case SML_NULL: sb_add(b, "null"); break;
        case SML_BOOL: sb_add(b, v->u.boolean ? "true" : "false"); break;
        case SML_INT: snprintf(num, sizeof(num), "%lld", v->u.i); sb_add(b, num); break;
        case SML_FLOAT: add_float(b, v->u.f); break;
        case SML_STR:
            if (needs_quote(v->u.s)) {
                sb_addc(b, '"');
                for (const char *p = v->u.s; *p; p++) {
                    if (*p == '"') sb_add(b, "\\\"");
                    else if (*p == '\\') sb_add(b, "\\\\");
                    else sb_addc(b, *p);
                }
                sb_addc(b, '"');
            } else {
                sb_add(b, v->u.s);
            }
            break;
        case SML_ARRAY: {
            sb_add(b, "[ ");
            size_t i;
            for (i = 0; i < v->u.arr.len; i++) {
                if (i) sb_add(b, ", ");
                dump_inline(b, v->u.arr.items[i]);
            }
            sb_add(b, " ]");
            break;
        }
        case SML_OBJECT: {
            sb_add(b, "{ ");
            int first = 1;
            sml_field *f;
            /* 同 dump_object_body：**不筛键**（含 __type / __name），与 Rust
               `dump_inline` 的 `m.iter()` 一致。 */
            for (f = v->u.obj.head; f; f = f->next) {
                if (!first) sb_add(b, ", ");
                first = 0;
                sb_add(b, f->key);
                sb_add(b, ": ");
                if (f->value && f->value->type == SML_STR && needs_quote(f->value->u.s)) {
                    sb_addc(b, '"');
                    for (const char *p = f->value->u.s; *p; p++) {
                        if (*p == '"') sb_add(b, "\\\"");
                        else if (*p == '\\') sb_add(b, "\\\\");
                        else sb_addc(b, *p);
                    }
                    sb_addc(b, '"');
                } else {
                    dump_inline(b, f->value);
                }
            }
            sb_add(b, " }");
            break;
        }
    }
}

/* 写一个「元素」：扁平的走单行（dump_inline），含结构的**就地展开**成多行。
   调用方负责**已**写好本元素开头的换行与缩进（`indent` 即该缩进级别），
   因此这里不在开头补缩进；展开出来的续行由各自递归负责对齐。
   对应 Rust `dump.rs::dump_element`（W4 ② 的对齐点）。 */
static void dump_element(sbuf *b, const sml_value *v, int indent) {
    if (!v) { sb_add(b, "null"); return; }
    if (is_flat(v)) { dump_inline(b, v); return; }
    switch (v->type) {
        case SML_OBJECT:
            /* 与 dump_value 的 OBJECT 分支的差别就在这一行：这里是数组元素位置，
               `{` 要跟在**当前行**（缩进已由调用方写好），不能再另起一行。 */
            sb_addc(b, '{');
            dump_object_body(b, v, indent);
            break;
        case SML_ARRAY:
            dump_array_body(b, v, indent);
            break;
        default:
            /* is_flat 为假只可能是容器；标量一律扁平。真到这儿也退化成单行。 */
            dump_inline(b, v);
            break;
    }
}

char *sml_dump(const sml_value *v) {
    if (!v) return NULL;
    sbuf b;
    memset(&b, 0, sizeof(b));
    if (v->type == SML_OBJECT) {
        sml_field *f;
        /* 顶层分叉与 Rust `to_sml` 逐字对应：带 `__type` 的对象是**裸块的树形**
           （`type [name] { … }` 解析出来的），按 `dump_block(0,0)` 渲染 —— 先换行再
           `{`、逐键、收尾 `}`；空对象写 `{}`。不带 `__type` 的才是「顶层逐键成行」。
           （A 类：原先这里无条件逐键、并把 __type/__name 跳过。） */
        if (sml_obj_get(v, "__type") != NULL) {
            if (obj_has_body(v)) {
                sb_add(&b, "\n{");
                dump_object_body(&b, v, 0);
            } else {
                sb_add(&b, "{}");
            }
        } else {
            for (f = v->u.obj.head; f; f = f->next) {
                sb_add(&b, f->key);
                /* 同 dump_value：值是有体的对象时（另起一行渲染）**不留**行尾空格（W4 ①）。
                   顶层这里是绝大多数 `key: ` 尾随空格的来源。 */
                if (obj_has_body(f->value)) sb_add(&b, ":");
                else                        sb_add(&b, ": ");
                dump_value(&b, f->value, 0);
                sb_addc(&b, '\n');
            }
        }
    } else {
        /* 顶层非对象：与数组元素同一套规则（扁平单行 / 含结构展开），
           Rust 侧 to_sml 走的也是 dump_element。W4 ② 之前这里无脑 dump_inline，
           顶层一个嵌套数组会被整坨压进一行。 */
        dump_element(&b, v, 0);
    }
    return b.buf ? b.buf : strdup("");
}

/* =====================================================================
** 5. JSON 互转 (自带极简 JSON 解析/序列化, 零外部依赖)
** ===================================================================== */

/* 最小 JSON -> sml_value (字符串/数字/bool/null/数组/对象) */
static sml_value *json_to_value(const char **pp);
static sml_value *json_to_value_depth(const char **pp, int depth);

/* 入口 wrapper：JSON 侧同样需要深度上限，
   否则 `[[[[ … ]]]]` 这种输入会把递归下降的栈打穿（段错误，接不住）。 */
static sml_value *json_to_value(const char **pp) {
    return json_to_value_depth(pp, 0);
}

static sml_value *json_to_value_depth(const char **pp, int depth) {
    const char *p = *pp;
    /* 超限即返回 null：调用方只有 `if (v) ... push/set`，会自然跳过，
       且外层循环仍靠 *p 推进，不会死循环。 */
    if (depth > SML_MAX_VALUE_DEPTH) return sml_new_null();
    while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r') p++;
    if (*p == '{') {
        p++;
        sml_value *obj = sml_new_object();
        while (*p && *p != '}') {
            while (*p == ' ' || *p == '"' || *p == '\t' || *p == '\n' || *p == '\r') p++;
            char key[256];
            size_t kl = 0;
            while (*p && *p != '"') { if (kl < 255) key[kl++] = *p; p++; }
            key[kl] = '\0';
            if (*p == '"') p++;
            while (*p && *p != ':') p++;
            if (*p == ':') p++;
            sml_value *v = json_to_value_depth(&p, depth + 1);
            if (v) sml_obj_set(obj, key, v);
            while (*p && *p != ',' && *p != '}') p++;
            if (*p == ',') p++;
        }
        if (*p == '}') p++;
        *pp = p;
        return obj;
    } else if (*p == '[') {
        p++;
        sml_value *arr = sml_new_array();
        while (*p && *p != ']') {
            sml_value *v = json_to_value_depth(&p, depth + 1);
            if (v) sml_arr_push(arr, v);
            while (*p && *p != ',' && *p != ']') p++;
            if (*p == ',') p++;
        }
        if (*p == ']') p++;
        *pp = p;
        return arr;
    } else if (*p == '"') {
        p++;
        char buf[4096];
        size_t bl = 0;
        while (*p && *p != '"') {
            if (*p == '\\' && p[1]) {
                p++;
                switch (*p) {
                    case 'n': buf[bl++] = '\n'; break;
                    case 't': buf[bl++] = '\t'; break;
                    case 'r': buf[bl++] = '\r'; break;
                    case '"': buf[bl++] = '"'; break;
                    case '\\': buf[bl++] = '\\'; break;
                    case 'u': {
                        int has_brace = 0;
                        if (*(p + 1) == '{') { has_brace = 1; p++; }
                        unsigned long cp = 0;
                        int cnt = 0;
                        while (1) {
                            if (has_brace) {
                                if (*(p + 1) == '}') { p++; break; }
                                if (!*(p + 1)) break;
                            } else {
                                if (cnt >= 4) break;
                            }
                            int h = hexdigit((unsigned char)*(p + 1));
                            if (h < 0) break;
                            cp = cp * 16 + (unsigned long)h;
                            cnt++;
                            p++;
                        }
                        put_utf8(buf, &bl, cp);
                        break;
                    }
                    default: buf[bl++] = *p; break;
                }
                p++;
            } else {
                buf[bl++] = *p++;
            }
            if (bl >= sizeof(buf) - 5) break;
        }
        if (*p == '"') p++;
        buf[bl] = '\0';
        *pp = p;
        return sml_new_str(buf);
    } else if (strncmp(p, "true", 4) == 0) { *pp = p + 4; return sml_new_bool(1); }
    else if (strncmp(p, "false", 5) == 0) { *pp = p + 5; return sml_new_bool(0); }
    else if (strncmp(p, "null", 4) == 0) { *pp = p + 4; return sml_new_null(); }
    else {
        char num[64];
        size_t nl = 0;
        while (*p && (isdigit((unsigned char)*p) || *p == '-' || *p == '+' ||
                      *p == '.' || *p == 'e' || *p == 'E')) {
            if (nl < 63) num[nl++] = *p;
            p++;
        }
        num[nl] = '\0';
        if (nl) {
            char *end = NULL;
            long long iv = strtoll(num, &end, 10);
            if (end && *end == '\0') return sml_new_int(iv);
            double fv = strtod(num, &end);
            if (end && *end == '\0') return sml_new_float(fv);
        }
        return sml_new_null();
    }
}

static void value_to_json(sbuf *b, const sml_value *v) {
    char num[64];
    if (!v) { sb_add(b, "null"); return; }
    switch (v->type) {
        case SML_NULL: sb_add(b, "null"); break;
        case SML_BOOL: sb_add(b, v->u.boolean ? "true" : "false"); break;
        case SML_INT: snprintf(num, sizeof(num), "%lld", v->u.i); sb_add(b, num); break;
        case SML_FLOAT: add_float(b, v->u.f); break;
        case SML_STR:
            sb_addc(b, '"');
            for (const char *p = v->u.s; *p; p++) {
                if (*p == '"') sb_add(b, "\\\"");
                else if (*p == '\\') sb_add(b, "\\\\");
                else if (*p == '\n') sb_add(b, "\\n");
                else if (*p == '\t') sb_add(b, "\\t");
                else sb_addc(b, *p);
            }
            sb_addc(b, '"');
            break;
        case SML_ARRAY: {
            sb_addc(b, '[');
            size_t i;
            for (i = 0; i < v->u.arr.len; i++) {
                if (i) sb_addc(b, ',');
                value_to_json(b, v->u.arr.items[i]);
            }
            sb_addc(b, ']');
            break;
        }
        case SML_OBJECT: {
            sb_addc(b, '{');
            int first = 1;
            sml_field *f;
            for (f = v->u.obj.head; f; f = f->next) {
                if (!first) sb_addc(b, ',');
                first = 0;
                sb_addc(b, '"');
                sb_add(b, f->key);
                sb_addc(b, '"');
                sb_addc(b, ':');
                value_to_json(b, f->value);
            }
            sb_addc(b, '}');
            break;
        }
    }
}

char *sml_parse_json(const char *text) {
    if (!text) return NULL;
    sml_value *v = sml_parse(text, NULL, 0);
    if (!v) return NULL;
    sbuf b;
    memset(&b, 0, sizeof(b));
    value_to_json(&b, v);
    sml_free(v);
    return b.buf ? b.buf : strdup("null");
}

char *sml_dump_from_json(const char *json) {
    if (!json) return NULL;
    const char *p = json;
    sml_value *v = json_to_value(&p);
    if (!v) return NULL;
    char *out = sml_dump(v);
    sml_free(v);
    return out;
}

/* =====================================================================
** 6. include / @include 文本内联 (对齐 Rust resolve_includes)
** ===================================================================== */

#define MAX_INC_DEPTH 32
/* 全局 include 展开次数上限：防「菱形包含」（A 含 B、C，B 与 C 又各含 D…）
   造成指数级文件读取。与 Rust 侧 MAX_INCLUDE_EXPANSIONS 同口径。 */
#define MAX_INC_EXPANSIONS 256

static void path_dir(const char *path, char *out, size_t outsz) {
    const char *slash = strrchr(path, '/');
    const char *bslash = strrchr(path, '\\');
    const char *last = (slash > bslash) ? slash : bslash;
    if (!last) { strncpy(out, ".", outsz - 1); out[outsz - 1] = '\0'; return; }
    size_t n = (size_t)(last - path);
    if (n >= outsz) n = outsz - 1;
    memcpy(out, path, n);
    out[n] = '\0';
}

/* 若行是 include 指令, 返回目标路径 (调用方 free); 否则返回 NULL */
static char *try_include_target(const char *line, char *err, size_t errsz) {
    lexer lx;
    memset(&lx, 0, sizeof(lx));
    /* 给词法器一个错误缓冲：下面要区分「引号未闭合」（`E-LEX-001`）与其它词法错，
       所以这次**不能**像以前那样把 errbuf 留空。 */
    char lexbuf[256];
    lexbuf[0] = '\0';
    lx.errbuf = lexbuf;
    lx.errsz = sizeof(lexbuf);
    lex_run(&lx, line);
    char *res = NULL;
    if (lx.n >= 2) {
        token *a = &lx.toks[0];
        token *b = &lx.toks[1];
        if (a->t == T_AT && lx.n >= 3) { a = &lx.toks[1]; b = &lx.toks[2]; }
        if (a->t == T_WORD && strcmp(a->v, "include") == 0) {
            if (b->t == T_STR) {
                /* 引号未闭合时词法器**仍会**推进一个 T_STR（内容是残缺路径），于是旧实现拿它去
                   fopen ⇒ 报 `E-INCLUDE-001`（"读取失败"），把**写法**错误导成"文件不存在"。
                   与 Rust / Lua 对齐：引号未闭合归 `E-INCLUDE-012`。
                   ⚠️ 只认"未闭合"这一种词法错（比对码），其它词法错（如未知转义）保持旧行为。 */
                if (lx.failed && strncmp(lexbuf, SML_E_LEX_001, strlen(SML_E_LEX_001)) == 0) {
                    set_err(err, errsz,
                            SML_E_INCLUDE_012 " include 路径写法非法（引号未闭合）");
                } else {
                    res = strdup(b->v);
                }
            } else if (b->t == T_WORD) {
                /* **未加引号**：旧实现这里返回 NULL ⇒ 整行被当普通内容写回，指令意图被**静默**
                   丢弃（连错都不报）。与 Rust / Lua 对齐：明确报 `E-INCLUDE-012`。
                   注：`include:`（T_COLON）/ `include {`（T_LBRACE）/ 只有 `include`（lx.n<2）
                   都不会命中此分支 —— 那些不是"路径写法错"。 */
                set_err(err, errsz, SML_E_INCLUDE_012 " include 路径写法非法（未加引号）");
            }
        }
    }
    lex_free(&lx);
    return res;
}

/* 递归展开 include。stack 为已展开文件规范路径 (防环) */
static int resolve_includes(const char *text, const char *base,
                            sbuf *out, char (*stack)[1024], int depth,
                            long *expansions,
                            char *err, size_t errsz, unsigned features) {
    if (depth >= MAX_INC_DEPTH) {
        set_err(err, errsz, SML_E_INCLUDE_004 " include 嵌套超过 %d 层", MAX_INC_DEPTH);
        return -1;
    }
    /* 展开次数是**全局**计数（跨整棵包含树），嵌套深度限制挡不住菱形包含 */
    if (++(*expansions) > MAX_INC_EXPANSIONS) {
        set_err(err, errsz, SML_E_LIMIT_003 " include 展开次数超过 %d 次上限", MAX_INC_EXPANSIONS);
        return -1;
    }
    const char *p = text;
    while (*p) {
        const char *nl = strchr(p, '\n');
        size_t linelen = nl ? (size_t)(nl - p) : strlen(p);
        char *line = (char *)malloc(linelen + 1);
        if (!line) { set_err(err, errsz, SML_E_LIMIT_010 " 内存分配失败（include 行缓冲）"); return -1; }
        memcpy(line, p, linelen);
        line[linelen] = '\0';

        /* 先查「写法非法」：是 include 指令、但路径没加引号（或引号未闭合）⇒ 明确报码，
           而不是像旧实现那样当普通内容**静默**放过。 */
        char terr[512];
        terr[0] = '\0';
        char *inc = try_include_target(line, terr, sizeof(terr));
        if (!inc && terr[0]) {
            set_err(err, errsz, "%s", terr);
            free(line);
            return -1;
        }
        if (inc) {
            /* include 门控：`@feature disable include` 后必须拒绝（与 Rust 同码 001）。 */
            if (!(features & (1u << 1))) {
                set_err(err, errsz, SML_E_FEATURE_001 " include 特性未启用");
                free(line); free(inc);
                return -1;
            }
            char path[1024];
            snprintf(path, sizeof(path), "%s/%s", base, inc);
            FILE *f = fopen(path, "rb");
            if (!f) {
                set_err(err, errsz, SML_E_INCLUDE_001 " include 读取失败 %s", path);
                free(line); free(inc);
                return -1;
            }
            fseek(f, 0, SEEK_END);
            long sz = ftell(f);
            fseek(f, 0, SEEK_SET);
            char *content = (char *)malloc((size_t)sz + 1);
            if (!content) { fclose(f); set_err(err, errsz, SML_E_LIMIT_010 " 内存分配失败（include 文件内容）"); free(line); free(inc); return -1; }
            fread(content, 1, (size_t)sz, f);
            content[sz] = '\0';
            /* 子文件同样可能带 BOM：它在**拼接后的文本中间**，`sml_parse` 那处的开头检查救不了，
               而且不去掉的话连它的第一行 `include "x"` 都认不出来（BOM 会粘在首词上）。
               就地把 BOM 移掉（含结尾 NUL 一起搬）。 */
            if ((size_t)sz >= 3 && (unsigned char)content[0] == 0xEF
                && (unsigned char)content[1] == 0xBB && (unsigned char)content[2] == 0xBF) {
                memmove(content, content + 3, (size_t)sz - 2);
            }
            fclose(f);

            /* 规范化为绝对路径。改用**动态分配**：固定 1024 缓冲在长路径下
               会规范化失败，若那时回落到未规范化的 path，下面的越界校验与
               循环检测就等于失效（Rust 侧口径是 canonicalize + starts_with）。 */
            char *canon = NULL;
#ifdef _WIN32
            canon = _fullpath(NULL, path, 0);
#else
            canon = realpath(path, NULL);
#endif
            if (!canon) {
                set_err(err, errsz, SML_E_INCLUDE_001 " include 路径无法解析: %s", path);
                free(content); free(line); free(inc);
                return -1;
            }
            /* 路径穿越防护：被包含文件必须仍在基准目录之内。
               缺这道校验时 `include "../../etc/passwd"` 会把任意文件内容
               内联进解析结果。 */
            char *basec = NULL;
#ifdef _WIN32
            basec = _fullpath(NULL, base, 0);
#else
            basec = realpath(base, NULL);
#endif
            if (basec) {
                size_t bl = strlen(basec);
                int inside = strncmp(canon, basec, bl) == 0 &&
                             (canon[bl] == '/' || canon[bl] == '\\' || canon[bl] == '\0');
                free(basec);
                if (!inside) {
                    set_err(err, errsz, SML_E_INCLUDE_003 " include 目标越出基准目录: %s", inc);
                    free(canon); free(content); free(line); free(inc);
                    return -1;
                }
            } else {
                /* 基准目录无法规范化 → **拒绝**，不能因为"没法比"就放行
                   （fail-open 会让越界读取在校验失败时静默通过）。 */
                set_err(err, errsz, SML_E_INCLUDE_010 " include 基准目录不可解析，已拒绝: %s", base);
                free(canon); free(content); free(line); free(inc);
                return -1;
            }
            int cyc = 0;
            for (int i = 0; i < depth; i++)
                if (strcmp(stack[i], canon) == 0) { cyc = 1; break; }
            if (cyc) {
                set_err(err, errsz, SML_E_INCLUDE_002 " include 循环引用: %s", canon);
                free(canon); free(content); free(line); free(inc);
                return -1;
            }
            strncpy(stack[depth], canon, 1023);
            stack[depth][1023] = '\0';

            /* 子基准用**规范化后的**路径，避免把未规范化的路径继续传下去 */
            char childbase[1024];
            path_dir(canon, childbase, sizeof(childbase));
            free(canon);
            if (resolve_includes(content, childbase, out, stack, depth + 1, expansions, err, errsz, features) != 0) {
                free(content); free(line); free(inc);
                return -1;
            }
            free(content);
        } else {
            sb_add(out, line);
            sb_addc(out, '\n');
        }
        free(line);
        free(inc);
        p = nl ? nl + 1 : p + strlen(p);
    }
    return 0;
}

/* 文件入口的核心：读文件 → 展开 include → 解析，并按需交出诊断 / 弱表。
   | Core of the file entries: read, expand includes, parse, collect outputs. */
static sml_value *parse_file_core(const char *path, char *err, size_t errsz,
                                  sml_diag **diags_out, size_t *n_diags_out,
                                  const sml_builtin *b,
                                  sml_weakbind **binds_out, size_t *n_binds_out) {
    if (!path) {
        if (err && errsz) set_err(err, errsz, SML_E_INTERNAL_001 " 空指针入参（path 为 NULL）");
        return NULL;
    }
    FILE *f = fopen(path, "rb");
    if (!f) {
        if (err && errsz) set_err(err, errsz, SML_E_IO_001 " 读取失败 %s", path);
        return NULL;
    }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *text = (char *)malloc((size_t)sz + 1);
    if (!text) { fclose(f); set_err(err, errsz, SML_E_LIMIT_010 " 内存分配失败（入口文件内容）"); return NULL; }
    fread(text, 1, (size_t)sz, f);
    text[sz] = '\0';
    fclose(f);

    char base[1024];
    path_dir(path, base, sizeof(base));
    sbuf out;
    memset(&out, 0, sizeof(out));
    char stack[MAX_INC_DEPTH + 1][1024];
    long expansions = 0;
    if (resolve_includes(text, base, &out, stack, 0, &expansions, err, errsz,
                         scan_features(text)) != 0) {
        free(text);
        free(out.buf);
        return NULL;
    }
    free(text);
    sml_value *v = parse_core(out.buf ? out.buf : "", err, errsz,
                              diags_out, n_diags_out, b, binds_out, n_binds_out);
    free(out.buf);
    return v;
}

sml_value *sml_parse_file(const char *path, char *err, size_t errsz) {
    return parse_file_core(path, err, errsz, NULL, NULL, NULL, NULL, NULL);
}

sml_value *sml_parse_file_ex(const char *path, char *err, size_t errsz,
                             sml_diag **diags, size_t *n_diags) {
    return parse_file_core(path, err, errsz, diags, n_diags, NULL, NULL, NULL);
}

sml_value *sml_parse_file_full(const char *path, char *err, size_t errsz,
                               const sml_builtin *b,
                               sml_diag **diags, size_t *n_diags,
                               sml_weakbind **binds, size_t *n_binds) {
    return parse_file_core(path, err, errsz, diags, n_diags, b, binds, n_binds);
}
