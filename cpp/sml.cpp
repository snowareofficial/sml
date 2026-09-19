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
            if (!brace && hex.size() != 4) { if(err)*err=code_prefix(SML_E_LEX_005, "sml: unicode 转义位数不足（须四位十六进制或 \\u{...}）"); return out; }
            // parse hex
            unsigned long cp = 0;
            try { cp = std::stoul(hex, nullptr, 16); } catch(...) { if(err)*err=code_prefix(SML_E_LEX_005, "sml: bad unicode codepoint"); return out; }
            if (cp > 0x10FFFF || (cp >= 0xD800 && cp <= 0xDFFF)) { if(err)*err=code_prefix(SML_E_LEX_005, "sml: unicode 码点非法（超出范围或可代理区）"); return out; }
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
            bool closed = false;
            while (i + 1 < n && !(text[i] == '*' && text[i+1] == '/')) {
                if (text[i] == '\n') line++;
                i++;
            }
            if (i + 1 < n) { i += 2; closed = true; }
            if (!closed) { if (err) *err = code_prefix(SML_E_LEX_002,
                             "sml: 未闭合的块注释 /* ... */（遇到文件结尾）"); return toks; }
            continue;
        }
        if (c == '_' && c2 == '*') {
            i += 2;
            bool closed = false;
            while (i + 1 < n && !(text[i] == '*' && text[i+1] == '_')) {
                if (text[i] == '\n') line++;
                i++;
            }
            if (i + 1 < n) { i += 2; closed = true; }
            if (!closed) { if (err) *err = code_prefix(SML_E_LEX_003,
                             "sml: 未闭合的块注释 _* ... *_（遇到文件结尾）"); return toks; }
            continue;
        }

        switch (c) {
            case '{': toks.emplace_back(Token::T::LBrace, "{", line); i++; continue;
            case '}': toks.emplace_back(Token::T::RBrace, "}", line); i++; continue;
            case '[': toks.emplace_back(Token::T::LBracket, "[", line); i++; continue;
            case ']': toks.emplace_back(Token::T::RBracket, "]", line); i++; continue;
            case ',': toks.emplace_back(Token::T::Comma, ",", line); i++; continue;
            case ':': toks.emplace_back(Token::T::Colon, ":", line); i++; continue;
            case '@': toks.emplace_back(Token::T::At, "@", line); i++; continue;
            /* ⚠️ `$` **不再**切出独立 token：Rust 的词法器不把 `$` 当特殊字符，
               `$env.NAME` 整个是一个**裸词**（`coerce_word` 里早有 `$env.` 分支，
               见 sml-lex/src/lib.rs 的 coerce_word 与 "$env 内联" 的既有注释）。
               改前这里发 Token::T::Dollar，于是值位置的 `k: $env.X` 被切成
               `$` + `env.X` 两个 token：值退化成 null，而 `env.X` 掉到**键位置变出一个新键**
               —— 实测 examples/secrets.sml 得到 `resendApiKey: null` 与凭空多出的
               `env.RESEND_API_KEY` 键（Rust 是 `resendApiKey: ""`）。
               删掉这一支后，下面的裸词循环会把 `$env.X` 整体读成一个词，与 Rust 同构。 */

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
        if (err) *err = code_prefix(SML_E_INCLUDE_006, "sml: 片段引用 " + t + " 指向一个不存在的片段");
        return Value::null();
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
    std::string* err = nullptr;
    /* 原先这里还有 include_dir / include_stack 两个字段：include 是**边解析边插 token**
       处理的（W18 前的写法），所以状态得挂在解析器上。现在展开挪到解析前的
       `expand_includes` 里，链栈是那次递归的局部变量，解析器不必再知道 include 的存在。 */
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
static ValuePtr parse_block_nested(PState& st);   // 定义在后（带深度守卫）

/* 本实现把**引号串**也存成 Word token（见 `coerce` 里的
   `t[0]=='"' && t.back()=='"'`），而 Rust 里引号串是独立的 `Tok::Str` ——
   那条分支**不试裸块**。故预扫描要显式排掉它，否则 `[ "sec" { x: 1 } ]`
   会被当成「类型名 "sec" 的裸块」（实测踩过一次）。 */
static bool is_quoted_word(const Token& tk) {
    return tk.t == Token::T::Word && tk.s.size() >= 2 &&
           tk.s.front() == '"' && tk.s.back() == '"';
}

/* 预扫描：当前位置起「连续词之后紧跟 `{`」⇒ 是裸块的参数部分。
   对应 Rust `bare_block_ahead()`。两点照抄它：
   ① 入口**只认「裸词」**（引自上面的 is_quoted_word）—— Rust 的 `Some(Tok::Str(_))`
      分支直接当字符串元素，故 `[ "sec" { } ]` 两端都**不是**块；
   ② 撞上别的 token（含 `]`、EOF）即「不是」⇒ `[ hello world ]` 仍是两个标量元素。 */
static bool bare_block_ahead(const PState& st) {
    if (st.i >= st.toks.size() || st.toks[st.i].t != Token::T::Word) return false;
    if (is_quoted_word(st.toks[st.i])) return false;
    size_t p = st.i;
    while (p < st.toks.size()) {
        auto tt = st.toks[p].t;
        if (tt == Token::T::Word) p++;
        else if (tt == Token::T::LBrace) return true;
        else return false;
    }
    return false;
}

/* 数组元素位置的裸块 `type [name…] { … }`（W4 ③ B 类）。
   改前这里只有「当标量」一条路 ⇒ `[ section 情节 { x: 1 } ]` 被拆成 `section`、`情节`、
   `{ x: 1 }` **三个**元素，而 Rust 是**一个**块对象。
   与键位置的裸块（parse_block 里那一段）同构：`__type` = 类型词、首个参数 ⇒ `__name`、
   其余 ⇒ `__args`（参数不再静默丢弃）。
   前置条件：当前位置是**类型词**；本函数消费掉整个「类型词 [参数…] { … }」。 */
static ValuePtr parse_bare_block(PState& st) {
    std::string type_word = st.toks[st.i].s;
    st.i++;                                   // 先消费类型名（Rust: self.next()）
    std::vector<ValuePtr> args;
    while (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word) {
        args.push_back(coerce(st.toks[st.i], st.fragments, st.err));
        st.i++;
    }
    if (st.i >= st.toks.size() || st.toks[st.i].t != Token::T::LBrace) return Value::null();
    st.i++;                                   // `{`
    auto sub = parse_block_nested(st);
    if (sub && sub->tag == Value::Tag::Obj) {
        sub->obj.push_back({"__type", Value::string(type_word)});
        if (!args.empty()) {
            sub->obj.push_back({"__name", args[0]});
            if (args.size() > 1) {
                auto extra = Value::array();
                for (size_t k = 1; k < args.size(); k++) extra->arr.push_back(args[k]);
                sub->obj.push_back({"__args", extra});
            }
        }
    }
    return sub;
}

static ValuePtr parse_array(PState& st) {
    auto arr = Value::array();
    bool closed = false;
    while (st.i < st.toks.size()) {
        if (st.aborted) break;          // 出错/超限后立刻收手，别继续建树
        auto& t = st.toks[st.i];
        if (t.t == Token::T::RBracket) { st.i++; closed = true; break; }
        if (t.t == Token::T::RBrace) {
            // 数组里多余的 `}`（没有与之匹配的开始符号）⇒ E-PARSE-003（W16）
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_PARSE_003, "sml: 数组里出现多余的 '}'（没有与之匹配的开始符号）");
            st.i++; st.aborted = true; break;
        }
        if (t.t == Token::T::Comma) { st.i++; continue; }
        /* 数组位置的裸块（W4 ③ B 类）：判据见 bare_block_ahead / parse_bare_block。
           ⚠️ 只放在**数组**分派处（Rust 也只在 parse_array_inner 里调 bare_block_ahead）；
           值位置 `k: section 情节 { … }` 在两端都**不是**块，别顺手也改那里。 */
        if (t.t == Token::T::Word && bare_block_ahead(st)) {
            arr->arr.push_back(parse_bare_block(st));
            continue;
        }
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

/* 深入一层**数组**：`key: [ ... ]`。
   与 parse_block_nested 同一个理由 —— 这个位置原先**直接**调 parse_array，绕过了
   parse_value 上的守卫，于是 depth 不增长，`a: [[[ … ]]]` 的**第一层数组白送一层**。
   实测（闭合嵌套扫过五端）：块嵌套上限 128，数组却到 129 —— 本实现**自身两条路径
   就不一致**。这与 W13 修的「块嵌套直接递归绕过守卫」是同一个洞，当时只补了块、漏了数组。
   （`parse_value_inner` 里的 `[` 与顶层的 `[` 不在这里：前者本就在 parse_value 的守卫
    之内，后者是**文档根**，按各端口径根不计层。） */
static ValuePtr parse_array_nested(PState& st) {
    if (depth_exceeded(st)) return Value::null();
    st.depth++;
    ValuePtr v = parse_array(st);
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
        if (tok.t == Token::T::RBrace) {
            if (require_close) { st.i++; closed = true; break; }
            // 顶层（require_close=false）遇到 `}` ⇒ 多余的（stray）→ E-PARSE-003（W16）
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_PARSE_003, "sml: 多余的 '}'（没有与之匹配的开始符号）");
            st.i++; st.aborted = true; break;
        }
        if (tok.t == Token::T::RBracket) {
            // 块内遇到 `]`（期望 `}`）⇒ 闭合符错配；顶层遇到 `]` ⇒ 多余的（stray）
            if (require_close) {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_PARSE_002, "sml: 闭合符错配（块期望 '}'，却遇到 ']'）");
            } else {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_PARSE_003, "sml: 多余的 ']'（没有与之匹配的开始符号）");
            }
            st.i++; st.aborted = true; break;
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
            // W16：未注册指令（`@foo ...`）只允许**显式** `type:` / `name:` 参数；
            //   位置参数（如 `@foo bar`）与缺少片段体 `{ ... }` ⇒ E-PARSE-005。
            //   `@foo { x: 1 }`（无参数）仍合法；`@foo type: X name: Y { ... }` 须成功。
            std::string ftype, fname_arg;
            while (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word &&
                   (st.toks[st.i].s == "type" || st.toks[st.i].s == "name") &&
                   st.i+1 < st.toks.size() && st.toks[st.i+1].t == Token::T::Colon) {
                bool is_type = (st.toks[st.i].s == "type");
                st.i += 2; // type/name + ':'
                const auto& vt = st.toks[st.i];
                if (vt.t != Token::T::Word) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_PARSE_005, "sml: 片段 `" + fname + "` 的参数缺少值");
                    st.aborted = true; st.i++; break;
                }
                if (is_type) {
                    if (!ftype.empty()) { if(st.err&&st.err->empty())*st.err=code_prefix(SML_E_PARSE_005,"sml: 片段 `"+fname+"` 的 type 参数重复"); st.aborted=true; break; }
                    ftype = vt.s;
                } else {
                    if (!fname_arg.empty()) { if(st.err&&st.err->empty())*st.err=code_prefix(SML_E_PARSE_005,"sml: 片段 `"+fname+"` 的 name 参数重复"); st.aborted=true; break; }
                    fname_arg = vt.s;
                }
                st.i++; // 消费参数值
            }
            if (!st.aborted) {
                if (!(st.i < st.toks.size() && st.toks[st.i].t == Token::T::LBrace)) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_PARSE_005,
                            "sml: `@" + fname + "` 不是合法指令且缺少片段体 { ... }；若本意是片段定义，参数须显式写作 `type: X` 与 `name: Y`（位置参数形式已废弃；不带参数时写作 `@" + fname + " { ... }`）");
                    st.aborted = true;
                    break;
                }
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
                /* 展开已在解析**之前**的 expand_includes 里做完（W18）。走到这里只剩两种
                   情形：① 调用方没传 include_dir（按 sml.hpp 的约定 = include 功能关闭）；
                   ② 这一句已在展开时被内联成子文件的 token 段。故此处只把三个 token
                   （`@`、`include`、路径）吃掉，不做文件工作。
                   ⚠️ 别在此处改回「边解析边插 token」：往 st.toks 中间插一段再靠 st.i++
                   找位置，正是 W18 那个 off-by-one 的温床 —— 先插到路径 token 之前、
                   再 st.i++ 吃掉路径，恰好跳过插入段的**首 token**，于是 `include`
                   退化成裸块键、把 includer 后续字段和被包含文件的字段一起吞掉。 */
                st.i += 2;
                if (st.i < st.toks.size()) st.i++;
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
            /* 契约级修饰符：`loose`（允许未声明字段）与 `strict`（默认；显式写出也合法）。
               ⚠️ 改前**只认 `loose`**：`@contract 事项 strict { … }` 里的 `strict` 不被消费
               ⇒ 后面那个 `{` 就不再是「紧跟契约名」⇒ 整条契约声明被跳过、紧随的块体被当成
               普通键值解析（实测 rust/tests/fixtures/gov_demo.sml 报 E-PARSE-006）。
               与 Rust 对照：`@contract X strict` 两边都应成功且 allow_extra=false。 */
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Word &&
                (st.toks[st.i].s=="loose" || st.toks[st.i].s=="strict")) {
                c.allow_extra = (st.toks[st.i].s == "loose");
                st.i++;
            }
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::LBrace) {
                st.i++; // consume {
                bool closed = false;
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
                if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::RBrace) { st.i++; closed = true; }
                /* 契约体未闭合必须报错（Rust 同格：E-PARSE-001「契约体未闭合（缺少结束符号 ）」）。
                   改前这里是 `if (…) st.i++;` —— **有 `}` 才吃、没有就算了**，于是
                   `@contract C { a: str`（缺 `}`）整份解析**成功**，与「未闭合块必报错」
                   的既有口径相反。这处松口一直存在，只是改前 strict 契约进不了本分支
                   才没被看见（探针：前缀 `@contract 事项 strict {` HEAD 报 E-PARSE-001、
                   只修 strict 的第一版反而变成 rc=0 —— 就是它）。 */
                if (!closed && !st.aborted) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_PARSE_001,
                                              "sml: 契约体未闭合（缺少结束符号 ）");
                    st.aborted = true;
                }
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
            /* 其余记号（`]` / `,` / `@`）仍**静默跳过** —— 这条兜底与上面 bad_token
               的三格是有意分开的（W16 只把那三格判成错误）。`$` 已不再是独立 token
               （见 tokenizer 里的说明），故本兜底不再有它那一格。 */
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
            /* ⚠️ 必须走**带守卫**的 parse_array_nested，不能直接调 parse_array ——
               直接调会绕过 depth 计数，让 `key: [ ... ]` 的第一层数组白送一层
               （实测：数组到 129 层而块只到 128 层）。理由见 parse_array_nested。 */
            st.i++;
            auto arr = parse_array_nested(st);
            set_field_local(node, key, arr);
        } else if (!colon && bare_block_ahead(st)) {
            // bare block: key is type, subsequent tokens until '{' are args
            /* 判据 = Rust 键位置那一支（parser.rs:1180：`if !colon && self.bare_block_ahead()`）。
               ⚠️ 改前这里只要求「后继是个词」，而且参数是**贪心**收的（一直吃到
               `{` / `}` / `,`，中间任何 token 都算参数）。那个过宽的判据会**吞掉**
               本实现后面几处语法缺陷的痕迹 —— 实测：单独收紧它会让
               `examples/secrets.sml`（`$` 独立 token）、`slint/login.sml`（反引号串）、
               `gov_demo.sml`（`@contract X strict {`）从「静默错解」变成
               `E-PARSE-006` **硬失败**。**故顺序是「先修那三处、再收紧判据」**
               （反过来做等于把能解析的文件变成不能解析）。三处修完后收紧的收益：
               `examples/common.sml` 不再把注释闭合符那一串当成裸块参数
               （改前 C++ 14 行 vs Rust 1 行）。
               参数**不再静默丢弃**：首个 ⇒ `__name`、其余 ⇒ `__args`；
               参数经 coerce（与 Rust `parse_bare_block`、C 侧一致）。 */
            std::vector<ValuePtr> args;
            while (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word) {
                args.push_back(coerce(st.toks[st.i], st.fragments, st.err));
                st.i++;
            }
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::LBrace) {
                st.i++;
                auto sub = parse_block_nested(st);
                sub->obj.push_back({"__type", Value::string(key)});
                if (!args.empty()) {
                    sub->obj.push_back({"__name", args[0]});
                    if (args.size() > 1) {
                        auto extra = Value::array();
                        for (size_t k = 1; k < args.size(); k++) extra->arr.push_back(args[k]);
                        sub->obj.push_back({"__args", extra});
                    }
                }
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
// include 展开（W18）
// ===========================================================================
/* 把 `@include "f"` 三个 token 原地换掉，换成 f 的 token 段（递归展开）。
 *
 * 为什么不继续「边解析边插 token」（W18 前的写法）：那种写法每次插入都要让 `st.i`
 * 恰好落在插入段的首 token 上，而「先插到路径 token 之前、再 st.i++ 吃掉路径」的顺序
 * 会让 st.i **跳过插入段的首 token**。后果是 `include` 退化成裸块键，把 includer 后面
 * 的字段和被包含文件的字段一起吞掉 —— 实测 a.sml = `@include "b.sml"` + `from_a: 1`
 * 解析完只剩一个垃圾键 `include = "include"`。
 *
 * ⚠️ 也**不能只改索引**：那个 off-by-one 恰好压住了无限展开（首 token 被跳过 ⇒ 嵌套的
 *    `@include` 永远不会被当成指令），而环检测用的栈当时是 push 完立刻 pop、**永远为空**，
 *    于是自包含/互包含根本拦不住。只修索引会把「被压住的无限展开」放出来变成真死循环。
 *    环检测必须同时给出：本函数用**链栈**（只装根到当前这条路径），故菱形包含仍合法。
 *
 * 语义与 Rust 的 sml-include::expand_includes / C 的 resolve_includes 对齐：
 *   - 环检测按**链栈**（只判当前路径）→ 菱形包含合法，自包含/互包含报 E-INCLUDE-002；
 *   - 嵌套深度上限 32（E-INCLUDE-004，与 Rust 的 MAX_INCLUDE_DEPTH 同值）；
 *   - 展开次数是**全局**计数（E-LIMIT-003）：深度上限挡不住菱形包含的 2^N 膨胀；
 *   - 被包含文件的基准目录换成**它自己的所在目录**（嵌套 include 相对父文件解析）；
 *   - 越界即拒绝（E-INCLUDE-003）；基准目录本身不可解析也拒绝（E-INCLUDE-010，
 *     fail-closed：不比一个比不了的对象就放行，等于把越界校验静默关掉）。
 */
#define SML_MAX_INCLUDE_DEPTH 32
#define SML_MAX_INCLUDE_EXPANSIONS 10000

static bool expand_includes(std::vector<Token>& toks,
                            const std::string& include_dir,
                            std::vector<std::string>& chain,
                            long long& expansions,
                            std::string* err) {
    namespace fs = std::filesystem;
    if (chain.size() >= SML_MAX_INCLUDE_DEPTH) {
        if (err) *err = code_prefix(SML_E_INCLUDE_004, "sml: include 嵌套超过 "
                                    + std::to_string(SML_MAX_INCLUDE_DEPTH) + " 层");
        return false;
    }
    /* 沙箱根用**严格** canonical（不用 weakly_canonical）：目录不存在时就要失败。
       weakly_canonical 只做词法规范化，一个不存在的基准目录会被它"成功"规范化，
       于是「基准目录不可解析」被降级成「目录里没这个文件」，报出另一个码。 */
    std::error_code ebase;
    fs::path basec = fs::canonical(fs::path(include_dir), ebase);
    if (ebase) {
        if (err) *err = code_prefix(SML_E_INCLUDE_010,
            "sml: include 基准目录不可解析，无法做越界校验，已拒绝继续: " + include_dir);
        return false;
    }
    std::vector<Token> out;
    out.reserve(toks.size());
    for (size_t k = 0; k < toks.size(); ) {
        const Token& t = toks[k];
        const bool is_inc = (t.t == Token::T::At && k + 2 < toks.size() &&
                             toks[k+1].t == Token::T::Word &&
                             (toks[k+1].s == "include" || toks[k+1].s == "include!") &&
                             toks[k+2].t == Token::T::Word);
        if (!is_inc) { out.push_back(t); k++; continue; }

        /* 路径**写法**非法（未加引号）⇒ `E-INCLUDE-012`。
           为什么需要这条：词法器把引号串与裸词**都存成 `Word`**（引号串只是内容两侧补了 `"`），
           于是旧实现无法区分 `@include b.sml` 与 `@include "b.sml"` —— 目标存在就照常展开、
           不存在则报 `E-INCLUDE-001`（"读取失败"），把**写法**错误导成"文件不存在"。
           与 Rust / Lua / C 对齐：明确报此码。
           注：本实现只认 `@include`（裸 `include` 是普通键），故判据天然只作用于该形式；
           而「引号未闭合」在**词法层**就已按 `E-LEX-001` 拦下（子文件里是 `E-INCLUDE-011`），
           走不到这里 —— 这条端间差异记在 `errors/codes.sml` 的 E-INCLUDE-012 note 里。 */
        if (toks[k+2].s.empty() || toks[k+2].s[0] != '"') {
            if (err) *err = code_prefix(SML_E_INCLUDE_012, "sml: include 路径写法非法（未加引号）");
            return false;
        }

        std::string path = toks[k+2].s;
        if (!path.empty() && path[0] == '"' && path.back() == '"')
            path = path.substr(1, path.size() - 2);

        /* 与 Rust 同序：先 canonicalize 目标，失败即 E-INCLUDE-001。故「越界**且**不存在」
           的路径报的是此码而不是 E-INCLUDE-003（C 侧 fopen 在前，结论相同）。 */
        std::error_code etgt;
        fs::path target = fs::canonical(fs::path(include_dir) / path, etgt);
        if (etgt) {
            if (err) *err = code_prefix(SML_E_INCLUDE_001, "sml: include 读取失败: " + path);
            return false;
        }
        /* 路径穿越防护：按**路径分量**比较，不能按字符串前缀 —— `/tmp/ab` 以 `/tmp/a`
           为字符串前缀，但那不是同一棵子树。缺这道校验时 `@include "../../etc/passwd"`
           会把任意文件内联进解析结果。 */
        bool inside = true;
        {
            auto cit = target.begin();
            for (auto bit = basec.begin(); bit != basec.end(); ++bit, ++cit) {
                if (cit == target.end() || *cit != *bit) { inside = false; break; }
            }
        }
        if (!inside) {
            if (err) *err = code_prefix(SML_E_INCLUDE_003,
                                        "sml: include 目标越出基准目录: " + path);
            return false;
        }
        const std::string canon = target.string();
        for (const auto& s : chain) {
            if (s == canon) {
                if (err) *err = code_prefix(SML_E_INCLUDE_002,
                                            "sml: include 循环引用: " + canon);
                return false;
            }
        }
        ++expansions;
        if (expansions > SML_MAX_INCLUDE_EXPANSIONS) {
            if (err) *err = code_prefix(SML_E_LIMIT_003, "sml: include 展开次数超过上限 "
                + std::to_string(SML_MAX_INCLUDE_EXPANSIONS) + "（疑似指数膨胀 DoS）");
            return false;
        }
        std::ifstream f(target, std::ios::binary);
        if (!f) {
            if (err) *err = code_prefix(SML_E_INCLUDE_001, "sml: include 读取失败: " + path);
            return false;
        }
        std::stringstream ss; ss << f.rdbuf();
        /* 被 include 的子文件也可能带 BOM：它在拼接后的中间位置，`Parser::parse` 那处的
           开头检查救不了它，而且不去掉的话子文件第一行 `@include "x"` 都认不出来。 */
        {
            std::string child = ss.str();
            if (child.size() >= 3 && static_cast<unsigned char>(child[0]) == 0xEF
                && static_cast<unsigned char>(child[1]) == 0xBB
                && static_cast<unsigned char>(child[2]) == 0xBF) {
                child.erase(0, 3);
            }
            ss.str(child);
            ss.clear();
        }
        std::string e2;
        std::vector<Token> sub = tokenize(ss.str(), &e2);
        if (!e2.empty()) {
            /* 子文件的词法失败 → E-INCLUDE-011（与 Rust 同口径：失败发生在 include
               预处理阶段）。⚠️ 修复前这里**丢掉了 e2**、把残缺 token 段插进去 ——
               子文件里的未闭合字符串会变成静默截断的文档。 */
            if (err) *err = code_prefix(SML_E_INCLUDE_011,
                                        "sml: include 预处理词法错误：" + e2);
            return false;
        }
        chain.push_back(canon);
        const bool ok = expand_includes(sub, target.parent_path().string(),
                                        chain, expansions, err);
        chain.pop_back();
        if (!ok) return false;
        out.insert(out.end(), sub.begin(), sub.end());
        k += 3;
    }
    toks.swap(out);
    return true;
}

// ===========================================================================
// Parser::parse
// ===========================================================================
/* 真正的解析实现。公开入口 `Parser::parse` 是它的薄包装，只为接住
   std::bad_alloc（E-LIMIT-010）；抽成 static 而不是给函数体加 try 再整体缩进，
   是为了让这次改动只有两行、看得清。 */
static ValuePtr parse_impl(const std::string& text, std::string* err, const std::string& include_dir) {
    PState st;
    st.err = err;
    st.toks = tokenize(text, err);
    if (err && !err->empty()) return nullptr;
    /* include 在**解析前**一次性展开完（W18）：解析器拿到的 token 流里不再有 include
       指令，也就没有「插一段 token 再对齐 st.i」这回事。include_dir 为空 = 按 sml.hpp
       的约定关闭 include，指令留给 parse_block 吃掉。 */
    if (!include_dir.empty()) {
        std::vector<std::string> chain;
        long long expansions = 0;
        if (!expand_includes(st.toks, include_dir, chain, expansions, err)) return nullptr;
    }
    if (st.toks.empty()) return Value::object();

    auto& first = st.toks[0];
    // W16 B0：顶层只能是容器（键值块/对象块/数组），单个标量无法往返 ⇒ E-PARSE-008。
    //   判定：只有一个 token 且它不是 [ 或 {（即裸词 / 引号串 / 数字，tokenizer 都记为 Word）。
    if (st.toks.size() == 1 && first.t == Token::T::Word) {
        if (err && err->empty())
            *err = code_prefix(SML_E_PARSE_008,
                "sml: 顶层只能是容器（键值块/对象块/数组），单个标量无法往返");
        return nullptr;
    }
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
        /* 文件级规范化：跳过开头的 UTF-8 BOM（EF BB BF）。BOM **不是空白** ⇒ 会被词法器吞进
           第一个单词 ⇒ 第一个键名变成 "\xEF\xBB\xBFx"（**静默**改键名）。来源很常见：
           Windows 记事本「另存为 UTF-8」。与 Rust / JS / C / Lua 对齐。 */
        const bool has_bom = text.size() >= 3
            && static_cast<unsigned char>(text[0]) == 0xEF
            && static_cast<unsigned char>(text[1]) == 0xBB
            && static_cast<unsigned char>(text[2]) == 0xBF;
        const std::string body = has_bom ? text.substr(3) : text;
        return parse_impl(body, err, include_dir);
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
//
// ⚠️ W4 ②③（2026-09-19）：本段与 Rust `sml-value/src/dump.rs` 以及 C `c/sml.c`
//    的结构**逐函数对应** —— `is_flat` / `starts_inline` / `dump_object_body` /
//    `dump_array_body` / `dump_element` / `dump_inline` / `dump_value` / `to_sml`。
//    改这里请三侧一起对照，别只改一侧（本实现的 ② 与 ③ 一直是缺的：
//    数组元素里的容器被压成一行、`__type`/`__name` 被当内部标记跳过）。
// ===========================================================================
static void dump_value(const ValuePtr& v, int indent, std::string& out);
static void dump_element(const ValuePtr& v, int indent, std::string& out);
static void dump_inline(const ValuePtr& v, std::string& out);
static void dump_object_body(const ValuePtr& v, int indent, std::string& out);
static void dump_array_body(const ValuePtr& v, int indent, std::string& out);
static bool is_flat(const ValuePtr& v);
static bool starts_inline(const ValuePtr& v);

/* 值是否要紧跟在 `键:` 之后、**同一行**开始写（Rust `starts_inline`）：
   只有**空**对象才同行；非空对象由 dump_value 先换行再写 `{` ⇒ 键后**不留**行尾空格（W4 ①）。 */
static bool starts_inline(const ValuePtr& v) {
    return !(v && v->tag == Value::Tag::Obj && !v->obj.empty());
}

/* 容器是否「扁平」：**直接子项全是标量**（不再嵌对象 / 数组）。
   与 Rust `dump.rs::is_flat`、C `sml.c::is_flat` 同一判据，是「一行写完」还是
   「展开多行」的唯一开关：
     `{ type: home }`                      扁平 → `[ { type: home } ]` 仍是一行
     `{ children: [ … ] }`                 非扁平 → `\n{ … }` 展开
   ⚠️ **只看一层，不递归**：递归版（「子孙全是标量」）是**恒真判据** —— 任何对象的
   子孙最终都会落到标量，于是所有东西都被判成扁平、排版分毫不改。Rust 侧当初写这段时
   真踩过这个坑（编译与测试全绿，只是完全没生效）。 */
static bool is_flat(const ValuePtr& v) {
    if (!v) return true;
    if (v->tag == Value::Tag::Obj) {
        for (auto& kv : v->obj) {
            const ValuePtr& fv = kv.second;
            if (fv && (fv->tag == Value::Tag::Obj || fv->tag == Value::Tag::Arr)) return false;
        }
        return true;
    }
    if (v->tag == Value::Tag::Arr) {
        for (auto& iv : v->arr)
            if (iv && (iv->tag == Value::Tag::Obj || iv->tag == Value::Tag::Arr)) return false;
        return true;
    }
    return true;   // 标量一律扁平
}

/* 写对象体：**不含**开头的 `{`，逐键换行 + 收尾 `}`。
   ⚠️ **所有键都写**，包括 `__type` / `__name`：裸块 `type [name] { … }` 解析后就长成
   这两个键，序列化必须能写回去（W4 ③ A 类）。改前这几处把它们当「内部标记」跳过，
   后果不只是每个块少两行 —— **只有元数据的块会被判成空体 ⇒ 输成 `{}`**，元数据静默消失。 */
static void dump_object_body(const ValuePtr& v, int indent, std::string& out) {
    for (auto& kv : v->obj) {
        out += "\n";
        out += std::string((size_t)(indent + 1) * 2, ' ');
        out += kv.first;
        out += starts_inline(kv.second) ? ": " : ":";
        dump_value(kv.second, indent + 1, out);
    }
    out += "\n";
    out += std::string((size_t)indent * 2, ' ');
    out += "}";
}

/* 写数组体：**不含**开头的 `[`。每元素先换行 + 缩进，再由 dump_element 决定
   「一行写完」还是「就地展开」。 */
static void dump_array_body(const ValuePtr& v, int indent, std::string& out) {
    out += "[";
    for (auto& e : v->arr) {
        out += "\n";
        out += std::string((size_t)(indent + 1) * 2, ' ');
        dump_element(e, indent + 1, out);
    }
    out += "\n";
    out += std::string((size_t)indent * 2, ' ');
    out += "]";
}

/* 把值压成**一行**写。契约：只对「扁平」值调用（见 is_flat）——
   因此它内部的递归永远不会撞上容器，压出来的行里不会再有换行。
   ⚠️ **不筛键**（含 __type / __name），与 Rust `dump_inline` 的 `m.iter()` 一致。 */
static void dump_inline(const ValuePtr& v, std::string& out) {
    if (!v) { out += "null"; return; }
    switch (v->tag) {
        case Value::Tag::Null: out += "null"; break;
        case Value::Tag::Bool: out += v->b ? "true" : "false"; break;
        case Value::Tag::Int:  out += std::to_string(v->i); break;
        case Value::Tag::Float: {
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
            out += "[ ";
            for (size_t k = 0; k < v->arr.size(); k++) {
                if (k) out += ", ";
                dump_inline(v->arr[k], out);
            }
            out += " ]";
            break;
        }
        case Value::Tag::Obj: {
            out += "{ ";
            bool first = true;
            for (auto& kv : v->obj) {
                if (!first) out += ", ";
                first = false;
                out += kv.first + ": ";
                dump_inline(kv.second, out);
            }
            out += " }";
            break;
        }
    }
}

static void dump_value(const ValuePtr& v, int indent, std::string& out) {
    if (!v) { out += "null"; return; }
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
            dump_array_body(v, indent, out);
            break;
        }
        case Value::Tag::Obj: {
            /* 空对象写 `{}`、非空写 `\n{ … }` —— 与 Rust `dump_block` 同判据
               （A 类：这里原先排除 __type/__name 判「空体」，只有元数据的块会被误判）。 */
            if (v->obj.empty()) { out += "{}"; break; }
            out += "\n";
            out += std::string((size_t)indent * 2, ' ');
            out += "{";
            dump_object_body(v, indent, out);
            break;
        }
    }
}

/* 写一个「元素」：扁平的走单行（dump_inline），含结构的**就地展开**成多行。
   调用方负责**已**写好本元素开头的换行与缩进（`indent` 即该缩进级别），
   因此这里不在开头补缩进；展开出来的续行由各自递归负责对齐。
   对应 Rust `dump.rs::dump_element`（W4 ② 的对齐点）。 */
static void dump_element(const ValuePtr& v, int indent, std::string& out) {
    if (!v) { out += "null"; return; }
    if (is_flat(v)) { dump_inline(v, out); return; }
    switch (v->tag) {
        case Value::Tag::Obj:
            /* 与 dump_value 的 Obj 分支只差这一行：这里是数组元素位置，
               `{` 要跟在**当前行**（缩进已由调用方写好），不能再另起一行。 */
            out += "{";
            dump_object_body(v, indent, out);
            break;
        case Value::Tag::Arr:
            dump_array_body(v, indent, out);
            break;
        default:
            dump_inline(v, out);
            break;
    }
}

std::string Parser::to_sml(const ValuePtr& v) {
    std::string out;
    if (!v) return out;
    if (v->tag == Value::Tag::Obj) {
        /* 顶层分叉与 Rust `to_sml` 逐字对应：带 `__type` 的对象是**裸块的树形**
           （`type [name] { … }` 解析出来的），按 `dump_block(0,0)` 渲染 —— 先换行再
           `{`、逐键、收尾 `}`；空对象写 `{}`。不带 `__type` 的才是「顶层逐键成行」。
           （A 类：原先这里无条件逐键、并把 __type/__name 跳过。） */
        if (v->has("__type")) {
            if (v->obj.empty()) { out += "{}"; return out; }
            out += "\n{";
            dump_object_body(v, 0, out);
            return out;
        }
        for (auto& kv : v->obj) {
            out += kv.first;
            out += starts_inline(kv.second) ? ": " : ":";
            dump_value(kv.second, 0, out);
            out += "\n";
        }
        return out;
    }
    /* 顶层非对象：与数组元素同一套规则（扁平单行 / 含结构展开），
       Rust 侧 to_sml 走的也是 dump_element（顶层数组曾被整坨压成一行并非此处问题，
       见 dump_element/is_flat）。 */
    dump_element(v, 0, out);
    return out;
}

} // namespace sml
