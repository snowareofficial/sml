// SPDX-License-Identifier: MulanPSL-2.0
// sml.hpp - SNOWARE Markup Language, C++17 zero-dependency implementation.
// 100% semantic alignment with the Rust `swsml` crate.
#ifndef SML_HPP
#define SML_HPP

#include <string>
#include <vector>
#include <map>
#include <memory>
#include <optional>
#include <variant>

namespace sml {

// ---------------------------------------------------------------------------
// Value model
// ---------------------------------------------------------------------------
struct Value;
using ValuePtr = std::shared_ptr<Value>;

struct Value {
    // tags
    enum class Tag { Null, Bool, Int, Float, Str, Arr, Obj } tag = Tag::Null;

    bool        b   = false;
    long long   i   = 0;
    double      f   = 0.0;
    std::string s;
    std::vector<ValuePtr> arr;
    // ordered object preserving insertion order (BTreeMap in Rust)
    std::vector<std::pair<std::string, ValuePtr>> obj;

    Value() = default;

    static ValuePtr null()  { auto v = std::make_shared<Value>(); v->tag = Tag::Null;  return v; }
    static ValuePtr boolean(bool x){ auto v=std::make_shared<Value>(); v->tag=Tag::Bool; v->b=x; return v; }
    static ValuePtr integer(long long x){ auto v=std::make_shared<Value>(); v->tag=Tag::Int; v->i=x; return v; }
    static ValuePtr floating(double x){ auto v=std::make_shared<Value>(); v->tag=Tag::Float; v->f=x; return v; }
    static ValuePtr string(const std::string& x){ auto v=std::make_shared<Value>(); v->tag=Tag::Str; v->s=x; return v; }
    static ValuePtr array()  { auto v=std::make_shared<Value>(); v->tag=Tag::Arr; return v; }
    static ValuePtr object() { auto v=std::make_shared<Value>(); v->tag=Tag::Obj; return v; }
    static ValuePtr array_with(const std::vector<ValuePtr>& elems);

    // object helpers
    bool has(const std::string& k) const {
        for (auto& kv : obj) if (kv.first == k) return true;
        return false;
    }
    ValuePtr get(const std::string& k) const {
        for (auto& kv : obj) if (kv.first == k) return kv.second;
        return nullptr;
    }
    // deep copy (preserves __type / __name meta)
    ValuePtr clone() const;
};

// ---------------------------------------------------------------------------
// Contract system (mirrors Rust TypeSpec / TypeModifiers / Contract)
// ---------------------------------------------------------------------------
struct TypeSpec {
    enum class Kind { Any, Str, Int, Num, Bool, Enum, Array, ContractRef } kind = Kind::Any;
    // Enum: allowed list of string values
    std::vector<std::string> enum_values;
    // Array: element type
    std::shared_ptr<TypeSpec> elem;
    // ContractRef: referenced contract name
    std::string contract_ref;
};

struct TypeModifiers {
    bool optional = false;
    std::optional<std::string> default_value;   // raw token (coerced at apply time)
    std::optional<double> min;
    std::optional<double> max;
    bool loose = false;                         // (unused at field level; kept for API)
};

struct FieldSpec {
    std::string name;
    TypeSpec    type;
    TypeModifiers mods;
};

struct Contract {
    std::string name;
    std::vector<FieldSpec> fields;
    bool allow_extra = false;   // contract-level `loose`
};

// ---------------------------------------------------------------------------
// Parser / API
// ---------------------------------------------------------------------------
class Parser {
public:
    // Parse SML text. On error returns nullptr and sets err.
    // include_dir enables `include "file"` / `@include "file"` text-inlining.
    static ValuePtr parse(const std::string& text,
                          std::string* err = nullptr,
                          const std::string& include_dir = "");

    // Serialize a Value back to SML text (round-trip friendly).
    static std::string to_sml(const ValuePtr& v);

    // Apply a contract to an already-parsed value (used by @is).
    static bool apply_contract(const ValuePtr& val,
                               const std::map<std::string, Contract>& contracts,
                               const std::string& name,
                               std::string* err = nullptr);
};

// Convenience free functions
inline ValuePtr parse(const std::string& text, std::string* err = nullptr,
                      const std::string& include_dir = "") {
    return Parser::parse(text, err, include_dir);
}
inline std::string to_sml(const ValuePtr& v) { return Parser::to_sml(v); }

} // namespace sml

#endif // SML_HPP
