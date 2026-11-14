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
static bool is_hex_digit(char c){
    return (c >= '0' && c <= '9') || (c >= 'A' && c <= 'F') || (c >= 'a' && c <= 'f');
}
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
            if (brace) {
                while (j < n && is_hex_digit(inner[j])) { hex.push_back(inner[j]); j++; }
                if (j >= n || inner[j] != '}') { if(err)*err=code_prefix(SML_E_LEX_005, "sml: bad \\u{...} escape"); return out; }
                j++; // consume }
                if (hex.empty()) { if(err)*err=code_prefix(SML_E_LEX_005, "sml: empty unicode escape"); return out; }
            } else {
                // Fixed four digits, consumed **exactly** — scanning greedily would swallow a
                // trailing hex-looking char: `\u0041b` used to become "0041b" (5 digits) and
                // report E-LEX-005 instead of decoding `aAb`.
                // ｜ 固定四位要**恰好**吃四位：贪婪扫描会把 `\u0041b` 的 b 也算进去。
                for (int k = 0; k < 4; ++k) {
                    if (j >= n || !is_hex_digit(inner[j])) {
                        if(err)*err=code_prefix(SML_E_LEX_005, "sml: unicode 转义位数不足（须四位十六进制或 \\u{...}）");
                        return out;
                    }
                    hex.push_back(inner[j]); j++;
                }
            }
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
    enum class T { Word, Colon, LBrace, RBrace, LBracket, RBracket, Comma, At, BareAt, Dollar } t;
    std::string s; // for Word
    size_t line = 1;
    Token(Token::T tt, const std::string& ss="", size_t ln=1): t(tt), s(ss), line(ln){}
};
