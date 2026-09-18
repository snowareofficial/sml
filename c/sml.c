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
    return "sml 0.1.0 (SNOWARE Markup Language, pure C)";
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
    T_LBRACE, T_RBRACE, T_LBRACK, T_RBRACK, T_COMMA, T_COLON, T_AT,
    T_QMARK, T_EQ,   /* 契约修饰符 ? 与默认值 = (独立 token, 确保 str? / = 不并入裸词) */
    T_STR,   /* 引号串 (已解码, 含 \u 转义) */
    T_WORD,  /* 裸词 */
    T_EOF,
} tok_type;

typedef struct {
    tok_type t;
    char *v;   /* T_STR / T_WORD 的内容 */
} token;

typedef struct {
    token *toks;
    size_t n, cap, pos;
    char *errbuf;
    size_t errsz;
} lexer;

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
            /* 斜杠星 多行注释，直到 星斜杠 */
            p += 2;
            while (*p) {
                if (*p == '*' && p[1] == '/') { p += 2; break; }
                p++;
            }
        } else if (c == '_' && p[1] == '*') {
            /* `_*` 多行注释，直到 `*_` */
            p += 2;
            while (*p) {
                if (*p == '*' && p[1] == '_') { p += 2; break; }
                p++;
            }
        } else if (c == '"') {
            FLUSH();
            p++;
            char sb[4096];
            size_t slen = 0;
            while (*p) {
                if (*p == '"') { p++; break; }
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
                            put_utf8(sb, &slen, cp);
                            break;
                        }
                        default:  sb[slen++] = e; break;
                    }
                    p++;
                } else {
                    sb[slen++] = *p++;
                }
                if (slen >= sizeof(sb) - 5) break;
            }
            sb[slen] = '\0';
            lex_push(lx, T_STR, strdup(sb));
        } else if (c == '{') { FLUSH(); lex_push(lx, T_LBRACE, NULL); p++; }
        else if (c == '}') { FLUSH(); lex_push(lx, T_RBRACE, NULL); p++; }
        else if (c == '[') { FLUSH(); lex_push(lx, T_LBRACK, NULL); p++; }
        else if (c == ']') { FLUSH(); lex_push(lx, T_RBRACK, NULL); p++; }
        else if (c == ',') { FLUSH(); lex_push(lx, T_COMMA, NULL); p++; }
        else if (c == ':') { FLUSH(); lex_push(lx, T_COLON, NULL); p++; }
        else if (c == '?') { FLUSH(); lex_push(lx, T_QMARK, NULL); p++; }
        else if (c == '=') { FLUSH(); lex_push(lx, T_EQ, NULL); p++; }
        /* `@` 仅当位于**词首**时才是片段定义标记（`@base { ... }`）。
        ** 出现在词中间时（典型如邮箱 `a@b.c`）必须作为普通字符保留：
        ** 否则 `a@b.c` 会被切成 WORD("a") + AT + WORD("b.c")，
        ** 后半段在解析时被丢弃，导致邮箱静默损坏为 `a`。 */
        else if (c == '@') {
            if (blen == 0) { FLUSH(); lex_push(lx, T_AT, NULL); }
            else { if (blen < sizeof(buf) - 1) buf[blen++] = c; }
            p++;
        }
        else if (c == ' ' || c == '\t' || c == '\n' || c == '\r') { FLUSH(); p++; }
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
} parser;

/* 值嵌套深度上限：与 Rust 侧 MAX_VALUE_DEPTH 同口径。
   `a{a{a{ ... }}}` 这类输入会让递归下降一路压栈；栈溢出在 C 里是段错误，
   **无法被错误处理接住**，只能在递归入口主动限深。 */
#define SML_MAX_VALUE_DEPTH 128

static sml_value *parse_block(parser *ps, tok_type closing);
static sml_value *parse_block_inner(parser *ps, tok_type closing);
static sml_value *parse_array(parser *ps);
static sml_value *parse_array_inner(parser *ps);

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
    sml_value *v = parse_block_inner(ps, closing);
    if (ps->depth > 0) ps->depth--;
    return v;
}

/* 未闭合的块/数组/契约体（遇到文件结尾就收尾了）：E-PARSE-001。
   与 Rust 同码：rust/sml-parse/src/parser.rs 在「本层期望闭合符号（closing 非 None）
   却遇到文件结尾」时报 E_PARSE_001，契约体未闭合也归同一条。
   注：顶层（closing==T_EOF）遇到文件结尾是**正常**结束，不算未闭合，不报。 */
static void err_unclosed(parser *ps, const char *what) {
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
        return sml_new_str(w);
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
    return sml_new_str(w);
}

static void frag_put(parser *ps, const char *name, sml_value *v) {
    struct frag *f = (struct frag *)calloc(1, sizeof(struct frag));
    if (!f) return;
    f->name = strdup(name);
    f->val = v;
    f->next = ps->frags;
    ps->frags = f;
}

/* 对象内同名键冲突 -> 提升为数组 */
static void obj_set_dup(sml_value *obj, const char *key, sml_value *val) {
    sml_value *ex = sml_obj_get(obj, key);
    if (!ex) {
        sml_obj_set(obj, key, val);
        return;
    }
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

/* 应用契约；失败则置 parser.failed 标志 (不依赖 errbuf 是否有遗留内容) */
static void apply_or_fail(parser *ps, sml_value *node, const char *name) {
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
        /* 修饰符: ?  = min max loose */
        for (;;) {
            token *mt = peek(ps);
            if (mt->t == T_QMARK) {
                next(ps); cf->required = 0; continue;
            }
            if (mt->t == T_EQ) {
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
            next(ps);
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
            sml_arr_push(arr, coerce_word(next(ps)->v, ps));
        } else {
            /* 兜底：其余 token（例如数组里多余的 `}`）仍然**静默跳过**。
               ⚠️ 这是已知的端间不一致，**不属 W17 范围，别顺手改**：Rust 对多余的 `}`
               报 `E-PARSE-003`，而 C 与 JS 一样静默（`m: [ } ]` 实测两端都得到 `{"m":[]}`）。
               已登记进 errors/README 的静默清单，归 W16 判定「改成报错」还是
               「写进规范、明确允许静默」。 */
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
            if (closing == T_EOF) break; /* 顶层遇右括号也停 */
        }
        if (t->t == T_COMMA) { next(ps); continue; }
        if (t->t == T_AT) {
            token *nxt = peek_at(ps, 1);
            /* `@version vN` 是版本声明指令，不是片段定义（与 Rust/Lua/JS 对齐）。 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "version") == 0) {
                next(ps); next(ps); /* @ version */
                token *lit = peek(ps);
                if (lit->t == T_WORD || lit->t == T_STR) {
                    int ver = 0;
                    if (strcmp(lit->v, "v1") == 0 || strcmp(lit->v, "1") == 0) ver = 1;
                    else if (strcmp(lit->v, "v2") == 0 || strcmp(lit->v, "2") == 0) ver = 2;
                    else if (strcmp(lit->v, "v3") == 0 || strcmp(lit->v, "3") == 0) ver = 3;
                    if (ver == 0) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_004 " 未知版本 `%s`；仅支持 v1/v2/v3", lit->v);
                        ps->failed = 1;
                    } else if (ver > 3) {
                        /* 超出本实现支持的版本范围 (V1..V3)。注意这条分支当前不可达：
                           上面只可能解析出 ver ∈ {1,2,3}（见审计记录）。保留它是因为将来
                           放宽版本时仍需要兜底，但别以为它被测试覆盖到了。 */
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_004 " 版本 v%d 超出本库接受范围 (v1..v3)", ver);
                        ps->failed = 1;
                    } else {
                        ps->version = ver;
                    }
                    next(ps);
                }
                continue;
            }
            /* `@contract Name [loose] { ... }` 定义契约 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "contract") == 0) {
                next(ps); next(ps); /* @ contract */
                char *cname = next(ps)->v;
                int loose = 0;
                if (peek(ps)->t == T_WORD && strcmp(peek(ps)->v, "loose") == 0) {
                    loose = 1; next(ps);
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
                next(ps); next(ps); /* @ is */
                token *cn = next(ps);
                if (cn && (cn->t == T_WORD || cn->t == T_STR)) {
                    char *cname_is = cn->v;
                    if (peek(ps)->t == T_LBRACE) {
                        /* `@is Name { ... }` 匿名块/当前块应用契约:
                        ** 解析块体、应用契约、合并字段回当前对象 */
                        next(ps);
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
            /* 片段定义: @name [type [name]] { ... } */
            next(ps);
            token *ft = next(ps);
            if (ft->t != T_WORD && ft->t != T_STR) break;
            char *fname = ft->v;
            if (peek(ps)->t == T_COLON) next(ps);
            char *ftype = NULL, *farg = NULL;
            if (peek(ps)->t == T_WORD) {
                ftype = next(ps)->v;
                if (peek(ps)->t == T_WORD) farg = next(ps)->v;
            }
            if (peek(ps)->t == T_LBRACE) {
                next(ps);
                sml_value *sub = parse_block(ps, T_RBRACE);
                if (ftype) {
                    sml_obj_set(sub, "__type", sml_new_str(ftype));
                    if (farg) sml_obj_set(sub, "__name", sml_new_str(farg));
                }
                frag_put(ps, fname, sub);
            }
            continue;
        }
        /* key */
        token *kt = next(ps);
        if (kt->t != T_WORD && kt->t != T_STR) break;
        char *key = kt->v;
        /* 字段级 @is: key 后紧跟 @is Name */
        char *field_is = NULL;
        if (peek(ps)->t == T_AT) {
            token *nn = peek_at(ps, 1);
            if (nn && nn->t == T_WORD && strcmp(nn->v, "is") == 0) {
                next(ps); next(ps); /* @ is */
                token *cn = next(ps);
                if (cn && (cn->t == T_WORD || cn->t == T_STR)) field_is = cn->v;
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
                while (peek(ps)->t == T_WORD || peek(ps)->t == T_STR) {
                    token *at = next(ps);
                    sml_arr_push(args, at->t == T_STR ? sml_new_str(at->v)
                                                      : coerce_word(at->v, ps));
                }
                if (peek(ps)->t == T_LBRACE) {
                    next(ps);
                    sml_value *sub = parse_block(ps, T_RBRACE);
                    sml_obj_set(sub, "__type", sml_new_str(key));
                    if (sml_arr_len(args) == 1)
                        /* 必须克隆再存：args 下一行就被 sml_free，直接借用
                           args[0] 会让 `__name` 变成悬垂指针，根节点释放时
                           二次释放 —— 即已知的「带名块堆损坏」。 */
                        sml_obj_set(sub, "__name", sml_clone(sml_arr_get(args, 0)));
                    sml_free(args);
                    if (field_is)
                        apply_or_fail(ps, sub, field_is);
                    obj_set_dup(obj, key, sub);
                    continue;
                }
                sml_free(args);
            }
        }
        nt = peek(ps);
        if (nt->t == T_LBRACE) {
            next(ps);
            sml_value *sub = parse_block(ps, T_RBRACE);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            obj_set_dup(obj, key, sub);
        } else if (nt->t == T_LBRACK) {
            next(ps);
            sml_value *sub = parse_array(ps);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            obj_set_dup(obj, key, sub);
        } else if (nt->t == T_STR) {
            sml_value *sub = sml_new_str(next(ps)->v);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            obj_set_dup(obj, key, sub);
        } else if (nt->t == T_WORD) {
            sml_value *sub = coerce_word(next(ps)->v, ps);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            obj_set_dup(obj, key, sub);
        } else if (colon) {
            sml_value *sub = sml_new_null();
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            obj_set_dup(obj, key, sub);
        } else {
            /* key 本身即值 (片段引用/裸词) */
            sml_value *sub = coerce_word(key, ps);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            obj_set_dup(obj, key, sub);
        }
    }
    /* 块级 @is 应用 */
    if (block_is)
        apply_or_fail(ps, obj, block_is);
    return obj;
}

sml_value *sml_parse(const char *text, char *err, size_t errsz) {
    if (!text) {
        if (err && errsz) set_err(err, errsz, SML_E_INTERNAL_001 " 空指针入参（text 为 NULL）");
        return NULL;
    }
    if (err && errsz) err[0] = '\0';   /* 成功时保持为空，避免误判 */
    lexer lx;
    memset(&lx, 0, sizeof(lx));
    lx.errbuf = err;
    lx.errsz = errsz;
    lex_run(&lx, text);
    parser ps;
    memset(&ps, 0, sizeof(ps));
    ps.lx = &lx;
    ps.frags = NULL;
    ps.contracts = NULL;
    /* 顶层支持三种形态，与 sml_dump 的输出对称：
    **   - `[ ... ]` 数组
    **   - `{ ... }` 顶层对象块
    **   - 键值块（传统形态） */
    sml_value *v;
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
    if (!v) {
        if (err && errsz && err[0] == '\0')
            set_err(err, errsz, SML_E_PARSE_012 " 解析失败：未能给出更具体的原因");
        return NULL;
    }
    return v;
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
            *s == ':' || *s == '#' || *s == '{' || *s == '}')
            return 1;
    }
    return 0;
}

static void dump_value(sbuf *b, const sml_value *v, int indent);
static void dump_inline(sbuf *b, const sml_value *v);

static void dump_value(sbuf *b, const sml_value *v, int indent) {
    char num[64];
    if (!v) { sb_add(b, "null"); return; }
    switch (v->type) {
        case SML_NULL: sb_add(b, "null"); break;
        case SML_BOOL: sb_add(b, v->u.boolean ? "true" : "false"); break;
        case SML_INT: snprintf(num, sizeof(num), "%lld", v->u.i); sb_add(b, num); break;
        case SML_FLOAT: snprintf(num, sizeof(num), "%g", v->u.f); sb_add(b, num); break;
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
            sb_addc(b, '[');
            size_t i;
            for (i = 0; i < v->u.arr.len; i++) {
                sb_add(b, "\n");
                int j;
                for (j = 0; j < indent + 1; j++) sb_add(b, "  ");
                dump_inline(b, v->u.arr.items[i]);
            }
            sb_add(b, "\n");
            int j;
            for (j = 0; j < indent; j++) sb_add(b, "  ");
            sb_addc(b, ']');
            break;
        }
        case SML_OBJECT: {
            int has_body = 0;
            sml_field *f;
            for (f = v->u.obj.head; f; f = f->next)
                if (strcmp(f->key, "__type") && strcmp(f->key, "__name")) { has_body = 1; break; }
            if (!has_body) { sb_add(b, "{}"); break; }
            sb_add(b, "\n");
            int j;
            for (j = 0; j < indent; j++) sb_add(b, "  ");
            sb_addc(b, '{');
            for (f = v->u.obj.head; f; f = f->next) {
                if (!strcmp(f->key, "__type") || !strcmp(f->key, "__name")) continue;
                sb_add(b, "\n");
                for (j = 0; j < indent + 1; j++) sb_add(b, "  ");
                sb_add(b, f->key);
                sb_add(b, ": ");
                dump_value(b, f->value, indent + 1);
            }
            sb_add(b, "\n");
            for (j = 0; j < indent; j++) sb_add(b, "  ");
            sb_addc(b, '}');
            break;
        }
    }
}

static void dump_inline(sbuf *b, const sml_value *v) {
    char num[64];
    if (!v) { sb_add(b, "null"); return; }
    switch (v->type) {
        case SML_NULL: sb_add(b, "null"); break;
        case SML_BOOL: sb_add(b, v->u.boolean ? "true" : "false"); break;
        case SML_INT: snprintf(num, sizeof(num), "%lld", v->u.i); sb_add(b, num); break;
        case SML_FLOAT: snprintf(num, sizeof(num), "%g", v->u.f); sb_add(b, num); break;
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
            for (f = v->u.obj.head; f; f = f->next) {
                if (!strcmp(f->key, "__type") || !strcmp(f->key, "__name")) continue;
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

char *sml_dump(const sml_value *v) {
    if (!v) return NULL;
    sbuf b;
    memset(&b, 0, sizeof(b));
    if (v->type == SML_OBJECT) {
        sml_field *f;
        for (f = v->u.obj.head; f; f = f->next) {
            if (!strcmp(f->key, "__type") || !strcmp(f->key, "__name")) continue;
            sb_add(&b, f->key);
            sb_add(&b, ": ");
            dump_value(&b, f->value, 0);
            sb_addc(&b, '\n');
        }
    } else {
        dump_inline(&b, v);
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
        case SML_FLOAT: snprintf(num, sizeof(num), "%g", v->u.f); sb_add(b, num); break;
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
static char *try_include_target(const char *line) {
    lexer lx;
    memset(&lx, 0, sizeof(lx));
    lex_run(&lx, line);
    char *res = NULL;
    if (lx.n >= 2) {
        token *a = &lx.toks[0];
        token *b = &lx.toks[1];
        if (a->t == T_AT && lx.n >= 3) { a = &lx.toks[1]; b = &lx.toks[2]; }
        if (a->t == T_WORD && strcmp(a->v, "include") == 0 && b->t == T_STR) {
            res = strdup(b->v);
        }
    }
    lex_free(&lx);
    return res;
}

/* 递归展开 include。stack 为已展开文件规范路径 (防环) */
static int resolve_includes(const char *text, const char *base,
                            sbuf *out, char (*stack)[1024], int depth,
                            long *expansions,
                            char *err, size_t errsz) {
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

        char *inc = try_include_target(line);
        if (inc) {
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
            if (resolve_includes(content, childbase, out, stack, depth + 1, expansions, err, errsz) != 0) {
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

sml_value *sml_parse_file(const char *path, char *err, size_t errsz) {
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
    if (resolve_includes(text, base, &out, stack, 0, &expansions, err, errsz) != 0) {
        free(text);
        free(out.buf);
        return NULL;
    }
    free(text);
    sml_value *v = sml_parse(out.buf ? out.buf : "", err, errsz);
    free(out.buf);
    return v;
}
