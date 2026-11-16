
static bool expand_includes(std::vector<Token>& toks,
                            const std::string& include_dir,
                            std::vector<std::string>& chain,
                            long long& expansions,
                            std::string* err,
                            unsigned features) {
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
        /* 两种形态：
           ① `@include "path"` —— `@` + Word(include/include!) + 带引号 Word；
           ② **裸** `include "path"` / `import "path"` —— 2026-11-03 补齐（雷 6-2 / I-16）：
              判据对齐其余四端的「**行首**指令」，因为 C 侧是逐行判定、Rust 侧是
              `parse_include_line` 只看行首。这里用 `Token::line` 还原同一语义：
              **该 Word 是本行第一个 token**，且路径串也在同一行。
              没有这条约束的话 `k: include "x.sml"` 这种**普通字段**会被吞掉。 */
        const bool at_form = (t.t == Token::T::At && k + 2 < toks.size() &&
                              toks[k+1].t == Token::T::Word &&
                              (toks[k+1].s == "include" || toks[k+1].s == "include!") &&
                              toks[k+2].t == Token::T::Word);
        const bool line_start = (k == 0 || toks[k-1].line != t.line);
        /* 裸形式**不再要求**路径带引号：`include x.sml`（忘了加引号）必须落到下面的
           012 判据里。改前要求 `s[0]=='"'`，于是这一行直接溜进普通字段解析，
           得到 `include: x.sml`（静默错树，与 Rust / C / Lua 都不一致）。 */
        const bool bare_form = (t.t == Token::T::Word && line_start &&
                                (t.s == "include" || t.s == "import") &&
                                k + 1 < toks.size() &&
                                toks[k+1].t == Token::T::Word &&
                                toks[k+1].line == t.line);
        /* 部分引用 `include/import { keys }`：本端不实现 ⇒ 显式拒绝。
           改前 `import { a }` 静默当普通内容（错树）、`include "x" { a }` 遗留 `{` 报 006。 */
        const bool partial_brace = (t.t == Token::T::Word && line_start &&
                                    (t.s == "include" || t.s == "import") &&
                                    k + 1 < toks.size() &&
                                    toks[k+1].t == Token::T::LBrace &&
                                    toks[k+1].line == t.line);
        if (partial_brace) {
            if (err && err->empty())
                *err = code_prefix(SML_E_FEATURE_001,
                    "sml: 部分引用（{ keys }）本实现未启用");
            return false;
        }
        if (!at_form && !bare_form) { out.push_back(t); k++; continue; }
        /* include 门控：`@feature disable include` 后必须拒绝（与 Rust 同码 001）。 */
        if (!(features & (1u << 1))) {
            if (err && err->empty())
                *err = code_prefix(SML_E_FEATURE_001, "sml: include 特性未启用");
            return false;
        }
        /* 路径 token 的下标随形态变化（`@include` 是 2，裸形式是 1）。 */
        const size_t pidx = at_form ? k + 2 : k + 1;
        /* `include "x" { keys }`：路径后紧跟 `{` ⇒ 部分引用 ⇒ 显式拒绝。 */
        if (pidx + 1 < toks.size() && toks[pidx + 1].t == Token::T::LBrace) {
            if (err && err->empty())
                *err = code_prefix(SML_E_FEATURE_001,
                    "sml: 部分引用（{ keys }）本实现未启用");
            return false;
        }

        /* 路径**写法**非法（未加引号）⇒ `E-INCLUDE-012`。
           为什么需要这条：词法器把引号串与裸词**都存成 `Word`**（引号串只是内容两侧补了 `"`），
           于是旧实现无法区分 `@include b.sml` 与 `@include "b.sml"` —— 目标存在就照常展开、
           不存在则报 `E-INCLUDE-001`（"读取失败"），把**写法**错误导成"文件不存在"。
           与 Rust / Lua / C 对齐：明确报此码。
           注：本实现同时认 `@include` 与**行首裸** `include`（2026-11-03 补齐），
           两种形态在此共用同一条判据；
           而「引号未闭合」在**词法层**就已按 `E-LEX-001` 拦下（子文件里是 `E-INCLUDE-011`），
           走不到这里 —— 这条端间差异记在 `errors/codes.sml` 的 E-INCLUDE-012 note 里。 */
        if (toks[pidx].s.empty() || toks[pidx].s[0] != '"') {
            if (err) *err = code_prefix(SML_E_INCLUDE_012, "sml: include 路径写法非法（未加引号）");
            return false;
        }

        std::string path = toks[pidx].s;
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
                                        chain, expansions, err, features);
        chain.pop_back();
        if (!ok) return false;
        out.insert(out.end(), sub.begin(), sub.end());
        k = pidx + 1;
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
static ValuePtr parse_impl(const std::string& text, std::string* err,
                           const std::string& include_dir, std::vector<Diag>* diags,
                           const BuiltinTable* builtins,
                           std::vector<WeakBind>* weak_binds) {
    PState st;
    st.err = err;
    st.diags = diags;
    st.builtins = builtins;
    st.weak_binds = weak_binds;
    st.features = scan_features(text);   /* 预处理算出的初始特性集 */
    st.toks = tokenize(text, err);
    if (err && !err->empty()) return nullptr;
    /* include 在**解析前**一次性展开完（W18）：解析器拿到的 token 流里不再有 include
       指令，也就没有「插一段 token 再对齐 st.i」这回事。include_dir 为空 = 按 sml.hpp
       的约定关闭 include，指令留给 parse_block 吃掉。 */
    if (!include_dir.empty()) {
        std::vector<std::string> chain;
        long long expansions = 0;
        if (!expand_includes(st.toks, include_dir, chain, expansions, err, st.features)) return nullptr;
    } else {
        /* 明确拒绝裸 `include` / `import`（TODO 排雷区 雷 6-2、台账 #4/#5）。
           本实现的 include 只认 `@include "path"`（见 expand_includes 里那条注释），
           而**其余四端都把裸 `include "x.sml"` 当指令**展开。
           旧行为：裸形式落进普通键 ⇒ 静默产出 `include: "x.sml"` 这个**错树**
           （不报错、也不展开，用户以为子文件已经并进来了）。
           与其静默错树，不如响亮失败 —— 判据刻意收窄，只看「Word `include`
           紧跟一个引号串」：`include: "x"`（带冒号）是**名为 include 的字段**，合法。 */
        for (size_t k = 0; k + 1 < st.toks.size(); ++k) {
            const Token& t = st.toks[k];
            if (t.t != Token::T::Word) continue;
            if (t.s != "include" && t.s != "import") continue;
            /* `@include` 里的 `include` 同样是 Word（`@` 是独立 token）——
               `@` 形式走 expand_includes，不在此拒绝。 */
            if (k > 0 && st.toks[k - 1].t == Token::T::At) continue;
            const Token& nx = st.toks[k + 1];
            if (nx.t == Token::T::Word && !nx.s.empty() && nx.s[0] == '"') {
                if (err) *err = code_prefix(SML_E_FEATURE_001,
                    "sml: 裸 `" + t.s + "` 不支持：本实现的 include 只认 `@"
                    + t.s + " \"path\"` 形式（裸形式没有展开，会产出错树）");
                return nullptr;
            }
        }
    }
    if (st.toks.empty()) return Value::object();

    auto& first = st.toks[0];
    // W16 B0：顶层只能是容器（键值块/对象块/数组），单个标量无法往返 ⇒ E-PARSE-008。
    //   判定：只有一个 token 且它不是 [ 或 {（即裸词 / 引号串 / 数字，tokenizer 都记为 Word）。
    // T9 收紧：判据 = 顶层第一个结构是裸标量、且其后紧跟块/数组/键值（与 Rust 同判据）。
    //   裸标量 = 单个 Word 叶子（裸词/引号串/数字都记为 Word），且其后的 token 不是 `{` / `:`。
    //   `hello world`（2 token 裸键对）、`@version v1` + `42`（首结构是 @ 指令）、
    //   `42 { a: 1 }`（名为 42 的块）均不报。
    // T9 收紧：判据 = 值型标量（数字/引号串/bool/null）后跟块/数组/键值 ⇒ 报；
    //   裸词标识符（server/a 作块类型名/块名）不算值型，故 `server web { }` 合法、`42 a { }` 报错。
    auto is_scalar_value = [&](const Token& t) -> bool {
        if (t.t != Token::T::Word) return false;
        const std::string& s = t.s;
        if (!s.empty() && s[0] == '"') return true;
        if (s == "true" || s == "false" || s == "null") return true;
        try { size_t idx; std::stod(s, &idx); if (idx == s.size() && !s.empty()) return true; } catch (...) {}
        return false;
    };
    bool lone_scalar = (st.toks.size() == 1 && first.t == Token::T::Word);
    if (lone_scalar) {
        if (err && err->empty())
            *err = code_prefix(SML_E_PARSE_008, "sml: 顶层只能是容器（键值块/对象块或数组），单个标量无法往返");
        return nullptr;
    }
    if (is_scalar_value(first)) {
        bool followed = (st.toks.size() >= 2 && st.toks[1].t == Token::T::LBracket)
            || (st.toks.size() >= 2 && st.toks[1].t == Token::T::Word
                && st.toks.size() >= 3
                && (st.toks[2].t == Token::T::LBrace || st.toks[2].t == Token::T::Colon));
        if (followed) {
            if (err && err->empty())
                *err = code_prefix(SML_E_PARSE_008, "sml: 顶层只能是容器（键值块/对象块或数组），单个标量无法往返");
            return nullptr;
        }
    }
    bool scalar0 = (first.t == Token::T::Word);
    bool bare0 = scalar0 && (st.toks.size() == 1
                 || (st.toks.size() >= 2
                     && st.toks[1].t != Token::T::LBrace
                     && st.toks[1].t != Token::T::Colon));
    bool only = (st.toks.size() == 1);
    bool followed = (st.toks.size() >= 2 && st.toks[1].t == Token::T::LBracket)
        || (st.toks.size() >= 2 && st.toks[1].t == Token::T::Word
            && st.toks.size() >= 3
            && (st.toks[2].t == Token::T::LBrace || st.toks[2].t == Token::T::Colon));
    if (false) {  // T9：旧裸词判据已废弃，改用上方「值型标量后跟容器」判定
        if (err && err->empty())
            *err = code_prefix(SML_E_PARSE_008,
                "sml: 顶层只能是容器（键值块/对象块/数组），单个标量无法往返");
        return nullptr;
    }
    /* 顶层数组门控：跳过开头的 `@version` / `@feature` 行，看实质第一个 token。
       `@feature` 行在前时 `first` 是 `At`，直接判 `first == LBracket` 永远不成立。 */
    {
        size_t k = 0;
        while (k < st.toks.size() && st.toks[k].t == Token::T::At) {
            size_t li = st.toks[k].line;
            k++;
            while (k < st.toks.size() && st.toks[k].line == li) k++;
        }
        if (k < st.toks.size() && st.toks[k].t == Token::T::LBracket
            && !(st.features & (1u << 5))) {
            if (err && err->empty())
                *err = code_prefix(SML_E_FEATURE_001,
                    "sml: `top-level-array`：当前文档已禁用该特性");
            return nullptr;
        }
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
ValuePtr Parser::parse(const std::string& text, std::string* err,
                       const std::string& include_dir, std::vector<Diag>* diags) {
    ParseOptions opts;
    opts.include_dir = include_dir;
    opts.diags = diags;
    return parse_with(text, opts, err);
}

ValuePtr Parser::parse_with(const std::string& text, const ParseOptions& opts,
                            std::string* err) {
    try {
        /* 文件级规范化：跳过开头的 UTF-8 BOM（EF BB BF）。BOM **不是空白** ⇒ 会被词法器吞进
           第一个单词 ⇒ 第一个键名变成 "\xEF\xBB\xBFx"（**静默**改键名）。来源很常见：
           Windows 记事本「另存为 UTF-8」。与 Rust / JS / C / Lua 对齐。 */
        const bool has_bom = text.size() >= 3
            && static_cast<unsigned char>(text[0]) == 0xEF
            && static_cast<unsigned char>(text[1]) == 0xBB
            && static_cast<unsigned char>(text[2]) == 0xBF;
        const std::string body = has_bom ? text.substr(3) : text;
        if (opts.weak_binds) opts.weak_binds->clear();
        return parse_impl(body, err, opts.include_dir, opts.diags,
                          opts.builtins, opts.weak_binds);
    } catch (const std::bad_alloc&) {
        if (err) *err = code_prefix(SML_E_LIMIT_010, "sml: 内存分配失败");
        return nullptr;
    }
}

ValuePtr Parser::load_file(const std::string& path, const ParseOptions& opts,
                           std::string* err) {
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        if (err) *err = code_prefix(SML_E_IO_001, "sml: 读取失败 " + path);
        return nullptr;
    }
    std::string text((std::istreambuf_iterator<char>(in)), std::istreambuf_iterator<char>());
    /* include 基准默认取**文件所在目录**：与 Rust `parse_file` / C `sml_parse_file` 同口径。
       调用方显式给了 include_dir 就不覆盖。 */
    ParseOptions o = opts;
    if (o.include_dir.empty()) {
        const size_t cut = path.find_last_of("/\\");
        o.include_dir = (cut == std::string::npos) ? std::string(".") : path.substr(0, cut);
    }
    return parse_with(text, o, err);
}

ValuePtr Parser::load_file(const std::string& path, std::string* err) {
    return load_file(path, ParseOptions{}, err);
}

// ===========================================================================
// BuiltinTable（宿主注入的内置契约表）
// ===========================================================================
void BuiltinTable::add_field(const std::string& contract, const std::string& field,
                             const std::string& type, bool required) {
    BuiltinField f;
    f.type = type;
    f.required = required;
    contracts_[contract].fields[field] = f;
}

void BuiltinTable::set_range(const std::string& contract, const std::string& field,
                             double min, double max) {
    auto c = contracts_.find(contract);
    if (c == contracts_.end()) return;
    auto f = c->second.fields.find(field);
    if (f == c->second.fields.end()) return;
    f->second.has_min = true;
    f->second.min = min;
    f->second.has_max = true;
    f->second.max = max;
}

void BuiltinTable::add_enum(const std::string& contract, const std::string& field,
                            const std::string& value) {
    auto c = contracts_.find(contract);
    if (c == contracts_.end()) return;
    auto f = c->second.fields.find(field);
    if (f == c->second.fields.end()) return;
    f->second.type = "enum";
    f->second.values.push_back(value);
}

// ===========================================================================
// Parser::apply_contract
// ===========================================================================
/* 同上：真正的契约校验实现，公开入口只负责接 std::bad_alloc。
   注意 resolve_and_check 里对嵌套契约的递归走的是公开入口 Parser::apply_contract，
   于是每层各有一个 try —— 开销可忽略，换来「任何一层的分配失败都带码」。 */
static bool apply_contract_obj(const ValuePtr& val, const Contract& c,
                               const std::map<std::string,Contract>& contracts,
                               std::string* err) {
    if (val->tag != Value::Tag::Obj) { if(err)*err=code_prefix(SML_E_CONTRACT_008, "sml: contract applied to non-object"); return false; }
    // strict mode (default): reject undeclared fields unless contract is `loose`
    if (!c.allow_extra) {
    for (auto& kv : val->obj) {
        /* `__type` / `__featags` 不参与契约校验（Rust 实测：`S !t { }` 在严格契约下通过）；
           `__name` / `__args` **参与** —— `x { @is A addr Sub { } }` 报 `addr.__name`。
           逐个列举是刻意的，别改成 `__` 前缀。 */
        if (kv.first == "__type" || kv.first == "__featags") continue;
        bool declared = false;
        for (auto& f : c.fields) if (f.name == kv.first) { declared=true; break; }
        if (!declared) {
            if (err)*err = code_prefix(SML_E_CONTRACT_004, "sml: field '" + kv.first + "' not declared in contract '" + c.name + "'");
            return false;
        }
    }
    }
    for (auto& f : c.fields) {
        if (!apply_one_field(val, contracts, f, err)) return false;
    }
    return true;
}

static bool apply_contract_impl(const ValuePtr& val,
                                const std::map<std::string,Contract>& contracts,
                                const std::string& name,
                                std::string* err) {
    auto it = contracts.find(name);
    if (it == contracts.end()) { if(err)*err=code_prefix(SML_E_CONTRACT_001, "sml: unknown contract '"+name+"'"); return false; }
    return apply_contract_obj(val, it->second, contracts, err);
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

/* 多契约叠加（D3）：合并成一份临时 `Contract` 后复用 `apply_contract_obj`。
   声明见 `02_coerce_type.inc`（`struct ContractSet` 与 `apply_contract_set` 原型）。 */
static bool apply_contract_set_impl(const ValuePtr& val,
                                    const std::map<std::string,Contract>& contracts,
                                    const ContractSet& set,
                                    std::string* err) {
    if (set.items.empty()) return true;
    if (set.items.size() == 1 && set.items[0].second < 0)
        return Parser::apply_contract(val, contracts, set.items[0].first, err);
    // 1) 逐个解析（未定义 ⇒ 001，同单契约）
    std::vector<const Contract*> list;
    for (size_t i = 0; i < set.items.size(); i++) {
        auto it = contracts.find(set.items[i].first);
        if (it == contracts.end()) {
            if (err) *err = code_prefix(SML_E_CONTRACT_001,
                "sml: unknown contract '" + set.items[i].first + "'");
            return false;
        }
        list.push_back(&it->second);
    }
    // 2) 冲突预检：按**字段名**两两比较，与书写顺序无关
    for (size_t i = 0; i < list.size(); i++) {
        for (size_t j = i + 1; j < list.size(); j++) {
            for (const auto& fa : list[i]->fields) {
                for (const auto& fb : list[j]->fields) {
                    if (fb.name != fa.name) continue;
                    if (!field_clash(fa, fb)) continue;
                    if (err) *err = code_prefix(SML_E_CONTRACT_016,
                        "sml: field '" + fa.name + "' differs between contract '" +
                        list[i]->name + "' and '" + list[j]->name + "'");
                    return false;
                }
            }
        }
    }
    // 3) 合并视图：字段并集；同名字段的类型/默认值/区间已一致，
    //    但 `optional` **不在**预检内 ⇒ 取 AND（任一份要求必填即必填），才与顺序无关。
    Contract merged;
    for (size_t i = 0; i < list.size(); i++) {
        for (const auto& f : list[i]->fields) {
            bool found = false;
            for (auto& dst : merged.fields) {
                if (dst.name != f.name) continue;
                if (!f.mods.optional) dst.mods.optional = false;
                found = true;
                break;
            }
            if (!found) merged.fields.push_back(f);
        }
    }
    // 4) 严格度：块级覆盖 unwrap_or 契约级，多契约取**最严**（`allow_extra` 取 AND）
    merged.allow_extra = true;
    for (size_t k = 0; k < list.size(); k++) {
        int ov = set.items[k].second;
        bool loose = (ov < 0) ? list[k]->allow_extra : (ov == 0);
        if (!loose) merged.allow_extra = false;
        if (list[k]->strict_paths) merged.strict_paths = true;
        if (list[k]->allow_any) merged.allow_any = true;
        for (const auto& d : list[k]->featags) merged.featags.push_back(d);
    }
    // 名字只用于错误文案（`A+B`，与 Rust 一致）
    merged.name.clear();
    for (size_t k = 0; k < list.size(); k++) {
        if (k) merged.name += "+";
        merged.name += list[k]->name;
    }
    return apply_contract_obj(val, merged, contracts, err);
}

static bool apply_contract_set(const ValuePtr& val,
                               const std::map<std::string, Contract>& contracts,
                               const ContractSet& set,
                               std::string* err) {
    try {
        return apply_contract_set_impl(val, contracts, set, err);
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
