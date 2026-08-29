/*
** SPDX-License-Identifier: MulanPSL-2.0
** sml.c — SML (SNOWARE Markup Language) 纯 C 实现
**
** 零依赖 C99 单文件实现。API 见 sml.h。
**
** 实现结构:
**   1. 值构造/释放/容器操作
**   2. 词法 (token 流)
**   3. 递归下降解析 (块/数组/标量/片段)
**   4. 序列化 (round-trip)
**   5. JSON 桥 (C-ABI 兼容)
**
** 注释：单行 `#` 与 `--`；多行 `/* */` 与 `_* *_`(与 Rust/JS/Lua 实现对齐)
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
** 2. 词法
** ===================================================================== */

typedef enum {
    T_LBRACE, T_RBRACE, T_LBRACK, T_RBRACK, T_COMMA, T_COLON, T_AT,
    T_STR,   /* 引号串 (已解码) */
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
        } else if (c == '/' && p[1] == '*') {
            /* `/*` 多行注释，直到 `*\/` */
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
                        default:  sb[slen++] = e; break;
                    }
                    p++;
                } else {
                    sb[slen++] = *p++;
                }
                if (slen >= sizeof(sb) - 4) break;
            }
            sb[slen] = '\0';
            lex_push(lx, T_STR, strdup(sb));
        } else if (c == '{') { FLUSH(); lex_push(lx, T_LBRACE, NULL); p++; }
        else if (c == '}') { FLUSH(); lex_push(lx, T_RBRACE, NULL); p++; }
        else if (c == '[') { FLUSH(); lex_push(lx, T_LBRACK, NULL); p++; }
        else if (c == ']') { FLUSH(); lex_push(lx, T_RBRACK, NULL); p++; }
        else if (c == ',') { FLUSH(); lex_push(lx, T_COMMA, NULL); p++; }
        else if (c == ':') { FLUSH(); lex_push(lx, T_COLON, NULL); p++; }
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
** 3. 解析 (递归下降)
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
} parser;

static sml_value *parse_block(parser *ps, tok_type closing);

static token *peek(parser *ps) {
    return &ps->lx->toks[ps->lx->pos];
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
                return f->val; /* 简化: 共享引用 (只读场景够用) */
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
    for (;;) {
        token *t = peek(ps);
        if (t->t == T_EOF) break;
        if (t->t == T_RBRACE || t->t == T_RBRACK) {
            if (closing == t->t) { next(ps); break; }
            if (closing == T_EOF) break; /* 顶层遇右括号也停 */
        }
        if (t->t == T_COMMA) { next(ps); continue; }
        if (t->t == T_AT) {
            /* 片段定义: @name [type [name]] { ... } */
            next(ps);
            token *ft = next(ps);
            if (ft->t != T_WORD && ft->t != T_STR) break;
            char *fname = ft->v;
            /* `@version v1` 是版本声明指令，不是片段定义（与 Rust/Lua/JS 对齐）。
            ** 版本声明不进主树；未声明时按 v1 处理。此前未处理会导致
            ** `version` 被当作片段名、后续内容被整体吞掉。 */
            if (strcmp(fname, "version") == 0) {
                token *lit = next(ps);
                if (lit->t != T_WORD && lit->t != T_STR) break;
                if (strcmp(lit->v, "v1") != 0 && strcmp(lit->v, "1") != 0) {
                    snprintf(ps->lx->errbuf ? ps->lx->errbuf : NULL,
                             ps->lx->errsz,
                             "sml: @version 须写作 `@version v1`；`version` 不可作为片段名");
                    break;
                }
                continue;
            }
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
        int colon = 0;
        if (peek(ps)->t == T_COLON) { colon = 1; next(ps); }
        token *nt = peek(ps);
        /* 裸块预扫描: 无冒号且后继是词, 可能 `type name { }` */
        if (!colon && nt->t == T_WORD) {
            /* 向后找 { (仅经过 word/str) */
            size_t probe = ps->lx->pos;
            int found = 0;
            while (probe < ps->lx->n) {
                tok_type pt = ps->lx->toks[probe].t;
                if (pt == T_WORD || pt == T_STR) probe++;
                else if (pt == T_LBRACE) { found = 1; break; }
                else break;
            }
            if (found) {
                sml_value *args = NULL;
                /* 收集参数 (最多几个, 简化: 存数组) */
                args = sml_new_array();
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
                    obj_set_dup(obj, key, sub);
                    continue;
                }
                sml_free(args);
            }
        }
        nt = peek(ps);
        if (nt->t == T_LBRACE) {
            next(ps);
            obj_set_dup(obj, key, parse_block(ps, T_RBRACE));
        } else if (nt->t == T_LBRACK) {
            next(ps);
            obj_set_dup(obj, key, parse_array(ps));
        } else if (nt->t == T_STR) {
            obj_set_dup(obj, key, sml_new_str(next(ps)->v));
        } else if (nt->t == T_WORD) {
            obj_set_dup(obj, key, coerce_word(next(ps)->v, ps));
        } else if (colon) {
            obj_set_dup(obj, key, sml_new_null());
        } else {
            /* key 本身即值 (片段引用/裸词) */
            obj_set_dup(obj, key, coerce_word(key, ps));
        }
    }
    return obj;
}

sml_value *sml_parse(const char *text, char *err, size_t errsz) {
    if (!text) {
        if (err && errsz) snprintf(err, errsz, "sml: null text");
        return NULL;
    }
    lexer lx;
    memset(&lx, 0, sizeof(lx));
    lx.errbuf = err;
    lx.errsz = errsz;
    lex_run(&lx, text);
    parser ps;
    memset(&ps, 0, sizeof(ps));
    ps.lx = &lx;
    ps.frags = NULL;
    /* 顶层支持三种形态，与 sml_dump 的输出对称：
    **   - `[ ... ]` 数组：dump 对非对象输出顶层数组（如「历史记录」这类对象数组）。
    **     此前顶层只认键值块，导致能序列化却读不回（"expected key"）。
    **   - `{ ... }` 顶层对象块
    **   - 键值块（传统形态）
    ** 注：顶层标量不可往返（SML 顶层需为容器），这是格式固有限制。 */
    sml_value *v;
    tok_type first = peek(&ps)->t;
    if (first == T_LBRACK) { next(&ps); v = parse_array(&ps); }
    else if (first == T_LBRACE) { next(&ps); v = parse_block(&ps, T_RBRACE); }
    else v = parse_block(&ps, T_EOF);
    /* 清理片段 (共享引用, 不 double free: 只释放链本身) */
    struct frag *f = ps.frags;
    while (f) { struct frag *nx = f->next; free(f->name); free(f); f = nx; }
    lex_free(&lx);
    if (!v) {
        if (err && errsz) snprintf(err, errsz, "sml: parse failed");
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
** 5. JSON 桥 (C-ABI 兼容 sml-rs: sml_parse_json / sml_dump_from_json)
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
                    default: buf[bl++] = *p; break;
                }
                p++;
            } else {
                buf[bl++] = *p++;
            }
            if (bl >= sizeof(buf) - 4) break;
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
