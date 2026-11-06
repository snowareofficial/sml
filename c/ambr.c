/*
** SPDX-License-Identifier: MulanPSL-2.0
** ambr.c — read-only `.ambr` decoder (T7 / N03).
** ambr.c —— `.ambr` 只读解码器（T7 / N03）。
**
** Layout follows docs/amberice-smldb-spec.md §19:
**   header 40 B: magic[4] edition:u16 fmt:u8 hdr_len:u8 table_off:u64 count:u32 flags:u16 …
**   entry 41 B : id:u8 comp:u8 enc:u8 flags:u16 len:u32 sha256[32]
**   bodies      : aligned to 8 B (flags bit 0 = 0) or 4 KiB (bit 0 = 1), walked as
**                 align_up(prev + prev.len)
**   VALUE node  : tag:u8 flags:u8 subtree_len:u32 payload…
**   STRTAB body : (len:varint ‖ utf8)* — no count prefix
*/
#include "ambr.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SEG_VALUE  0x01
#define SEG_STRTAB 0x02
#define SEG_HDR    41
#define ALIGN_MIN  8
#define ALIGN_BLOCK 4096
#define MAX_DEPTH  128
#define MAX_ELEMS  1000000

typedef struct {
    const unsigned char *b;
    size_t len;
    size_t pos;
} rd;

typedef struct {
    char **items;
    size_t len, cap;
} strtab;

static void set_err(char *err, size_t errsz, const char *msg) {
    if (!err || errsz == 0) return;
    snprintf(err, errsz, "AMBR: %s", msg);
}

static size_t align_up(size_t x, size_t granule) {
    size_t a = granule ? granule : 1;
    return (x + a - 1) / a * a;
}

static int rd_u8(rd *r, unsigned char *out) {
    if (r->pos >= r->len) return 0;
    *out = r->b[r->pos++];
    return 1;
}

static int rd_u32(rd *r, unsigned long *out) {
    if (r->pos + 4 > r->len) return 0;
    *out = (unsigned long)r->b[r->pos] | ((unsigned long)r->b[r->pos + 1] << 8) |
           ((unsigned long)r->b[r->pos + 2] << 16) | ((unsigned long)r->b[r->pos + 3] << 24);
    r->pos += 4;
    return 1;
}

static int rd_u64(rd *r, unsigned long long *out) {
    unsigned long lo, hi;
    if (!rd_u32(r, &lo) || !rd_u32(r, &hi)) return 0;
    *out = ((unsigned long long)hi << 32) | lo;
    return 1;
}

static int rd_varint(rd *r, unsigned long long *out) {
    unsigned long long v = 0;
    int shift = 0;
    for (int i = 0; i < 10; i++) {
        unsigned char byte;
        if (!rd_u8(r, &byte)) return 0;
        v |= (unsigned long long)(byte & 0x7f) << shift;
        if ((byte & 0x80) == 0) {
            *out = v;
            return 1;
        }
        shift += 7;
    }
    return 0;
}

static void strtab_free(strtab *t) {
    if (!t) return;
    for (size_t i = 0; i < t->len; i++) free(t->items[i]);
    free(t->items);
    t->items = NULL;
    t->len = t->cap = 0;
}

static int strtab_push(strtab *t, const unsigned char *s, size_t n) {
    if (t->len == t->cap) {
        size_t cap = t->cap ? t->cap * 2 : 16;
        char **items = (char **)realloc(t->items, cap * sizeof(char *));
        if (!items) return 0;
        t->items = items;
        t->cap = cap;
    }
    char *copy = (char *)malloc(n + 1);
    if (!copy) return 0;
    memcpy(copy, s, n);
    copy[n] = '\0';
    t->items[t->len++] = copy;
    return 1;
}

static int parse_strtab(const unsigned char *body, size_t len, strtab *out) {
    rd r = { body, len, 0 };
    while (r.pos < r.len) {
        unsigned long long n;
        if (!rd_varint(&r, &n)) return 0;
        if (n > r.len - r.pos) return 0;
        if (!strtab_push(out, body + r.pos, (size_t)n)) return 0;
        r.pos += (size_t)n;
    }
    return 1;
}

/* --- SHA-256 (FIPS 180-4), only used to verify segment digests. ｜ 仅用于校验段摘要。 --- */
typedef struct {
    unsigned int h[8];
    unsigned char buf[64];
    size_t fill;
    unsigned long long bits;
} sha256_t;

static const unsigned int SHA_K[64] = {
    0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u, 0x3956c25bu, 0x59f111f1u, 0x923f82a4u, 0xab1c5ed5u,
    0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u, 0x72be5d74u, 0x80deb1feu, 0x9bdc06a7u, 0xc19bf174u,
    0xe49b69c1u, 0xefbe4786u, 0x0fc19dc6u, 0x240ca1ccu, 0x2de92c6fu, 0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau,
    0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u, 0xc6e00bf3u, 0xd5a79147u, 0x06ca6351u, 0x14292967u,
    0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcu, 0x53380d13u, 0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u,
    0xa2bfe8a1u, 0xa81a664bu, 0xc24b8b70u, 0xc76c51a3u, 0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u,
    0x19a4c116u, 0x1e376c08u, 0x2748774cu, 0x34b0bcb5u, 0x391c0cb3u, 0x4ed8aa4au, 0x5b9cca4fu, 0x682e6ff3u,
    0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u, 0x90befffau, 0xa4506cebu, 0xbef9a3f7u, 0xc67178f2u,
};

static unsigned int rotr32(unsigned int x, int n) { return (x >> n) | (x << (32 - n)); }

static void sha256_init(sha256_t *s) {
    static const unsigned int H0[8] = {0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
                                       0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u};
    memcpy(s->h, H0, sizeof H0);
    s->fill = 0;
    s->bits = 0;
}

static void sha256_block(sha256_t *s, const unsigned char *p) {
    unsigned int w[64], a, b, c, d, e, f, g, h;
    for (int i = 0; i < 16; i++) {
        w[i] = ((unsigned)p[i * 4] << 24) | ((unsigned)p[i * 4 + 1] << 16) |
               ((unsigned)p[i * 4 + 2] << 8) | (unsigned)p[i * 4 + 3];
    }
    for (int i = 16; i < 64; i++) {
        unsigned int s0 = rotr32(w[i - 15], 7) ^ rotr32(w[i - 15], 18) ^ (w[i - 15] >> 3);
        unsigned int s1 = rotr32(w[i - 2], 17) ^ rotr32(w[i - 2], 19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16] + s0 + w[i - 7] + s1;
    }
    a = s->h[0]; b = s->h[1]; c = s->h[2]; d = s->h[3];
    e = s->h[4]; f = s->h[5]; g = s->h[6]; h = s->h[7];
    for (int i = 0; i < 64; i++) {
        unsigned int t1 = h + (rotr32(e, 6) ^ rotr32(e, 11) ^ rotr32(e, 25)) +
                          ((e & f) ^ (~e & g)) + SHA_K[i] + w[i];
        unsigned int t2 = (rotr32(a, 2) ^ rotr32(a, 13) ^ rotr32(a, 22)) +
                          ((a & b) ^ (a & c) ^ (b & c));
        h = g; g = f; f = e; e = d + t1;
        d = c; c = b; b = a; a = t1 + t2;
    }
    s->h[0] += a; s->h[1] += b; s->h[2] += c; s->h[3] += d;
    s->h[4] += e; s->h[5] += f; s->h[6] += g; s->h[7] += h;
}

static void sha256_update(sha256_t *s, const unsigned char *p, size_t n) {
    s->bits += (unsigned long long)n * 8u;
    while (n > 0) {
        size_t take = 64 - s->fill;
        if (take > n) take = n;
        memcpy(s->buf + s->fill, p, take);
        s->fill += take; p += take; n -= take;
        if (s->fill == 64) { sha256_block(s, s->buf); s->fill = 0; }
    }
}

static void sha256_final(sha256_t *s, unsigned char out[32]) {
    unsigned long long bits = s->bits;
    unsigned char pad = 0x80;
    sha256_update(s, &pad, 1);
    pad = 0;
    while (s->fill != 56) sha256_update(s, &pad, 1);
    for (int i = 7; i >= 0; i--) {
        unsigned char byte = (unsigned char)(bits >> (i * 8));
        sha256_update(s, &byte, 1);
    }
    for (int i = 0; i < 8; i++) {
        out[i * 4] = (unsigned char)(s->h[i] >> 24);
        out[i * 4 + 1] = (unsigned char)(s->h[i] >> 16);
        out[i * 4 + 2] = (unsigned char)(s->h[i] >> 8);
        out[i * 4 + 3] = (unsigned char)s->h[i];
    }
}

/* 1 = digest matches. ｜ 摘要一致返回 1。 */
static int digest_ok(const unsigned char *body, size_t len, const unsigned char *want) {
    sha256_t s;
    unsigned char got[32];
    sha256_init(&s);
    sha256_update(&s, body, len);
    sha256_final(&s, got);
    return memcmp(got, want, 32) == 0;
}

/* Forward declaration: arrays and objects recurse. ｜ 数组/对象递归解码。 */
static sml_value *decode_node(rd *r, const strtab *st, int depth, char *err, size_t errsz);

static sml_value *decode_node(rd *r, const strtab *st, int depth, char *err, size_t errsz) {
    if (depth > MAX_DEPTH) {
        set_err(err, errsz, "嵌套超过上限");
        return NULL;
    }
    size_t start = r->pos;
    unsigned char tag, flags;
    unsigned long sub;
    if (!rd_u8(r, &tag) || !rd_u8(r, &flags) || !rd_u32(r, &sub)) {
        set_err(err, errsz, "节点头越界");
        return NULL;
    }
    (void)flags; /* bit 0 = "has __type": an optimisation hint only ｜ 仅加速提示 */
    size_t payload = r->pos;
    sml_value *v = NULL;

    switch (tag) {
    case 0: /* Null */
        v = sml_new_null();
        break;
    case 1:
        v = sml_new_bool(0);
        break;
    case 2:
        v = sml_new_bool(1);
        break;
    case 3: { /* Int: zigzag varint */
        unsigned long long z;
        if (!rd_varint(r, &z)) break;
        long long n = (long long)(z >> 1) ^ -(long long)(z & 1);
        v = sml_new_int(n);
        break;
    }
    case 4: { /* Float: f64 LE */
        if (r->pos + 8 > r->len) break;
        double d;
        memcpy(&d, r->b + r->pos, 8); /* host is little-endian on every target we ship */
        r->pos += 8;
        v = sml_new_float(d);
        break;
    }
    case 5: { /* Str: STRTAB id */
        unsigned long long id;
        if (!rd_varint(r, &id)) break;
        if (id >= st->len) {
            set_err(err, errsz, "字符串 id 越界");
            return NULL;
        }
        v = sml_new_str(st->items[id]);
        break;
    }
    case 6: { /* Arr */
        unsigned long long count;
        if (!rd_varint(r, &count)) break;
        if (count > MAX_ELEMS) {
            set_err(err, errsz, "数组元素数超过上限");
            return NULL;
        }
        v = sml_new_array();
        if (!v) return NULL;
        for (unsigned long long i = 0; i < count; i++) {
            sml_value *item = decode_node(r, st, depth + 1, err, errsz);
            if (!item) {
                sml_free(v);
                return NULL;
            }
            sml_arr_push(v, item);
        }
        break;
    }
    case 7: { /* Obj */
        unsigned long long count;
        if (!rd_varint(r, &count)) break;
        if (count > MAX_ELEMS) {
            set_err(err, errsz, "对象字段数超过上限");
            return NULL;
        }
        v = sml_new_object();
        if (!v) return NULL;
        for (unsigned long long i = 0; i < count; i++) {
            unsigned long long key_id;
            if (!rd_varint(r, &key_id) || key_id >= st->len) {
                set_err(err, errsz, "键 id 越界");
                sml_free(v);
                return NULL;
            }
            /* D15: duplicate keys are rejected, not last-wins ｜ 重复键拒绝，不做 last-wins */
            if (sml_obj_get(v, st->items[key_id]) != NULL) {
                set_err(err, errsz, "对象里出现重复键");
                sml_free(v);
                return NULL;
            }
            sml_value *child = decode_node(r, st, depth + 1, err, errsz);
            if (!child) {
                sml_free(v);
                return NULL;
            }
            sml_obj_set(v, st->items[key_id], child);
        }
        break;
    }
    default:
        set_err(err, errsz, "未定义的节点 tag");
        return NULL;
    }

    if (!v) {
        set_err(err, errsz, "解码失败（截断或内存不足）");
        return NULL;
    }
    size_t consumed = r->pos - payload;
    if (consumed != (size_t)sub) {
        set_err(err, errsz, "subtree_len 与实际消费不符");
        sml_free(v);
        return NULL;
    }
    (void)start;
    return v;
}

sml_value *sml_ambr_load(const unsigned char *buf, size_t len, char *err, size_t errsz) {
    if (err && errsz) err[0] = '\0';
    if (!buf || len < 40) {
        set_err(err, errsz, "文件短于 40 B 头");
        return NULL;
    }
    if (memcmp(buf, "AMBR", 4) != 0) {
        set_err(err, errsz, "不是 .ambr 文件（magic 不是 AMBR）");
        return NULL;
    }
    unsigned fmt = buf[6];
    unsigned hdr_len = buf[7];
    rd h = { buf, len, 8 };
    unsigned long long table_off;
    unsigned long count;
    unsigned flags;
    if (!rd_u64(&h, &table_off)) {
        set_err(err, errsz, "头被截断");
        return NULL;
    }
    h.pos = 16;
    if (!rd_u32(&h, &count)) {
        set_err(err, errsz, "头被截断");
        return NULL;
    }
    h.pos = 20;
    if (h.pos + 2 > len) {
        set_err(err, errsz, "头被截断");
        return NULL;
    }
    flags = (unsigned)buf[20] | ((unsigned)buf[21] << 8);
    if (fmt != 0) {
        set_err(err, errsz, "不支持的字节布局版本");
        return NULL;
    }
    if (hdr_len < 40 || table_off < hdr_len) {
        set_err(err, errsz, "头部长度或段表偏移非法");
        return NULL;
    }
    size_t granule = (flags & 1u) ? ALIGN_BLOCK : ALIGN_MIN;
    size_t body_at = align_up((size_t)table_off + (size_t)count * SEG_HDR, granule);

    const unsigned char *value = NULL, *strtab_body = NULL;
    size_t value_len = 0, strtab_len = 0;
    for (unsigned long i = 0; i < count; i++) {
        size_t o = (size_t)table_off + i * SEG_HDR;
        if (o + SEG_HDR > len) {
            set_err(err, errsz, "段表越界");
            return NULL;
        }
        unsigned id = buf[o];
        unsigned comp = buf[o + 1];
        unsigned enc = buf[o + 2];
        unsigned long seg_len = (unsigned long)buf[o + 5] | ((unsigned long)buf[o + 6] << 8) |
                                ((unsigned long)buf[o + 7] << 16) |
                                ((unsigned long)buf[o + 8] << 24);
        if (body_at + seg_len > len) {
            set_err(err, errsz, "段体长度越界");
            return NULL;
        }
        if (id == SEG_VALUE || id == SEG_STRTAB) {
            if (comp != 0 || enc != 0) {
                set_err(err, errsz, "VALUE/STRTAB 压缩或加密过，本解码器不解");
                return NULL;
            }
            if (!digest_ok(buf + body_at, seg_len, buf + o + 9)) {
                set_err(err, errsz, "段摘要不符（段体被改动？）");
                return NULL;
            }
            if (id == SEG_VALUE) {
                value = buf + body_at;
                value_len = seg_len;
            } else {
                strtab_body = buf + body_at;
                strtab_len = seg_len;
            }
        }
        body_at = align_up(body_at + seg_len, granule);
    }
    if (!value) {
        set_err(err, errsz, "找不到 VALUE 段");
        return NULL;
    }

    strtab st = { NULL, 0, 0 };
    if (strtab_body && !parse_strtab(strtab_body, strtab_len, &st)) {
        set_err(err, errsz, "STRTAB 解析失败");
        strtab_free(&st);
        return NULL;
    }
    rd r = { value, value_len, 0 };
    sml_value *v = decode_node(&r, &st, 0, err, errsz);
    if (v && r.pos != r.len) {
        set_err(err, errsz, "VALUE 段读完仍有剩余字节");
        sml_free(v);
        v = NULL;
    }
    strtab_free(&st);
    return v;
}

sml_value *sml_ambr_load_file(const char *path, char *err, size_t errsz) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        set_err(err, errsz, "打不开文件");
        return NULL;
    }
    if (fseek(f, 0, SEEK_END) != 0) {
        fclose(f);
        set_err(err, errsz, "定位失败");
        return NULL;
    }
    long n = ftell(f);
    if (n < 0) {
        fclose(f);
        set_err(err, errsz, "取文件长度失败");
        return NULL;
    }
    rewind(f);
    unsigned char *buf = (unsigned char *)malloc((size_t)n);
    if (!buf) {
        fclose(f);
        set_err(err, errsz, "内存不足");
        return NULL;
    }
    size_t got = fread(buf, 1, (size_t)n, f);
    fclose(f);
    if (got != (size_t)n) {
        free(buf);
        set_err(err, errsz, "读文件失败");
        return NULL;
    }
    sml_value *v = sml_ambr_load(buf, (size_t)n, err, errsz);
    free(buf);
    return v;
}
