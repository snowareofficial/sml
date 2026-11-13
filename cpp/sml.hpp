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
// 弱绑定（`@weakbind`）—— 内置契约表 + 旁路表
// `docs/swsml/spec/weakbind.md`：库里**不内置任何业务契约**，内置表由宿主注入。
// 软校验**只读**：不填默认值、不删改键，`Value` 一个字段都不动，只出
// `W-CONTRACT-001` 诊断。
// ---------------------------------------------------------------------------
struct BuiltinField {
    std::string type = "any";           // "any" / "str" / "int" / "num" / "bool" / "enum"
    bool required = false;
    bool has_min = false, has_max = false;
    double min = 0.0, max = 0.0;
    std::vector<std::string> values;    // type == "enum" 时的取值
};

struct BuiltinContract {
    std::map<std::string, BuiltinField> fields;
};

// 宿主侧的内置契约表（与 C 的 `sml_builtin` 同定位，机制按 C++ 惯例）。
class BuiltinTable {
public:
    void add_field(const std::string& contract, const std::string& field,
                   const std::string& type, bool required = false);
    void set_range(const std::string& contract, const std::string& field,
                   double min, double max);
    void add_enum(const std::string& contract, const std::string& field,
                  const std::string& value);
    const std::map<std::string, BuiltinContract>& all() const { return contracts_; }
private:
    std::map<std::string, BuiltinContract> contracts_;
};

// 一条弱绑定记录（旁路产物，**不进主树**）。
struct WeakBind {
    std::string path;   // 键名用 `.` 连接，数组元素写成 `[n]`（如 `srv.web`）
    std::string name;   // 契约名
    int line = 0;       // `@weakbind` 所在行（1 起；未知 0）
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
// 一条诊断（warning）：码是稳定契约、文案不是（与 Rust `ParseOutput.diagnostics` 同定位）。
// One diagnostic; codes are the stable contract, messages are not.
struct Diag {
    std::string code;      // 如 "W-PARSE-001"
    std::string message;
    int line = 0;          // 1 起；未知时 0
};

// `parse_with` 的选项：能力按字段扩，不再把参数越加越长。
// 功能对等口径见 `docs/swsml/plan/crossend-sync.md` §0.6。
struct ParseOptions {
    // include_dir 非空时启用 include 的文本内联与越界校验；为空 = 关闭。
    std::string include_dir;
    // 诊断出口（可为 nullptr = 不收集）。
    std::vector<Diag>* diags = nullptr;
    // 内置契约表（宿主注入）；nullptr = 软校验整段跳过（空表护栏）。
    const BuiltinTable* builtins = nullptr;
    // 弱绑定旁路表出口（可为 nullptr = 不收集）。
    std::vector<WeakBind>* weak_binds = nullptr;
};

class Parser {
public:
    // 解析 SML 文本。失败返回 nullptr，并把以码开头的消息写进 err（见上）。
    // include_dir 非空时启用 include 的文本内联与越界校验。
    // 两种写法都认（2026-11-03 起与其余四端对齐）：`@include "file"` 与**行首**
    // `include "file"` / `import "file"`。`k: include "x"` 这类带冒号的是普通字段。
    // include_dir 为空 = 关闭 include：此时**行首裸**形式会明确报 `E-FEATURE-001`，
    // 而不是静默产出 `include: "x.sml"` 这条错树（`@` 形式仍留给解析器当指令吃掉）。
    static ValuePtr parse(const std::string& text,
                          std::string* err = nullptr,
                          const std::string& include_dir = "",
                          std::vector<Diag>* diags = nullptr);

    // 同 parse，选项走结构体：诊断 / 内置契约表 / 弱绑定旁路表。
    // 软校验只产出 `W-CONTRACT-001`，**绝不改变返回值**。
    static ValuePtr parse_with(const std::string& text, const ParseOptions& opts,
                               std::string* err = nullptr);

    // 文件入口：读文件 + 以**文件所在目录**为 include 基准（`opts.include_dir` 非空则不覆盖）。
    // 诊断 / 弱表 / 内置表仍走 `opts` —— 与 `parse_with` 一套出参，不另造形状。
    // File entry: reads the file and defaults include_dir to its directory.
    static ValuePtr load_file(const std::string& path, const ParseOptions& opts,
                              std::string* err = nullptr);
    static ValuePtr load_file(const std::string& path, std::string* err = nullptr);

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
