/*
** SPDX-License-Identifier: MulanPSL-2.0
** ambr.c — read-only `.ambr` decoder (T7 / N03).
** ambr.c —— `.ambr` 只读解码器（T7 / N03）。
**
** Layout follows docs/amberice/spec/format.md §19:
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


/* --- SM3 (GB/T 32905-2016)，按 manifest 的 hash_id=0x31 校验段摘要。 -------- */
typedef struct {
    unsigned int h[8];
    unsigned long long bitlen;
    unsigned char buf[64];
    size_t fill;
} sm3_t;

static const unsigned int SM3_IV[8] = {
    0x7380166fu, 0x4914b2b9u, 0x172442d7u, 0xda8a0600u,
    0xa96f30bcu, 0x163138aau, 0xe38dee4du, 0xb0fb0e4eu,
};

static unsigned int rotl32(unsigned int x, int n) {
    n &= 31;
    return n ? ((x << n) | (x >> (32 - n))) : x;
}

static unsigned int sm3_p0(unsigned int x) { return x ^ rotl32(x, 9) ^ rotl32(x, 17); }
static unsigned int sm3_p1(unsigned int x) { return x ^ rotl32(x, 15) ^ rotl32(x, 23); }

static void sm3_block(sm3_t *s, const unsigned char *p) {
    unsigned int w[68], w1[64];
    for (int j = 0; j < 16; j++) {
        w[j] = ((unsigned int)p[j * 4] << 24) | ((unsigned int)p[j * 4 + 1] << 16) |
               ((unsigned int)p[j * 4 + 2] << 8) | (unsigned int)p[j * 4 + 3];
    }
    for (int j = 16; j < 68; j++) {
        w[j] = sm3_p1(w[j - 16] ^ w[j - 9] ^ rotl32(w[j - 3], 15)) ^ rotl32(w[j - 13], 7) ^ w[j - 6];
    }
    for (int j = 0; j < 64; j++) w1[j] = w[j] ^ w[j + 4];

    unsigned int a = s->h[0], b = s->h[1], c = s->h[2], d = s->h[3];
    unsigned int e = s->h[4], f = s->h[5], g = s->h[6], h = s->h[7];
    for (int j = 0; j < 64; j++) {
        unsigned int tj = (j < 16) ? 0x79cc4519u : 0x7a879d8au;
        unsigned int ss1 = rotl32(rotl32(a, 12) + e + rotl32(tj, j), 7);
        unsigned int ss2 = ss1 ^ rotl32(a, 12);
        unsigned int ff = (j < 16) ? (a ^ b ^ c) : ((a & b) | (a & c) | (b & c));
        unsigned int gg = (j < 16) ? (e ^ f ^ g) : ((e & f) | (~e & g));
        unsigned int tt1 = ff + d + ss2 + w1[j];
        unsigned int tt2 = gg + h + ss1 + w[j];
        d = c;
        c = rotl32(b, 9);
        b = a;
        a = tt1;
        h = g;
        g = rotl32(f, 19);
        f = e;
        e = sm3_p0(tt2);
    }
    s->h[0] ^= a; s->h[1] ^= b; s->h[2] ^= c; s->h[3] ^= d;
    s->h[4] ^= e; s->h[5] ^= f; s->h[6] ^= g; s->h[7] ^= h;
}

static void sm3_init(sm3_t *s) {
    memcpy(s->h, SM3_IV, sizeof SM3_IV);
    s->bitlen = 0;
    s->fill = 0;
}

static void sm3_update(sm3_t *s, const unsigned char *p, size_t n) {
    s->bitlen += (unsigned long long)n * 8u;
    while (n--) {
        s->buf[s->fill++] = *p++;
        if (s->fill == 64) { sm3_block(s, s->buf); s->fill = 0; }
    }
}

static void sm3_final(sm3_t *s, unsigned char out[32]) {
    unsigned long long bits = s->bitlen;
    unsigned char pad = 0x80;
    sm3_update(s, &pad, 1);
    pad = 0;
    while (s->fill != 56) sm3_update(s, &pad, 1);
    unsigned char len8[8];
    for (int i = 0; i < 8; i++) len8[i] = (unsigned char)(bits >> (56 - i * 8));
    sm3_update(s, len8, 8);
    for (int i = 0; i < 8; i++) {
        out[i * 4] = (unsigned char)(s->h[i] >> 24);
        out[i * 4 + 1] = (unsigned char)(s->h[i] >> 16);
        out[i * 4 + 2] = (unsigned char)(s->h[i] >> 8);
        out[i * 4 + 3] = (unsigned char)(s->h[i]);
    }
}

static void sm3_digest(const unsigned char *p, size_t n, unsigned char out[32]) {
    sm3_t s;
    sm3_init(&s);
    sm3_update(&s, p, n);
    sm3_final(&s, out);
}

/* --- BLAKE3（无密钥 32 B 摘要），hash_id=0x32。与 rust/amberice/src/blake3.rs 同构。 -- */
#define B3_BLOCK_LEN 64
#define B3_CHUNK_LEN 1024
#define B3_MAX_DEPTH 54
#define B3_CHUNK_START (1u << 0)
#define B3_CHUNK_END   (1u << 1)
#define B3_PARENT      (1u << 2)
#define B3_ROOT        (1u << 3)

static const unsigned int B3_IV[8] = {
    0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
    0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u,
};

static const int B3_PERM[16] = {2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8};

static void b3_g(unsigned int s[16], int a, int b, int c, int d, unsigned int mx, unsigned int my) {
    s[a] = s[a] + s[b] + mx;
    s[d] = rotr32(s[d] ^ s[a], 16);
    s[c] = s[c] + s[d];
    s[b] = rotr32(s[b] ^ s[c], 12);
    s[a] = s[a] + s[b] + my;
    s[d] = rotr32(s[d] ^ s[a], 8);
    s[c] = s[c] + s[d];
    s[b] = rotr32(s[b] ^ s[c], 7);
}

static void b3_round(unsigned int s[16], const unsigned int m[16]) {
    b3_g(s, 0, 4, 8, 12, m[0], m[1]);
    b3_g(s, 1, 5, 9, 13, m[2], m[3]);
    b3_g(s, 2, 6, 10, 14, m[4], m[5]);
    b3_g(s, 3, 7, 11, 15, m[6], m[7]);
    b3_g(s, 0, 5, 10, 15, m[8], m[9]);
    b3_g(s, 1, 6, 11, 12, m[10], m[11]);
    b3_g(s, 2, 7, 8, 13, m[12], m[13]);
    b3_g(s, 3, 4, 9, 14, m[14], m[15]);
}

static void b3_permute(unsigned int m[16]) {
    unsigned int old[16];
    memcpy(old, m, sizeof old);
    for (int i = 0; i < 16; i++) m[i] = old[B3_PERM[i]];
}

static void b3_compress(const unsigned int cv[8], const unsigned int m_in[16],
                        unsigned long long counter, unsigned int block_len,
                        unsigned int flags, unsigned int out[16]) {
    unsigned int s[16], m[16];
    memcpy(s, cv, 8 * sizeof(unsigned int));
    s[8] = B3_IV[0]; s[9] = B3_IV[1]; s[10] = B3_IV[2]; s[11] = B3_IV[3];
    s[12] = (unsigned int)(counter & 0xffffffffu);
    s[13] = (unsigned int)(counter >> 32);
    s[14] = block_len;
    s[15] = flags;
    memcpy(m, m_in, sizeof m);
    for (int i = 0; i < 7; i++) { b3_round(s, m); b3_permute(m); }
    for (int i = 0; i < 8; i++) {
        out[i] = s[i] ^ s[i + 8];
        out[i + 8] = s[i + 8] ^ cv[i];
    }
}

static void b3_words(const unsigned char *buf, size_t len, unsigned int m[16]) {
    memset(m, 0, 16 * sizeof(unsigned int));
    size_t full = len / 4;
    for (size_t i = 0; i < full; i++) {
        m[i] = (unsigned int)buf[i * 4] | ((unsigned int)buf[i * 4 + 1] << 8) |
               ((unsigned int)buf[i * 4 + 2] << 16) | ((unsigned int)buf[i * 4 + 3] << 24);
    }
    size_t rem = len % 4;
    if (rem) {
        unsigned int w = 0;
        for (size_t i = 0; i < rem; i++) w |= (unsigned int)buf[full * 4 + i] << (8 * i);
        m[full] = w;
    }
}

typedef struct {
    unsigned int cv[8];
    unsigned int words[16];
    unsigned long long counter;
    unsigned int block_len;
    unsigned int flags;
} b3_output;

static void b3_output_cv(const b3_output *o, unsigned int cv[8]) {
    unsigned int s[16];
    b3_compress(o->cv, o->words, o->counter, o->block_len, o->flags, s);
    memcpy(cv, s, 8 * sizeof(unsigned int));
}

static void b3_output_root(const b3_output *o, unsigned char out[32]) {
    unsigned int s[16];
    b3_compress(o->cv, o->words, 0, o->block_len, o->flags | B3_ROOT, s);
    for (int i = 0; i < 8; i++) {
        out[i * 4] = (unsigned char)(s[i]);
        out[i * 4 + 1] = (unsigned char)(s[i] >> 8);
        out[i * 4 + 2] = (unsigned char)(s[i] >> 16);
        out[i * 4 + 3] = (unsigned char)(s[i] >> 24);
    }
}

static void b3_parent_output(const unsigned int left[8], const unsigned int right[8], b3_output *o) {
    memcpy(o->cv, B3_IV, sizeof B3_IV);
    memcpy(o->words, left, 8 * sizeof(unsigned int));
    memcpy(o->words + 8, right, 8 * sizeof(unsigned int));
    o->counter = 0;
    o->block_len = B3_BLOCK_LEN;
    o->flags = B3_PARENT;
}

static void b3_parent_cv(const unsigned int left[8], const unsigned int right[8], unsigned int cv[8]) {
    b3_output o;
    b3_parent_output(left, right, &o);
    b3_output_cv(&o, cv);
}

typedef struct {
    unsigned int cv[8];
    unsigned long long counter;
    unsigned char buf[B3_BLOCK_LEN];
    size_t buf_len;
    size_t blocks;
} b3_chunk;

static void b3_chunk_init(b3_chunk *c, unsigned long long counter) {
    memcpy(c->cv, B3_IV, sizeof B3_IV);
    c->counter = counter;
    c->buf_len = 0;
    c->blocks = 0;
}

static size_t b3_chunk_len(const b3_chunk *c) { return B3_BLOCK_LEN * c->blocks + c->buf_len; }

static unsigned int b3_chunk_start_flag(const b3_chunk *c) {
    return c->blocks == 0 ? B3_CHUNK_START : 0u;
}

static void b3_chunk_update(b3_chunk *c, const unsigned char *p, size_t n) {
    while (n) {
        if (c->buf_len == B3_BLOCK_LEN) {
            unsigned int words[16], s[16];
            b3_words(c->buf, B3_BLOCK_LEN, words);
            b3_compress(c->cv, words, c->counter, B3_BLOCK_LEN, b3_chunk_start_flag(c), s);
            memcpy(c->cv, s, 8 * sizeof(unsigned int));
            c->blocks++;
            c->buf_len = 0;
        }
        size_t take = B3_BLOCK_LEN - c->buf_len;
        if (take > n) take = n;
        memcpy(c->buf + c->buf_len, p, take);
        c->buf_len += take;
        p += take;
        n -= take;
    }
}

static void b3_chunk_output(const b3_chunk *c, b3_output *o) {
    memcpy(o->cv, c->cv, sizeof c->cv);
    b3_words(c->buf, c->buf_len, o->words);
    o->counter = c->counter;
    o->block_len = (unsigned int)c->buf_len;
    o->flags = b3_chunk_start_flag(c) | B3_CHUNK_END;
}

static void b3_digest(const unsigned char *data, size_t len, unsigned char out[32]) {
    b3_chunk chunk;
    b3_chunk_init(&chunk, 0);
    unsigned int stack[B3_MAX_DEPTH][8];
    size_t stack_len = 0;
    size_t at = 0;

    while (at < len) {
        if (b3_chunk_len(&chunk) == B3_CHUNK_LEN) {
            b3_output o;
            unsigned int cv[8];
            b3_chunk_output(&chunk, &o);
            b3_output_cv(&o, cv);
            unsigned long long total = chunk.counter + 1;
            /* push + 按「total 为偶数就合并」收敛 */
            memcpy(stack[stack_len++], cv, 8 * sizeof(unsigned int));
            while (total % 2 == 0 && stack_len >= 2) {
                unsigned int right[8], left[8];
                memcpy(right, stack[--stack_len], sizeof right);
                memcpy(left, stack[--stack_len], sizeof left);
                b3_parent_cv(left, right, stack[stack_len++]);
                total /= 2;
            }
            b3_chunk_init(&chunk, chunk.counter + 1);
        }
        size_t take = B3_CHUNK_LEN - b3_chunk_len(&chunk);
        if (take > len - at) take = len - at;
        b3_chunk_update(&chunk, data + at, take);
        at += take;
    }

    b3_output o;
    b3_chunk_output(&chunk, &o);
    for (size_t i = stack_len; i > 0; i--) {
        unsigned int right[8];
        b3_output_cv(&o, right);
        b3_parent_output(stack[i - 1], right, &o);
    }
    b3_output_root(&o, out);
}

/* 段摘要校验：算法由 manifest 的 `hash_id` 决定（§20），不再写死 SHA-256。
 * 返回 1 = 匹配，0 = 不符，-1 = 本端算不了（预留号 / 多级元组）⇒ 跳过并标注。
 * ｜ 1 = match, 0 = mismatch, -1 = cannot verify (reserved id) ⇒ skip explicitly. */
static int digest_check(int hash_id, const unsigned char *body, size_t len,
                        const unsigned char *want) {
    unsigned char got[32];
    if (hash_id == 0x30) {
        sha256_t s;
        sha256_init(&s);
        sha256_update(&s, body, len);
        sha256_final(&s, got);
    } else if (hash_id == 0x31) {
        sm3_digest(body, len, got);
    } else if (hash_id == 0x32) {
        b3_digest(body, len, got);
    } else {
        return -1; /* 预留号 / 多级元组：不猜算法 */
    }
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

/* Segment bodies we care about; `walk_segments` fills them. ｜ 关心的段体。 */
typedef struct {
    const unsigned char *value, *strtab, *typed, *schema;
    size_t value_len, strtab_len, typed_len, schema_len;
    int typed_opaque, schema_opaque;
    /* manifest 声明的摘要算法（§20）；digest_skipped = 本端算不了，已跳过校验。 */
    int hash_id;
    int digest_skipped;
} ambr_parts;

/* 从 MANIFEST(0x04) 段体读 `hash_id`。段体是**自包含**的 canonical 编码：
 * `varint count` + `count × (varint len ‖ utf8)` + 节点树。
 * 多级元组（数组形态）本端不实现：统一返回 0x3E（转义号），由调用方按「跳过」处理。 */
static int manifest_hash_id(const unsigned char *body, size_t len, int *out) {
    rd r = { body, len, 0 };
    unsigned long long count;
    if (!rd_varint(&r, &count)) return 0;
    if (count > (unsigned long long)len) return 0;
    strtab st = {0};
    for (unsigned long long i = 0; i < count; i++) {
        unsigned long long n;
        if (!rd_varint(&r, &n) || n > (unsigned long long)(r.len - r.pos)) {
            strtab_free(&st);
            return 0;
        }
        if (!strtab_push(&st, body + r.pos, (size_t)n)) {
            strtab_free(&st);
            return 0;
        }
        r.pos += (size_t)n;
    }
    char errbuf[128];
    sml_value *tree = decode_node(&r, &st, 0, errbuf, sizeof errbuf);
    strtab_free(&st);
    if (!tree) return 0;

    int hash = 0x30; /* 缺 hash_id ⇒ §21 默认 SHA-256 */
    sml_value *h = sml_obj_get(tree, "hash_id");
    if (h && h->type == SML_INT) {
        long long v = h->u.i;
        if (v < 0 || v > 255) { sml_free(tree); return 0; }
        hash = (int)v;
    } else if (h && h->type == SML_ARRAY) {
        hash = 0x3E; /* 多级元组：转义号，本端不实现 */
    } else if (h) {
        sml_free(tree);
        return 0;
    }
    sml_free(tree);
    *out = hash;
    return 1;
}

static int walk_segments(const unsigned char *buf, size_t len, ambr_parts *out,
                         char *err, size_t errsz) {
    if (err && errsz) err[0] = '\0';
    memset(out, 0, sizeof *out);
    if (!buf || len < 40) {
        set_err(err, errsz, "文件短于 40 B 头");
        return 0;
    }
    if (memcmp(buf, "AMBR", 4) != 0) {
        set_err(err, errsz, "不是 .ambr 文件（magic 不是 AMBR）");
        return 0;
    }
    unsigned fmt = buf[6];
    unsigned hdr_len = buf[7];
    rd h = { buf, len, 8 };
    unsigned long long table_off;
    unsigned long count;
    if (!rd_u64(&h, &table_off)) {
        set_err(err, errsz, "头被截断");
        return 0;
    }
    h.pos = 16;
    if (!rd_u32(&h, &count)) {
        set_err(err, errsz, "头被截断");
        return 0;
    }
    if (20 + 2 > len) {
        set_err(err, errsz, "头被截断");
        return 0;
    }
    unsigned flags = (unsigned)buf[20] | ((unsigned)buf[21] << 8);
    if (fmt != 0) {
        set_err(err, errsz, "不支持的字节布局版本");
        return 0;
    }
    if (hdr_len < 40 || table_off < hdr_len) {
        set_err(err, errsz, "头部长度或段表偏移非法");
        return 0;
    }
    size_t granule = (flags & 1u) ? ALIGN_BLOCK : ALIGN_MIN;
    size_t body_at = align_up((size_t)table_off + (size_t)count * SEG_HDR, granule);

    /* 引导顺序（与 Rust/JS 一致）：先扫一遍找 MANIFEST，解出 hash_id，
     * 再用它验其余段（连 manifest 自己一起验）。
     * ｜ Bootstrap: read hash_id from MANIFEST first, then verify with it. */
    out->hash_id = 0x30;
    {
        size_t scan = body_at;
        for (unsigned long i = 0; i < count; i++) {
            size_t o = (size_t)table_off + i * SEG_HDR;
            if (o + SEG_HDR > len) break;
            unsigned id = buf[o];
            unsigned comp = buf[o + 1], enc = buf[o + 2];
            unsigned long seg_len = (unsigned long)buf[o + 5] | ((unsigned long)buf[o + 6] << 8) |
                                    ((unsigned long)buf[o + 7] << 16) |
                                    ((unsigned long)buf[o + 8] << 24);
            if (scan + seg_len > len) break;
            if (id == 0x04 && comp == 0 && enc == 0) {
                int found = 0x30;
                if (manifest_hash_id(buf + scan, seg_len, &found)) out->hash_id = found;
                break;
            }
            scan = align_up(scan + seg_len, granule);
        }
    }

    for (unsigned long i = 0; i < count; i++) {
        size_t o = (size_t)table_off + i * SEG_HDR;
        if (o + SEG_HDR > len) {
            set_err(err, errsz, "段表越界");
            return 0;
        }
        unsigned id = buf[o];
        unsigned comp = buf[o + 1];
        unsigned enc = buf[o + 2];
        unsigned long seg_len = (unsigned long)buf[o + 5] | ((unsigned long)buf[o + 6] << 8) |
                                ((unsigned long)buf[o + 7] << 16) |
                                ((unsigned long)buf[o + 8] << 24);
        if (body_at + seg_len > len) {
            set_err(err, errsz, "段体长度越界");
            return 0;
        }
        if (id == SEG_VALUE || id == SEG_STRTAB) {
            if (comp != 0 || enc != 0) {
                set_err(err, errsz, "VALUE/STRTAB 压缩或加密过，本解码器不解");
                return 0;
            }
            int dc = digest_check(out->hash_id, buf + body_at, seg_len, buf + o + 9);
            if (dc == 0) {
                set_err(err, errsz, "段摘要不符（段体被改动？）");
                return 0;
            }
            if (dc < 0) out->digest_skipped = 1;
            if (id == SEG_VALUE) {
                out->value = buf + body_at;
                out->value_len = seg_len;
            } else {
                out->strtab = buf + body_at;
                out->strtab_len = seg_len;
            }
        } else if (id == 0x04) {
            if (comp == 0 && enc == 0) {
                int dm = digest_check(out->hash_id, buf + body_at, seg_len, buf + o + 9);
                if (dm == 0) {
                    set_err(err, errsz, "段摘要不符（段体被改动？）");
                    return 0;
                }
                if (dm < 0) out->digest_skipped = 1;
            }
        } else if (id == 0x70 || id == 0x71) {
            /* 可选旁路：压缩/加密形态按 Store 的口径忽略，不阻断 VALUE 读取。 */
            if (comp != 0 || enc != 0) {
                if (id == 0x70) out->typed_opaque = 1;
                else out->schema_opaque = 1;
            } else {
                int dc2 = digest_check(out->hash_id, buf + body_at, seg_len, buf + o + 9);
                if (dc2 == 0) {
                    set_err(err, errsz, "段摘要不符（段体被改动？）");
                    return 0;
                }
                if (dc2 < 0) out->digest_skipped = 1;
                if (id == 0x70) {
                    out->typed = buf + body_at;
                    out->typed_len = seg_len;
                } else {
                    out->schema = buf + body_at;
                    out->schema_len = seg_len;
                }
            }
        }
        body_at = align_up(body_at + seg_len, granule);
    }
    return 1;
}

sml_value *sml_ambr_load(const unsigned char *buf, size_t len, char *err, size_t errsz) {
    return sml_ambr_load_ex(buf, len, NULL, err, errsz);
}

sml_value *sml_ambr_load_ex(const unsigned char *buf, size_t len, sml_ambr_info *info,
                            char *err, size_t errsz) {
    ambr_parts p;
    if (!walk_segments(buf, len, &p, err, errsz)) return NULL;
    if (!p.value) {
        set_err(err, errsz, "找不到 VALUE 段");
        return NULL;
    }

    strtab st = { NULL, 0, 0 };
    if (p.strtab && !parse_strtab(p.strtab, p.strtab_len, &st)) {
        set_err(err, errsz, "STRTAB 解析失败");
        strtab_free(&st);
        return NULL;
    }
    rd r = { p.value, p.value_len, 0 };
    sml_value *v = decode_node(&r, &st, 0, err, errsz);
    if (v && r.pos != r.len) {
        set_err(err, errsz, "VALUE 段读完仍有剩余字节");
        sml_free(v);
        v = NULL;
    }
    strtab_free(&st);
    if (v && info) {
        info->hash_id = p.hash_id;
        info->digest_skipped = p.digest_skipped;
    }
    return v;
}

/* ------------------------------------------------------------------------- */
/* SCHEMA（0x71）+ TYPED（0x70）动态渲染（D45 / §25.8）                      */
/* ------------------------------------------------------------------------- */

typedef enum {
    T_BOOL, T_U8, T_U16, T_U32, T_U64, T_I8, T_I16, T_I32, T_I64,
    T_F32, T_F64, T_STR, T_BYTES, T_VEC, T_OPT, T_NAMED
} tref_kind;

typedef struct tref {
    tref_kind kind;
    unsigned long long named;
    struct tref *inner;
} tref;

typedef struct {
    char *name;
    tref *type;
} field_def;

typedef struct {
    unsigned long long id;
    char *name;
    field_def *fields;
    size_t nfields;
} type_def;

typedef struct {
    type_def *items;
    size_t len, cap;
} schema_def;

typedef struct {
    unsigned long long id, type_id;
    unsigned long len, off;
} typed_entry;

static char *dup_str(const char *s) {
    size_t n = strlen(s);
    char *p = (char *)malloc(n + 1);
    if (!p) return NULL;
    memcpy(p, s, n + 1);
    return p;
}

static int parse_type_id(const char *s, unsigned long long *out) {
    if (!s || strlen(s) != 16) return 0;
    unsigned long long v = 0;
    for (size_t i = 0; i < 16; i++) {
        char c = s[i];
        unsigned d;
        if (c >= '0' && c <= '9') d = (unsigned)(c - '0');
        else if (c >= 'a' && c <= 'f') d = (unsigned)(c - 'a' + 10);
        else return 0;
        v = (v << 4) | d;
    }
    *out = v;
    return 1;
}

static void tref_free(tref *t) {
    if (!t) return;
    tref_free(t->inner);
    free(t);
}

static int parse_ty(const sml_value *v, tref **out, char *err, size_t errsz);

static int parse_ty(const sml_value *v, tref **out, char *err, size_t errsz) {
    if (!v) {
        set_err(err, errsz, "SCHEMA 缺 ty");
        return 0;
    }
    tref *t = (tref *)calloc(1, sizeof *t);
    if (!t) {
        set_err(err, errsz, "内存不足");
        return 0;
    }
    if (v->type == SML_STR) {
        const char *s = v->u.s;
        if (!strcmp(s, "bool")) t->kind = T_BOOL;
        else if (!strcmp(s, "u8")) t->kind = T_U8;
        else if (!strcmp(s, "u16")) t->kind = T_U16;
        else if (!strcmp(s, "u32")) t->kind = T_U32;
        else if (!strcmp(s, "u64")) t->kind = T_U64;
        else if (!strcmp(s, "i8")) t->kind = T_I8;
        else if (!strcmp(s, "i16")) t->kind = T_I16;
        else if (!strcmp(s, "i32")) t->kind = T_I32;
        else if (!strcmp(s, "i64")) t->kind = T_I64;
        else if (!strcmp(s, "f32")) t->kind = T_F32;
        else if (!strcmp(s, "f64")) t->kind = T_F64;
        else if (!strcmp(s, "str")) t->kind = T_STR;
        else if (!strcmp(s, "bytes")) t->kind = T_BYTES;
        else {
            set_err(err, errsz, "SCHEMA 里有未知 ty");
            free(t);
            return 0;
        }
    } else if (v->type == SML_OBJECT) {
        sml_value *inner = sml_obj_get(v, "vec");
        if (inner) {
            t->kind = T_VEC;
            if (!parse_ty(inner, &t->inner, err, errsz)) { free(t); return 0; }
        } else if ((inner = sml_obj_get(v, "opt")) != NULL) {
            t->kind = T_OPT;
            if (!parse_ty(inner, &t->inner, err, errsz)) { free(t); return 0; }
        } else if ((inner = sml_obj_get(v, "type")) != NULL) {
            if (inner->type != SML_STR || !parse_type_id(inner->u.s, &t->named)) {
                set_err(err, errsz, "SCHEMA 的 type 引用非法");
                free(t);
                return 0;
            }
            t->kind = T_NAMED;
        } else {
            set_err(err, errsz, "SCHEMA 的 ty 对象缺 vec/opt/type");
            free(t);
            return 0;
        }
    } else {
        set_err(err, errsz, "SCHEMA 的 ty 必须是字符串或对象");
        free(t);
        return 0;
    }
    *out = t;
    return 1;
}

static void schema_free(schema_def *sd) {
    if (!sd) return;
    for (size_t i = 0; i < sd->len; i++) {
        free(sd->items[i].name);
        for (size_t j = 0; j < sd->items[i].nfields; j++) {
            free(sd->items[i].fields[j].name);
            tref_free(sd->items[i].fields[j].type);
        }
        free(sd->items[i].fields);
    }
    free(sd->items);
    sd->items = NULL;
    sd->len = sd->cap = 0;
}

static int schema_push(schema_def *sd, type_def td) {
    if (sd->len == sd->cap) {
        size_t cap = sd->cap ? sd->cap * 2 : 8;
        type_def *p = (type_def *)realloc(sd->items, cap * sizeof *p);
        if (!p) return 0;
        sd->items = p;
        sd->cap = cap;
    }
    sd->items[sd->len++] = td;
    return 1;
}

static int parse_schema(const sml_value *schema, schema_def *sd, char *err, size_t errsz) {
    if (schema->type != SML_OBJECT) {
        set_err(err, errsz, "SCHEMA 必须是对象");
        return 0;
    }
    sml_value *ver = sml_obj_get(schema, "schema_version");
    if (!ver || ver->type != SML_INT || ver->u.i != 1) {
        set_err(err, errsz, "schema_version 只支持 1");
        return 0;
    }
    sml_value *types = sml_obj_get(schema, "types");
    if (!types || types->type != SML_ARRAY) {
        set_err(err, errsz, "SCHEMA 缺 types");
        return 0;
    }
    for (size_t i = 0; i < types->u.arr.len; i++) {
        sml_value *tv = types->u.arr.items[i];
        if (!tv || tv->type != SML_OBJECT) {
            set_err(err, errsz, "SCHEMA 的 types[] 必须是对象");
            schema_free(sd);
            return 0;
        }
        sml_value *idv = sml_obj_get(tv, "id");
        sml_value *nv = sml_obj_get(tv, "name");
        sml_value *kv = sml_obj_get(tv, "kind");
        sml_value *fv = sml_obj_get(tv, "fields");
        unsigned long long id;
        if (!idv || idv->type != SML_STR || !parse_type_id(idv->u.s, &id)) {
            set_err(err, errsz, "SCHEMA 的 type id 非法");
            schema_free(sd);
            return 0;
        }
        if (!nv || nv->type != SML_STR) {
            set_err(err, errsz, "SCHEMA 的 type 缺 name");
            schema_free(sd);
            return 0;
        }
        if (kv && (kv->type != SML_STR || strcmp(kv->u.s, "struct") != 0)) {
            set_err(err, errsz, "v1 只支持 kind: struct");
            schema_free(sd);
            return 0;
        }
        if (!fv || fv->type != SML_ARRAY) {
            set_err(err, errsz, "SCHEMA 的 type 缺 fields");
            schema_free(sd);
            return 0;
        }
        for (size_t k = 0; k < sd->len; k++) {
            if (sd->items[k].id == id) {
                set_err(err, errsz, "SCHEMA 的 type id 重复");
                schema_free(sd);
                return 0;
            }
        }
        type_def td;
        td.id = id;
        td.name = dup_str(nv->u.s);
        td.fields = (field_def *)calloc(fv->u.arr.len, sizeof(field_def));
        td.nfields = fv->u.arr.len;
        if (!td.name || (fv->u.arr.len && !td.fields)) {
            free(td.name);
            free(td.fields);
            set_err(err, errsz, "内存不足");
            schema_free(sd);
            return 0;
        }
        long long last = -1;
        for (size_t j = 0; j < fv->u.arr.len; j++) {
            sml_value *fo = fv->u.arr.items[j];
            sml_value *fid = fo && fo->type == SML_OBJECT ? sml_obj_get(fo, "id") : NULL;
            sml_value *fn = fo && fo->type == SML_OBJECT ? sml_obj_get(fo, "name") : NULL;
            sml_value *ft = fo && fo->type == SML_OBJECT ? sml_obj_get(fo, "ty") : NULL;
            if (!fid || fid->type != SML_INT || fid->u.i <= last) {
                set_err(err, errsz, "SCHEMA 的 fields 必须按 id 严格升序");
                goto fail_td;
            }
            last = fid->u.i;
            if (!fn || fn->type != SML_STR) {
                set_err(err, errsz, "SCHEMA 的 field 缺 name");
                goto fail_td;
            }
            td.fields[j].name = dup_str(fn->u.s);
            if (!td.fields[j].name || !parse_ty(ft, &td.fields[j].type, err, errsz)) {
                goto fail_td;
            }
            continue;
        fail_td:
            free(td.name);
            for (size_t q = 0; q < td.nfields; q++) {
                free(td.fields[q].name);
                tref_free(td.fields[q].type);
            }
            free(td.fields);
            schema_free(sd);
            return 0;
        }
        if (!schema_push(sd, td)) {
            free(td.name);
            for (size_t q = 0; q < td.nfields; q++) {
                free(td.fields[q].name);
                tref_free(td.fields[q].type);
            }
            free(td.fields);
            set_err(err, errsz, "内存不足");
            schema_free(sd);
            return 0;
        }
    }
    return 1;
}

static const type_def *schema_find(const schema_def *sd, unsigned long long id) {
    for (size_t i = 0; i < sd->len; i++) {
        if (sd->items[i].id == id) return &sd->items[i];
    }
    return NULL;
}

static sml_value *decode_typed(rd *r, const tref *t, const schema_def *sd, int depth,
                               char *err, size_t errsz);

static sml_value *decode_struct(rd *r, const type_def *td, const schema_def *sd, int depth,
                                char *err, size_t errsz) {
    if (depth > MAX_DEPTH) {
        set_err(err, errsz, "TYPED 嵌套超过上限");
        return NULL;
    }
    sml_value *o = sml_new_object();
    if (!o) return NULL;
    for (size_t i = 0; i < td->nfields; i++) {
        sml_value *fv = decode_typed(r, td->fields[i].type, sd, depth + 1, err, errsz);
        if (!fv) {
            sml_free(o);
            return NULL;
        }
        sml_obj_set(o, td->fields[i].name, fv);
    }
    return o;
}

static int read_len_bytes(rd *r, const unsigned char **b, size_t *n) {
    unsigned long long len;
    if (!rd_varint(r, &len)) return 0;
    if (len > (unsigned long long)(r->len - r->pos)) return 0;
    *b = r->b + r->pos;
    *n = (size_t)len;
    r->pos += (size_t)len;
    return 1;
}

static sml_value *bytes_hex(const unsigned char *b, size_t n) {
    static const char *H = "0123456789abcdef";
    char *s = (char *)malloc(n * 2 + 1);
    if (!s) return NULL;
    for (size_t i = 0; i < n; i++) {
        s[i * 2] = H[b[i] >> 4];
        s[i * 2 + 1] = H[b[i] & 15];
    }
    s[n * 2] = '\0';
    sml_value *v = sml_new_str(s);
    free(s);
    return v;
}

static sml_value *decode_typed(rd *r, const tref *t, const schema_def *sd, int depth,
                               char *err, size_t errsz) {
    if (depth > MAX_DEPTH) {
        set_err(err, errsz, "TYPED 嵌套超过上限");
        return NULL;
    }
    unsigned long long z;
    switch (t->kind) {
    case T_BOOL:
        if (!rd_varint(r, &z) || z > 1) {
            set_err(err, errsz, "布尔的 tag 只能是 0/1");
            return NULL;
        }
        return sml_new_bool((int)z);
    case T_U8:
    case T_U16:
    case T_U32: {
        unsigned long long max = t->kind == T_U8 ? 255ULL : t->kind == T_U16 ? 65535ULL : 4294967295ULL;
        if (!rd_varint(r, &z) || z > max) {
            set_err(err, errsz, "整数超出目标类型范围");
            return NULL;
        }
        return sml_new_int((long long)z);
    }
    case T_U64:
        if (!rd_varint(r, &z)) {
            set_err(err, errsz, "TYPED 载荷截断");
            return NULL;
        }
        if (z > 9223372036854775807ULL) {
            set_err(err, errsz, "u64 超出 i64 范围，无法用 Int 表示");
            return NULL;
        }
        return sml_new_int((long long)z);
    case T_I8:
    case T_I16:
    case T_I32:
    case T_I64: {
        if (!rd_varint(r, &z)) {
            set_err(err, errsz, "TYPED 载荷截断");
            return NULL;
        }
        long long n = (long long)(z >> 1) ^ -(long long)(z & 1);
        long long lim = t->kind == T_I8 ? 127LL : t->kind == T_I16 ? 32767LL
                       : t->kind == T_I32 ? 2147483647LL : 9223372036854775807LL;
        if (n > lim || n < -lim - 1) {
            set_err(err, errsz, "整数超出目标类型范围");
            return NULL;
        }
        return sml_new_int(n);
    }
    case T_F32: {
        if (r->pos + 4 > r->len) {
            set_err(err, errsz, "TYPED 载荷截断");
            return NULL;
        }
        float f;
        memcpy(&f, r->b + r->pos, 4);
        r->pos += 4;
        return sml_new_float((double)f);
    }
    case T_F64: {
        if (r->pos + 8 > r->len) {
            set_err(err, errsz, "TYPED 载荷截断");
            return NULL;
        }
        double d;
        memcpy(&d, r->b + r->pos, 8);
        r->pos += 8;
        return sml_new_float(d);
    }
    case T_STR: {
        const unsigned char *b;
        size_t n;
        if (!read_len_bytes(r, &b, &n)) {
            set_err(err, errsz, "字符串长度越界");
            return NULL;
        }
        return sml_new_strn((const char *)b, n);
    }
    case T_BYTES: {
        const unsigned char *b;
        size_t n;
        if (!read_len_bytes(r, &b, &n)) {
            set_err(err, errsz, "字节长度越界");
            return NULL;
        }
        return bytes_hex(b, n);
    }
    case T_VEC: {
        if (!rd_varint(r, &z) || z > MAX_ELEMS) {
            set_err(err, errsz, "数组元素数越界");
            return NULL;
        }
        sml_value *a = sml_new_array();
        if (!a) return NULL;
        for (unsigned long long i = 0; i < z; i++) {
            sml_value *item = decode_typed(r, t->inner, sd, depth + 1, err, errsz);
            if (!item) {
                sml_free(a);
                return NULL;
            }
            sml_arr_push(a, item);
        }
        return a;
    }
    case T_OPT: {
        if (!rd_varint(r, &z) || z > 1) {
            set_err(err, errsz, "Option 的 tag 只能是 0/1");
            return NULL;
        }
        if (z == 0) return sml_new_null();
        return decode_typed(r, t->inner, sd, depth + 1, err, errsz);
    }
    case T_NAMED: {
        const type_def *td = schema_find(sd, t->named);
        if (!td) {
            set_err(err, errsz, "SCHEMA 里没有这个 type_id");
            return NULL;
        }
        return decode_struct(r, td, sd, depth + 1, err, errsz);
    }
    }
    set_err(err, errsz, "未知 ty");
    return NULL;
}

static int parse_l0_strings(rd *r, strtab *st, char *err, size_t errsz) {
    unsigned long long count;
    if (!rd_varint(r, &count) || count > MAX_ELEMS) {
        set_err(err, errsz, "L0 字符串个数非法");
        return 0;
    }
    for (unsigned long long i = 0; i < count; i++) {
        const unsigned char *b;
        size_t n;
        if (!read_len_bytes(r, &b, &n)) {
            set_err(err, errsz, "L0 字符串长度越界");
            return 0;
        }
        if (!strtab_push(st, b, n)) {
            set_err(err, errsz, "内存不足");
            return 0;
        }
    }
    return 1;
}

sml_value *sml_ambr_load_typed(const unsigned char *buf, size_t len, char *err, size_t errsz) {
    ambr_parts p;
    if (!walk_segments(buf, len, &p, err, errsz)) return NULL;
    if (p.typed_opaque || p.schema_opaque) {
        set_err(err, errsz, "TYPED/SCHEMA 压缩或加密过，本解码器不解");
        return NULL;
    }
    if (!p.typed) return sml_new_array();
    if (!p.schema) {
        set_err(err, errsz, "没有 SCHEMA 段（0x71），无法按名渲染 TYPED");
        return NULL;
    }

    rd sr = { p.schema, p.schema_len, 0 };
    strtab sst = { NULL, 0, 0 };
    if (!parse_l0_strings(&sr, &sst, err, errsz)) {
        strtab_free(&sst);
        return NULL;
    }
    sml_value *schema = decode_node(&sr, &sst, 0, err, errsz);
    strtab_free(&sst);
    if (!schema) return NULL;
    if (sr.pos != sr.len) {
        set_err(err, errsz, "SCHEMA 段读完仍有剩余字节");
        sml_free(schema);
        return NULL;
    }

    schema_def sd = { NULL, 0, 0 };
    if (!parse_schema(schema, &sd, err, errsz)) {
        sml_free(schema);
        return NULL;
    }
    sml_free(schema);

    rd tr = { p.typed, p.typed_len, 0 };
    unsigned long long count;
    if (!rd_varint(&tr, &count) || count > MAX_ELEMS) {
        set_err(err, errsz, "TYPED 条目数非法");
        schema_free(&sd);
        return NULL;
    }
    typed_entry *ents = count ? (typed_entry *)calloc((size_t)count, sizeof *ents) : NULL;
    if (count && !ents) {
        set_err(err, errsz, "内存不足");
        schema_free(&sd);
        return NULL;
    }
    for (unsigned long long i = 0; i < count; i++) {
        unsigned long long id, type_id;
        unsigned long off, elen;
        if (!rd_varint(&tr, &id) || id == 0 || id > 4294967295ULL ||
            !rd_u64(&tr, &type_id) || !rd_u32(&tr, &off) || !rd_u32(&tr, &elen)) {
            set_err(err, errsz, "TYPED 目录被截断");
            free(ents);
            schema_free(&sd);
            return NULL;
        }
        ents[i].id = id;
        ents[i].type_id = type_id;
        ents[i].off = off;
        ents[i].len = elen;
    }
    size_t base = tr.pos;

    sml_value *out = sml_new_array();
    if (!out) {
        free(ents);
        schema_free(&sd);
        return NULL;
    }
    for (unsigned long long i = 0; i < count; i++) {
        const type_def *td = schema_find(&sd, ents[i].type_id);
        if (!td) {
            set_err(err, errsz, "SCHEMA 里没有这个 type_id");
            sml_free(out);
            free(ents);
            schema_free(&sd);
            return NULL;
        }
        if ((size_t)ents[i].off + ents[i].len > p.typed_len - base) {
            set_err(err, errsz, "TYPED 载荷越界");
            sml_free(out);
            free(ents);
            schema_free(&sd);
            return NULL;
        }
        rd er = { p.typed + base + ents[i].off, ents[i].len, 0 };
        sml_value *val = decode_struct(&er, td, &sd, 0, err, errsz);
        if (!val) {
            sml_free(out);
            free(ents);
            schema_free(&sd);
            return NULL;
        }
        if (er.pos != er.len) {
            set_err(err, errsz, "TYPED 载荷解完仍有剩余字节");
            sml_free(val);
            sml_free(out);
            free(ents);
            schema_free(&sd);
            return NULL;
        }
        sml_value *item = sml_new_object();
        if (!item) {
            sml_free(val);
            sml_free(out);
            free(ents);
            schema_free(&sd);
            return NULL;
        }
        sml_obj_set(item, "id", sml_new_int((long long)ents[i].id));
        sml_obj_set(item, "name", sml_new_str(td->name));
        sml_obj_set(item, "value", val);
        sml_arr_push(out, item);
    }
    free(ents);
    schema_free(&sd);
    return out;
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
