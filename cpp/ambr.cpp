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

std::optional<ValuePtr> ambr_load(const unsigned char* buf, size_t len, std::string* err) {
    if (err) err->clear();
    if (!buf || len < 40) {
        set_err(err, "文件短于 40 B 头");
        return std::nullopt;
    }
    if (std::memcmp(buf, "AMBR", 4) != 0) {
        set_err(err, "不是 .ambr 文件（magic 不是 AMBR）");
        return std::nullopt;
    }
    const unsigned fmt = buf[6];
    const unsigned hdr_len = buf[7];
    Reader h{buf, len, 8};
    unsigned long long table_off = 0;
    if (!h.u64(&table_off)) {
        set_err(err, "头被截断");
        return std::nullopt;
    }
    unsigned long count = 0;
    h.pos = 16;
    if (!h.u32(&count)) {
        set_err(err, "头被截断");
        return std::nullopt;
    }
    if (20 + 2 > len) {
        set_err(err, "头被截断");
        return std::nullopt;
    }
    const unsigned flags = (unsigned)buf[20] | ((unsigned)buf[21] << 8);
    if (fmt != 0) {
        set_err(err, "不支持的字节布局版本");
        return std::nullopt;
    }
    if (hdr_len < 40 || table_off < hdr_len) {
        set_err(err, "头部长度或段表偏移非法");
        return std::nullopt;
    }
    const size_t granule = (flags & 1u) ? ALIGN_BLOCK : ALIGN_MIN;
    size_t body_at = align_up((size_t)table_off + (size_t)count * SEG_HDR, granule);

    const unsigned char* value = nullptr;
    const unsigned char* strtab_body = nullptr;
    size_t value_len = 0, strtab_len = 0;
    for (unsigned long i = 0; i < count; i++) {
        const size_t o = (size_t)table_off + i * SEG_HDR;
        if (o + SEG_HDR > len) {
            set_err(err, "段表越界");
            return std::nullopt;
        }
        const unsigned id = buf[o];
        const unsigned comp = buf[o + 1];
        const unsigned enc = buf[o + 2];
        const unsigned long seg_len = (unsigned long)buf[o + 5] | ((unsigned long)buf[o + 6] << 8) |
                                      ((unsigned long)buf[o + 7] << 16) |
                                      ((unsigned long)buf[o + 8] << 24);
        if (body_at + seg_len > len) {
            set_err(err, "段体长度越界");
            return std::nullopt;
        }
        if (id == SEG_VALUE || id == SEG_STRTAB) {
            if (comp != 0 || enc != 0) {
                set_err(err, "VALUE/STRTAB 压缩或加密过，本解码器不解");
                return std::nullopt;
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
        set_err(err, "找不到 VALUE 段");
        return std::nullopt;
    }

    StrTab st;
    if (strtab_body && !parse_strtab(strtab_body, strtab_len, &st)) {
        set_err(err, "STRTAB 解析失败");
        return std::nullopt;
    }
    Reader r{value, value_len};
    ValuePtr v = decode_node(r, st, 0, err);
    if (v && r.pos != r.len) {
        set_err(err, "VALUE 段读完仍有剩余字节");
        return std::nullopt;
    }
    return v;
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
