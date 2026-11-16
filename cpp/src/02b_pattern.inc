// ===========================================================================
// 2b. 模式语言（`@type name: X { … }`）—— 无回溯 L1 匹配器
//
// 移植自 `rust/sml-pattern`（权威实现），与 C 端 `c/src/02b_pattern.inc` 逐段同构：
//   - 规则就是普通 SML 数据（序列 / 类 / 次 / 任一 …），不新增 token；
//   - 编译成 Thompson NFA，模拟时所有线程并行推进 ⇒ **结构上不存在回溯**，
//     最坏时间 O(文本长 × 程序长)，不存在 ReDoS；
//   - 字符类按 **Unicode** 判定，四端查的是同一张由 Rust 导出的区间表
//     （`tools/gen/gen_unicode_tables.py` 生成 `c/sml_pattern_unicode.h`）。
// ===========================================================================

#include "../../c/sml_pattern_unicode.h"

// ---- 字符类 ---------------------------------------------------------------

struct PatClass {
    enum class Kind { Digit, Alpha, Space, Word, Any, One, Range, Set, Not } kind = Kind::Any;
    unsigned long ch1 = 0, ch2 = 0;
    std::vector<std::shared_ptr<PatClass>> set;
};

// ---- 模式 IR --------------------------------------------------------------

struct Pat {
    enum class Kind { Class, Lit, Seq, Alt, Repeat, Named, Until } kind = Kind::Lit;
    std::shared_ptr<PatClass> cls;
    std::string lit;
    std::vector<std::shared_ptr<Pat>> items;
    std::shared_ptr<Pat> sub;
    size_t min = 0;
    long max = -1;   // < 0 表示无上界（`+` / `*`）
};

// 与 Rust `QUANT_MAX` / `MAX_PROG` / `DEFAULT_BUDGET` 同数。
static const size_t PAT_QUANT_MAX = 1000;
static const size_t PAT_MAX_PROG = 100000;
static const long   PAT_BUDGET = 10000000L;

// ---- 关键字表（中英等价，与 Rust `BILINGUAL` 逐条同序）---------------------

enum class Concept {
    None, Lit, Class, Alt, Group, Seq, Use, Until, Times, Optional, Name, Regex,
    Min, Max, Digit, Alpha, Space, Word, Any, Eol
};

struct KwEntry { const char* w; Concept c; };

static const KwEntry PAT_KEYWORDS[] = {
    {"字面", Concept::Lit}, {"类", Concept::Class}, {"任一", Concept::Alt},
    {"组", Concept::Group}, {"序列", Concept::Seq}, {"用", Concept::Use},
    {"直到", Concept::Until}, {"次", Concept::Times}, {"可选", Concept::Optional},
    {"名", Concept::Name}, {"正则", Concept::Regex}, {"最小", Concept::Min},
    {"最大", Concept::Max}, {"数字", Concept::Digit}, {"字母", Concept::Alpha},
    {"空白", Concept::Space}, {"字", Concept::Word}, {"任意", Concept::Any},
    {"行尾", Concept::Eol}, {"末尾", Concept::Eol},
    {"lit", Concept::Lit}, {"literal", Concept::Lit}, {"class", Concept::Class},
    {"alt", Concept::Alt}, {"any-of", Concept::Alt}, {"group", Concept::Group},
    {"seq", Concept::Seq}, {"use", Concept::Use}, {"until", Concept::Until},
    {"times", Concept::Times}, {"repeat", Concept::Times},
    {"optional", Concept::Optional}, {"name", Concept::Name},
    {"regex", Concept::Regex}, {"re", Concept::Regex}, {"min", Concept::Min},
    {"max", Concept::Max}, {"digit", Concept::Digit}, {"alpha", Concept::Alpha},
    {"space", Concept::Space}, {"word", Concept::Word}, {"any", Concept::Any},
    {"eol", Concept::Eol}, {"end", Concept::Eol},
};

static Concept pat_lookup(const std::string& w) {
    for (const auto& e : PAT_KEYWORDS) if (w == e.w) return e.c;   // 先到先得
    return Concept::None;
}

// ---- UTF-8 解码（非法字节按单字节码位；Rust 侧 `&str` 保证合法，走不到）----

static unsigned long pat_next_cp(const std::string& s, size_t len, size_t& pos) {
    unsigned char b0 = (unsigned char)s[pos];
    if (b0 < 0x80) { pos++; return b0; }
    unsigned long cp;
    size_t need;
    if ((b0 & 0xE0) == 0xC0) { cp = b0 & 0x1F; need = 1; }
    else if ((b0 & 0xF0) == 0xE0) { cp = b0 & 0x0F; need = 2; }
    else if ((b0 & 0xF8) == 0xF0) { cp = b0 & 0x07; need = 3; }
    else { pos++; return b0; }
    if (pos + need >= len) { pos++; return b0; }
    for (size_t i = 1; i <= need; i++) {
        unsigned char b = (unsigned char)s[pos + i];
        if ((b & 0xC0) != 0x80) { pos++; return b0; }
        cp = (cp << 6) | (b & 0x3F);
    }
    pos += need + 1;
    return cp;
}

static bool pat_class_matches(const PatClass& c, unsigned long cp) {
    switch (c.kind) {
        case PatClass::Kind::Digit:
            return sml_pat_cp_in((unsigned)cp, SML_PAT_NUMERIC, SML_PAT_NUMERIC_N) != 0;
        case PatClass::Kind::Alpha:
            return sml_pat_cp_in((unsigned)cp, SML_PAT_ALPHABETIC, SML_PAT_ALPHABETIC_N) != 0;
        case PatClass::Kind::Space:
            return sml_pat_cp_in((unsigned)cp, SML_PAT_WHITESPACE, SML_PAT_WHITESPACE_N) != 0;
        case PatClass::Kind::Word:
            return sml_pat_cp_in((unsigned)cp, SML_PAT_NUMERIC, SML_PAT_NUMERIC_N) != 0 ||
                   sml_pat_cp_in((unsigned)cp, SML_PAT_ALPHABETIC, SML_PAT_ALPHABETIC_N) != 0 ||
                   cp == '_';
        case PatClass::Kind::Any:   return true;
        case PatClass::Kind::One:   return cp == c.ch1;
        case PatClass::Kind::Range: return cp >= c.ch1 && cp <= c.ch2;
        case PatClass::Kind::Set:
            for (const auto& s : c.set) if (pat_class_matches(*s, cp)) return true;
            return false;
        case PatClass::Kind::Not:
            return c.set.empty() ? true : !pat_class_matches(*c.set[0], cp);
    }
    return false;
}

// ---- 取值辅助 -------------------------------------------------------------

static const char* pat_kind_name(const ValuePtr& v) {
    if (!v) return "缺失";
    switch (v->tag) {
        case Value::Tag::Str:   return "字符串";
        case Value::Tag::Int:   return "整数";
        case Value::Tag::Float: return "浮点数";
        case Value::Tag::Bool:  return "布尔";
        case Value::Tag::Null:  return "null";
        case Value::Tag::Arr:   return "数组";
        case Value::Tag::Obj:   return "对象";
    }
    return "?";
}

static std::shared_ptr<PatClass> pat_new_class(PatClass::Kind k) {
    auto c = std::make_shared<PatClass>();
    c->kind = k;
    return c;
}

static std::shared_ptr<Pat> pat_new(Pat::Kind k) {
    auto p = std::make_shared<Pat>();
    p->kind = k;
    return p;
}

// ---- 编译：SML 值 → 模式 IR -----------------------------------------------

static std::shared_ptr<PatClass> pat_parse_class(const std::string& s, std::string* err) {
    switch (pat_lookup(s)) {
        case Concept::Digit: return pat_new_class(PatClass::Kind::Digit);
        case Concept::Alpha: return pat_new_class(PatClass::Kind::Alpha);
        case Concept::Space: return pat_new_class(PatClass::Kind::Space);
        case Concept::Word:  return pat_new_class(PatClass::Kind::Word);
        case Concept::Any:   return pat_new_class(PatClass::Kind::Any);
        default: break;
    }
    // 非内置类：单个字符按字面（`类: "-"`）
    {
        size_t pos = 0, len = s.size();
        unsigned long cp1 = pat_next_cp(s, len, pos);
        if (pos == len) {
            auto c = pat_new_class(PatClass::Kind::One);
            c->ch1 = cp1;
            return c;
        }
    }
    if (err) *err = "sml: 未知字符类 `" + s + "`";
    return nullptr;
}

// 量词：`次` 的取值（整数 / `+` `*` `?` / `a-b` / `{最小,最大}` 对象）。
static bool pat_parse_times(const ValuePtr& v, size_t& mn, long& mx, std::string* err) {
    long lo = 0, hi = -1;
    bool have_max = false;
    if (v->tag == Value::Tag::Int) {
        if (v->i < 0) { if (err) *err = "sml: 量词最小不能为负（得 " + std::to_string(v->i) + "）"; return false; }
        lo = (long)v->i;
        hi = (long)v->i;
        have_max = true;
    } else if (v->tag == Value::Tag::Str) {
        const std::string& s = v->s;
        if (s == "+") { mn = 1; mx = -1; return true; }
        if (s == "*") { mn = 0; mx = -1; return true; }
        if (s == "?") { mn = 0; mx = 1; return true; }
        size_t dash = s.find('-');
        if (dash == std::string::npos) {
            if (err) *err = "sml: 未知量词 `" + s + "`（可用 数字 / + / * / ? / a-b）";
            return false;
        }
        {
            /* `stoll` 对 `x-y` 会抛 invalid_argument —— 必须接住，否则一个写错的
               量词会让宿主进程崩掉（C 侧 strtol 返回 0，不会崩）。 */
            try {
                lo = (long)std::stoll(s.substr(0, dash));
                hi = (long)std::stoll(s.substr(dash + 1));
            } catch (...) {
                if (err) *err = "sml: 量词 `" + s + "` 非法";
                return false;
            }
        }
        have_max = true;
    } else if (v->tag == Value::Tag::Obj) {
        for (const auto& kv : v->obj) {
            Concept kc = pat_lookup(kv.first);
            if (kc == Concept::Min) {
                if (kv.second->tag != Value::Tag::Int) { if (err) *err = "sml: 量词最小必须为整数"; return false; }
                lo = (long)kv.second->i;
            } else if (kc == Concept::Max) {
                if (kv.second->tag != Value::Tag::Int) { if (err) *err = "sml: 量词最大必须为整数"; return false; }
                hi = (long)kv.second->i;
                have_max = true;
            }
        }
    } else {
        if (err) *err = std::string("sml: 量词类型不支持: ") + pat_kind_name(v);
        return false;
    }
    if (lo < 0) { if (err) *err = "sml: 量词最小不能为负（得 " + std::to_string(lo) + "）"; return false; }
    if (have_max && hi < lo) {
        if (err) *err = "sml: 量词最大(" + std::to_string(hi) + ")不能小于最小(" + std::to_string(lo) + ")";
        return false;
    }
    if (lo > (long)PAT_QUANT_MAX || (have_max && hi > (long)PAT_QUANT_MAX)) {
        if (err) *err = "sml: 量词过大（> " + std::to_string(PAT_QUANT_MAX) +
                        "），拒绝编译（防病态规则；嵌套膨胀另有程序规模上限）";
        return false;
    }
    mn = (size_t)lo;
    mx = have_max ? hi : -1;
    return true;
}

// ---- regex 逃生舱：把 regex 子集编译进同一个 IR ---------------------------
//
// 与 Rust `sml-pattern/src/regex.rs`、C 端 `c/src/02b_pattern.inc` 逐条同构：
// `正则/regex/re` 只是模式的另一种**前端**，解析后产出同一棵 `Pat`，仍交给
// 无回溯 NFA 执行 —— 逃生舱不逃掉线性时间保证。明确拒绝（报错，不静默降级）：
//   - 反向引用 `\1`…`\9`（与自动机模型冲突，NP 完全）
//   - lookaround / 其它 `(?...)` 扩展（破坏线性时间与流式处理）
// 锚点 `^` `$` 是**隐含**的（全文匹配），出现时直接跳过。

struct RegexParser {
    std::vector<unsigned long> cp;   // 已解码的码位序列
    size_t n = 0;
    size_t i = 0;
    std::string* err = nullptr;
};

static std::shared_ptr<Pat> rex_alt(RegexParser& p);

static unsigned long rex_peek(const RegexParser& p) {
    return p.i < p.n ? p.cp[p.i] : 0UL;   // 0 = EOF（std::string 不含内嵌 NUL）
}

static void rex_set_err(const RegexParser& p, const std::string& msg) {
    if (p.err) *p.err = msg;
}

static std::shared_ptr<PatClass> rex_class_one(unsigned long cp) {
    auto c = pat_new_class(PatClass::Kind::One);
    c->ch1 = cp;
    return c;
}

static std::shared_ptr<PatClass> rex_class_not(std::shared_ptr<PatClass> inner) {
    auto c = pat_new_class(PatClass::Kind::Not);
    c->set.push_back(inner);
    return c;
}

static std::shared_ptr<Pat> rex_pat_from_class(std::shared_ptr<PatClass> cl,
                                               const RegexParser& p) {
    if (!cl) { rex_set_err(p, "sml: 内存不足"); return nullptr; }
    auto pat = pat_new(Pat::Kind::Class);
    pat->cls = cl;
    return pat;
}

// 把码位编码成 UTF-8，供错误消息里显示那个无法解析的字符。
static std::string rex_utf8(unsigned long cp) {
    std::string out;
    if (cp < 0x80) {
        out.push_back((char)cp);
    } else if (cp < 0x800) {
        out.push_back((char)(0xC0 | (cp >> 6)));
        out.push_back((char)(0x80 | (cp & 0x3F)));
    } else if (cp < 0x10000) {
        out.push_back((char)(0xE0 | (cp >> 12)));
        out.push_back((char)(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back((char)(0x80 | (cp & 0x3F)));
    } else {
        out.push_back((char)(0xF0 | (cp >> 18)));
        out.push_back((char)(0x80 | ((cp >> 12) & 0x3F)));
        out.push_back((char)(0x80 | ((cp >> 6) & 0x3F)));
        out.push_back((char)(0x80 | (cp & 0x3F)));
    }
    return out;
}

// 字符类内的一个成员：`\d` / `\w` / `\s`、`a-z` 范围、或单字符。
static std::shared_ptr<PatClass> rex_class_item(RegexParser& p) {
    if (p.i >= p.n) { rex_set_err(p, "sml: 字符类意外结束"); return nullptr; }
    unsigned long c = p.cp[p.i];
    if (c == '\\') {
        p.i++;
        if (p.i >= p.n) { rex_set_err(p, "sml: `\\` 后缺少字符"); return nullptr; }
        unsigned long e = p.cp[p.i++];
        switch (e) {
            case 'd': return pat_new_class(PatClass::Kind::Digit);
            case 'w': return pat_new_class(PatClass::Kind::Word);
            case 's': return pat_new_class(PatClass::Kind::Space);
            default:  return rex_class_one(e);
        }
    }
    p.i++;
    // 范围 a-z（`-` 紧邻 `]` 时按字面 `-`，与 Rust 同）
    if (rex_peek(p) == '-' && p.i + 1 < p.n && p.cp[p.i + 1] != ']') {
        p.i++;
        unsigned long hi = p.cp[p.i++];
        auto r = pat_new_class(PatClass::Kind::Range);
        r->ch1 = c;
        r->ch2 = hi;
        return r;
    }
    return rex_class_one(c);
}

// `[ ... ]` / `[^ ... ]`。单成员直接那个类，多成员 Set（并集）。
static std::shared_ptr<PatClass> rex_class(RegexParser& p) {
    bool negated = false;
    if (rex_peek(p) != '[') { rex_set_err(p, "sml: 期望 `[`"); return nullptr; }
    p.i++;
    if (rex_peek(p) == '^') { negated = true; p.i++; }
    std::vector<std::shared_ptr<PatClass>> items;
    for (;;) {
        if (p.i >= p.n) { rex_set_err(p, "sml: 字符类缺少 `]`"); return nullptr; }
        if (p.cp[p.i] == ']') { p.i++; break; }
        auto it = rex_class_item(p);
        if (!it) return nullptr;
        items.push_back(it);
    }
    std::shared_ptr<PatClass> first;
    if (items.size() == 1) {
        first = items[0];
    } else {
        first = pat_new_class(PatClass::Kind::Set);
        first->set = items;
    }
    if (negated) return rex_class_not(first);
    return first;
}

// `\d` `\D` `\w` `\W` `\s` `\S`；反向引用 `\1`…`\9` 拒绝；其余按字面。
static std::shared_ptr<Pat> rex_escape(RegexParser& p) {
    if (rex_peek(p) != '\\') { rex_set_err(p, "sml: 期望 `\\`"); return nullptr; }
    p.i++;
    if (p.i >= p.n) { rex_set_err(p, "sml: `\\` 后缺少字符"); return nullptr; }
    unsigned long c = p.cp[p.i++];
    switch (c) {
        case 'd': return rex_pat_from_class(pat_new_class(PatClass::Kind::Digit), p);
        case 'D': return rex_pat_from_class(rex_class_not(pat_new_class(PatClass::Kind::Digit)), p);
        case 'w': return rex_pat_from_class(pat_new_class(PatClass::Kind::Word), p);
        case 'W': return rex_pat_from_class(rex_class_not(pat_new_class(PatClass::Kind::Word)), p);
        case 's': return rex_pat_from_class(pat_new_class(PatClass::Kind::Space), p);
        case 'S': return rex_pat_from_class(rex_class_not(pat_new_class(PatClass::Kind::Space)), p);
        default:
            if (c >= '1' && c <= '9') {
                rex_set_err(p, std::string("sml: regex 不支持反向引用 `\\") + (char)c +
                               "`（与线性时间匹配根本冲突）；请改用命名捕获 + 等值比较");
                return nullptr;
            }
            return rex_pat_from_class(rex_class_one(c), p);
    }
}

static std::shared_ptr<Pat> rex_atom(RegexParser& p) {
    if (p.i >= p.n) { rex_set_err(p, "sml: regex 意外结束"); return nullptr; }
    unsigned long c = p.cp[p.i];
    if (c == '(') {
        p.i++;
        if (rex_peek(p) == '?') {
            rex_set_err(p, "sml: regex 不支持 `(?...)` 扩展（lookaround / 命名组等）；"
                           "它们会破坏线性时间保证");
            return nullptr;
        }
        auto inner = rex_alt(p);
        if (!inner) return nullptr;
        if (rex_peek(p) != ')') { rex_set_err(p, "sml: regex 分组缺少 `)`"); return nullptr; }
        p.i++;
        return inner;
    }
    if (c == '[') return rex_pat_from_class(rex_class(p), p);
    if (c == '.') { p.i++; return rex_pat_from_class(pat_new_class(PatClass::Kind::Any), p); }
    if (c == '\\') return rex_escape(p);
    p.i++;
    return rex_pat_from_class(rex_class_one(c), p);
}

// 读一串 ASCII 数字（`{n}` / `{n,m}` 的 n / m）。无数字返回 false。
static bool rex_read_uint(RegexParser& p, long& out) {
    long v = 0;
    size_t start = p.i;
    while (p.i < p.n && p.cp[p.i] >= '0' && p.cp[p.i] <= '9') {
        v = v * 10 + (long)(p.cp[p.i] - '0');
        if (v > 1000000000L) v = 1000000001L;   // 夹住，交给 QUANT_MAX 闸门
        p.i++;
    }
    if (p.i == start) return false;
    out = v;
    return true;
}

// `{n}` / `{n,}` / `{n,m}`。max = -1 表示无上界（与 IR 同约定）。
static bool rex_counted(RegexParser& p, size_t& mn, long& mx) {
    long lo, hi;
    if (rex_peek(p) != '{') { rex_set_err(p, "sml: 期望 `{`"); return false; }
    p.i++;
    if (!rex_read_uint(p, lo)) { rex_set_err(p, "sml: `{n}` 量词缺少数字"); return false; }
    if (rex_peek(p) == ',') {
        p.i++;
        if (!rex_read_uint(p, hi)) hi = -1;   // {n,} 无上界
    } else {
        hi = lo;                               // {n} 恰好 n 次
    }
    if (rex_peek(p) != '}') { rex_set_err(p, "sml: 量词缺少 `}`"); return false; }
    p.i++;
    if (lo > (long)PAT_QUANT_MAX || (hi >= 0 && hi > (long)PAT_QUANT_MAX)) {
        rex_set_err(p, "sml: 量词过大（> " + std::to_string(PAT_QUANT_MAX) +
                       "），拒绝编译（防病态规则）");
        return false;
    }
    mn = (size_t)lo;
    mx = hi;
    return true;
}

static std::shared_ptr<Pat> rex_repeat(RegexParser& p) {
    auto atom = rex_atom(p);
    if (!atom) return nullptr;
    unsigned long c = rex_peek(p);
    size_t mn = 0;
    long mx = -1;
    bool has = false;
    if (c == '*') { p.i++; mn = 0; mx = -1; has = true; }
    else if (c == '+') { p.i++; mn = 1; mx = -1; has = true; }
    else if (c == '?') { p.i++; mn = 0; mx = 1; has = true; }
    else if (c == '{') {
        if (!rex_counted(p, mn, mx)) return nullptr;
        has = true;
    }
    if (has) {
        if (rex_peek(p) == '?') p.i++;   // 无回溯引擎没有贪婪概念：懒惰标记忽略
        auto r = pat_new(Pat::Kind::Repeat);
        r->sub = atom;
        r->min = mn;
        r->max = mx;
        return r;
    }
    return atom;
}

static std::shared_ptr<Pat> rex_seq(RegexParser& p) {
    auto seq = pat_new(Pat::Kind::Seq);
    while (p.i < p.n) {
        unsigned long c = p.cp[p.i];
        if (c == '|' || c == ')') break;
        if (c == '^' || c == '$') { p.i++; continue; }   // 锚点隐含
        auto r = rex_repeat(p);
        if (!r) return nullptr;
        seq->items.push_back(r);
    }
    return seq;
}

static std::shared_ptr<Pat> rex_alt(RegexParser& p) {
    auto first = rex_seq(p);
    if (!first) return nullptr;
    if (rex_peek(p) != '|') return first;
    auto alt = pat_new(Pat::Kind::Alt);
    alt->items.push_back(first);
    while (rex_peek(p) == '|') {
        p.i++;
        auto s = rex_seq(p);
        if (!s) return nullptr;
        alt->items.push_back(s);
    }
    return alt;
}

// 对外入口：把 regex 源码编译成 Pat（失败返回 nullptr 并写 err）。
static std::shared_ptr<Pat> pat_parse_regex(const std::string& src, std::string* err) {
    RegexParser p;
    p.err = err;
    size_t pos = 0;
    while (pos < src.size()) p.cp.push_back(pat_next_cp(src, src.size(), pos));
    p.n = p.cp.size();
    auto pat = rex_alt(p);
    if (pat && p.i < p.n) {
        if (err) *err = "sml: regex 在位置 " + std::to_string(p.i) +
                        " 处有无法解析的内容 `" + rex_utf8(p.cp[p.i]) + "`";
        return nullptr;
    }
    return pat;
}

static std::shared_ptr<Pat> pat_compile_value(const ValuePtr& v,
                                              const std::map<std::string, ValuePtr>* rules,
                                              std::vector<std::string>& stack,
                                              std::string* err);

static std::shared_ptr<Pat> pat_compile_elem(const ValuePtr& v,
                                             const std::map<std::string, ValuePtr>* rules,
                                             std::vector<std::string>& stack,
                                             std::string* err) {
    // 中英两套键等价（与 Rust `BILINGUAL` 关键字表同义）；先中文后英文。
    auto pick = [&v](const char* zh, const char* en) -> ValuePtr {
        ValuePtr x = v->get(zh);
        return x ? x : v->get(en);
    };
    const ValuePtr lit   = pick("字面", "lit");
    const ValuePtr cls   = pick("类", "class");
    const ValuePtr alt   = pick("任一", "alt");
    const ValuePtr grp   = pick("组", "group");
    const ValuePtr seq   = pick("序列", "seq");
    const ValuePtr use   = pick("用", "use");
    const ValuePtr until = pick("直到", "until");
    const ValuePtr times = pick("次", "times");
    const ValuePtr opt   = pick("可选", "optional");
    const ValuePtr name  = pick("名", "name");
    const ValuePtr rex   = pick("正则", "regex");
    const ValuePtr fmin  = pick("最小", "min");
    const ValuePtr fmax  = pick("最大", "max");

    std::shared_ptr<Pat> base;

    if (lit) {
        if (lit->tag != Value::Tag::Str) {
            if (err) *err = std::string("sml: `字面/lit` 须为字符串，实际 ") + pat_kind_name(lit);
            return nullptr;
        }
        base = pat_new(Pat::Kind::Lit);
        base->lit = lit->s;
    } else if (cls) {
        if (cls->tag != Value::Tag::Str) {
            if (err) *err = std::string("sml: `类/class` 须为字符串，实际 ") + pat_kind_name(cls);
            return nullptr;
        }
        base = pat_new(Pat::Kind::Class);
        base->cls = pat_parse_class(cls->s, err);
        if (!base->cls) return nullptr;
    } else if (alt) {
        if (alt->tag != Value::Tag::Arr) {
            if (err) *err = std::string("sml: `任一/alt` 须为数组，实际 ") + pat_kind_name(alt);
            return nullptr;
        }
        base = pat_new(Pat::Kind::Alt);
        for (const auto& it : alt->arr) {
            auto c = pat_compile_value(it, rules, stack, err);
            if (!c) return nullptr;
            base->items.push_back(c);
        }
    } else if (grp) {
        base = pat_compile_value(grp, rules, stack, err);
        if (!base) return nullptr;
    } else if (seq) {
        base = pat_compile_value(seq, rules, stack, err);
        if (!base) return nullptr;
    } else if (use) {
        if (use->tag != Value::Tag::Str) {
            if (err) *err = std::string("sml: `用/use` 须为字符串，实际 ") + pat_kind_name(use);
            return nullptr;
        }
        const std::string& rname = use->s;
        for (const auto& s : stack) {
            if (s == rname) {
                if (err) *err = "sml: 规则 `" + rname +
                    "` 存在循环引用（L1 不支持递归；如需嵌套结构请先升级到 L2）";
                return nullptr;
            }
        }
        ValuePtr target;
        if (rules) {
            auto it = rules->find(rname);
            if (it != rules->end()) target = it->second;
        }
        if (!target) { if (err) *err = "sml: 未定义的规则 `" + rname + "`"; return nullptr; }
        stack.push_back(rname);
        base = pat_compile_value(target, rules, stack, err);
        stack.pop_back();
        if (!base) return nullptr;
    } else if (until) {
        if (until->tag != Value::Tag::Str) {
            if (err) *err = std::string("sml: `直到/until` 须为字符串，实际 ") + pat_kind_name(until);
            return nullptr;
        }
        if (pat_lookup(until->s) == Concept::Eol) {
            // 「直到行尾」= 吃掉剩余全部字符
            base = pat_new(Pat::Kind::Repeat);
            base->min = 0;
            base->max = -1;
            base->sub = pat_new(Pat::Kind::Class);
            base->sub->cls = pat_new_class(PatClass::Kind::Any);
        } else {
            base = pat_new(Pat::Kind::Until);
            base->sub = pat_new(Pat::Kind::Lit);
            base->sub->lit = until->s;
        }
    } else if (rex) {
        // `正则:` 逃生舱：解析成 regex 子集后编译进同一个 IR（Rust / C 同构）。
        if (rex->tag != Value::Tag::Str) {
            if (err) *err = std::string("sml: `正则/regex` 须为字符串，实际 ") + pat_kind_name(rex);
            return nullptr;
        }
        base = pat_parse_regex(rex->s, err);
        if (!base) {
            // 与 Rust 同：外层再包一句「regex 编译失败」（内层文案已带 `sml:` 前缀）。
            std::string inner = err ? *err : std::string();
            if (err) *err = "sml: regex 编译失败：" + inner;
            return nullptr;
        }
    } else {
        std::string keys;
        for (const auto& kv : v->obj) { if (!keys.empty()) keys += " "; keys += kv.first; }
        if (err) *err = "sml: 无法识别的模式元素（需 字面/类/任一/组/序列/用/直到/正则 之一），实际键: `" +
                        keys + "`";
        return nullptr;
    }

    // 量词：优先平铺 `最小`/`最大`，否则 `次` / `可选`。只包一层 Repeat。
    if (fmin || fmax) {
        long lo = 0, hi = -1;
        bool have_max = false;
        if (fmin) {
            if (fmin->tag != Value::Tag::Int) { if (err) *err = "sml: 量词最小必须为整数"; return nullptr; }
            lo = (long)fmin->i;
        }
        if (fmax) {
            if (fmax->tag != Value::Tag::Int) { if (err) *err = "sml: 量词最大必须为整数"; return nullptr; }
            hi = (long)fmax->i;
            have_max = true;
        }
        if (lo < 0) { if (err) *err = "sml: 量词最小不能为负"; return nullptr; }
        if (have_max && hi < lo) { if (err) *err = "sml: 量词最大不能小于最小"; return nullptr; }
        if (lo > (long)PAT_QUANT_MAX || (have_max && hi > (long)PAT_QUANT_MAX)) {
            if (err) *err = "sml: 量词过大（> 1000），拒绝编译";
            return nullptr;
        }
        auto r = pat_new(Pat::Kind::Repeat);
        r->sub = base;
        r->min = (size_t)lo;
        r->max = have_max ? hi : -1;
        base = r;
    } else if (times) {
        size_t mn = 0;
        long mx = -1;
        if (!pat_parse_times(times, mn, mx, err)) return nullptr;
        auto r = pat_new(Pat::Kind::Repeat);
        r->sub = base;
        r->min = mn;
        r->max = mx;
        base = r;
    } else if (opt && opt->tag == Value::Tag::Bool && opt->b) {
        auto r = pat_new(Pat::Kind::Repeat);
        r->sub = base;
        r->min = 0;
        r->max = 1;
        base = r;
    }

    if (name) {
        if (name->tag != Value::Tag::Str) {
            if (err) *err = std::string("sml: `名/name` 须为字符串，实际 ") + pat_kind_name(name);
            return nullptr;
        }
        auto nm = pat_new(Pat::Kind::Named);
        nm->sub = base;
        base = nm;
    }
    return base;
}

static std::shared_ptr<Pat> pat_compile_value(const ValuePtr& v,
                                              const std::map<std::string, ValuePtr>* rules,
                                              std::vector<std::string>& stack,
                                              std::string* err) {
    if (!v) { if (err) *err = "sml: 模式元素缺失"; return nullptr; }
    if (v->tag == Value::Tag::Arr) {
        auto p = pat_new(Pat::Kind::Seq);
        for (const auto& it : v->arr) {
            auto c = pat_compile_value(it, rules, stack, err);
            if (!c) return nullptr;
            p->items.push_back(c);
        }
        return p;
    }
    if (v->tag == Value::Tag::Str) {
        auto p = pat_new(Pat::Kind::Lit);
        p->lit = v->s;
        return p;
    }
    if (v->tag == Value::Tag::Obj) return pat_compile_elem(v, rules, stack, err);
    if (err) *err = std::string("sml: 模式元素类型不支持: ") + pat_kind_name(v);
    return nullptr;
}

static std::shared_ptr<Pat> pat_compile(const ValuePtr& rule,
                                        const std::map<std::string, ValuePtr>* rules,
                                        std::string* err) {
    std::vector<std::string> stack;
    return pat_compile_value(rule, rules, stack, err);
}

// ---- NFA ------------------------------------------------------------------

struct PatInst {
    enum class Kind { Char, Split, Jmp, Match } kind = Kind::Match;
    const PatClass* cls = nullptr;   // Char（借用 Pat 树，由 shared_ptr 保活）
    size_t x = 0, y = 0;             // Split(x, y) / Jmp(x)
};

struct PatProg {
    std::vector<PatInst> code;
    std::vector<std::shared_ptr<PatClass>> tmp;   // emit 期间现造的临时类
};

static bool pat_emit(const Pat& p, PatProg& pg, std::string* err) {
    if (pg.code.size() > PAT_MAX_PROG) {
        if (err) *err = "sml: 模式程序过大（超过 100000 条指令，疑似嵌套量词的病态膨胀）";
        return false;
    }
    switch (p.kind) {
        case Pat::Kind::Class: {
            PatInst in; in.kind = PatInst::Kind::Char; in.cls = p.cls.get();
            pg.code.push_back(in);
            return true;
        }
        case Pat::Kind::Lit: {
            size_t pos = 0;
            while (pos < p.lit.size()) {
                unsigned long cp = pat_next_cp(p.lit, p.lit.size(), pos);
                auto c = pat_new_class(PatClass::Kind::One);
                c->ch1 = cp;
                pg.tmp.push_back(c);
                PatInst in; in.kind = PatInst::Kind::Char; in.cls = c.get();
                pg.code.push_back(in);
            }
            return true;
        }
        case Pat::Kind::Seq:
            for (const auto& it : p.items) if (!pat_emit(*it, pg, err)) return false;
            return true;
        case Pat::Kind::Named:
            return pat_emit(*p.sub, pg, err);
        case Pat::Kind::Alt: {
            if (p.items.empty()) return true;
            std::vector<size_t> jmps;
            for (size_t i = 0; i < p.items.size(); i++) {
                size_t split_pc = 0;
                bool more = (i + 1 < p.items.size());
                if (more) {
                    split_pc = pg.code.size();
                    PatInst in; in.kind = PatInst::Kind::Split;
                    pg.code.push_back(in);
                }
                size_t start = pg.code.size();
                if (!pat_emit(*p.items[i], pg, err)) return false;
                if (more) {
                    size_t jmp_pc = pg.code.size();
                    PatInst jm; jm.kind = PatInst::Kind::Jmp;
                    pg.code.push_back(jm);
                    pg.code[split_pc].x = start;
                    pg.code[split_pc].y = pg.code.size();
                    jmps.push_back(jmp_pc);
                }
            }
            size_t end = pg.code.size();
            for (size_t j : jmps) pg.code[j].x = end;
            return true;
        }
        case Pat::Kind::Repeat: {
            if (p.max >= 0 && (size_t)p.max >= p.min) {
                for (size_t k = 0; k < p.min; k++) if (!pat_emit(*p.sub, pg, err)) return false;
                for (size_t k = 0; k < (size_t)p.max - p.min; k++) {
                    size_t split_pc = pg.code.size();
                    PatInst in; in.kind = PatInst::Kind::Split;
                    pg.code.push_back(in);
                    size_t start = pg.code.size();
                    if (!pat_emit(*p.sub, pg, err)) return false;
                    size_t end = pg.code.size();
                    pg.code[split_pc].x = start;
                    pg.code[split_pc].y = end;
                }
                return true;
            }
            if (p.max >= 0) {
                // max < min：不可能匹配，退化为「至少 min 次」（Rust 同）
                for (size_t k = 0; k < p.min; k++) if (!pat_emit(*p.sub, pg, err)) return false;
                return true;
            }
            for (size_t k = 0; k < p.min; k++) if (!pat_emit(*p.sub, pg, err)) return false;
            {
                size_t split_pc = pg.code.size();
                PatInst in; in.kind = PatInst::Kind::Split;
                pg.code.push_back(in);
                size_t start = pg.code.size();
                if (!pat_emit(*p.sub, pg, err)) return false;
                PatInst jm; jm.kind = PatInst::Kind::Jmp; jm.x = split_pc;
                pg.code.push_back(jm);
                pg.code[split_pc].x = start;
                pg.code[split_pc].y = pg.code.size();
            }
            return true;
        }
        case Pat::Kind::Until: {
            auto any = pat_new_class(PatClass::Kind::Any);
            pg.tmp.push_back(any);
            size_t split_pc = pg.code.size();
            PatInst in; in.kind = PatInst::Kind::Split;
            pg.code.push_back(in);
            size_t try_end = pg.code.size();
            if (!pat_emit(*p.sub, pg, err)) return false;
            size_t jmp_pc = pg.code.size();
            PatInst jm; jm.kind = PatInst::Kind::Jmp;
            pg.code.push_back(jm);
            size_t consume = pg.code.size();
            PatInst ch; ch.kind = PatInst::Kind::Char; ch.cls = any.get();
            pg.code.push_back(ch);
            PatInst back; back.kind = PatInst::Kind::Jmp; back.x = split_pc;
            pg.code.push_back(back);
            size_t end = pg.code.size();
            pg.code[split_pc].x = try_end;
            pg.code[split_pc].y = consume;
            pg.code[jmp_pc].x = end;
            return true;
        }
    }
    return true;
}

// ε 闭包：把 Split / Jmp 能到达的位置全部加入线程集合（防 ε 环无限展开）。
static void pat_add_thread(std::vector<size_t>& list, const PatProg& pg, size_t pc) {
    std::vector<size_t> stack;
    std::vector<size_t> seen;
    stack.push_back(pc);
    seen.push_back(pc);
    while (!stack.empty()) {
        size_t p = stack.back();
        stack.pop_back();
        const PatInst& in = pg.code[p];
        if (in.kind == PatInst::Kind::Split) {
            size_t kids[2] = { in.y, in.x };
            for (size_t i = 0; i < 2; i++) {
                size_t nx = kids[i];
                bool dup = false;
                for (size_t s : seen) if (s == nx) { dup = true; break; }
                if (dup) continue;
                seen.push_back(nx);
                stack.push_back(nx);
            }
        } else if (in.kind == PatInst::Kind::Jmp) {
            size_t nx = in.x;
            bool dup = false;
            for (size_t s : seen) if (s == nx) { dup = true; break; }
            if (!dup) { seen.push_back(nx); stack.push_back(nx); }
        } else {
            bool dup = false;
            for (size_t s : list) if (s == p) { dup = true; break; }
            if (!dup) list.push_back(p);
        }
    }
}

/* 全文匹配：返回 1 命中 / 0 不命中 / -1 出错（写 err）。 */
static int pat_match(const std::shared_ptr<Pat>& pat, const std::string& text, std::string* err) {
    if (!pat) { if (err) *err = "sml: 模式未编译"; return -1; }
    PatProg pg;
    if (!pat_emit(*pat, pg, err)) return -1;
    {
        PatInst in; in.kind = PatInst::Kind::Match;
        pg.code.push_back(in);
    }
    size_t match_pc = pg.code.size() - 1;
    std::vector<size_t> clist, nlist;
    pat_add_thread(clist, pg, 0);
    size_t pos = 0;
    long steps = 0;
    while (pos < text.size()) {
        unsigned long cp = pat_next_cp(text, text.size(), pos);
        nlist.clear();
        for (size_t pc : clist) {
            if (++steps > PAT_BUDGET) {
                if (err) *err = "sml: 模式匹配超出步数预算 10000000（疑似病态规则或超长输入）";
                return -1;
            }
            if (pg.code[pc].kind == PatInst::Kind::Char &&
                pat_class_matches(*pg.code[pc].cls, cp)) {
                pat_add_thread(nlist, pg, pc + 1);
            }
        }
        clist.swap(nlist);
        if (clist.empty()) return 0;
    }
    for (size_t pc : clist) if (pc == match_pc) return 1;
    return 0;
}
