/*
 * sml_rs.hpp — C++17 wrapper around the SML v3 Rust cdylib backend (sml_rs.h).
 *
 * Complements sml.hpp (the pure C++ native implementation): this header does
 * NOT implement a parser, it bridges the `swsml` Rust crate's cdylib to gain
 * the full v3 feature set ($env inlining / glob-include / @feature / @contract).
 *
 * Build (Windows / msys64 ucrt64):
 *   g++ -std=c++17 example_rs.cpp sml_rs.cpp -L<E:/snoware-target>/release -lsml -o example_rs
 *
 * Ownership model (mirrors the C API):
 *   Value   owns a root node; freed automatically on destruction.
 *   Ref     is a NON-OWNING borrowed view of a child node. It is invalidated
 *           when the owning Value is destroyed — never store it longer.
 */
#ifndef SML_RS_HPP
#define SML_RS_HPP

#include <cstdint>
#include <memory>
#include <optional>
#include <string>
#include <vector>

// Pull in the C declarations (sml_value, sml_error, SML_F_* flags, ...).
#include "../c/sml_rs.h"

namespace sml {

/* ------------------------------------------------------------------ *
 * Errors
 * ------------------------------------------------------------------ */

enum class Err {
    Ok = SML_OK,
    Syntax = SML_ERR_SYNTAX,
    FeatureDisabled = SML_ERR_FEATURE_DISABLED,
    VersionMismatch = SML_ERR_VERSION_MISMATCH,
    Contract = SML_ERR_CONTRACT,
    IncludeLoop = SML_ERR_INCLUDE_LOOP,
    Io = SML_ERR_IO,
    Utf8 = SML_ERR_UTF8,
    Internal = SML_ERR_INTERNAL,
};

struct Error {
    Err code = Err::Ok;
    int line = 0;   // 1-based; 0 when unknown
    int column = 0; // 1-based; 0 when unknown
    std::size_t position = 0;
    std::string source; // file name, or "<string>"
    std::string text;   // human-readable message

    bool ok() const { return code == Err::Ok; }
    explicit operator bool() const { return !ok(); }

    // One-line rendering, e.g. "config.sml:12:5: undefined contract X"
    std::string str() const;
};

enum class Type {
    Null = SML_TYPE_NULL,
    Bool = SML_TYPE_BOOL,
    Int = SML_TYPE_INT,
    Float = SML_TYPE_FLOAT,
    Str = SML_TYPE_STR,
    Array = SML_TYPE_ARRAY,
    Object = SML_TYPE_OBJECT,
};

/* ------------------------------------------------------------------ *
 * Ref — borrowed view of a node (non-owning)
 * ------------------------------------------------------------------ */

class Ref {
public:
    Ref() = default;
    explicit Ref(const sml_value *p) : p_(p) {}

    bool valid() const { return p_ != nullptr; }
    explicit operator bool() const { return p_ != nullptr; }
    const sml_value *raw() const { return p_; }

    Type type() const;
    std::size_t size() const; // array length / object field count

    // Scalars: std::nullopt when the node is missing or of another type.
    std::optional<std::string> str() const;
    std::optional<std::int64_t> as_int() const;
    std::optional<double> as_real() const;
    std::optional<bool> as_bool() const;

    // Child lookup; returns an invalid Ref when absent.
    Ref get(const std::string &key) const;
    Ref at(std::size_t index) const;

    // Dotted path, e.g. "server.host" — one call instead of chained get().
    Ref path(const std::string &dotted) const;

    // Typed convenience for the common "read a config value" case.
    std::optional<std::string> str_at(const std::string &dotted) const;
    std::optional<std::int64_t> int_at(const std::string &dotted) const;
    std::optional<bool> bool_at(const std::string &dotted) const;

    std::string dumps(unsigned flags = 0) const;

protected:
    const sml_value *p_ = nullptr;
};

/* ------------------------------------------------------------------ *
 * Value — owns a root node
 * ------------------------------------------------------------------ */

class Value : public Ref {
public:
    Value() = default;
    explicit Value(sml_value *p) : Ref(p) {}

    ~Value() { reset(); }

    Value(const Value &) = delete;
    Value &operator=(const Value &) = delete;

    Value(Value &&o) noexcept : Ref(o.p_) { o.p_ = nullptr; }
    Value &operator=(Value &&o) noexcept {
        if (this != &o) {
            reset();
            p_ = o.p_;
            o.p_ = nullptr;
        }
        return *this;
    }

    // Release the underlying node (idempotent).
    void reset();

    // Relinquish ownership without freeing (advanced use only).
    sml_value *release() {
        sml_value *p = const_cast<sml_value *>(p_);
        p_ = nullptr;
        return p;
    }
};

/* ------------------------------------------------------------------ *
 * Loading
 * ------------------------------------------------------------------ */

struct LoadResult {
    Value value;
    Error error;

    bool ok() const { return error.ok() && value.valid(); }
    explicit operator bool() const { return ok(); }
};

// Parse text. flags == 0 means "baseline feature set" (see sml_rs.h).
LoadResult loads(const std::string &text, unsigned flags = 0);

// Parse a file: expands `include`, relative paths resolve against the
// file's own directory. include paths must be quoted.
LoadResult load_file(const std::string &path, unsigned flags = 0);

/* ------------------------------------------------------------------ *
 * Metadata
 * ------------------------------------------------------------------ */

// Supported feature names, indexed by bit position.
std::vector<std::string> feature_names();

// Bit mask of supported features (usable with SML_F_* constants).
unsigned feature_mask();

std::string version(); // e.g. "sml 0.5.0"

/* ------------------------------------------------------------------ *
 * Legacy JSON-string API
 *
 * Kept for hosts that already have a JSON pipeline: parsing yields JSON
 * text directly. Prefer the value-tree API above — the legacy path forces
 * you to embed a second parser (cJSON & friends), at which point SML adds
 * little value over using that library directly.
 * ------------------------------------------------------------------ */

struct ParseOptions {
    std::vector<std::string> features;      // extra features to enable
    std::vector<std::string> env;           // "KEY=VALUE" pairs to inject
    std::vector<std::string> allow;         // allowed @version, e.g. {"v1","v3"}

    std::string to_json() const;
};

std::string parse(const std::string &text);                        // -> JSON
std::string parse_ex(const std::string &text, const ParseOptions &o = {});
std::string parse_file_json(const std::string &path);              // -> JSON
std::string dump(const std::string &json);                         // JSON -> SML

} // namespace sml

#endif // SML_RS_HPP
