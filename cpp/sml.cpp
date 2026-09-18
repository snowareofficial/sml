// SPDX-License-Identifier: MulanPSL-2.0
// sml.cpp - SNOWARE Markup Language, C++17 zero-dependency implementation.
// 100% semantic alignment with the Rust `swsml` crate (rust/src/lib.rs).
#include "sml.hpp"
// 错误码宏：由 errors/gen_codes.py 从 errors/codes.sml 生成，勿手改。
#include "../c/sml_codes.h"

#include <cctype>
#include <cerrno>
#include <cstdlib>
#include <cmath>
#include <sstream>
#include <fstream>
#include <iomanip>
#include <limits>
#include <new>
#include <algorithm>
#include <filesystem>
#include <iostream>

namespace sml {

// ===========================================================================
// 错误码（W10）：码是稳定契约、文案不是 —— 见 errors/codes.sml。
// ===========================================================================
// 原生实现的报错出口只有 `std::string* err` 一个缓冲区，故把码写进同一条
// 消息的**最前面**，形如 `E-LEX-001 sml: unterminated quoted string`：
// 调用方既能整条展示，也能取首个空格前的 token 当码用。
static std::string code_prefix(const char* code, const std::string& msg) {
    return std::string(code) + " " + msg;
}
// 首段是否形如 `E-LEX-001`（用于把内层错误的码提到最前）。
static bool is_code_token(const std::string& t) {
    if (t.size() < 5) return false;
    if ((t[0] != 'E' && t[0] != 'W' && t[0] != 'I') || t[1] != '-') return false;
    for (size_t k = 2; k < t.size(); k++) {
        char c = t[k];
        if (c == '-') continue;
        if (!((c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9'))) return false;
    }
    return true;
}
// 外壳包装（如 `field 'x': ...`）：内层错误已带码时把该码提到最前，避免
// 一条消息里出现两个码、或码被埋进中间；内层无码时退回 fallback_code。
static std::string wrap_error(const std::string& detail, const std::string& shell,
                              const char* fallback_code) {
    size_t sp = detail.find(' ');
    std::string first = (sp == std::string::npos) ? detail : detail.substr(0, sp);
    if (is_code_token(first)) {
        std::string rest = (sp == std::string::npos) ? std::string() : detail.substr(sp + 1);
        return first + " " + shell + rest;
    }
    return code_prefix(fallback_code, shell + detail);
}

static bool is_xdigit_(unsigned char c){
    return (c>='0'&&c<='9')||(c>='A'&&c<='F')||(c>='a'&&c<='f');
}

// ===========================================================================
// Value::clone
// ===========================================================================
ValuePtr Value::clone() const {
    auto out = std::make_shared<Value>();
    out->tag = tag;
    out->b = b; out->i = i; out->f = f; out->s = s;
    for (auto& e : arr) out->arr.push_back(e->clone());
    for (auto& kv : obj) out->obj.push_back({kv.first, kv.second->clone()});
    return out;
}

// ===========================================================================
// Helpers
// ===========================================================================
static std::string lower(std::string s) {
    std::transform(s.begin(), s.end(), s.begin(),
                   [](unsigned char c){ return (char)std::tolower(c); });
    return s;
}
static bool is_digit(char c){ return c >= '0' && c <= '9'; }
static bool is_space(char c){ return c==' '||c=='\t'||c=='\n'||c=='\r'; }

// unescape \u{XXXX} / \uXXXX inside a quoted string body (inner, without quotes)
static std::string unescape_unicode(const std::string& inner, std::string* err) {
    std::string out;
    size_t i = 0, n = inner.size();
    while (i < n) {
        char c = inner[i];
        if (c == '\\' && i + 1 < n && inner[i+1] == 'u') {
            // \u{XXXX} or \uXXXX
            size_t j = i + 2;
            bool brace = (j < n && inner[j] == '{');
            if (brace) j++;
            std::string hex;
            while (j < n && (is_xdigit_((unsigned char)inner[j]) ||
                             (inner[j]>='A'&&inner[j]<='F') ||
                             (inner[j]>='a'&&inner[j]<='f'))) {
                hex.push_back(inner[j]); j++;
            }
            if (brace) {
                if (j >= n || inner[j] != '}') { if(err)*err=code_prefix(SML_E_LEX_005, "sml: bad \\u{...} escape"); return out; }
                j++; // consume }
            }
            if (hex.empty()) { if(err)*err=code_prefix(SML_E_LEX_005, "sml: empty unicode escape"); return out; }
            // parse hex
            unsigned long cp = 0;
            try { cp = std::stoul(hex, nullptr, 16); } catch(...) { if(err)*err=code_prefix(SML_E_LEX_005, "sml: bad unicode codepoint"); return out; }
            // encode UTF-8
            if (cp <= 0x7F) out.push_back((char)cp);
            else if (cp <= 0x7FF) {
                out.push_back((char)(0xC0 | (cp >> 6)));
                out.push_back((char)(0x80 | (cp & 0x3F)));
            } else if (cp <= 0xFFFF) {
                out.push_back((char)(0xE0 | (cp >> 12)));
                out.push_back((char)(0x80 | ((cp >> 6) & 0x3F)));
                out.push_back((char)(0x80 | (cp & 0x3F)));
            } else {
                out.push_back((char)(0xF0 | (cp >> 18)));
                out.push_back((char)(0x80 | ((cp >> 12) & 0x3F)));
                out.push_back((char)(0x80 | ((cp >> 6) & 0x3F)));
                out.push_back((char)(0x80 | (cp & 0x3F)));
            }
            i = j;
            continue;
        }
        out.push_back(c);
        i++;
    }
    return out;
}

// ===========================================================================
// Tokenizer  (mirrors Rust tokenize)
// ===========================================================================
struct Token {
    enum class T { Word, Colon, LBrace, RBrace, LBracket, RBracket, Comma, At, Dollar } t;
    std::string s; // for Word
    size_t line = 1;
    Token(Token::T tt, const std::string& ss="", size_t ln=1): t(tt), s(ss), line(ln){}
};

static std::vector<Token> tokenize(const std::string& text, std::string* err) {
    std::vector<Token> toks;
    size_t i = 0, n = text.size();
    size_t line = 1;
    while (i < n) {
        char c = text[i];
        char c2 = (i + 1 < n) ? text[i+1] : '\0';

        if (c == '\n') { line++; i++; continue; }
        if (is_space(c)) { i++; continue; }

        // comments
        if (c == '#') { while (i < n && text[i] != '\n') i++; continue; }
        if (c == '-' && c2 == '-') { while (i < n && text[i] != '\n') i++; continue; }
        if (c == '/' && c2 == '/') { while (i < n && text[i] != '\n') i++; continue; }
        if (c == '/' && c2 == '*') {
            i += 2;
            while (i + 1 < n && !(text[i] == '*' && text[i+1] == '/')) {
                if (text[i] == '\n') line++;
                i++;
            }
            i += 2; continue;
        }
        if (c == '_' && c2 == '*') {
            i += 2;
            while (i + 1 < n && !(text[i] == '*' && text[i+1] == '_')) {
                if (text[i] == '\n') line++;
                i++;
            }
            i += 2; continue;
        }

        switch (c) {
            case '{': toks.emplace_back(Token::T::LBrace, "{", line); i++; continue;
            case '}': toks.emplace_back(Token::T::RBrace, "}", line); i++; continue;
            case '[': toks.emplace_back(Token::T::LBracket, "[", line); i++; continue;
            case ']': toks.emplace_back(Token::T::RBracket, "]", line); i++; continue;
            case ',': toks.emplace_back(Token::T::Comma, ",", line); i++; continue;
            case ':': toks.emplace_back(Token::T::Colon, ":", line); i++; continue;
            case '@': toks.emplace_back(Token::T::At, "@", line); i++; continue;
            case '$': toks.emplace_back(Token::T::Dollar, "$", line); i++; continue;
            case '"': {
                i++;
                std::string buf;
                bool closed = false;
                while (i < n) {
                    char cc = text[i];
                    if (cc == '"') { closed = true; i++; break; }
                    if (cc == '\\' && i + 1 < n) {
                        char e = text[i+1];
                        if (e == 'n') buf.push_back('\n');
                        else if (e == 't') buf.push_back('\t');
                        else if (e == 'r') buf.push_back('\r');
                        else if (e == '0') buf.push_back('\0');
                        /* 接受集必须与 Rust 的严格集一致（见 sml-lex/src/lib.rs:210 的
                           自述文案）：\n \t \r \0 \" \\ \uXXXX。
                           此前这里还接受 C 风格的 \a \b \f \v \'，而 Rust 对它们报
                           E-LEX-004（「未知转义」）—— 同一输入两端行为不同。按
                           E-LEX-004 的 impls([rust cpp]) 与「严格策略」note 收窄。 */
                        else if (e == '\\') buf.push_back('\\');
                        else if (e == '"') buf.push_back('"');
                        else if (e == 'u') {
                            // \u{XXXX} or \uXXXX handled later in unescape_unicode
                            buf.push_back('\\'); buf.push_back('u');
                            i += 2;
                            // copy the rest of the escape verbatim until non-hex (or closing brace)
                            while (i < n) {
                                char h = text[i];
                                if (h == '}') { buf.push_back('}'); i++; break; }
                                if (is_xdigit_( (unsigned char)h ) ||
                                    (h>='A'&&h<='F')||(h>='a'&&h<='f')) {
                                    buf.push_back(h); i++;
                                } else break;
                            }
                            continue;
                        }
                        else {
                            /* 未知转义：**报错，绝不静默保留**。
                               codes.sml 的 E-LEX-004 明确把 cpp 列进 impls，note 是
                               「严格策略：未知转义即失败，避免路径与正则被静默损坏」——
                               但此处此前把 `\` 与字符双双塞回 buf 静默放行
                               （探针实测 `k: "\z"` 返回成功、无错误，见 _w10_probe_log.txt
                               「LEX-004 unknown escape | ok=1 | err=」）。
                               报错后立即收手，与未闭合字符串走同一出口约定。 */
                            if (err) *err = code_prefix(SML_E_LEX_004,
                                std::string("sml: 字符串含未知转义符 \\") + e +
                                "（仅支持 \\n \\t \\r \\0 \\\" \\\\ \\uXXXX）");
                            return toks;
                        }
                        i += 2;
                        continue;
                    }
                    buf.push_back(cc);
                    if (cc == '\n') line++;
                    i++;
                }
                if (!closed) { if(err)*err=code_prefix(SML_E_LEX_001, "sml: unterminated quoted string"); return toks; }
                // store quoted literal with surrounding quotes to mark as string
                toks.emplace_back(Token::T::Word, "\"" + buf + "\"", line);
                continue;
            }
            default: break;
        }

        // bare word: read until whitespace or structural char
        // Note: '@','$' handled above as separate tokens ONLY at word start;
        // inside a word they are ordinary characters.
        std::string buf;
        while (i < n) {
            char cc = text[i];
            if (is_space(cc)) break;
            if (cc == '{' || cc == '}' || cc == '[' || cc == ']' ||
                cc == ',' || cc == ':' || cc == '#') break;
            if (cc == '/' && i+1 < n && (text[i+1]=='/' || text[i+1]=='*')) break;
            if (cc == '-' && i+1 < n && text[i+1]=='-') break;
            if (cc == '_' && i+1 < n && text[i+1]=='*') break;
            buf.push_back(cc);
            i++;
        }
        if (!buf.empty()) toks.emplace_back(Token::T::Word, buf, line);
    }
    return toks;
}

// ===========================================================================
// coerce  (mirrors Rust coerce / coerce_word)
// ===========================================================================
static bool looks_like_int(const std::string& t) {
    if (t.empty()) return false;
    size_t s = 0;
    if (t[0]=='+'||t[0]=='-') s=1;
    if (s>=t.size()) return false;
    bool any=false;
    for (size_t k=s;k<t.size();k++){ if(!is_digit(t[k])) return false; any=true; }
    return any;
}
static bool looks_like_float(const std::string& t) {
    // matches Rust float regex
    // ^[+-]?(\d+\.\d*|\.\d+|\d+)([eE][+-]?\d+)?$
    std::string s = t;
    size_t i=0;
    if (!s.empty() && (s[0]=='+'||s[0]=='-')) i=1;
    if (i>=s.size()) return false;
    bool has_dot=false, has_digit=false, has_e=false;
    for (; i<s.size(); i++){
        char c=s[i];
        if (is_digit(c)) has_digit=true;
        else if (c=='.') { if(has_dot||has_e) return false; has_dot=true; }
        else if (c=='e'||c=='E') { if(!has_digit||has_e) return false; has_e=true; has_digit=false; }
        else if ((c=='+'||c=='-') && has_e && (i+1<s.size()) && is_digit(s[i+1])) { /* ok */ }
        else return false;
    }
    return has_digit && (has_dot || has_e);
}

static ValuePtr coerce_word(const std::string& raw,
                            const std::map<std::string,ValuePtr>& fragments,
                            std::string* err) {
    std::string t = raw;
    // $env.VAR inline (bareword)
    if (t.rfind("$env.", 0) == 0) {
        const char* e = std::getenv(t.c_str()+5);
        return Value::string(e ? e : "");
    }
    // fragment reference &name
    if (t.size() > 0 && t[0] == '&') {
        auto it = fragments.find(t.substr(1));
        if (it != fragments.end()) return it->second->clone();
        return Value::string(t); // undefined -> keep as-is
    }
    // bool
    if (t == "true")  return Value::boolean(true);
    if (t == "false") return Value::boolean(false);
    if (t == "null")  return Value::null();
    // number
    if (looks_like_float(t)) {
        /* 用 strtod 而不是 std::stod：Rust 的 `parse::<f64>()` 对**溢出**给 ±inf、
           对**下溢**给 0（`sml-lex/src/lib.rs:405-411`），而 stod 在两种情况下都抛
           out_of_range（errno=ERANGE 即抛），一个 catch 接不住两种语义 ——
           若在 catch 里一律返回 inf，`1e-400` 就会被误判成 inf。
           strtod 天然给出 ±HUGE_VAL(inf) / 0（或次正规数），与 Rust 一致。
           looks_like_float 已保证整串是合法十进制浮点，无需再看 endptr。 */
        double f = std::strtod(t.c_str(), nullptr);
        /* 记录 raw 的能力本实现没有（Rust 是 Value::Float(f64, Option<String>)）；
           这是「1e10 不再被写成 10000000000.0」那类保真度议题，不在此处扩范围。 */
        return Value::floating(f);
    }
    if (looks_like_int(t)) {
        /* ⚠️ errno 必须**进 stoll 之前清零**。std::stoll 成功时不碰 errno，
           所以若同一线程先前有过一次 strtod/stoll 溢出（文档里出现 `1e400`
           或超 i64 的大整数即可），errno 会一直停在 ERANGE —— 于是**后面每一个
           普通整数**都会命中下面那条 Float 分支变成 Float(123.0)。
           后果不是"类型标签不好看"：声明为 int 的字段会被契约判成类型不符，
           对**合法数据**报出**不该报的 E-CONTRACT-002**（实测复现）。
           修的是 errno 串味；「超 i64 的整数该归 Float 还是 Str」是值模型
           设计问题，不在这里顺手决定。 */
        errno = 0;
        try {
            long long v = std::stoll(t);
            /* ⚠️ 这个分支**不可达**，而且**不该可达**：
               (a) libstdc++ 的 stoll 溢出是抛 std::out_of_range（不是返回 + 置
                   errno），且 errno 已在上方清零，所以条件永远为假；
               (b) 更要紧的是语义 —— 按 Rust 的 B10（sml-lex/src/lib.rs:379-388），
                   **超出 i64 的纯整数应保留为 Str**（round-trip 安全、零精度损失），
                   **不是** Float。故正确出口是 catch 之后的 `return Value::string(t)`。
               保留此分支只为防御性说明；建议后续删除（已列入最终报告的待办）。 */
            if (errno == ERANGE) return Value::floating(std::stod(t));
            return Value::integer(v);
        } catch(...) {}
    }
    return Value::string(t);
}

static ValuePtr coerce(const Token& tok,
                       const std::map<std::string,ValuePtr>& fragments,
                       std::string* err) {
    if (tok.t != Token::T::Word) return Value::null();
    const std::string& t = tok.s;
    if (!t.empty() && t[0] == '"' && t.back() == '"') {
        std::string inner = t.substr(1, t.size()-2);
        // $env."VAR" or $env.VAR (quoted)
        if (inner.rfind("$env.", 0) == 0) {
            const char* e = std::getenv(inner.c_str()+5);
            return Value::string(e ? e : "");
        }
        std::string ue = unescape_unicode(inner, err);
        return Value::string(ue);
    }
    return coerce_word(t, fragments, err);
}

// ===========================================================================
// Contract parsing  (mirrors Rust parse_contract / parse_type)
// ===========================================================================
static bool parse_type(const std::string& raw, TypeSpec& out, std::string* err) {
    // trim
    std::string s = raw;
    size_t a=s.find_first_not_of(" \t"); size_t b=s.find_last_not_of(" \t");
    if (a==std::string::npos){ if(err)*err=code_prefix(SML_E_PARSE_024, "sml: empty type"); return false; }
    s = s.substr(a, b-a+1);

    // enum:  enum [ a b c ]   (or enum [ "a" "b" ])
    if (s.rfind("enum", 0)==0) {
        std::string rest = s.substr(4);
        size_t ab = rest.find('[');
        if (ab==std::string::npos){ if(err)*err=code_prefix(SML_E_PARSE_023, "sml: enum needs [ ... ]"); return false; }
        size_t bb = rest.find(']', ab);
        if (bb==std::string::npos){ if(err)*err=code_prefix(SML_E_PARSE_024, "sml: enum missing ]"); return false; }
        std::string body = rest.substr(ab+1, bb-ab-1);
        TypeSpec sp; sp.kind = TypeSpec::Kind::Enum;
        std::istringstream iss(body);
        std::string tok;
        while (iss >> tok) {
            if (!tok.empty() && tok[0]=='"' && tok.back()=='"') tok = tok.substr(1, tok.size()-2);
            sp.enum_values.push_back(tok);
        }
        out = sp;
        return true;
    }
    // array:  [ Type ]
    if (s[0]=='[') {
        size_t bb = s.rfind(']');
        if (bb==std::string::npos){ if(err)*err=code_prefix(SML_E_PARSE_024, "sml: array missing ]"); return false; }
        std::string elem = s.substr(1, bb-1);
        TypeSpec sp; sp.kind = TypeSpec::Kind::Array;
        TypeSpec e;
        if (elem.empty()) e.kind = TypeSpec::Kind::Any;
        else if (!parse_type(elem, e, err)) return false;
        sp.elem = std::make_shared<TypeSpec>(e);
        out = sp;
        return true;
    }
    // scalar / contract ref
    std::string low = lower(s);
    if (low=="str" || low=="string") out = TypeSpec{TypeSpec::Kind::Str};
    else if (low=="int" || low=="integer") out = TypeSpec{TypeSpec::Kind::Int};
    else if (low=="num" || low=="number" || low=="float") out = TypeSpec{TypeSpec::Kind::Num};
    else if (low=="bool" || low=="boolean") out = TypeSpec{TypeSpec::Kind::Bool};
    else if (low=="any") out = TypeSpec{TypeSpec::Kind::Any};
    else {
        // contract reference
        TypeSpec sp; sp.kind = TypeSpec::Kind::ContractRef; sp.contract_ref = s;
        out = sp;
    }
    return true;
}

/* min/max 边界的字面量解析（与 Rust `parse_spec_number` 同口径，见
   rust/sml-parse/src/parser.rs:568）。三件事必须一起做，缺一条就会出现
   「声明了边界、却按另一个边界校验」或「边界被静默丢弃」：

     1) 按 **f64** 解析 —— 不能用 std::stoll：它按 strtoll 语义把 "0.5" 截断成 0
        （且**不抛异常**），于是 `max 0.5` 的上界变成 0（合法值被误判越界）、
        `min 0.5` 的下界变成 0（越界值被漏放）。C 侧用 atof、Rust 用 f64，
        只有 C++ 会丢小数。
     2) 非数字要报码 —— `E-PARSE-022`（errors/codes.sml 该条 note 正指此条件）。
        原先 `catch(...){}` 把 stoll 的失败整个吞掉，边界被静默丢弃。
     3) 非有限数要报码 —— `E-CONTRACT-010`。NaN 的一切比较均为假，会让该边界
        被静默绕过（Rust 侧 parser.rs:574 的「审计 #2」记的就是这个洞）。

   返回 false 时已写好 err（err 允许为 NULL，与全局口径一致）。 */
static bool parse_bound(const std::string& lit, std::optional<double>& out,
                        const char* what, std::string* err) {
    /* 与 Rust 的 f64 FromStr 对齐：先认 nan / inf / infinity（可带正负号），
       其余必须命中 looks_like_* 认得的十进制写法（该正则与 Rust 的同源）。
       先做闸门是为了挡住 std::stod 的**宽松前缀**解析：它会接受 "1abc" 并
       返回 1.0，而 Rust 的 `parse::<f64>()` 会因尾随字符报 E-PARSE-022。 */
    std::string low = lower(lit);
    std::size_t s = (!low.empty() && (low[0] == '+' || low[0] == '-')) ? 1 : 0;
    std::string mag = low.substr(s);
    bool special = (mag == "nan" || mag == "inf" || mag == "infinity");
    if (!special && !looks_like_float(lit) && !looks_like_int(lit)) {
        if (err) *err = code_prefix(SML_E_PARSE_022,
            std::string("sml: ") + what + " 边界取值不是数字: " + lit);
        return false;
    }

    double bound = 0.0;
    try {
        bound = std::stod(lit);
    } catch (const std::out_of_range&) {
        /* 超出 double 表示范围（如 1e400）：Rust 的 `parse::<f64>()` 给 inf，
           所以这里也按 inf 处理，走下面「非有限数」那条码，而不是语法错误。 */
        bound = std::numeric_limits<double>::infinity();
    } catch (...) {
        if (err) *err = code_prefix(SML_E_PARSE_022,
            std::string("sml: ") + what + " 边界取值不是数字: " + lit);
        return false;
    }

    if (!std::isfinite(bound)) {
        if (err) *err = code_prefix(SML_E_CONTRACT_010,
            std::string("sml: ") + what + " 边界必须为有限值: " + lit);
        return false;
    }
    out = bound;
    return true;
}

static bool parse_field(const std::string& line, FieldSpec& out, std::string* err) {
    // line like:  name: Type optional default "x" min 0 max 10 loose
    std::istringstream iss(line);
    std::vector<std::string> parts;
    std::string p;
    while (iss >> p) parts.push_back(p);
    if (parts.empty()){ if(err)*err=code_prefix(SML_E_PARSE_024, "sml: empty field"); return false; }

    // name (strip trailing colon if present)
    std::string name = parts[0];
    if (!name.empty() && name.back()==':') name.pop_back();

    TypeSpec type{TypeSpec::Kind::Any};
    TypeModifiers mods;

    bool in_type = false;
    std::string type_buf;
    for (size_t k=1; k<parts.size(); ) {
        std::string w = parts[k];
        if (w==":") { k++; continue; }   // skip the colon separator token
        if (w=="optional") { mods.optional=true; k++; continue; }
        if (w=="loose")   { mods.loose=true; k++; continue; }
        if (w=="default") {
            if (k+1 >= parts.size()){ if(err)*err=code_prefix(SML_E_PARSE_021, "sml: default needs value"); return false; }
            mods.default_value = parts[k+1]; k+=2; continue;
        }
        if (w=="min") {
            if (k+1 >= parts.size()){ if(err)*err=code_prefix(SML_E_PARSE_022, "sml: min needs value"); return false; }
            if (!parse_bound(parts[k+1], mods.min, "min", err)) return false;
            k+=2; continue;
        }
        if (w=="max") {
            if (k+1 >= parts.size()){ if(err)*err=code_prefix(SML_E_PARSE_022, "sml: max needs value"); return false; }
            if (!parse_bound(parts[k+1], mods.max, "max", err)) return false;
            k+=2; continue;
        }
        // otherwise part of type spec (collect until next modifier keyword)
        // type can be multiple tokens (e.g. "enum [ a b ]" or "[ int ]")
        // We accumulate; but enum/array contain spaces. Reparse whole remainder as type.
        // Simpler: take everything from k to before next known modifier.
        std::string rest;
        while (k < parts.size()) {
            std::string m = parts[k];
            if (m=="optional"||m=="loose"||m=="default"||m=="min"||m=="max") break;
            if (!rest.empty()) rest += " ";
            rest += m;
            k++;
        }
        if (!parse_type(rest, type, err)) return false;
        in_type = true;
    }
    // if no type given, Any
    out.name = name;
    out.type = type;
    out.mods = mods;
    return true;
}

// ===========================================================================
// contract value checks  (mirrors Rust check_contract_value)
// ===========================================================================
static bool check_value(const ValuePtr& v, const TypeSpec& spec, bool loose, std::string* err) {
    switch (spec.kind) {
        case TypeSpec::Kind::Any: return true;
        case TypeSpec::Kind::Bool:
            if (v->tag==Value::Tag::Bool) return true;
            if (loose && v->tag==Value::Tag::Str && (lower(v->s)=="true"||lower(v->s)=="false")) return true;
            if (err)*err=code_prefix(SML_E_CONTRACT_002, "sml: expected bool"); return false;
        case TypeSpec::Kind::Int:
            if (v->tag==Value::Tag::Int) return true;
            if (loose && v->tag==Value::Tag::Float && v->f==std::floor(v->f)) return true;
            if (loose && v->tag==Value::Tag::Str) {
                if (looks_like_int(v->s)) return true;
            }
            if (err)*err=code_prefix(SML_E_CONTRACT_002, "sml: expected int"); return false;
        case TypeSpec::Kind::Num:
            if (v->tag==Value::Tag::Int||v->tag==Value::Tag::Float) return true;
            if (loose && v->tag==Value::Tag::Str && (looks_like_int(v->s)||looks_like_float(v->s))) return true;
            if (err)*err=code_prefix(SML_E_CONTRACT_002, "sml: expected num"); return false;
        case TypeSpec::Kind::Str:
            if (v->tag==Value::Tag::Str) return true;
            if (loose && (v->tag==Value::Tag::Int||v->tag==Value::Tag::Float||v->tag==Value::Tag::Bool)) return true;
            if (err)*err=code_prefix(SML_E_CONTRACT_002, "sml: expected str"); return false;
        case TypeSpec::Kind::Enum: {
            if (v->tag==Value::Tag::Str) {
                for (auto& e : spec.enum_values) if (e==v->s) return true;
                if (err)*err=code_prefix(SML_E_CONTRACT_006, "sml: value not in enum"); return false;
            }
            if (v->tag==Value::Tag::Int) {
                // Rust: enum also accepts a value coerced to scalar (int -> string)
                std::string s = std::to_string(v->i);
                for (auto& e : spec.enum_values) if (e==s) return true;
                if (err)*err=code_prefix(SML_E_CONTRACT_006, "sml: value not in enum"); return false;
            }
            if (err)*err=code_prefix(SML_E_CONTRACT_006, "sml: enum needs str"); return false;
        }
        case TypeSpec::Kind::Array: {
            if (v->tag!=Value::Tag::Arr){ if(err)*err=code_prefix(SML_E_CONTRACT_002, "sml: expected array"); return false; }
            if (spec.elem) {
                for (auto& e : v->arr) if (!check_value(e, *spec.elem, loose, err)) return false;
            }
            return true;
        }
        case TypeSpec::Kind::ContractRef:
            // not resolved here (top-level @is handles refs); accept
            return true;
    }
    return true;
}

static bool resolve_and_check(const ValuePtr& v, const TypeSpec& spec,
                              const std::map<std::string,Contract>& contracts,
                              bool loose, std::string* err);

static bool apply_one_field(const ValuePtr& val,
                            const std::map<std::string,Contract>& contracts,
                            const FieldSpec& f, std::string* err) {
    ValuePtr raw = val->get(f.name);
    if (raw == nullptr) {
        if (f.mods.optional) return true;
        if (f.mods.default_value) {
            ValuePtr dv = coerce(Token{Token::T::Word, *f.mods.default_value}, {}, err);
            val->obj.push_back({f.name, dv});
            raw = dv;
        } else {
            if (err)*err = code_prefix(SML_E_CONTRACT_003, "sml: missing required field '" + f.name + "'");
            return false;
        }
    }
    bool loose = f.mods.loose;
    // type check
    if (!resolve_and_check(raw, f.type, contracts, loose, err)) {
        std::string detail = (err && !err->empty()) ? *err : std::string("sml: type mismatch");
        if (err) *err = wrap_error(detail, "sml: field '" + f.name + "': ", SML_E_CONTRACT_002);
        return false;
    }
    // min/max (only meaningful for int/num scalars; Rust uses f64)
    if (f.mods.min || f.mods.max) {
        double num = 0;
        if (raw->tag==Value::Tag::Int) num = (double)raw->i;
        else if (raw->tag==Value::Tag::Float) num = raw->f;
        else { /* not numeric, min/max ignored like Rust */ }
        if (f.mods.min && num < *f.mods.min) { if(err)*err=code_prefix(SML_E_CONTRACT_005, "sml: field '" + f.name + "': below min"); return false; }
        if (f.mods.max && num > *f.mods.max) { if(err)*err=code_prefix(SML_E_CONTRACT_005, "sml: field '" + f.name + "': above max"); return false; }
    }
    return true;
}

static bool resolve_and_check(const ValuePtr& v, const TypeSpec& spec,
                              const std::map<std::string,Contract>& contracts,
                              bool loose, std::string* err) {
    if (spec.kind == TypeSpec::Kind::ContractRef) {
        auto it = contracts.find(spec.contract_ref);
        if (it == contracts.end()) { if(err)*err=code_prefix(SML_E_CONTRACT_001, "sml: unknown contract '"+spec.contract_ref+"'"); return false; }
        return Parser::apply_contract(v, contracts, spec.contract_ref, err);
    }
    return check_value(v, spec, loose, err);
}

// ===========================================================================
// Parser internals
// ===========================================================================
/* 值嵌套深度上限：与 C / Rust 侧同口径（Rust 为 MAX_VALUE_DEPTH=128）。
   `a{a{a{ … }}}` 会把递归下降一路压栈，栈溢出在 C++ 里同样接不住。 */
#define SML_MAX_VALUE_DEPTH 128

struct PState {
    std::vector<Token> toks;
    size_t i = 0;
    std::map<std::string,ValuePtr> fragments;
    std::map<std::string,Contract> contracts;
    std::string include_dir;
    std::string* err = nullptr;
    std::vector<std::string> include_stack;
    int depth = 0;              /* 当前块/数组嵌套深度（栈溢出防护） */
    /* 一旦置位就**放弃解析**：各层循环立即 break，递归随之退栈。
       为什么需要它：光把 depth 复位并不够 —— 复位不会让栈帧退回去，外层循环接着又
       从 0 往下钻，于是「128 层一轮」地反复压栈，最终照样打穿（实测）。
       中止与 err 是否存在无关（err 允许为 NULL）。 */
    bool aborted = false;
};

// forward decls
static void set_field_local(const ValuePtr& node, const std::string& k, const ValuePtr& v);

static ValuePtr parse_value(PState& st);
static ValuePtr parse_value_inner(PState& st);

static ValuePtr parse_array(PState& st) {
    auto arr = Value::array();
    bool closed = false;
    while (st.i < st.toks.size()) {
        if (st.aborted) break;          // 出错/超限后立刻收手，别继续建树
        auto& t = st.toks[st.i];
        if (t.t == Token::T::RBracket) { st.i++; closed = true; break; }
        if (t.t == Token::T::Comma) { st.i++; continue; }
        arr->arr.push_back(parse_value(st));
    }
    /* 走到文件结尾还没见到 `]` → E-PARSE-001。原先这里是**静默返回残缺数组**，
       于是 `a: [ 1` 会成功解析出一个只含 1 的数组（探针实测 ok=1）。
       `parse_array` 的每个调用点都消费过 `[`（parse_value_inner / 顶层），
       所以「没闭合」在任何调用点都是错误。 */
    if (!closed && !st.aborted) {
        if (st.err && st.err->empty())
            *st.err = code_prefix(SML_E_PARSE_001,
                                  "sml: 未闭合的数组（遇到文件结尾，缺少结束符号 ]）");
        st.aborted = true;
    }
    return arr;
}

static ValuePtr parse_block(PState& st, bool top = false, bool require_close = false);

// 深度守卫：超限就报错并**置中止标志**。返回 true 表示「不该继续深入」，调用方立即
// 返回 null。
//
// 抽成函数是因为有**两个**入口共用同一套计数（parse_value 与 parse_block_nested）——
// 守卫逻辑复制一份迟早会漂移，而漂移的后果是「某一侧的上限悄悄变大」。
//
// ⚠️ 这里**不能**改成「把 depth 复位为 0」了事（最初就是这么写的，实测仍会崩）：
// 复位不会让已经压上去的栈帧退回来，外层循环随即又从 0 开始往下钻，于是「每 128 层
// 一轮」反复压栈，10 万层块嵌套照样打穿栈。真正的收手是置 aborted，由各层循环 break
// 让栈一层层退掉。
static bool depth_exceeded(PState& st) {
    if (st.aborted) return true;      // 已经决定放弃，别再往下走
    if (st.depth < SML_MAX_VALUE_DEPTH) return false;
    if (st.err)
        *st.err = code_prefix(SML_E_LIMIT_001, "sml: 嵌套过深（超过 "
                  + std::to_string(SML_MAX_VALUE_DEPTH)
                  + " 层），疑似递归或恶意输入");
    st.aborted = true;
    return true;
}

// parse a value: object / array / scalar
//
// 守卫 wrapper：真正的实现在 parse_value_inner。包一层而不是改每个 return 点，
// 是为了不漏任何出口（parse_value 有多个提前返回）。
static ValuePtr parse_value(PState& st) {
    if (depth_exceeded(st)) return Value::null();
    st.depth++;
    ValuePtr v = parse_value_inner(st);
    if (st.depth > 0) st.depth--;
    return v;
}

// 深入一层**块**：`key { ... }`、`key @is C { ... }`、片段体 `@f { ... }`、裸块。
//
// ⚠️ 这些位置原先**直接**递归 parse_block，绕过了 parse_value 上的守卫 —— 于是 depth
// 根本不增长，`a{b{c{...}}}` 这种纯块嵌套可以一路递归下去打穿栈（栈溢出在 C++ 里同样是
// 不可捕获的崩溃）。块嵌套必须与值嵌套共用同一个 depth 计数，否则「128 层上限」只对
// 数组生效，对块形同虚设 —— 这正是审计里记的那条。四类调用点全部改走这里。
static ValuePtr parse_block_nested(PState& st) {
    if (depth_exceeded(st)) return Value::null();
    st.depth++;
    /* 四个调用点（片段体、`key @is C {`、`key {`、裸块 `type name {`）都已消费 `{`，
       故一律要求闭合 —— 缺 `}` 到 EOF 时由 parse_block 报 E-PARSE-001。 */
    ValuePtr v = parse_block(st, false, true);
    if (st.depth > 0) st.depth--;
    return v;
}

static ValuePtr parse_value_inner(PState& st) {
    if (st.i >= st.toks.size()) return Value::null();
    auto& t = st.toks[st.i];
    /* 必须**先消费 `{`** 再进 parse_block，并声明本块需要闭合的 `}`：
       - 不消费 `{` 的话，它会落在 parse_block 的「键位置」而被 E-PARSE-006 判为非法，
         但 `x: [ { a: 1 } ]` 这种「数组元素是对象」是合法写法；
       - 不声明需要闭合就发现不了 EOF 未闭合（E-PARSE-001）。 */
    if (t.t == Token::T::LBrace)  { st.i++; return parse_block(st, false, true); }
    if (t.t == Token::T::LBracket) { st.i++; return parse_array(st); }
    if (t.t == Token::T::Word) {
        ValuePtr v = coerce(t, st.fragments, st.err);
        st.i++;
        return v;
    }
    // unexpected token (e.g. '}' or ']') -> null and skip
    st.i++;
    return Value::null();
}

static ValuePtr parse_block(PState& st, bool top, bool require_close) {
    auto node = Value::object();
    std::string pending_is;   // local: @is applies only to THIS block
    /* require_close：本块是否由 `{` 开启（因而**必须**见到 `}`）。
       顶层由 `{` 开启的块、以及所有 parse_block_nested 的调用点都要求闭合；
       顶层裸块（`a: 1\nb: 2` 这种没有外层花括号的）到文件结尾收尾是合法的。
       缺 `}` 走到 EOF 要报 E-PARSE-001，而不是静默返回一棵残缺的树
       （原先 `a {` 会解析成 {a:{}}，探针实测 ok=1）。 */
    bool closed = !require_close;
    // support both { ... } and bare block; caller has consumed '{' if any
    // Here we are at the first token INSIDE a block (caller passed after '{' or at start).
    // For top-level, we are at token 0.
    while (st.i < st.toks.size()) {
        if (st.aborted) break;          // 出错/超限后立刻收手，别继续建树
        auto& tok = st.toks[st.i];
        if (tok.t == Token::T::RBrace) { st.i++; closed = true; break; }
        if (tok.t == Token::T::RBracket) {
            if (top) { /* top-level array handled elsewhere */ }
            // stray ']' in block -> error
            if (st.err) *st.err = code_prefix(SML_E_PARSE_003, "sml: unexpected ']' at line " + std::to_string(tok.line));
            st.i++; break;
        }
        if (tok.t == Token::T::Comma) { st.i++; continue; }

        if (tok.t == Token::T::At) {
            // directive keywords are handled by the directive branch below; do not
            // treat them as fragments here.
            bool is_directive = (st.i+1 < st.toks.size() && st.toks[st.i+1].t == Token::T::Word &&
                (st.toks[st.i+1].s == "is" || st.toks[st.i+1].s == "contract" ||
                 st.toks[st.i+1].s == "version" || st.toks[st.i+1].s == "include" ||
                 st.toks[st.i+1].s == "include!"));
            if (!is_directive) {
            // fragment @name { } or @name type args { }
            // (Rust: fragments parsed but ignored in tree; stored in snippets)
            st.i++;
            if (st.i >= st.toks.size()) break;
            std::string fname = st.toks[st.i].s; st.i++;
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Colon) st.i++;
            // optional type + name
            std::string ftype, fname_arg;
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Word &&
                st.toks[st.i].s != "{") {
                ftype = st.toks[st.i].s; st.i++;
                if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Word &&
                    st.toks[st.i].s != "{") {
                    fname_arg = st.toks[st.i].s; st.i++;
                }
            }
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::LBrace) {
                st.i++; // consume {
                auto sub = parse_block_nested(st);
                if (!ftype.empty()) {
                    sub->obj.push_back({"__type", Value::string(ftype)});
                    if (!fname_arg.empty()) sub->obj.push_back({"__name", Value::string(fname_arg)});
                }
                st.fragments[fname] = sub;
            }
            } // end if (!is_directive)
            if (!is_directive) continue;
        }

        // directive: @version / @include / @contract (tokenized as At + Word)
        if (tok.t == Token::T::At && st.i+1 < st.toks.size() &&
            st.toks[st.i+1].t == Token::T::Word) {
            std::string dir = st.toks[st.i+1].s;
            if (dir == "version") {
                st.i += 2; // consume @ version
                if (st.i < st.toks.size()) st.i++; // skip value
                continue;
            }
            if (dir == "include" || dir == "include!") {
                st.i += 2;
                if (st.i < st.toks.size()) {
                    std::string path = st.toks[st.i].s;
                    if (!path.empty() && path[0]=='"' && path.back()=='"') path = path.substr(1, path.size()-2);
                    if (!st.include_dir.empty()) {
                        std::string full = st.include_dir + "/" + path;
                        /* 路径穿越防护：规范化后必须仍在基准目录之内。
                           缺这道校验时 `@include "../../etc/passwd"` 会把任意文件读进来。 */
                        namespace fs = std::filesystem;
                        std::error_code ec1, ec2;
                        fs::path canon = fs::weakly_canonical(fs::path(full), ec1);
                        fs::path basec = fs::weakly_canonical(fs::path(st.include_dir), ec2);
                        /* 初值 false = fail-closed：规范化失败时**拒绝**，
                           而不是"比不了就放行"。 */
                        bool inside = false;
                        if (!ec1 && !ec2) {
                            inside = true;
                            auto cit = canon.begin();
                            for (auto bit = basec.begin(); bit != basec.end(); ++bit, ++cit) {
                                if (cit == canon.end() || *cit != *bit) { inside = false; break; }
                            }
                        }
                        if (!inside && st.err)
                            *st.err = code_prefix(SML_E_INCLUDE_003, "sml: include 目标越出基准目录: " + path);
                        /* 越界时不打开任何文件（传空路径必然失败），控制流保持原样 */
                        std::ifstream f(inside ? full : std::string());
                        if (f) {
                            std::stringstream ss; ss << f.rdbuf();
                            std::string inc = ss.str();
                            bool cyc=false;
                            for (auto& s:st.include_stack) if(s==full){cyc=true;break;}
                            if (!cyc && st.include_stack.size() < 32) {
                                st.include_stack.push_back(full);
                                std::string e2;
                                auto sub_toks = tokenize(inc, &e2);
                                st.toks.insert(st.toks.begin() + (long long)st.i,
                                               sub_toks.begin(), sub_toks.end());
                                st.include_stack.pop_back();
                            }
                        } else if (inside && st.err) {
                            /* 目标不存在或读不出来 → E-INCLUDE-001。
                               codes.sml 该条的 impls 明列 cpp（[rust c cpp js lua]），
                               但此前这里是**静默跳过**：探针实测 `@include "nope.sml"`
                               返回成功且无任何错误（见 _w10_probe_log.txt 的
                               「INCLUDE-001 missing | ok=1 | err=」）。
                               闸在 `inside` 上是必须的：越界那条已经写过 E-INCLUDE-003，
                               不设此闸会把正确的码覆盖成 E-INCLUDE-001（码反而报错）。 */
                            *st.err = code_prefix(SML_E_INCLUDE_001,
                                                  "sml: include 读取失败: " + path);
                        }
                    }
                    st.i++;
                }
                continue;
            }
            if (dir == "contract") {
                st.i += 2; // consume @ contract
            // parse: name { fields... }
            if (st.i >= st.toks.size()) break;
            std::string cname = st.toks[st.i].s; st.i++;
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Colon) st.i++;
            // contract-level `loose` (allow undeclared fields)
            Contract c; c.name = cname;
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Word && st.toks[st.i].s=="loose") {
                c.allow_extra = true; st.i++;
            }
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::LBrace) {
                st.i++; // consume {
                while (st.i < st.toks.size() && st.toks[st.i].t != Token::T::RBrace) {
                    // each field is "name: Type mods..." until next field (newline not tokenized)
                    // We collect tokens until we see a Word followed by Colon that starts a new field,
                    // OR until RBrace. Simpler: accumulate raw token strings until next 'name:' pattern.
                    std::string fieldbuf;
                    while (st.i < st.toks.size()) {
                        auto& ft = st.toks[st.i];
                        if (ft.t == Token::T::RBrace) break;
                        // new field detection: Word ':' where previous was not part of type
                        // Use heuristic: a Word immediately followed by Colon and not inside [ ]
                        if (ft.t == Token::T::Word && st.i+1 < st.toks.size() &&
                            st.toks[st.i+1].t == Token::T::Colon && !fieldbuf.empty()) {
                            break;
                        }
                        if (!fieldbuf.empty()) fieldbuf += " ";
                        fieldbuf += ft.s;
                        st.i++;
                    }
                    if (!fieldbuf.empty()) {
                        FieldSpec fs;
                        std::string e;
                        if (parse_field(fieldbuf, fs, &e)) c.fields.push_back(fs);
                        else if (st.err) *st.err = e;
                    }
                }
                if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::RBrace) st.i++;
            }
            st.contracts[cname] = c;
            continue;
        }
        if (tok.t == Token::T::At && st.i+1 < st.toks.size() &&
            st.toks[st.i+1].t == Token::T::Word && st.toks[st.i+1].s == "is") {
            st.i += 2;
            // @is ContractName  -> applied after this block (Rust applies to current block)
            if (st.i < st.toks.size()) {
                std::string cname = st.toks[st.i].s; st.i++;
                // store pending; applied after block completes
                pending_is = cname;
            }
            continue;
        }
        } // close `if (tok.t == At ...)`

        // normal key
        if (tok.t != Token::T::Word) {
            /* 键位置只接受裸词/引号串（引号串在 tokenizer 里也是 Word）。
               其余**结构记号是语法错误**，不能静默跳过 —— Rust 在同一位置报
               E-PARSE-006（parser.rs:967-970 的 `_ => … 期望键, 得 {:?}`）。
               原先这里是 `st.i++; continue;`，于是 `a { { x } }` 会静默解析成功
               （探针实测 ok=1）。注意 `{` 落在这里只可能是**多余的**：所有合法
               的 `{` 都由调用方先消费（见 parse_value_inner 与 parse_block_nested）。
               ⚠️ `$` 例外，见下。 */
            const bool bad_token = (tok.t == Token::T::LBrace ||
                                    tok.t == Token::T::LBracket ||
                                    tok.t == Token::T::Colon);
            if (bad_token) {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_PARSE_006,
                                          "sml: 期望键或标识符，得结构记号");
                st.aborted = true;
                break;
            }
            /* `$` 在本实现的 tokenizer 里是**独立 token**，而 Rust 的 Tok 没有它
               （`$` 属于裸词，见 js/sml.mjs 的注释）。在这里报 E-PARSE-006 会与
               Rust 相反（Rust 把 `$` 当键名接受），故保持跳过 —— 已登记待定，
               与 E-PARSE-006 的其余口径分开处理。 */
            st.i++; continue;
        }
        std::string key = tok.s;
        st.i++;
        bool colon = false;
        if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Colon) { colon=true; st.i++; }
        if (st.i >= st.toks.size()) {
            // key with nothing after -> coerce key itself as value (Rust: bare block name case)
            set_field_local(node, key, coerce_word(key, st.fragments, st.err));
            break;
        }
        auto& nxt = st.toks[st.i];
        if (nxt.t == Token::T::At && st.i+1 < st.toks.size() &&
            st.toks[st.i+1].t == Token::T::Word && st.toks[st.i+1].s == "is") {
            // key @is Contract { ... }   -> set key = block, apply contract
            st.i += 2; // consume @ is
            if (st.i < st.toks.size()) {
                std::string cname = st.toks[st.i].s; st.i++;
                if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::LBrace) {
                    st.i++;
                    auto sub = parse_block_nested(st);
                    std::string e;
                    Parser::apply_contract(sub, st.contracts, cname, &e);
                    if (!e.empty() && st.err) *st.err = e;
                    set_field_local(node, key, sub);
                } else {
                    set_field_local(node, key, Value::null());
                }
            }
            continue;
        }
        if (nxt.t == Token::T::LBrace) {
            st.i++;
            auto sub = parse_block_nested(st);
            set_field_local(node, key, sub);
        } else if (nxt.t == Token::T::LBracket) {
            st.i++;
            auto arr = parse_array(st);
            set_field_local(node, key, arr);
        } else if (!colon && nxt.t==Token::T::Word && nxt.s != "}" && nxt.s != "]" && nxt.s != ",") {
            // bare block: key is type, subsequent tokens until '{' are args
            std::vector<std::string> args;
            while (st.i < st.toks.size() && st.toks[st.i].t != Token::T::LBrace &&
                   st.toks[st.i].t != Token::T::RBrace && st.toks[st.i].t != Token::T::Comma) {
                args.push_back(st.toks[st.i].s); st.i++;
            }
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::LBrace) {
                st.i++;
                auto sub = parse_block_nested(st);
                sub->obj.push_back({"__type", Value::string(key)});
                if (args.size()==1) sub->obj.push_back({"__name", Value::string(args[0])});
                set_field_local(node, key, sub);
            } else {
                // no body -> treat as scalar string
                set_field_local(node, key, coerce_word(key, st.fragments, st.err));
            }
        } else if (nxt.t == Token::T::Word) {
            ValuePtr v = coerce(nxt, st.fragments, st.err);
            st.i++;
            set_field_local(node, key, v);
        } else if (nxt.t == Token::T::RBrace || nxt.t==Token::T::RBracket) {
            // key }  -> key itself is the value (Rust: coerce key)
            set_field_local(node, key, coerce_word(key, st.fragments, st.err));
        } else {
            // nxt is some structural token with no value
            set_field_local(node, key, Value::null());
            if (nxt.t==Token::T::Comma) st.i++;
        }
    }

    /* 由 `{` 开启却走到文件结尾 → E-PARSE-001（原先静默返回残缺的树）。
       置 aborted 与 err 是否存在无关：调用方可以不传 err，但「解析已失败」这件事
       照样要让整体失败（与深度守卫同一约定）。 */
    if (!closed && !st.aborted) {
        if (st.err && st.err->empty())
            *st.err = code_prefix(SML_E_PARSE_001,
                                  "sml: 未闭合的块（遇到文件结尾，缺少结束符号 }）");
        st.aborted = true;
    }

    // apply pending @is for this block
    /* ⚠️ 已中止时**不再应用** @is：否则契约错误会覆盖掉上面刚写好的
       E-PARSE-001/E-LIMIT-001，把码报成别的（码是契约，不许被后来者改写）。 */
    if (!st.aborted && !pending_is.empty()) {
        std::string cname = pending_is;
        pending_is.clear();
        std::string e;
        Parser::apply_contract(node, st.contracts, cname, &e);
        if (!e.empty() && st.err) *st.err = e;
    }
    return node;
}

// standalone set_field used inside parse_block (promote same-name to array)
static void set_field_local(const ValuePtr& node, const std::string& k, const ValuePtr& v) {
    for (auto& kv : node->obj) {
        if (kv.first == k) {
            if (kv.second->tag == Value::Tag::Arr) {
                kv.second->arr.push_back(v);
            } else {
                auto arr = Value::array();
                arr->arr.push_back(kv.second);
                arr->arr.push_back(v);
                kv.second = arr;
            }
            return;
        }
    }
    node->obj.push_back({k, v});
}

// Value::array_with helper
ValuePtr Value::array_with(const std::vector<ValuePtr>& elems) {
    auto a = Value::array();
    a->arr = elems;
    return a;
}

// ===========================================================================
// Parser::parse
// ===========================================================================
/* 真正的解析实现。公开入口 `Parser::parse` 是它的薄包装，只为接住
   std::bad_alloc（E-LIMIT-010）；抽成 static 而不是给函数体加 try 再整体缩进，
   是为了让这次改动只有两行、看得清。 */
static ValuePtr parse_impl(const std::string& text, std::string* err, const std::string& include_dir) {
    PState st;
    st.include_dir = include_dir;
    st.err = err;
    st.toks = tokenize(text, err);
    if (err && !err->empty()) return nullptr;
    if (st.toks.empty()) return Value::object();

    auto& first = st.toks[0];
    ValuePtr result;
    if (first.t == Token::T::LBracket) {
        st.i = 1;
        result = parse_array(st);
    } else if (first.t == Token::T::LBrace) {
        st.i = 1;
        result = parse_block(st, false, true);   // 由 `{` 开启 → 必须见到 `}`
    } else {
        result = parse_block(st, true);          // 顶层裸块：EOF 收尾合法
    }
    // aborted 必须独立于 err 判断：调用方可以不传 err（为 NULL 时文案被丢弃），
    // 但「解析已中止」这件事照样要让整体失败，否则会静默返回一棵被截断的树。
    if (st.aborted || (err && !err->empty())) return nullptr;
    return result;
}

// 公开入口：把分配失败也变成带码的失败。见 sml.hpp 的「错误码约定」——
// 「没有例外：连内存分配失败也带码」。std::make_shared / 容器增长在 OOM 时抛
// std::bad_alloc，本实现只有 err 一条通道，故在此统一转码后按失败返回。
ValuePtr Parser::parse(const std::string& text, std::string* err, const std::string& include_dir) {
    try {
        return parse_impl(text, err, include_dir);
    } catch (const std::bad_alloc&) {
        if (err) *err = code_prefix(SML_E_LIMIT_010, "sml: 内存分配失败");
        return nullptr;
    }
}

// ===========================================================================
// Parser::apply_contract
// ===========================================================================
/* 同上：真正的契约校验实现，公开入口只负责接 std::bad_alloc。
   注意 resolve_and_check 里对嵌套契约的递归走的是公开入口 Parser::apply_contract，
   于是每层各有一个 try —— 开销可忽略，换来「任何一层的分配失败都带码」。 */
static bool apply_contract_impl(const ValuePtr& val,
                                const std::map<std::string,Contract>& contracts,
                                const std::string& name,
                                std::string* err) {
    auto it = contracts.find(name);
    if (it == contracts.end()) { if(err)*err=code_prefix(SML_E_CONTRACT_001, "sml: unknown contract '"+name+"'"); return false; }
    if (val->tag != Value::Tag::Obj) { if(err)*err=code_prefix(SML_E_CONTRACT_008, "sml: contract applied to non-object"); return false; }
    const Contract& c = it->second;
    // strict mode (default): reject undeclared fields unless contract is `loose`
    if (!c.allow_extra) {
    for (auto& kv : val->obj) {
        if (kv.first == "__type" || kv.first == "__name") continue;
        bool declared = false;
        for (auto& f : c.fields) if (f.name == kv.first) { declared=true; break; }
        if (!declared) {
            if (err)*err = code_prefix(SML_E_CONTRACT_004, "sml: field '" + kv.first + "' not declared in contract '" + name + "'");
            return false;
        }
    }
    }
    for (auto& f : c.fields) {
        if (!apply_one_field(val, contracts, f, err)) return false;
    }
    return true;
}

// 公开入口（同上：只为接住分配失败）。
bool Parser::apply_contract(const ValuePtr& val,
                            const std::map<std::string,Contract>& contracts,
                            const std::string& name,
                            std::string* err) {
    try {
        return apply_contract_impl(val, contracts, name, err);
    } catch (const std::bad_alloc&) {
        if (err) *err = code_prefix(SML_E_LIMIT_010, "sml: 内存分配失败");
        return false;
    }
}

// ===========================================================================
// to_sml  (round-trip friendly, mirrors Rust to_sml)
// ===========================================================================
static void dump_value(const ValuePtr& v, int indent, std::string& out);

static void dump_inline_obj(const ValuePtr& v, std::string& out) {
    std::string parts;
    bool first = true;
    for (auto& kv : v->obj) {
        if (kv.first=="__type"||kv.first=="__name") continue;
        if (!first) parts += ", ";
        first = false;
        std::string vs;
        if (kv.second->tag==Value::Tag::Str) {
            if (kv.second->s.find(' ') != std::string::npos || kv.second->s.empty())
                vs = "\"" + kv.second->s + "\"";
            else vs = kv.second->s;
        } else {
            std::string tmp; dump_value(kv.second, 0, tmp); vs = tmp;
        }
        parts += kv.first + ": " + vs;
    }
    out += parts;
}

static void dump_value(const ValuePtr& v, int indent, std::string& out) {
    std::string pad((size_t)indent*2, ' ');
    switch (v->tag) {
        case Value::Tag::Null: out += "null"; break;
        case Value::Tag::Bool: out += v->b ? "true" : "false"; break;
        case Value::Tag::Int:  out += std::to_string(v->i); break;
        case Value::Tag::Float: {
            // shortest representation
            std::ostringstream os; os << std::setprecision(17) << v->f;
            out += os.str();
            break;
        }
        case Value::Tag::Str:
            if (v->s.find(' ') != std::string::npos || v->s.empty())
                out += "\"" + v->s + "\"";
            else out += v->s;
            break;
        case Value::Tag::Arr: {
            if (v->arr.empty()) { out += "[]"; break; }
            out += "[\n";
            for (auto& e : v->arr) {
                out += pad + "  ";
                if (e->tag==Value::Tag::Obj) {
                    out += "{ ";
                    dump_inline_obj(e, out);
                    out += " }\n";
                } else {
                    std::string tmp; dump_value(e, indent+1, tmp);
                    out += tmp + "\n";
                }
            }
            out += pad + "]";
            break;
        }
        case Value::Tag::Obj: {
            bool has_body = false;
            for (auto& kv : v->obj) if (kv.first!="__type"&&kv.first!="__name"){has_body=true;break;}
            if (!has_body) { out += "{}"; break; }
            out += "\n" + pad + "{";
            for (auto& kv : v->obj) {
                if (kv.first=="__type"||kv.first=="__name") continue;
                out += "\n" + pad + "  " + kv.first + ": ";
                dump_value(kv.second, indent+1, out);
            }
            out += "\n" + pad + "}";
            break;
        }
    }
}

std::string Parser::to_sml(const ValuePtr& v) {
    std::string out;
    // top-level array
    if (v->tag == Value::Tag::Arr) {
        dump_value(v, 0, out);
        out += "\n";
        return out;
    }
    if (v->tag == Value::Tag::Obj) {
        std::string body;
        for (auto& kv : v->obj) {
            if (kv.first=="__type"||kv.first=="__name") continue;
            out += kv.first + ": ";
            std::string tmp; dump_value(kv.second, 0, tmp);
            out += tmp + "\n";
        }
        return out;
    }
    dump_value(v, 0, out);
    out += "\n";
    return out;
}

} // namespace sml
