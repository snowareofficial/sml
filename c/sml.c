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

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <math.h>

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
} parser;

static sml_value *parse_block(parser *ps, tok_type closing);

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
    long long iv = strtoll(w, &end, 10);
    if (end && *end == '\0' && end != w) return sml_new_int(iv);
    /* 浮点 (含 .5 / 1e3 等 strtod 可解析) */
    if (strchr(w, '.') || strchr(w, 'e') || strchr(w, 'E')) {
        char *fend = NULL;
        double fv = strtod(w, &fend);
        if (fend && *fend == '\0' && fend != w) return sml_new_float(fv);
    }
    /* V2/V3 严格模式：自由字符串必须加引号 */
    if (ps->version >= 2) {
        if (ps->lx->errbuf) {
            snprintf(ps->lx->errbuf, ps->lx->errsz,
                     "sml v2/v3: 字符串必须加引号，裸词 `%s` 应写作 \"%s\"", w, w);
        }
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
                ok = 0;
                break;
            }
            ccontract *tgt = contract_find(ps, spec->ref_name);
            if (!tgt) {
                snprintf(err, errsz,
                         "sml: 字段 `%s` 引用了未定义的契约 `%s`（契约 `%s`）",
                         spec->name, spec->ref_name, cname);
                return -1;
            }
            if (apply_contract_rec(ps, tgt, (sml_value *)v, err, errsz) != 0) return -1;
            ok = 1;
            break;
        }
    }
    if (!ok) {
        snprintf(err, errsz,
                 "sml: 字段 `%s` 类型应为 %s，实际为 %s（契约 `%s`）",
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
                snprintf(err, errsz,
                         "sml: 字段 `%s` 值 %g 小于下界 %g（契约 `%s`）",
                         spec->name, n, spec->min, cname);
                return -1;
            }
            if (spec->max_set && n > spec->max) {
                snprintf(err, errsz,
                         "sml: 字段 `%s` 值 %g 大于上界 %g（契约 `%s`）",
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
                snprintf(err, errsz,
                         "sml: 字段 `%s` 未在契约 `%s` 中声明（严格模式；如需允许额外字段请在契约名后写 `loose`）",
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
                snprintf(err, errsz,
                         "sml: 字段 `%s` 必填但缺失（契约 `%s`）", cf->name, c->name);
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
        snprintf(err, errsz, "sml: 引用了未定义的契约 `%s`", name);
        return -1;
    }
    return apply_contract_rec(ps, c, node, err, errsz);
}

/* 应用契约；失败则置 parser.failed 标志 (不依赖 errbuf 是否有遗留内容) */
static void apply_or_fail(parser *ps, sml_value *node, const char *name) {
    if (apply_contract_name(ps, node, name, ps->lx->errbuf, ps->lx->errsz) != 0)
        ps->failed = 1;
}

/* 解析契约体 (调用前已消费 '{'；负责消费 '}') */
static void parse_contract_body(parser *ps, ccontract *c) {
    if (peek(ps)->t != T_LBRACE) return;
    next(ps); /* consume { */
    while (1) {
        token *ft = peek(ps);
        if (ft->t == T_RBRACE) { next(ps); break; }
        if (ft->t == T_EOF) break;
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
                token *nv = next(ps);
                if (nv && nv->t == T_WORD) { cf->min = atof(nv->v); cf->min_set = 1; }
                continue;
            }
            if (strcmp(mt->v, "max") == 0) {
                next(ps);
                token *nv = next(ps);
                if (nv && nv->t == T_WORD) { cf->max = atof(nv->v); cf->max_set = 1; }
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

static sml_value *parse_array(parser *ps) {
    sml_value *arr = sml_new_array();
    for (;;) {
        token *t = peek(ps);
        if (t->t == T_RBRACK) { next(ps); break; }
        if (t->t == T_EOF) { break; }
        if (t->t == T_COMMA) { next(ps); continue; }
        if (t->t == T_LBRACE) {
            next(ps);
            sml_arr_push(arr, parse_block(ps, T_RBRACE));
        } else if (t->t == T_STR) {
            sml_arr_push(arr, sml_new_str(next(ps)->v));
        } else if (t->t == T_WORD) {
            sml_arr_push(arr, coerce_word(next(ps)->v, ps));
        } else {
            next(ps);
        }
    }
    return arr;
}

/* 解析块/对象。closing=T_RBRACE 或 T_EOF(顶层) */
static sml_value *parse_block(parser *ps, tok_type closing) {
    sml_value *obj = sml_new_object();
    char *block_is = NULL;  /* 块级 @is 契约名 (作用于本块) */
    for (;;) {
        token *t = peek(ps);
        if (t->t == T_EOF) break;
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
                        snprintf(ps->lx->errbuf ? ps->lx->errbuf : (char[1]){0},
                                 ps->lx->errsz,
                                 "sml: 未知版本 `%s`；仅支持 v1/v2/v3", lit->v);
                        ps->failed = 1;
                    } else if (ver > 3) {
                        /* 超出本实现支持的版本范围 (V1..V3) */
                        snprintf(ps->lx->errbuf ? ps->lx->errbuf : (char[1]){0},
                                 ps->lx->errsz,
                                 "sml: 版本 v%d 超出本库接受范围 (v1..v3)", ver);
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
                        sml_obj_set(sub, "__name", sml_arr_get(args, 0));
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
        if (err && errsz) snprintf(err, errsz, "sml: null text");
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
        if (err && errsz && err[0] == '\0') snprintf(err, errsz, "sml: parse failed");
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
static sml_value *json_to_value(const char **pp) {
    const char *p = *pp;
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
            sml_value *v = json_to_value(&p);
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
            sml_value *v = json_to_value(&p);
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
                            char *err, size_t errsz) {
    if (depth >= MAX_INC_DEPTH) {
        snprintf(err, errsz, "sml: include 嵌套超过 %d 层", MAX_INC_DEPTH);
        return -1;
    }
    const char *p = text;
    while (*p) {
        const char *nl = strchr(p, '\n');
        size_t linelen = nl ? (size_t)(nl - p) : strlen(p);
        char *line = (char *)malloc(linelen + 1);
        if (!line) { snprintf(err, errsz, "sml: oom"); return -1; }
        memcpy(line, p, linelen);
        line[linelen] = '\0';

        char *inc = try_include_target(line);
        if (inc) {
            char path[1024];
            snprintf(path, sizeof(path), "%s/%s", base, inc);
            FILE *f = fopen(path, "rb");
            if (!f) {
                snprintf(err, errsz, "sml: include 读取失败 %s", path);
                free(line); free(inc);
                return -1;
            }
            fseek(f, 0, SEEK_END);
            long sz = ftell(f);
            fseek(f, 0, SEEK_SET);
            char *content = (char *)malloc((size_t)sz + 1);
            if (!content) { fclose(f); snprintf(err, errsz, "sml: oom"); free(line); free(inc); return -1; }
            fread(content, 1, (size_t)sz, f);
            content[sz] = '\0';
            fclose(f);

            char canon[1024];
#ifdef _WIN32
            if (_fullpath(canon, path, sizeof(canon)) == NULL) strcpy(canon, path);
#else
            if (realpath(path, canon) == NULL) strcpy(canon, path);
#endif
            int cyc = 0;
            for (int i = 0; i < depth; i++)
                if (strcmp(stack[i], canon) == 0) { cyc = 1; break; }
            if (cyc) {
                snprintf(err, errsz, "sml: include 循环引用: %s", canon);
                free(content); free(line); free(inc);
                return -1;
            }
            strcpy(stack[depth], canon);

            char childbase[1024];
            path_dir(path, childbase, sizeof(childbase));
            if (resolve_includes(content, childbase, out, stack, depth + 1, err, errsz) != 0) {
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
        if (err && errsz) snprintf(err, errsz, "sml: null path");
        return NULL;
    }
    FILE *f = fopen(path, "rb");
    if (!f) {
        if (err && errsz) snprintf(err, errsz, "sml: 读取失败 %s", path);
        return NULL;
    }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *text = (char *)malloc((size_t)sz + 1);
    if (!text) { fclose(f); if (err && errsz) snprintf(err, errsz, "sml: oom"); return NULL; }
    fread(text, 1, (size_t)sz, f);
    text[sz] = '\0';
    fclose(f);

    char base[1024];
    path_dir(path, base, sizeof(base));
    sbuf out;
    memset(&out, 0, sizeof(out));
    char stack[MAX_INC_DEPTH + 1][1024];
    if (resolve_includes(text, base, &out, stack, 0, err, errsz) != 0) {
        free(text);
        free(out.buf);
        return NULL;
    }
    free(text);
    sml_value *v = sml_parse(out.buf ? out.buf : "", err, errsz);
    free(out.buf);
    return v;
}
