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
// 错误码约定 (W10) —— 与 c/sml.h 同一口径，各端一致
//
//   失败时写入 err 的消息**以错误码开头**，码与文案之间用一个空格分隔，形如
//       "E-LIMIT-001 嵌套过深（超过 128 层），疑似递归或恶意输入"
//   码内不含空格，故调用方取「第一个空格之前」的部分即为码：
//       std::string code = err.substr(0, err.find(' '));   // -> "E-LIMIT-001"
//   码是稳定契约、文案不是：码相同即同一件事，各端措辞可以不同。
//   码表（唯一事实来源）见 errors/codes.sml；C++ 侧宏见 c/sml_codes.h，
//   **用宏而非手打字符串**。
//   err 允许为 nullptr，此时文案被丢弃，但「失败」仍由返回值 nullptr 表达。
//   没有例外：连内存分配失败也带码（E-LIMIT-010）—— std::make_shared 抛出的
//   std::bad_alloc 会在 Parser::parse / apply_contract 内被转成该码。
//
// ⚠️ 为什么码写进「消息前缀」而不是只放返回值：原生实现的报错出口只有 err 一个
//    通道，做成前缀后调用方既能整条展示、也能取首个 token 当码用（与 C 侧同样的
//    取舍，见 c/sml.h 的「错误码约定」）。
// ---------------------------------------------------------------------------
class Parser {
public:
    // 解析 SML 文本。失败返回 nullptr，并把以码开头的消息写进 err（见上）。
    // include_dir 非空时启用 `@include "file"` 的文本内联与越界校验。
    // ⚠️ 只认 `@include`：裸 `include "file"` 会被当成普通键（不展开、不报错）。
    static ValuePtr parse(const std::string& text,
                          std::string* err = nullptr,
                          const std::string& include_dir = "");

    // 序列化回 SML 文本（round-trip friendly）。无错误通道。
    static std::string to_sml(const ValuePtr& v);

    // 对已解析的值应用契约（用于 @is）。失败返回 false，并把带码的消息写进 err。
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
