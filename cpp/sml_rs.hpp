// SPDX-License-Identifier: MulanPSL-2.0
// SML v1..v4 桥接头文件（Rust cdylib 后端，C++ 封装）。
// 仓库：https://gitee.com/snoware/sml
// @file sml_rs.hpp
// @brief SML v1..v4 C++ 桥接封装：经 Rust cdylib 获得 v1..v4 全集能力。
// @version 由 sml_version() 提供（不在此写死包版本）

/*
 * sml_rs.hpp — C++17 wrapper around the SML v1..v4 Rust cdylib backend (sml_rs.h).
 *
 * Complements sml.hpp (the pure C++ native implementation): this header does
 * NOT implement a parser, it bridges the `swsml` Rust crate's cdylib to gain
 * the full v1..v4 feature set ($env inlining / glob-include / @feature / @contract).
 *
 * Build (installed, via pkg-config — library names follow the Debian packages):
 *   g++ -std=c++17 example_rs.cpp sml_rs.cpp $(pkg-config --cflags --libs sml) -o example_rs
 * Build (in-repo, before installing):
 *   g++ -std=c++17 example_rs.cpp sml_rs.cpp -I../c -L../../target/release -lsml -o example_rs
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
// 双轨 include：安装布局（deb）走 <sml/sml_rs.h>，仓库内构建退回相对路径。
#if __has_include(<sml/sml_rs.h>)
#include <sml/sml_rs.h>      /* 安装布局（deb） */
#else
#include "../c/sml_rs.h"     /* 仓库内构建 */
#endif

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
    std::string text;   // human-readable message (**without** the code)
    // 错误码，如 "E-PARSE-008"（见 errors/codes.sml）。
    // code 是粗粒度枚举（供 switch），code_str 才是稳定契约：同因同码，且不随文案改写而变。
    std::string code_str;

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
 * Diagnostics (non-fatal)
 *
 * Key conflict (W-PARSE-001) / fragment overwrite (W-PARSE-002) / weak
 * declaration mismatch (W-CONTRACT-001) never abort parsing; without this
 * exit they are simply dropped. Fatal problems still go through `Error`.
 * ------------------------------------------------------------------ */

enum class Severity {
    Warning = SML_SEV_WARNING,
    Error = SML_SEV_ERROR, // reserved for the multi-error exit
};

struct Diagnostic {
    std::string code_str; // "W-PARSE-001"; empty when this kind has no code
    std::string text;     // human-readable message (**without** the code)
    int line = 0;         // 1-based; 0 when unknown
    Severity severity = Severity::Warning;
};

struct DiagLoadResult {
    Value value;
    Error error;
    std::vector<Diagnostic> diagnostics;

    bool ok() const { return error.ok() && value.valid(); }
    explicit operator bool() const { return ok(); }
};

// Parse text and hand out the non-fatal diagnostics too. The value tree is
// byte-identical to loads() with the same flags.
DiagLoadResult loads_with_diags(const std::string &text, unsigned flags = 0);

/* ------------------------------------------------------------------ *
 * Builtin contract table + full options (v3.1)
 *
 * A builtin table is the host's own contract set; `@weakbind` soft checks
 * compare against it (docs/swsml/spec/weakbind.md §6.1). It cannot come from
 * JSON (contracts carry validators), hence the step-by-step builder.
 * ------------------------------------------------------------------ */

class Builtin {
public:
    Builtin() = default;
    ~Builtin() { reset(); }

    Builtin(const Builtin &) = delete;
    Builtin &operator=(const Builtin &) = delete;
    Builtin(Builtin &&o) noexcept : p_(o.p_), owns_(o.owns_) { o.p_ = nullptr; }
    Builtin &operator=(Builtin &&o) noexcept {
        if (this != &o) {
            reset();
            p_ = o.p_;
            owns_ = o.owns_;
            o.p_ = nullptr;
        }
        return *this;
    }

    // Allocate an empty table; an empty table disables the soft check.
    static Builtin create();

    // Borrow a table owned elsewhere (e.g. `Result::contracts()`).
    // The wrapper must NOT outlive its owner and never frees it.
    static Builtin borrow(const sml_builtin *p);

    // Take ownership of a table (e.g. the clone from `sml_builtin_clone`).
    static Builtin adopt(const sml_builtin *p);

    bool valid() const { return p_ != nullptr; }
    std::size_t count() const; // number of contracts
    const sml_builtin *raw() const { return p_; }

    // Builders. false = bad argument (NULL / unknown type / no such field);
    // on failure the table is left untouched.
    bool add_field(const std::string &contract, const std::string &field,
                   const std::string &type, bool required);
    bool set_range(const std::string &contract, const std::string &field,
                   double min, double max);
    bool add_enum(const std::string &contract, const std::string &field,
                  const std::string &value);

    void reset();

private:
    sml_builtin *p_ = nullptr;
    bool owns_ = true; // borrow() 时为 false：句柄归别人，析构不释放
};

struct Opts {
    unsigned flags = 0;                // SML_F_* cap; 0 = baseline (no type.weak)
    std::vector<std::string> env;      // "KEY=VALUE" entries
    const Builtin *builtins = nullptr; // may be null
};

// ------------------------------------------------------------------ *
// Weak side tables (v3.1)
//
// `@weakbind` / `@weakdata` / `@weakdoc` never enter the value tree; they are
// handed out here. RAII: the whole set is released on destruction, including
// each sml_weakdata payload (never free those separately).
// ------------------------------------------------------------------ */

class WeakLists {
public:
    WeakLists() = default;
    ~WeakLists() { reset(); }

    WeakLists(const WeakLists &) = delete;
    WeakLists &operator=(const WeakLists &) = delete;

    // Release everything; the object is reusable afterwards.
    void reset();

    // Target slot for the C call (resets first, then returns the inner struct).
    sml_weaklists *out();

    std::size_t n_binds() const { return l_.n_binds; }
    std::size_t n_data() const { return l_.n_data; }
    std::size_t n_docs() const { return l_.n_docs; }

    const sml_weakbind *binds() const { return l_.binds; }
    const sml_weakdata *data() const { return l_.data; }
    const sml_weakdoc *docs() const { return l_.docs; }

private:
    sml_weaklists l_{};
};

// Parse text with the full option set; the value tree is byte-identical to
// loads(text, flags) with the same flags.
DiagLoadResult loads_full(const std::string &text, const Opts &opts);

// Same, plus the three weak side tables (lists may be null).
DiagLoadResult loads_full(const std::string &text, const Opts &opts, WeakLists *lists);

// File entry: expands `include` relative to the file's own directory.
DiagLoadResult load_file_full(const std::string &path, const Opts &opts, WeakLists *lists = nullptr);

/* ------------------------------------------------------------------ *
 * Aggregated result (v3.1)
 *
 * One parse -> one handle -> getters. Everything the handle owns dies with
 * it; the value and the tables are BORROWED views.
 * ------------------------------------------------------------------ */

// SML_WANT_* bits, spelled for C++.
enum Want : unsigned {
    WantDiags = SML_WANT_DIAGS,
    WantErrors = SML_WANT_ERRORS,
    WantWeak = SML_WANT_WEAK,
    WantContractMap = SML_WANT_CONTRACT_MAP,
    WantContracts = SML_WANT_CONTRACTS,
};

class Result {
public:
    Result() = default;
    ~Result() { reset(); }

    Result(const Result &) = delete;
    Result &operator=(const Result &) = delete;
    Result(Result &&o) noexcept : r_(o.r_) { o.r_ = nullptr; }
    Result &operator=(Result &&o) noexcept;

    // Parse text; `err` (optional) receives the fatal error when the result is empty.
    static Result parse(const std::string &text, const Opts &opts, unsigned want,
                        Error *err = nullptr);

    bool valid() const { return r_ != nullptr; }
    explicit operator bool() const { return valid(); }

    Ref value() const { return Ref(sml_result_value(r_)); }          // borrowed
    std::vector<Diagnostic> diagnostics() const;                     // copies
    std::vector<Diagnostic> errors() const;                          // severity == Error
    const sml_weaklists *weak() const { return sml_result_weak(r_); }
    const sml_contractmap *contract_map() const { return sml_result_contract_map(r_); }
    const sml_builtin *contracts() const { return sml_result_contracts(r_); }

    // Owned clone of the captured table: stays usable after this Result dies.
    Builtin contracts_owned() const { return Builtin::adopt(sml_builtin_clone(sml_result_contracts(r_))); }

    void reset();

private:
    sml_result *r_ = nullptr;
};

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
