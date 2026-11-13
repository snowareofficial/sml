// SPDX-License-Identifier: MulanPSL-2.0
// ambr.cpp — read-only `.ambr` decoder (T7 / N03).
//
// Layout: spec §19 — header 40 B, entry 41 B (id/comp/enc/flags/len/sha256),
// segment bodies aligned to 8 B or 4 KiB, VALUE nodes tag/flags/subtree_len/payload,
// STRTAB body = (len:varint ‖ utf8)*.
#include "ambr.hpp"

#include <cstdio>
#include <cstring>
#include <vector>

namespace sml {
namespace {

constexpr unsigned char SEG_VALUE = 0x01;
constexpr unsigned char SEG_STRTAB = 0x02;
constexpr size_t SEG_HDR = 41;
constexpr size_t ALIGN_MIN = 8;
constexpr size_t ALIGN_BLOCK = 4096;
constexpr int MAX_DEPTH = 128;
constexpr unsigned long long MAX_ELEMS = 1000000;

struct Reader {
    const unsigned char* b;
    size_t len;
    size_t pos = 0;

    bool u8(unsigned char* out) {
        if (pos >= len) return false;
        *out = b[pos++];
        return true;
    }
    bool u32(unsigned long* out) {
        if (pos + 4 > len) return false;
        *out = (unsigned long)b[pos] | ((unsigned long)b[pos + 1] << 8) |
               ((unsigned long)b[pos + 2] << 16) | ((unsigned long)b[pos + 3] << 24);
        pos += 4;
        return true;
    }
    bool u64(unsigned long long* out) {
        unsigned long lo, hi;
        if (!u32(&lo) || !u32(&hi)) return false;
        *out = ((unsigned long long)hi << 32) | lo;
        return true;
    }
    bool varint(unsigned long long* out) {
        unsigned long long v = 0;
        int shift = 0;
        for (int i = 0; i < 10; i++) {
            unsigned char byte;
            if (!u8(&byte)) return false;
            v |= (unsigned long long)(byte & 0x7f) << shift;
            if ((byte & 0x80) == 0) {
                *out = v;
                return true;
            }
            shift += 7;
        }
        return false;
    }
};

size_t align_up(size_t x, size_t granule) {
    size_t a = granule ? granule : 1;
    return (x + a - 1) / a * a;
}

void set_err(std::string* err, const char* msg) {
    if (err) *err = std::string("AMBR: ") + msg;
}

using StrTab = std::vector<std::string>;

bool parse_strtab(const unsigned char* body, size_t len, StrTab* out) {
    Reader r{body, len};
    while (r.pos < r.len) {
        unsigned long long n;
        if (!r.varint(&n)) return false;
        if (n > r.len - r.pos) return false;
        out->emplace_back(reinterpret_cast<const char*>(body + r.pos), (size_t)n);
        r.pos += (size_t)n;
    }
    return true;
}



constexpr unsigned int AMBR_SHA_K[64] = {
    0x428a2f98u, 0x71374491u, 0xb5c0fbcFu, 0xe9b5dba5u, 0x3956c25bu, 0x59f111f1u, 0x923f82a4u, 0xab1c5ed5u,
    0xd807aa98u, 0x12835b01u, 0x243185beu, 0x550c7dc3u, 0x72be5d74u, 0x80deb1feu, 0x9bdc06a7u, 0xc19bf174u,
    0xe49b69c1u, 0xefbe4786u, 0x0fc19dc6u, 0x240ca1ccu, 0x2de92c6fu, 0x4a7484aau, 0x5cb0a9dcu, 0x76f988dau,
    0x983e5152u, 0xa831c66du, 0xb00327c8u, 0xbf597fc7u, 0xc6e00bf3u, 0xd5a79147u, 0x06ca6351u, 0x14292967u,
    0x27b70a85u, 0x2e1b2138u, 0x4d2c6dfcu, 0x53380d13u, 0x650a7354u, 0x766a0abbu, 0x81c2c92eu, 0x92722c85u,
    0xa2bfe8a1u, 0xa81a664bu, 0xc24b8b70u, 0xc76c51a3u, 0xd192e819u, 0xd6990624u, 0xf40e3585u, 0x106aa070u,
    0x19a4c116u, 0x1e376c08u, 0x2748774cu, 0x34b0bcb5u, 0x391c0cb3u, 0x4ed8aa4au, 0x5b9cca4fu, 0x682e6ff3u,
    0x748f82eeu, 0x78a5636fu, 0x84c87814u, 0x8cc70208u, 0x90befffau, 0xa4506cebu, 0xbef9a3f7u, 0xc67178f2u,
};

inline unsigned int ambr_sha_rotr(unsigned int x, int n) { return (x >> n) | (x << (32 - n)); }

void ambr_sha256(const unsigned char* data, size_t len, unsigned char out[32]) {
    unsigned int h[8] = {0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
                         0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u};
    const unsigned long long bits = (unsigned long long)len * 8u;
    unsigned char block[64];
    size_t fill = 0, at = 0;
    bool padded = false, finished = false;
    for (;;) {
        if (fill == 64) {
            unsigned int w[64];
            for (int j = 0; j < 16; j++) {
                w[j] = ((unsigned int)block[j * 4] << 24) | ((unsigned int)block[j * 4 + 1] << 16) |
                       ((unsigned int)block[j * 4 + 2] << 8) | (unsigned int)block[j * 4 + 3];
            }
            for (int j = 16; j < 64; j++) {
                unsigned int s0 = ambr_sha_rotr(w[j - 15], 7) ^ ambr_sha_rotr(w[j - 15], 18) ^ (w[j - 15] >> 3);
                unsigned int s1 = ambr_sha_rotr(w[j - 2], 17) ^ ambr_sha_rotr(w[j - 2], 19) ^ (w[j - 2] >> 10);
                w[j] = w[j - 16] + s0 + w[j - 7] + s1;
            }
            unsigned int a = h[0], b = h[1], c = h[2], d = h[3];
            unsigned int e = h[4], f = h[5], g = h[6], hh = h[7];
            for (int j = 0; j < 64; j++) {
                unsigned int S1 = ambr_sha_rotr(e, 6) ^ ambr_sha_rotr(e, 11) ^ ambr_sha_rotr(e, 25);
                unsigned int ch = (e & f) ^ (~e & g);
                unsigned int t1 = hh + S1 + ch + AMBR_SHA_K[j] + w[j];
                unsigned int S0 = ambr_sha_rotr(a, 2) ^ ambr_sha_rotr(a, 13) ^ ambr_sha_rotr(a, 22);
                unsigned int maj = (a & b) ^ (a & c) ^ (b & c);
                unsigned int t2 = S0 + maj;
                hh = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
            }
            h[0] += a; h[1] += b; h[2] += c; h[3] += d;
            h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
            fill = 0;
            if (finished) {
                for (int i = 0; i < 8; i++) {
                    out[i * 4] = (unsigned char)(h[i] >> 24);
                    out[i * 4 + 1] = (unsigned char)(h[i] >> 16);
                    out[i * 4 + 2] = (unsigned char)(h[i] >> 8);
                    out[i * 4 + 3] = (unsigned char)(h[i]);
                }
                return;
            }
        }
        if (at < len) { block[fill++] = data[at++]; continue; }
        if (!padded) { padded = true; block[fill++] = 0x80; continue; }
        if (fill > 56) {
            while (fill < 64) block[fill++] = 0;
            continue;
        }
        while (fill < 56) block[fill++] = 0;
        for (int i = 0; i < 8; i++) block[fill++] = (unsigned char)(bits >> (56 - i * 8));
        finished = true;
    }
}

/* --- SM3 / BLAKE3：按 manifest 的 `hash_id` 校验段摘要（§20）。 --------------- */
constexpr unsigned int AMBR_SM3_IV[8] = {
    0x7380166fu, 0x4914b2b9u, 0x172442d7u, 0xda8a0600u,
    0xa96f30bcu, 0x163138aau, 0xe38dee4du, 0xb0fb0e4eu,
};

inline unsigned int ambr_rotl(unsigned int x, int n) {
    n &= 31;
    return n ? ((x << n) | (x >> (32 - n))) : x;
}

inline unsigned int ambr_sm3_p0(unsigned int x) { return x ^ ambr_rotl(x, 9) ^ ambr_rotl(x, 17); }
inline unsigned int ambr_sm3_p1(unsigned int x) { return x ^ ambr_rotl(x, 15) ^ ambr_rotl(x, 23); }

void ambr_sm3_digest(const unsigned char* data, size_t len, unsigned char out[32]) {
    unsigned int h[8];
    std::memcpy(h, AMBR_SM3_IV, sizeof h);
    const unsigned long long bits = (unsigned long long)len * 8u;
    unsigned char block[64];
    size_t fill = 0, at = 0;
    bool padded = false, finished = false;
    for (;;) {
        if (fill == 64) {
            unsigned int w[68], w1[64];
            for (int j = 0; j < 16; j++) {
                w[j] = ((unsigned int)block[j * 4] << 24) | ((unsigned int)block[j * 4 + 1] << 16) |
                       ((unsigned int)block[j * 4 + 2] << 8) | (unsigned int)block[j * 4 + 3];
            }
            for (int j = 16; j < 68; j++) {
                w[j] = ambr_sm3_p1(w[j - 16] ^ w[j - 9] ^ ambr_rotl(w[j - 3], 15)) ^
                       ambr_rotl(w[j - 13], 7) ^ w[j - 6];
            }
            for (int j = 0; j < 64; j++) w1[j] = w[j] ^ w[j + 4];
            unsigned int a = h[0], b = h[1], c = h[2], d2 = h[3];
            unsigned int e = h[4], f = h[5], g = h[6], hh = h[7];
            for (int j = 0; j < 64; j++) {
                unsigned int tj = (j < 16) ? 0x79cc4519u : 0x7a879d8au;
                unsigned int ss1 = ambr_rotl(ambr_rotl(a, 12) + e + ambr_rotl(tj, j), 7);
                unsigned int ss2 = ss1 ^ ambr_rotl(a, 12);
                unsigned int ff = (j < 16) ? (a ^ b ^ c) : ((a & b) | (a & c) | (b & c));
                unsigned int gg = (j < 16) ? (e ^ f ^ g) : ((e & f) | (~e & g));
                unsigned int tt1 = ff + d2 + ss2 + w1[j];
                unsigned int tt2 = gg + hh + ss1 + w[j];
                d2 = c; c = ambr_rotl(b, 9); b = a; a = tt1;
                hh = g; g = ambr_rotl(f, 19); f = e; e = ambr_sm3_p0(tt2);
            }
            h[0] ^= a; h[1] ^= b; h[2] ^= c; h[3] ^= d2;
            h[4] ^= e; h[5] ^= f; h[6] ^= g; h[7] ^= hh;
            fill = 0;
            if (finished) {
                for (int i = 0; i < 8; i++) {
                    out[i * 4] = (unsigned char)(h[i] >> 24);
                    out[i * 4 + 1] = (unsigned char)(h[i] >> 16);
                    out[i * 4 + 2] = (unsigned char)(h[i] >> 8);
                    out[i * 4 + 3] = (unsigned char)(h[i]);
                }
                return;
            }
        }
        if (at < len) { block[fill++] = data[at++]; continue; }
        if (!padded) { padded = true; block[fill++] = 0x80; continue; }
        if (fill > 56) {
            while (fill < 64) block[fill++] = 0;
            continue;
        }
        while (fill < 56) block[fill++] = 0;
        for (int i = 0; i < 8; i++) block[fill++] = (unsigned char)(bits >> (56 - i * 8));
        finished = true;
    }
}

/* BLAKE3（无密钥 32 B）：与 rust/amberice/src/blake3.rs 同构。 */
constexpr unsigned int AMBR_B3_IV[8] = {
    0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au,
    0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u,
};
constexpr int AMBR_B3_PERM[16] = {2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8};
constexpr unsigned int B3_CHUNK_START = 1u << 0;
constexpr unsigned int B3_CHUNK_END = 1u << 1;
constexpr unsigned int B3_PARENT = 1u << 2;
constexpr unsigned int B3_ROOT = 1u << 3;

inline unsigned int ambr_rotr(unsigned int x, int n) { return (x >> n) | (x << (32 - n)); }

void ambr_b3_g(unsigned int s[16], int a, int b, int c, int d, unsigned int mx, unsigned int my) {
    s[a] = s[a] + s[b] + mx;
    s[d] = ambr_rotr(s[d] ^ s[a], 16);
    s[c] = s[c] + s[d];
    s[b] = ambr_rotr(s[b] ^ s[c], 12);
    s[a] = s[a] + s[b] + my;
    s[d] = ambr_rotr(s[d] ^ s[a], 8);
    s[c] = s[c] + s[d];
    s[b] = ambr_rotr(s[b] ^ s[c], 7);
}

void ambr_b3_round(unsigned int s[16], const unsigned int m[16]) {
    ambr_b3_g(s, 0, 4, 8, 12, m[0], m[1]);
    ambr_b3_g(s, 1, 5, 9, 13, m[2], m[3]);
    ambr_b3_g(s, 2, 6, 10, 14, m[4], m[5]);
    ambr_b3_g(s, 3, 7, 11, 15, m[6], m[7]);
    ambr_b3_g(s, 0, 5, 10, 15, m[8], m[9]);
    ambr_b3_g(s, 1, 6, 11, 12, m[10], m[11]);
    ambr_b3_g(s, 2, 7, 8, 13, m[12], m[13]);
    ambr_b3_g(s, 3, 4, 9, 14, m[14], m[15]);
}

void ambr_b3_permute(unsigned int m[16]) {
    unsigned int old[16];
    std::memcpy(old, m, sizeof old);
    for (int i = 0; i < 16; i++) m[i] = old[AMBR_B3_PERM[i]];
}

void ambr_b3_compress(const unsigned int cv[8], const unsigned int m_in[16],
                      unsigned long long counter, unsigned int block_len,
                      unsigned int flags, unsigned int out[16]) {
    unsigned int s[16], m[16];
    std::memcpy(s, cv, 8 * sizeof(unsigned int));
    s[8] = AMBR_B3_IV[0]; s[9] = AMBR_B3_IV[1]; s[10] = AMBR_B3_IV[2]; s[11] = AMBR_B3_IV[3];
    s[12] = (unsigned int)(counter & 0xffffffffu);
    s[13] = (unsigned int)(counter >> 32);
    s[14] = block_len;
    s[15] = flags;
    std::memcpy(m, m_in, sizeof m);
    for (int i = 0; i < 7; i++) { ambr_b3_round(s, m); ambr_b3_permute(m); }
    for (int i = 0; i < 8; i++) {
        out[i] = s[i] ^ s[i + 8];
        out[i + 8] = s[i + 8] ^ cv[i];
    }
}

void ambr_b3_words(const unsigned char* buf, size_t len, unsigned int m[16]) {
    std::memset(m, 0, 16 * sizeof(unsigned int));
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

struct B3Output {
    unsigned int cv[8];
    unsigned int words[16];
    unsigned long long counter;
    unsigned int block_len;
    unsigned int flags;
};

void ambr_b3_output_cv(const B3Output& o, unsigned int cv[8]) {
    unsigned int s[16];
    ambr_b3_compress(o.cv, o.words, o.counter, o.block_len, o.flags, s);
    std::memcpy(cv, s, 8 * sizeof(unsigned int));
}

void ambr_b3_parent_output(const unsigned int left[8], const unsigned int right[8], B3Output* o) {
    std::memcpy(o->cv, AMBR_B3_IV, sizeof AMBR_B3_IV);
    std::memcpy(o->words, left, 8 * sizeof(unsigned int));
    std::memcpy(o->words + 8, right, 8 * sizeof(unsigned int));
    o->counter = 0;
    o->block_len = 64;
    o->flags = B3_PARENT;
}

void ambr_b3_parent_cv(const unsigned int left[8], const unsigned int right[8], unsigned int cv[8]) {
    B3Output o;
    ambr_b3_parent_output(left, right, &o);
    ambr_b3_output_cv(o, cv);
}

void ambr_b3_digest(const unsigned char* data, size_t len, unsigned char out[32]) {
    unsigned int cv[8];
    std::memcpy(cv, AMBR_B3_IV, sizeof cv);
    unsigned long long counter = 0;
    unsigned char buf[64];
    size_t buf_len = 0, blocks = 0;
    unsigned int stack[54][8];
    size_t stack_len = 0;
    size_t at = 0;

    auto chunk_len = [&]() { return (size_t)64 * blocks + buf_len; };
    auto start_flag = [&]() { return blocks == 0 ? B3_CHUNK_START : 0u; };
    auto chunk_output = [&](B3Output* o) {
        std::memcpy(o->cv, cv, sizeof cv);
        ambr_b3_words(buf, buf_len, o->words);
        o->counter = counter;
        o->block_len = (unsigned int)buf_len;
        o->flags = start_flag() | B3_CHUNK_END;
    };

    while (at < len) {
        if (chunk_len() == 1024) {
            B3Output o;
            chunk_output(&o);
            unsigned int new_cv[8];
            ambr_b3_output_cv(o, new_cv);
            unsigned long long total = counter + 1;
            std::memcpy(stack[stack_len++], new_cv, sizeof new_cv);
            while (total % 2 == 0 && stack_len >= 2) {
                unsigned int right[8], left[8];
                std::memcpy(right, stack[--stack_len], sizeof right);
                std::memcpy(left, stack[--stack_len], sizeof left);
                ambr_b3_parent_cv(left, right, stack[stack_len++]);
                total /= 2;
            }
            counter++;
            std::memcpy(cv, AMBR_B3_IV, sizeof cv);
            buf_len = 0;
            blocks = 0;
        }
        size_t take = 1024 - chunk_len();
        if (take > len - at) take = len - at;
        while (take--) {
            if (buf_len == 64) {
                unsigned int words[16], s[16];
                ambr_b3_words(buf, 64, words);
                ambr_b3_compress(cv, words, counter, 64, start_flag(), s);
                std::memcpy(cv, s, 8 * sizeof(unsigned int));
                blocks++;
                buf_len = 0;
            }
            buf[buf_len++] = data[at++];
        }
    }

    B3Output o;
    chunk_output(&o);
    for (size_t i = stack_len; i > 0; i--) {
        unsigned int right[8];
        ambr_b3_output_cv(o, right);
        ambr_b3_parent_output(stack[i - 1], right, &o);
    }
    unsigned int s[16];
    ambr_b3_compress(o.cv, o.words, 0, o.block_len, o.flags | B3_ROOT, s);
    for (int i = 0; i < 8; i++) {
        out[i * 4] = (unsigned char)(s[i]);
        out[i * 4 + 1] = (unsigned char)(s[i] >> 8);
        out[i * 4 + 2] = (unsigned char)(s[i] >> 16);
        out[i * 4 + 3] = (unsigned char)(s[i] >> 24);
    }
}

/* 段摘要校验：算法由 manifest 的 `hash_id` 决定（§20），不再写死 SHA-256。
 * 1 = 匹配，0 = 不符，-1 = 本端算不了（预留号 / 多级元组）⇒ 跳过并标注。 */
int digest_check(int hash_id, const unsigned char* body, size_t len, const unsigned char* want) {
    unsigned char got[32];
    if (hash_id == 0x30) {
        /* C++ 端复用 SHA-256 的既有实现：整份文件里没有摘要函数，故在此补一个最小调用点。 */
        ambr_sha256(body, len, got);
    } else if (hash_id == 0x31) {
        ambr_sm3_digest(body, len, got);
    } else if (hash_id == 0x32) {
        ambr_b3_digest(body, len, got);
    } else {
        return -1;
    }
    return std::memcmp(got, want, 32) == 0 ? 1 : 0;
}

ValuePtr decode_node(Reader& r, const StrTab& st, int depth, std::string* err);

ValuePtr decode_node(Reader& r, const StrTab& st, int depth, std::string* err) {
    if (depth > MAX_DEPTH) {
        set_err(err, "嵌套超过上限");
        return nullptr;
    }
    unsigned char tag = 0, flags = 0;
    unsigned long sub = 0;
    if (!r.u8(&tag) || !r.u8(&flags) || !r.u32(&sub)) {
        set_err(err, "节点头越界");
        return nullptr;
    }
    (void)flags; // bit 0 = has __type (hint only)
    size_t payload = r.pos;
    ValuePtr v;

    switch (tag) {
    case 0:
        v = Value::null();
        break;
    case 1:
        v = Value::boolean(false);
        break;
    case 2:
        v = Value::boolean(true);
        break;
    case 3: {
        unsigned long long z;
        if (!r.varint(&z)) break;
        long long n = (long long)(z >> 1) ^ -(long long)(z & 1);
        v = Value::integer(n);
        break;
    }
    case 4: {
        if (r.pos + 8 > r.len) break;
        double d;
        std::memcpy(&d, r.b + r.pos, 8);
        r.pos += 8;
        v = Value::floating(d);
        break;
    }
    case 5: {
        unsigned long long id;
        if (!r.varint(&id)) break;
        if (id >= st.size()) {
            set_err(err, "字符串 id 越界");
            return nullptr;
        }
        v = Value::string(st[(size_t)id]);
        break;
    }
    case 6: {
        unsigned long long count;
        if (!r.varint(&count)) break;
        if (count > MAX_ELEMS) {
            set_err(err, "数组元素数超过上限");
            return nullptr;
        }
        v = Value::array();
        for (unsigned long long i = 0; i < count; i++) {
            ValuePtr item = decode_node(r, st, depth + 1, err);
            if (!item) return nullptr;
            v->arr.push_back(item);
        }
        break;
    }
    case 7: {
        unsigned long long count;
        if (!r.varint(&count)) break;
        if (count > MAX_ELEMS) {
            set_err(err, "对象字段数超过上限");
            return nullptr;
        }
        v = Value::object();
        for (unsigned long long i = 0; i < count; i++) {
            unsigned long long key_id;
            if (!r.varint(&key_id) || key_id >= st.size()) {
                set_err(err, "键 id 越界");
                return nullptr;
            }
            const std::string& key = st[(size_t)key_id];
            // D15: duplicates are rejected, not last-wins ｜ 重复键拒绝
            if (v->has(key)) {
                set_err(err, "对象里出现重复键");
                return nullptr;
            }
            ValuePtr child = decode_node(r, st, depth + 1, err);
            if (!child) return nullptr;
            v->obj.emplace_back(key, child);
        }
        break;
    }
    default:
        set_err(err, "未定义的节点 tag");
        return nullptr;
    }

    if (!v) {
        set_err(err, "解码失败（截断）");
        return nullptr;
    }
    if (r.pos - payload != (size_t)sub) {
        set_err(err, "subtree_len 与实际消费不符");
        return nullptr;
    }
    return v;
}

} // namespace

struct Parts {
    const unsigned char* value = nullptr;
    const unsigned char* strtab = nullptr;
    const unsigned char* typed = nullptr;
    const unsigned char* schema = nullptr;
    size_t value_len = 0, strtab_len = 0, typed_len = 0, schema_len = 0;
    bool typed_opaque = false, schema_opaque = false;
    /* manifest 声明的摘要算法（§20）；digest_skipped = 本端算不了，已跳过校验。 */
    int hash_id = 0x30;
    bool digest_skipped = false;
};

/* 从 MANIFEST(0x04) 段体读 `hash_id`：段体是自包含 canonical 编码
 * （`varint count` + `count × (varint len ‖ utf8)` + 节点树）。 */
bool manifest_hash_id(const unsigned char* body, size_t len, int* out) {
    Reader r{body, len};
    unsigned long long count = 0;
    if (!r.varint(&count) || count > (unsigned long long)len) return false;
    StrTab st;
    for (unsigned long long i = 0; i < count; i++) {
        unsigned long long n = 0;
        if (!r.varint(&n) || n > (unsigned long long)(r.len - r.pos)) return false;
        st.emplace_back(reinterpret_cast<const char*>(body + r.pos), (size_t)n);
        r.pos += (size_t)n;
    }
    std::string e;
    ValuePtr tree = decode_node(r, st, 0, &e);
    if (!tree) return false;
    int hash = 0x30; /* 缺 hash_id ⇒ §21 默认 SHA-256 */
    ValuePtr h = tree->get("hash_id");
    if (h) {
        if (h->tag == Value::Tag::Int) {
            if (h->i < 0 || h->i > 255) return false;
            hash = (int)h->i;
        } else if (h->tag == Value::Tag::Arr) {
            hash = 0x3E; /* 多级元组：转义号，本端不实现 */
        } else {
            return false;
        }
    }
    *out = hash;
    return true;
}

bool walk_segments(const unsigned char* buf, size_t len, Parts* out, std::string* err) {
    *out = Parts{};
    if (!buf || len < 40) {
        set_err(err, "文件短于 40 B 头");
        return false;
    }
    if (std::memcmp(buf, "AMBR", 4) != 0) {
        set_err(err, "不是 .ambr 文件（magic 不是 AMBR）");
        return false;
    }
    const unsigned fmt = buf[6];
    const unsigned hdr_len = buf[7];
    Reader h{buf, len, 8};
    unsigned long long table_off = 0;
    if (!h.u64(&table_off)) {
        set_err(err, "头被截断");
        return false;
    }
    unsigned long count = 0;
    h.pos = 16;
    if (!h.u32(&count)) {
        set_err(err, "头被截断");
        return false;
    }
    if (20 + 2 > len) {
        set_err(err, "头被截断");
        return false;
    }
    const unsigned flags = (unsigned)buf[20] | ((unsigned)buf[21] << 8);
    if (fmt != 0) {
        set_err(err, "不支持的字节布局版本");
        return false;
    }
    if (hdr_len < 40 || table_off < hdr_len) {
        set_err(err, "头部长度或段表偏移非法");
        return false;
    }
    const size_t granule = (flags & 1u) ? ALIGN_BLOCK : ALIGN_MIN;
    size_t body_at = align_up((size_t)table_off + (size_t)count * SEG_HDR, granule);

    /* 引导顺序：先找 MANIFEST 取 hash_id，再用它验其余段（连 manifest 自己一起）。 */
    {
        size_t scan = body_at;
        for (unsigned long i = 0; i < count; i++) {
            const size_t o = (size_t)table_off + i * SEG_HDR;
            if (o + SEG_HDR > len) break;
            const unsigned sid = buf[o];
            const unsigned scomp = buf[o + 1], senc = buf[o + 2];
            const unsigned long slen = (unsigned long)buf[o + 5] | ((unsigned long)buf[o + 6] << 8) |
                                       ((unsigned long)buf[o + 7] << 16) |
                                       ((unsigned long)buf[o + 8] << 24);
            if (scan + slen > len) break;
            if (sid == 0x04 && scomp == 0 && senc == 0) {
                int found = 0x30;
                if (manifest_hash_id(buf + scan, slen, &found)) out->hash_id = found;
                break;
            }
            scan = align_up(scan + slen, granule);
        }
    }

    for (unsigned long i = 0; i < count; i++) {
        const size_t o = (size_t)table_off + i * SEG_HDR;
        if (o + SEG_HDR > len) {
            set_err(err, "段表越界");
            return false;
        }
        const unsigned id = buf[o];
        const unsigned comp = buf[o + 1];
        const unsigned enc = buf[o + 2];
        const unsigned long seg_len = (unsigned long)buf[o + 5] | ((unsigned long)buf[o + 6] << 8) |
                                      ((unsigned long)buf[o + 7] << 16) |
                                      ((unsigned long)buf[o + 8] << 24);
        if (body_at + seg_len > len) {
            set_err(err, "段体长度越界");
            return false;
        }
        if (id == SEG_VALUE || id == SEG_STRTAB) {
            if (comp != 0 || enc != 0) {
                set_err(err, "VALUE/STRTAB 压缩或加密过，本解码器不解");
                return false;
            }
            const int dc = digest_check(out->hash_id, buf + body_at, seg_len, buf + o + 9);
            if (dc == 0) {
                set_err(err, "段摘要不符（段体被改动？）");
                return false;
            }
            if (dc < 0) out->digest_skipped = true;
            if (id == SEG_VALUE) {
                out->value = buf + body_at;
                out->value_len = seg_len;
            } else {
                out->strtab = buf + body_at;
                out->strtab_len = seg_len;
            }
        } else if (id == 0x04) {
            if (comp == 0 && enc == 0) {
                const int dm = digest_check(out->hash_id, buf + body_at, seg_len, buf + o + 9);
                if (dm == 0) {
                    set_err(err, "段摘要不符（段体被改动？）");
                    return false;
                }
                if (dm < 0) out->digest_skipped = true;
            }
        } else if (id == 0x70 || id == 0x71) {
            if (comp != 0 || enc != 0) {
                if (id == 0x70) out->typed_opaque = true;
                else out->schema_opaque = true;
            } else if (id == 0x70) {
                const int dt = digest_check(out->hash_id, buf + body_at, seg_len, buf + o + 9);
                if (dt == 0) {
                    set_err(err, "段摘要不符（段体被改动？）");
                    return false;
                }
                if (dt < 0) out->digest_skipped = true;
                out->typed = buf + body_at;
                out->typed_len = seg_len;
            } else {
                const int ds = digest_check(out->hash_id, buf + body_at, seg_len, buf + o + 9);
                if (ds == 0) {
                    set_err(err, "段摘要不符（段体被改动？）");
                    return false;
                }
                if (ds < 0) out->digest_skipped = true;
                out->schema = buf + body_at;
                out->schema_len = seg_len;
            }
        }
        body_at = align_up(body_at + seg_len, granule);
    }
    return true;
}

std::optional<ValuePtr> ambr_load(const unsigned char* buf, size_t len, std::string* err) {
    return ambr_load(buf, len, nullptr, err);
}

std::optional<ValuePtr> ambr_load(const unsigned char* buf, size_t len, AmbrInfo* info,
                                  std::string* err) {
    if (err) err->clear();
    Parts p;
    if (!walk_segments(buf, len, &p, err)) return std::nullopt;
    if (!p.value) {
        set_err(err, "找不到 VALUE 段");
        return std::nullopt;
    }

    StrTab st;
    if (p.strtab && !parse_strtab(p.strtab, p.strtab_len, &st)) {
        set_err(err, "STRTAB 解析失败");
        return std::nullopt;
    }
    Reader r{p.value, p.value_len};
    ValuePtr v = decode_node(r, st, 0, err);
    if (v && r.pos != r.len) {
        set_err(err, "VALUE 段读完仍有剩余字节");
        return std::nullopt;
    }
    if (v && info) {
        info->hash_id = p.hash_id;
        info->digest_skipped = p.digest_skipped;
    }
    return v;
}

// ---------------------------------------------------------------------------
// SCHEMA（0x71）+ TYPED（0x70）动态渲染（D45 / §25.8）
// ---------------------------------------------------------------------------

struct Ty {
    enum class Kind {
        Bool, U8, U16, U32, U64, I8, I16, I32, I64, F32, F64, Str, Bytes, Vec, Opt, Named
    } kind = Kind::Bool;
    unsigned long long named = 0;
    std::shared_ptr<Ty> inner;
};

struct Field {
    std::string name;
    std::shared_ptr<Ty> ty;
};

struct TypeDef {
    unsigned long long id = 0;
    std::string name;
    std::vector<Field> fields;
};

bool parse_type_id(const std::string& s, unsigned long long* out) {
    if (s.size() != 16) return false;
    unsigned long long v = 0;
    for (char c : s) {
        unsigned d;
        if (c >= '0' && c <= '9') d = (unsigned)(c - '0');
        else if (c >= 'a' && c <= 'f') d = (unsigned)(c - 'a' + 10);
        else return false;
        v = (v << 4) | d;
    }
    *out = v;
    return true;
}

bool parse_ty(const ValuePtr& v, std::shared_ptr<Ty>& out, std::string* err) {
    if (!v) {
        set_err(err, "SCHEMA 缺 ty");
        return false;
    }
    auto t = std::make_shared<Ty>();
    if (v->tag == Value::Tag::Str) {
        const std::string& s = v->s;
        if (s == "bool") t->kind = Ty::Kind::Bool;
        else if (s == "u8") t->kind = Ty::Kind::U8;
        else if (s == "u16") t->kind = Ty::Kind::U16;
        else if (s == "u32") t->kind = Ty::Kind::U32;
        else if (s == "u64") t->kind = Ty::Kind::U64;
        else if (s == "i8") t->kind = Ty::Kind::I8;
        else if (s == "i16") t->kind = Ty::Kind::I16;
        else if (s == "i32") t->kind = Ty::Kind::I32;
        else if (s == "i64") t->kind = Ty::Kind::I64;
        else if (s == "f32") t->kind = Ty::Kind::F32;
        else if (s == "f64") t->kind = Ty::Kind::F64;
        else if (s == "str") t->kind = Ty::Kind::Str;
        else if (s == "bytes") t->kind = Ty::Kind::Bytes;
        else {
            set_err(err, "SCHEMA 里有未知 ty");
            return false;
        }
    } else if (v->tag == Value::Tag::Obj) {
        if (auto inner = v->get("vec")) {
            t->kind = Ty::Kind::Vec;
            if (!parse_ty(inner, t->inner, err)) return false;
        } else if (auto inner2 = v->get("opt")) {
            t->kind = Ty::Kind::Opt;
            if (!parse_ty(inner2, t->inner, err)) return false;
        } else if (auto idv = v->get("type")) {
            if (idv->tag != Value::Tag::Str || !parse_type_id(idv->s, &t->named)) {
                set_err(err, "SCHEMA 的 type 引用非法");
                return false;
            }
            t->kind = Ty::Kind::Named;
        } else {
            set_err(err, "SCHEMA 的 ty 对象缺 vec/opt/type");
            return false;
        }
    } else {
        set_err(err, "SCHEMA 的 ty 必须是字符串或对象");
        return false;
    }
    out = t;
    return true;
}

bool parse_schema(const ValuePtr& schema, std::vector<TypeDef>& out, std::string* err) {
    if (!schema || schema->tag != Value::Tag::Obj) {
        set_err(err, "SCHEMA 必须是对象");
        return false;
    }
    auto ver = schema->get("schema_version");
    if (!ver || ver->tag != Value::Tag::Int || ver->i != 1) {
        set_err(err, "schema_version 只支持 1");
        return false;
    }
    auto types = schema->get("types");
    if (!types || types->tag != Value::Tag::Arr) {
        set_err(err, "SCHEMA 缺 types");
        return false;
    }
    for (const auto& tv : types->arr) {
        if (!tv || tv->tag != Value::Tag::Obj) {
            set_err(err, "SCHEMA 的 types[] 必须是对象");
            return false;
        }
        auto idv = tv->get("id");
        auto nv = tv->get("name");
        auto kv = tv->get("kind");
        auto fv = tv->get("fields");
        TypeDef td;
        if (!idv || idv->tag != Value::Tag::Str || !parse_type_id(idv->s, &td.id)) {
            set_err(err, "SCHEMA 的 type id 非法");
            return false;
        }
        if (!nv || nv->tag != Value::Tag::Str) {
            set_err(err, "SCHEMA 的 type 缺 name");
            return false;
        }
        td.name = nv->s;
        if (kv && (kv->tag != Value::Tag::Str || kv->s != "struct")) {
            set_err(err, "v1 只支持 kind: struct");
            return false;
        }
        if (!fv || fv->tag != Value::Tag::Arr) {
            set_err(err, "SCHEMA 的 type 缺 fields");
            return false;
        }
        long long last = -1;
        for (const auto& fo : fv->arr) {
            if (!fo || fo->tag != Value::Tag::Obj) {
                set_err(err, "SCHEMA 的 fields[] 必须是对象");
                return false;
            }
            auto fid = fo->get("id");
            auto fn = fo->get("name");
            auto ft = fo->get("ty");
            if (!fid || fid->tag != Value::Tag::Int || fid->i <= last) {
                set_err(err, "SCHEMA 的 fields 必须按 id 严格升序");
                return false;
            }
            last = fid->i;
            if (!fn || fn->tag != Value::Tag::Str) {
                set_err(err, "SCHEMA 的 field 缺 name");
                return false;
            }
            Field fd;
            fd.name = fn->s;
            if (!parse_ty(ft, fd.ty, err)) return false;
            td.fields.push_back(std::move(fd));
        }
        for (const auto& e : out) {
            if (e.id == td.id) {
                set_err(err, "SCHEMA 的 type id 重复");
                return false;
            }
        }
        out.push_back(std::move(td));
    }
    return true;
}

bool read_len_bytes(Reader& r, const unsigned char** b, size_t* n) {
    unsigned long long len = 0;
    if (!r.varint(&len)) return false;
    if (len > r.len - r.pos) return false;
    *b = r.b + r.pos;
    *n = (size_t)len;
    r.pos += (size_t)len;
    return true;
}

std::string hex_string(const unsigned char* b, size_t n) {
    static const char* H = "0123456789abcdef";
    std::string s;
    s.resize(n * 2);
    for (size_t i = 0; i < n; i++) {
        s[i * 2] = H[b[i] >> 4];
        s[i * 2 + 1] = H[b[i] & 15];
    }
    return s;
}

const TypeDef* schema_find(const std::vector<TypeDef>& sd, unsigned long long id) {
    for (const auto& t : sd) if (t.id == id) return &t;
    return nullptr;
}

ValuePtr decode_typed(Reader& r, const Ty& t, const std::vector<TypeDef>& sd, int depth,
                      std::string* err);

ValuePtr decode_struct(Reader& r, const TypeDef& td, const std::vector<TypeDef>& sd, int depth,
                       std::string* err) {
    if (depth > MAX_DEPTH) {
        set_err(err, "TYPED 嵌套超过上限");
        return nullptr;
    }
    auto o = Value::object();
    for (const auto& f : td.fields) {
        ValuePtr v = decode_typed(r, *f.ty, sd, depth + 1, err);
        if (!v) return nullptr;
        o->obj.emplace_back(f.name, v);
    }
    return o;
}

ValuePtr decode_typed(Reader& r, const Ty& t, const std::vector<TypeDef>& sd, int depth,
                      std::string* err) {
    if (depth > MAX_DEPTH) {
        set_err(err, "TYPED 嵌套超过上限");
        return nullptr;
    }
    unsigned long long z = 0;
    switch (t.kind) {
    case Ty::Kind::Bool:
        if (!r.varint(&z) || z > 1) {
            set_err(err, "布尔的 tag 只能是 0/1");
            return nullptr;
        }
        return Value::boolean(z == 1);
    case Ty::Kind::U8:
    case Ty::Kind::U16:
    case Ty::Kind::U32: {
        const unsigned long long max =
            t.kind == Ty::Kind::U8 ? 255ULL : t.kind == Ty::Kind::U16 ? 65535ULL : 4294967295ULL;
        if (!r.varint(&z) || z > max) {
            set_err(err, "整数超出目标类型范围");
            return nullptr;
        }
        return Value::integer((long long)z);
    }
    case Ty::Kind::U64:
        if (!r.varint(&z)) {
            set_err(err, "TYPED 载荷截断");
            return nullptr;
        }
        if (z > 9223372036854775807ULL) {
            set_err(err, "u64 超出 i64 范围，无法用 Int 表示");
            return nullptr;
        }
        return Value::integer((long long)z);
    case Ty::Kind::I8:
    case Ty::Kind::I16:
    case Ty::Kind::I32:
    case Ty::Kind::I64: {
        if (!r.varint(&z)) {
            set_err(err, "TYPED 载荷截断");
            return nullptr;
        }
        const long long n = (long long)(z >> 1) ^ -(long long)(z & 1);
        const long long lim = t.kind == Ty::Kind::I8 ? 127LL
                            : t.kind == Ty::Kind::I16 ? 32767LL
                            : t.kind == Ty::Kind::I32 ? 2147483647LL
                                                      : 9223372036854775807LL;
        if (n > lim || n < -lim - 1) {
            set_err(err, "整数超出目标类型范围");
            return nullptr;
        }
        return Value::integer(n);
    }
    case Ty::Kind::F32: {
        if (r.pos + 4 > r.len) {
            set_err(err, "TYPED 载荷截断");
            return nullptr;
        }
        float f = 0;
        std::memcpy(&f, r.b + r.pos, 4);
        r.pos += 4;
        return Value::floating((double)f);
    }
    case Ty::Kind::F64: {
        if (r.pos + 8 > r.len) {
            set_err(err, "TYPED 载荷截断");
            return nullptr;
        }
        double d = 0;
        std::memcpy(&d, r.b + r.pos, 8);
        r.pos += 8;
        return Value::floating(d);
    }
    case Ty::Kind::Str: {
        const unsigned char* b = nullptr;
        size_t n = 0;
        if (!read_len_bytes(r, &b, &n)) {
            set_err(err, "字符串长度越界");
            return nullptr;
        }
        return Value::string(std::string(reinterpret_cast<const char*>(b), n));
    }
    case Ty::Kind::Bytes: {
        const unsigned char* b = nullptr;
        size_t n = 0;
        if (!read_len_bytes(r, &b, &n)) {
            set_err(err, "字节长度越界");
            return nullptr;
        }
        return Value::string(hex_string(b, n));
    }
    case Ty::Kind::Vec: {
        if (!r.varint(&z) || z > MAX_ELEMS) {
            set_err(err, "数组元素数越界");
            return nullptr;
        }
        auto a = Value::array();
        for (unsigned long long i = 0; i < z; i++) {
            ValuePtr item = decode_typed(r, *t.inner, sd, depth + 1, err);
            if (!item) return nullptr;
            a->arr.push_back(item);
        }
        return a;
    }
    case Ty::Kind::Opt:
        if (!r.varint(&z) || z > 1) {
            set_err(err, "Option 的 tag 只能是 0/1");
            return nullptr;
        }
        if (z == 0) return Value::null();
        return decode_typed(r, *t.inner, sd, depth + 1, err);
    case Ty::Kind::Named: {
        const TypeDef* td = schema_find(sd, t.named);
        if (!td) {
            set_err(err, "SCHEMA 里没有这个 type_id");
            return nullptr;
        }
        return decode_struct(r, *td, sd, depth + 1, err);
    }
    }
    set_err(err, "未知 ty");
    return nullptr;
}

std::optional<ValuePtr> ambr_load_typed(const unsigned char* buf, size_t len, std::string* err) {
    if (err) err->clear();
    Parts p;
    if (!walk_segments(buf, len, &p, err)) return std::nullopt;
    if (p.typed_opaque || p.schema_opaque) {
        set_err(err, "TYPED/SCHEMA 压缩或加密过，本解码器不解");
        return std::nullopt;
    }
    if (!p.typed) return Value::array();
    if (!p.schema) {
        set_err(err, "没有 SCHEMA 段（0x71），无法按名渲染 TYPED");
        return std::nullopt;
    }

    Reader sr{p.schema, p.schema_len};
    StrTab sst;
    unsigned long long scount = 0;
    if (!sr.varint(&scount) || scount > MAX_ELEMS) {
        set_err(err, "L0 字符串个数非法");
        return std::nullopt;
    }
    for (unsigned long long i = 0; i < scount; i++) {
        const unsigned char* b = nullptr;
        size_t n = 0;
        if (!read_len_bytes(sr, &b, &n)) {
            set_err(err, "L0 字符串长度越界");
            return std::nullopt;
        }
        sst.emplace_back(reinterpret_cast<const char*>(b), n);
    }
    ValuePtr schema = decode_node(sr, sst, 0, err);
    if (!schema) return std::nullopt;
    if (sr.pos != sr.len) {
        set_err(err, "SCHEMA 段读完仍有剩余字节");
        return std::nullopt;
    }

    std::vector<TypeDef> sd;
    if (!parse_schema(schema, sd, err)) return std::nullopt;

    Reader tr{p.typed, p.typed_len};
    unsigned long long count = 0;
    if (!tr.varint(&count) || count > MAX_ELEMS) {
        set_err(err, "TYPED 条目数非法");
        return std::nullopt;
    }
    struct Ent {
        unsigned long long id = 0, type_id = 0;
        unsigned long off = 0, len = 0;
    };
    std::vector<Ent> ents;
    ents.reserve((size_t)count);
    for (unsigned long long i = 0; i < count; i++) {
        Ent e;
        if (!tr.varint(&e.id) || e.id == 0 || e.id > 4294967295ULL ||
            !tr.u64(&e.type_id) || !tr.u32(&e.off) || !tr.u32(&e.len)) {
            set_err(err, "TYPED 目录被截断");
            return std::nullopt;
        }
        ents.push_back(e);
    }
    const size_t base = tr.pos;

    auto out = Value::array();
    for (const auto& e : ents) {
        const TypeDef* td = schema_find(sd, e.type_id);
        if (!td) {
            set_err(err, "SCHEMA 里没有这个 type_id");
            return std::nullopt;
        }
        if ((size_t)e.off + e.len > p.typed_len - base) {
            set_err(err, "TYPED 载荷越界");
            return std::nullopt;
        }
        Reader er{p.typed + base + e.off, e.len};
        ValuePtr val = decode_struct(er, *td, sd, 0, err);
        if (!val) return std::nullopt;
        if (er.pos != er.len) {
            set_err(err, "TYPED 载荷解完仍有剩余字节");
            return std::nullopt;
        }
        auto item = Value::object();
        item->obj.emplace_back("id", Value::integer((long long)e.id));
        item->obj.emplace_back("name", Value::string(td->name));
        item->obj.emplace_back("value", val);
        out->arr.push_back(item);
    }
    return out;
}

std::optional<ValuePtr> ambr_load_file(const std::string& path, std::string* err) {
    std::FILE* f = std::fopen(path.c_str(), "rb");
    if (!f) {
        set_err(err, "打不开文件");
        return std::nullopt;
    }
    std::fseek(f, 0, SEEK_END);
    const long n = std::ftell(f);
    if (n < 0) {
        std::fclose(f);
        set_err(err, "取文件长度失败");
        return std::nullopt;
    }
    std::rewind(f);
    std::vector<unsigned char> buf((size_t)n);
    const size_t got = std::fread(buf.data(), 1, (size_t)n, f);
    std::fclose(f);
    if (got != (size_t)n) {
        set_err(err, "读文件失败");
        return std::nullopt;
    }
    return ambr_load(buf.data(), buf.size(), err);
}

} // namespace sml
