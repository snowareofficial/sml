// SPDX-License-Identifier: MulanPSL-2.0
// SML v3 桥接实现（Rust cdylib 后端，C++ 封装）。
// 仓库：https://gitee.com/snoware/sml
// @version 0.1.0 (swsml 0.6.3)

/*
 * sml_rs.cpp — implementation of the sml_rs.hpp wrapper.
 *
 * Only forwards to the Rust cdylib symbols; no parsing logic lives here.
 */
#include "sml_rs.hpp"

#include <cstring>
#include <sstream>

// 这些 C-ABI 符号由 sml_rs.hpp → sml_rs.h 声明（extern "C" 包裹，类型以头文件为准），
// 此处**不再重复声明**。曾在本文件用 std::int64_t 重复声明 sml_int_value / sml_int_in，
// 在 LP64 平台（Linux：int64_t == long）与 sml_rs.h 的 long long 冲突 ⇒ 编译失败
// （Windows/mingw：int64_t == long long 才未暴露）。删除冗余块，统一以头文件为准。

namespace sml {
namespace {

// Take ownership of a Rust-allocated C string and copy it out.
std::string take(char *raw) {
    if (!raw) {
        return {};
    }
    std::string s(raw);
    sml_free_str(raw);
    return s;
}

} // namespace

/* ---------------------------- Error ---------------------------- */

std::string Error::str() const {
    std::ostringstream os;
    if (!source.empty()) {
        os << source;
        if (line > 0) {
            os << ':' << line;
            if (column > 0) {
                os << ':' << column;
            }
        }
        os << ": ";
    }
    os << text;
    return os.str();
}

/* ----------------------------- Ref ----------------------------- */

Type Ref::type() const {
    if (!p_) {
        return Type::Null;
    }
    return static_cast<Type>(sml_typeof(p_));
}

std::size_t Ref::size() const {
    return p_ ? sml_size(p_) : 0;
}

std::optional<std::string> Ref::str() const {
    if (!p_ || type() != Type::Str) {
        return std::nullopt;
    }
    // Two-call pattern: query the length, then copy.
    std::size_t need = sml_str_copy(p_, nullptr, 0);
    std::string out(need, '\0');
    if (need > 0) {
        sml_str_copy(p_, &out[0], need + 1);
    }
    return out;
}

std::optional<std::int64_t> Ref::as_int() const {
    if (!p_ || type() != Type::Int) {
        return std::nullopt;
    }
    return sml_int_value(p_);
}

std::optional<double> Ref::as_real() const {
    if (!p_) {
        return std::nullopt;
    }
    Type t = type();
    if (t != Type::Float && t != Type::Int) {
        return std::nullopt;
    }
    return sml_real_value(p_);
}

std::optional<bool> Ref::as_bool() const {
    if (!p_ || type() != Type::Bool) {
        return std::nullopt;
    }
    return sml_bool_value(p_) != 0;
}

Ref Ref::get(const std::string &key) const {
    return p_ ? Ref(sml_get(p_, key.c_str())) : Ref();
}

Ref Ref::at(std::size_t index) const {
    return p_ ? Ref(sml_at(p_, index)) : Ref();
}

Ref Ref::path(const std::string &dotted) const {
    return p_ ? Ref(sml_get_path(p_, dotted.c_str())) : Ref();
}

std::optional<std::string> Ref::str_at(const std::string &dotted) const {
    if (!p_) {
        return std::nullopt;
    }
    char *raw = sml_str_in(p_, dotted.c_str());
    if (!raw) {
        return std::nullopt;
    }
    std::string s(raw);
    sml_free_str(raw);
    return s;
}

std::optional<std::int64_t> Ref::int_at(const std::string &dotted) const {
    if (!p_) {
        return std::nullopt;
    }
    int ok = 0;
    std::int64_t v = sml_int_in(p_, dotted.c_str(), &ok);
    if (!ok) {
        return std::nullopt;
    }
    return v;
}

std::optional<bool> Ref::bool_at(const std::string &dotted) const {
    if (!p_) {
        return std::nullopt;
    }
    int ok = 0;
    int v = sml_bool_in(p_, dotted.c_str(), &ok);
    if (!ok) {
        return std::nullopt;
    }
    return v != 0;
}

std::string Ref::dumps(unsigned flags) const {
    return p_ ? take(sml_dumps(p_, flags)) : std::string();
}

/* ---------------------------- Value ---------------------------- */

void Value::reset() {
    if (p_) {
        sml_free(const_cast<sml_value *>(p_));
        p_ = nullptr;
    }
}

/* --------------------------- Loading --------------------------- */

namespace {

Error to_error(const sml_error &e) {
    Error out;
    out.code = static_cast<Err>(e.code);
    out.line = e.line;
    out.column = e.column;
    out.position = e.position;
    // Arrays are fixed-size and NUL-terminated by the Rust side.
    out.source = std::string(e.source);
    out.text = std::string(e.text);
    out.code_str = std::string(e.code_str);
    return out;
}

} // namespace

LoadResult loads(const std::string &text, unsigned flags) {
    sml_error e;
    std::memset(&e, 0, sizeof(e));
    LoadResult r;
    r.value = Value(sml_loads(text.c_str(), flags, &e));
    r.error = to_error(e);
    return r;
}

LoadResult load_file(const std::string &path, unsigned flags) {
    sml_error e;
    std::memset(&e, 0, sizeof(e));
    LoadResult r;
    r.value = Value(sml_load_file(path.c_str(), flags, &e));
    r.error = to_error(e);
    return r;
}

/* --------------------------- Metadata -------------------------- */

std::vector<std::string> feature_names() {
    std::vector<std::string> out;
    for (unsigned bit = 0; bit < 32; ++bit) {
        const char *n = sml_feature_name(bit);
        if (!n) {
            break;
        }
        out.emplace_back(n);
    }
    return out;
}

unsigned feature_mask() {
    return sml_features_mask();
}

std::string version() {
    const char *v = sml_version();
    return v ? std::string(v) : std::string();
}

/* --------------------- Legacy JSON-string API ------------------ */

std::string ParseOptions::to_json() const {
    std::ostringstream os;
    os << "{";
    bool first = true;
    if (!features.empty()) {
        os << "\"features\":[";
        for (std::size_t i = 0; i < features.size(); ++i) {
            if (i) {
                os << ",";
            }
            os << "\"" << features[i] << "\"";
        }
        os << "]";
        first = false;
    }
    if (!env.empty()) {
        if (!first) {
            os << ",";
        }
        os << "\"env\":{";
        for (std::size_t i = 0; i < env.size(); ++i) {
            if (i) {
                os << ",";
            }
            auto eq = env[i].find('=');
            std::string k = eq == std::string::npos ? env[i] : env[i].substr(0, eq);
            std::string v = eq == std::string::npos ? std::string() : env[i].substr(eq + 1);
            os << "\"" << k << "\":\"" << v << "\"";
        }
        os << "}";
        first = false;
    }
    if (!allow.empty()) {
        if (!first) {
            os << ",";
        }
        os << "\"allow\":[";
        for (std::size_t i = 0; i < allow.size(); ++i) {
            if (i) {
                os << ",";
            }
            os << "\"" << allow[i] << "\"";
        }
        os << "]";
    }
    os << "}";
    return os.str();
}

std::string parse(const std::string &text) {
    return take(sml_parse(text.c_str()));
}

std::string parse_ex(const std::string &text, const ParseOptions &o) {
    std::string opts = o.to_json();
    return take(sml_parse_ex(text.c_str(), opts.c_str()));
}

std::string parse_file_json(const std::string &path) {
    return take(sml_parse_file(path.c_str()));
}

std::string dump(const std::string &json) {
    return take(sml_dump(json.c_str()));
}

} // namespace sml
