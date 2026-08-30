// SPDX-License-Identifier: MulanPSL-2.0
// sml.cpp - SNOWARE Markup Language, C++17 zero-dependency implementation.
// 100% semantic alignment with the Rust `swsml` crate (rust/src/lib.rs).
#include "sml.hpp"

#include <cctype>
#include <cstdlib>
#include <cmath>
#include <sstream>
#include <fstream>
#include <iomanip>
#include <algorithm>
#include <iostream>

namespace sml {

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
                if (j >= n || inner[j] != '}') { if(err)*err="sml: bad \\u{...} escape"; return out; }
                j++; // consume }
            }
            if (hex.empty()) { if(err)*err="sml: empty unicode escape"; return out; }
            // parse hex
            unsigned long cp = 0;
            try { cp = std::stoul(hex, nullptr, 16); } catch(...) { if(err)*err="sml: bad unicode codepoint"; return out; }
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
                        else if (e == 'a') buf.push_back('\a');
                        else if (e == 'b') buf.push_back('\b');
                        else if (e == 'f') buf.push_back('\f');
                        else if (e == 'v') buf.push_back('\v');
                        else if (e == '\\') buf.push_back('\\');
                        else if (e == '"') buf.push_back('"');
                        else if (e == '\'') buf.push_back('\'');
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
                        else { buf.push_back('\\'); buf.push_back(e); i += 2; continue; }
                        i += 2;
                        continue;
                    }
                    buf.push_back(cc);
                    if (cc == '\n') line++;
                    i++;
                }
                if (!closed) { if(err)*err="sml: unterminated quoted string"; return toks; }
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
        try { return Value::floating(std::stod(t)); } catch(...) {}
    }
    if (looks_like_int(t)) {
        try {
            long long v = std::stoll(t);
            // overflow -> float
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
    if (a==std::string::npos){ if(err)*err="sml: empty type"; return false; }
    s = s.substr(a, b-a+1);

    // enum:  enum [ a b c ]   (or enum [ "a" "b" ])
    if (s.rfind("enum", 0)==0) {
        std::string rest = s.substr(4);
        size_t ab = rest.find('[');
        if (ab==std::string::npos){ if(err)*err="sml: enum needs [ ... ]"; return false; }
        size_t bb = rest.find(']', ab);
        if (bb==std::string::npos){ if(err)*err="sml: enum missing ]"; return false; }
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
        if (bb==std::string::npos){ if(err)*err="sml: array missing ]"; return false; }
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

static bool parse_field(const std::string& line, FieldSpec& out, std::string* err) {
    // line like:  name: Type optional default "x" min 0 max 10 loose
    std::istringstream iss(line);
    std::vector<std::string> parts;
    std::string p;
    while (iss >> p) parts.push_back(p);
    if (parts.empty()){ if(err)*err="sml: empty field"; return false; }

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
            if (k+1 >= parts.size()){ if(err)*err="sml: default needs value"; return false; }
            mods.default_value = parts[k+1]; k+=2; continue;
        }
        if (w=="min") {
            if (k+1 >= parts.size()){ if(err)*err="sml: min needs value"; return false; }
            try { mods.min = std::stoll(parts[k+1]); } catch(...){}
            k+=2; continue;
        }
        if (w=="max") {
            if (k+1 >= parts.size()){ if(err)*err="sml: max needs value"; return false; }
            try { mods.max = std::stoll(parts[k+1]); } catch(...){}
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
            if (err)*err="sml: expected bool"; return false;
        case TypeSpec::Kind::Int:
            if (v->tag==Value::Tag::Int) return true;
            if (loose && v->tag==Value::Tag::Float && v->f==std::floor(v->f)) return true;
            if (loose && v->tag==Value::Tag::Str) {
                if (looks_like_int(v->s)) return true;
            }
            if (err)*err="sml: expected int"; return false;
        case TypeSpec::Kind::Num:
            if (v->tag==Value::Tag::Int||v->tag==Value::Tag::Float) return true;
            if (loose && v->tag==Value::Tag::Str && (looks_like_int(v->s)||looks_like_float(v->s))) return true;
            if (err)*err="sml: expected num"; return false;
        case TypeSpec::Kind::Str:
            if (v->tag==Value::Tag::Str) return true;
            if (loose && (v->tag==Value::Tag::Int||v->tag==Value::Tag::Float||v->tag==Value::Tag::Bool)) return true;
            if (err)*err="sml: expected str"; return false;
        case TypeSpec::Kind::Enum: {
            if (v->tag==Value::Tag::Str) {
                for (auto& e : spec.enum_values) if (e==v->s) return true;
                if (err)*err="sml: value not in enum"; return false;
            }
            if (v->tag==Value::Tag::Int) {
                // Rust: enum also accepts a value coerced to scalar (int -> string)
                std::string s = std::to_string(v->i);
                for (auto& e : spec.enum_values) if (e==s) return true;
                if (err)*err="sml: value not in enum"; return false;
            }
            if (err)*err="sml: enum needs str"; return false;
        }
        case TypeSpec::Kind::Array: {
            if (v->tag!=Value::Tag::Arr){ if(err)*err="sml: expected array"; return false; }
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
            if (err)*err = "sml: missing required field '" + f.name + "'";
            return false;
        }
    }
    bool loose = f.mods.loose;
    // type check
    if (!resolve_and_check(raw, f.type, contracts, loose, err)) {
        std::string detail = (err && !err->empty()) ? *err : "type mismatch";
        if (err) *err = "sml: field '" + f.name + "': " + detail;
        return false;
    }
    // min/max (only meaningful for int/num scalars; Rust uses f64)
    if (f.mods.min || f.mods.max) {
        double num = 0;
        if (raw->tag==Value::Tag::Int) num = (double)raw->i;
        else if (raw->tag==Value::Tag::Float) num = raw->f;
        else { /* not numeric, min/max ignored like Rust */ }
        if (f.mods.min && num < *f.mods.min) { if(err)*err="sml: field '" + f.name + "': below min"; return false; }
        if (f.mods.max && num > *f.mods.max) { if(err)*err="sml: field '" + f.name + "': above max"; return false; }
    }
    return true;
}

static bool resolve_and_check(const ValuePtr& v, const TypeSpec& spec,
                              const std::map<std::string,Contract>& contracts,
                              bool loose, std::string* err) {
    if (spec.kind == TypeSpec::Kind::ContractRef) {
        auto it = contracts.find(spec.contract_ref);
        if (it == contracts.end()) { if(err)*err="sml: unknown contract '"+spec.contract_ref+"'"; return false; }
        return Parser::apply_contract(v, contracts, spec.contract_ref, err);
    }
    return check_value(v, spec, loose, err);
}

// ===========================================================================
// Parser internals
// ===========================================================================
struct PState {
    std::vector<Token> toks;
    size_t i = 0;
    std::map<std::string,ValuePtr> fragments;
    std::map<std::string,Contract> contracts;
    std::string include_dir;
    std::string* err = nullptr;
    std::vector<std::string> include_stack;
};

// forward decls
static void set_field_local(const ValuePtr& node, const std::string& k, const ValuePtr& v);

static ValuePtr parse_value(PState& st);

static ValuePtr parse_array(PState& st) {
    auto arr = Value::array();
    while (st.i < st.toks.size()) {
        auto& t = st.toks[st.i];
        if (t.t == Token::T::RBracket) { st.i++; break; }
        if (t.t == Token::T::Comma) { st.i++; continue; }
        arr->arr.push_back(parse_value(st));
    }
    return arr;
}

static ValuePtr parse_block(PState& st, bool top = false);

// parse a value: object / array / scalar
static ValuePtr parse_value(PState& st) {
    if (st.i >= st.toks.size()) return Value::null();
    auto& t = st.toks[st.i];
    if (t.t == Token::T::LBrace)  return parse_block(st);
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

static ValuePtr parse_block(PState& st, bool top) {
    auto node = Value::object();
    std::string pending_is;   // local: @is applies only to THIS block
    // support both { ... } and bare block; caller has consumed '{' if any
    // Here we are at the first token INSIDE a block (caller passed after '{' or at start).
    // For top-level, we are at token 0.
    while (st.i < st.toks.size()) {
        auto& tok = st.toks[st.i];
        if (tok.t == Token::T::RBrace) { st.i++; break; }
        if (tok.t == Token::T::RBracket) {
            if (top) { /* top-level array handled elsewhere */ }
            // stray ']' in block -> error
            if (st.err) *st.err = "sml: unexpected ']' at line " + std::to_string(tok.line);
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
                auto sub = parse_block(st);
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
                        std::ifstream f(full);
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
            // unexpected structural token; skip
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
                    auto sub = parse_block(st);
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
            auto sub = parse_block(st);
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
                auto sub = parse_block(st);
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

    // apply pending @is for this block
    if (!pending_is.empty()) {
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
ValuePtr Parser::parse(const std::string& text, std::string* err, const std::string& include_dir) {
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
        result = parse_block(st);
    } else {
        result = parse_block(st, true);
    }
    if (err && !err->empty()) return nullptr;
    return result;
}

// ===========================================================================
// Parser::apply_contract
// ===========================================================================
bool Parser::apply_contract(const ValuePtr& val,
                            const std::map<std::string,Contract>& contracts,
                            const std::string& name,
                            std::string* err) {
    auto it = contracts.find(name);
    if (it == contracts.end()) { if(err)*err="sml: unknown contract '"+name+"'"; return false; }
    if (val->tag != Value::Tag::Obj) { if(err)*err="sml: contract applied to non-object"; return false; }
    const Contract& c = it->second;
    // strict mode (default): reject undeclared fields unless contract is `loose`
    if (!c.allow_extra) {
    for (auto& kv : val->obj) {
        if (kv.first == "__type" || kv.first == "__name") continue;
        bool declared = false;
        for (auto& f : c.fields) if (f.name == kv.first) { declared=true; break; }
        if (!declared) {
            if (err)*err = "sml: field '" + kv.first + "' not declared in contract '" + name + "'";
            return false;
        }
    }
    }
    for (auto& f : c.fields) {
        if (!apply_one_field(val, contracts, f, err)) return false;
    }
    return true;
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
