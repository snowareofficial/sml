
// ---------------------------------------------------------------------------
// 弱绑定（`@weakbind`）：只读软校验 + 旁路表（`docs/swsml/spec/weakbind.md`）
// 与 `@is` 的 apply_contract 是**两条路径**：那条会填默认值、改树，这条只出诊断。
// ---------------------------------------------------------------------------
static int cur_line(const PState& st) {
    return (st.i < st.toks.size()) ? static_cast<int>(st.toks[st.i].line) : 0;
}

/* 拼出「祖先路径 + 本级键」：键名用 `.` 连接，`[n]` 段不加点。 */
static std::string weak_path(const PState& st, const std::string& leaf) {
    std::string out;
    for (size_t k = 0; k < st.path.size(); ++k) {
        if (k && !st.path[k].empty() && st.path[k][0] != '[') out += ".";
        out += st.path[k];
    }
    if (!leaf.empty()) {
        if (!out.empty() && leaf[0] != '[') out += ".";
        out += leaf;
    }
    return out;
}

static void weak_warn(PState& st, const std::string& msg) {
    if (!st.diags) return;
    st.diags->push_back(Diag{"W-CONTRACT-001", msg, cur_line(st)});
}

static const BuiltinContract* builtin_find(const PState& st, const std::string& name) {
    if (!st.builtins) return nullptr;
    auto it = st.builtins->all().find(name);
    return (it == st.builtins->all().end()) ? nullptr : &it->second;
}

/* 声明侧：`@weakbind X { 字段: 类型 }` 的每个字段与内置表比对（只读）。 */
static void weak_check_decl(PState& st, const std::string& cname,
                            const std::string& fname, const std::string& tyname) {
    const BuiltinContract* c = builtin_find(st, cname);
    if (!c) return;   // 契约名不在表里：已在声明处报过「未知契约」
    auto it = c->fields.find(fname);
    if (it == c->fields.end()) {
        weak_warn(st, "sml: 契约 `" + cname + "` 没有字段 `" + fname + "`");
        return;
    }
    if (!tyname.empty() && it->second.type != tyname)
        weak_warn(st, "sml: 字段 `" + cname + "." + fname + "` 的类型与内置契约不同");
}

/* 块内容侧：缺必填 / 类型 / 区间 / 枚举（四类），**只读**，绝不改树。 */
static void weak_check_block(PState& st, const ValuePtr& node, const std::string& cname) {
    const BuiltinContract* c = builtin_find(st, cname);
    if (!c) return;   // 内置表里没有 ⇒ 声明侧已处理，块侧不重复报
    if (!node || node->tag != Value::Tag::Obj) return;
    for (const auto& kv : c->fields) {
        const std::string& fname = kv.first;
        const BuiltinField& f = kv.second;
        ValuePtr v = node->get(fname);
        if (!v) {
            if (f.required)
                weak_warn(st, "sml: 块里缺必填字段 `" + cname + "." + fname + "`");
            continue;
        }
        bool ok = true;
        if (f.type == "str")       ok = (v->tag == Value::Tag::Str);
        else if (f.type == "int")  ok = (v->tag == Value::Tag::Int);
        else if (f.type == "num")  ok = (v->tag == Value::Tag::Int || v->tag == Value::Tag::Float);
        else if (f.type == "bool") ok = (v->tag == Value::Tag::Bool);
        else if (f.type == "enum") {
            ok = (v->tag == Value::Tag::Str);
            if (ok) {
                bool found = false;
                for (const auto& ev : f.values) {
                    if (ev == v->s) { found = true; break; }
                }
                ok = found;
            }
        }
        if (!ok) {
            weak_warn(st, "sml: 字段 `" + cname + "." + fname + "` 与内置契约类型不符");
            continue;
        }
        if (f.has_min || f.has_max) {
            double n = 0.0;
            bool isnum = false;
            if (v->tag == Value::Tag::Int) { n = static_cast<double>(v->i); isnum = true; }
            else if (v->tag == Value::Tag::Float) { n = v->f; isnum = true; }
            if (isnum && ((f.has_min && n < f.min) || (f.has_max && n > f.max)))
                weak_warn(st, "sml: 字段 `" + cname + "." + fname + "` 超出内置契约的区间");
        }
    }
}

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
    /* 开了 `type.featag` 时，块头可以有 `!tag [args]` 区与逗号分隔的块名
       （`Service !critical !obey[4], prod { }`）—— 这两类记号要算进「到 `{` 为止」的窗口。
       只在窗口里真的出现过 `!` 词时才放宽，避免把 `a [1] {` 这类旧输入改判成裸块。 */
    const bool feat_on = (st.features & (1u << 17)) != 0;
    bool seen_tag = false;
    size_t p = st.i;
    while (p < st.toks.size()) {
        auto tt = st.toks[p].t;
        if (tt == Token::T::Word) {
            /* featag 项的两种形状都算「窗口里有 featag 区」：`!名`（无参）与 `名[`（带参）
               —— 后者没有 `!`，漏判会让紧随的终止逗号被当成「裸词里的逗号」而拒
               （`Menu density[auto], main { }`）。 */
            if (feat_on && !st.toks[p].s.empty() &&
                (st.toks[p].s[0] == '!' ||
                 (p + 1 < st.toks.size() && st.toks[p + 1].t == Token::T::LBracket)))
                seen_tag = true;
            p++;
        } else if (feat_on && tt == Token::T::LBracket) {
            int depth = 0;
            while (p < st.toks.size()) {
                if (st.toks[p].t == Token::T::LBracket) depth++;
                else if (st.toks[p].t == Token::T::RBracket) {
                    if (--depth == 0) { p++; break; }
                }
                p++;
            }
        } else if (feat_on && seen_tag && tt == Token::T::Comma) {
            p++;
        } else if (tt == Token::T::LBrace) return true;
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
    bool key_is_contract = (st.contracts.find(type_word) != st.contracts.end());
    bool arg_has_contract = false;
    while (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word) {
        if (key_is_contract && st.contracts.find(st.toks[st.i].s) != st.contracts.end())
            arg_has_contract = true;
        args.push_back(coerce(st.toks[st.i], st.fragments, st.err, st.features));
        st.i++;
    }
    // 任务 D：数组位置的块级多契约同样「响亮拒绝」（head 命中契约表且参数里出现第 2 个契约词）。
    if (key_is_contract && arg_has_contract) {
        if (st.err && st.err->empty())
            *st.err = code_prefix(SML_E_CONTRACT_017, "sml: 该实现暂不支持一个块应用多份契约（块级多契约写法）");
        st.aborted = true;
        return Value::null();
    }
    if (st.i >= st.toks.size() || st.toks[st.i].t != Token::T::LBrace) return Value::null();
    st.i++;                                   // `{`
    st.enter_key = type_word;                 // 旁路表路径用块类型名当段
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

/* `@for var in a b c { ... }` —— 有界循环展开，返回数组。
   与 Rust `cond.rs::eval_for_header` + `Parser::parse_for` 同语义：
   只遍历有限枚举、变量只读、无 `while` / 无递归。
   Bounded loop only: finite enumeration, read-only variable, no recursion. */
static ValuePtr parse_for(PState& st) {
    auto fail = [&](const char* code, const std::string& msg) -> ValuePtr {
        if (st.err && st.err->empty()) *st.err = code_prefix(code, msg);
        st.aborted = true;
        return nullptr;
    };
    st.i += 2;   // 越过 `@` 与 `for`
    if (st.i >= st.toks.size() || st.toks[st.i].t != Token::T::Word)
        return fail(SML_E_PARSE_018, "sml: `@for` 缺少循环变量");
    const std::string var = st.toks[st.i].s;
    st.i++;
    if (st.i >= st.toks.size() || st.toks[st.i].s != "in")
        return fail(SML_E_PARSE_018, "sml: `@for` 缺少 `in`");
    st.i++;
    std::vector<std::string> items;
    while (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word) {
        items.push_back(st.toks[st.i].s);
        st.i++;
    }
    if (items.empty())
        return fail(SML_E_PARSE_018, "sml: `@for` 的 `in` 之后至少须有一个枚举项");
    if (st.i >= st.toks.size() || st.toks[st.i].t != Token::T::LBrace)
        return fail(SML_E_PARSE_018, "sml: `@for` 缺少循环体 `{ ... }`");
    const size_t start = st.i + 1;
    size_t j = start;
    int depth = 1;
    while (j < st.toks.size()) {
        if (st.toks[j].t == Token::T::LBrace) depth++;
        else if (st.toks[j].t == Token::T::RBrace) { depth--; if (depth == 0) break; }
        j++;
    }
    if (depth != 0)
        return fail(SML_E_PARSE_001, "sml: 未闭合的块（遇到文件结尾，缺少结束符号 }）");
    const size_t end = j;
    auto arr = Value::array();
    std::vector<Token> saved = std::move(st.toks);
    const std::string saved_var = g_loop_var, saved_val = g_loop_val;
    for (const auto& item : items) {
        std::vector<Token> sub(saved.begin() + static_cast<long>(start),
                               saved.begin() + static_cast<long>(end));
        sub.push_back(Token{Token::T::RBrace, "}", saved[end].line});
        st.toks = sub;
        st.i = 0;
        g_loop_var = var;
        g_loop_val = item;
        auto node = parse_block(st, false, true);
        if (st.aborted) break;
        arr->arr.push_back(node);
    }
    st.toks = std::move(saved);
    st.i = end + 1;
    g_loop_var = saved_var;
    g_loop_val = saved_val;
    return arr;
}

/* `@when` 条件求值（闭集：`$env.NAME` [+ `==`/`!=`/`=` + 值]）。
   与 Rust `cond.rs::eval_when_cond` 同语义：无运算符时做真值测试。
   Closed set, no expression evaluator — mirrors Rust `cond.rs::eval_when_cond`. */
static int eval_when(PState& st) {
    auto fail = [&](const char* code, const std::string& msg) -> int {
        if (st.err && st.err->empty()) *st.err = code_prefix(code, msg);
        st.aborted = true;
        return 0;
    };
    if (st.i >= st.toks.size() || st.toks[st.i].t != Token::T::Word)
        return fail(SML_E_PARSE_014, "sml: `@when` 后须条件（如 `$env.ENV == \"prod\"`）");
    const std::string lv = st.toks[st.i].s;
    if (lv.rfind("$env.", 0) != 0 || lv.size() <= 5)
        return fail(SML_E_PARSE_014,
                    "sml: `@when` 的条件左侧只支持 `$env.NAME`，得 `" + lv + "`");
    const std::string var = lv.substr(5);
    if (!(st.features & (1u << 2)))
        return fail(SML_E_FEATURE_002, "sml: `@when $env." + var + "` 需要特性 `env`");
    const char* ev = std::getenv(var.c_str());
    const std::string act = ev ? ev : "";
    st.i++;
    bool is_eq = false, is_ne = false;
    if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word) {
        const std::string& w = st.toks[st.i].s;
        if (w == "==" || w == "=") { is_eq = true; st.i++; }
        else if (w == "!=") { is_ne = true; st.i++; }
    }
    const std::string opname = is_ne ? "!=" : "==";
    if (!is_eq && !is_ne) {
        /* 真值测试：空串 / "0" / "false" 视为假 ｜ truthiness test */
        return (!act.empty() && act != "0" && act != "false") ? 1 : 0;
    }
    if (st.i >= st.toks.size() || st.toks[st.i].t != Token::T::Word)
        return fail(SML_E_PARSE_015, "sml: `@when` 的 `" + opname + "` 后须比较值");
    if (st.i + 1 < st.toks.size() && st.toks[st.i + 1].t == Token::T::Colon)
        return fail(SML_E_PARSE_015,
                    "sml: `@when` 的 `" + opname + "` 后缺少比较值"
                    "（写成了运算符后直接换行）");
    std::string rhs = st.toks[st.i].s;
    st.i++;
    if (rhs.size() >= 2 && rhs.front() == '"' && rhs.back() == '"')
        rhs = rhs.substr(1, rhs.size() - 2);
    const bool eq = (act == rhs);
    return is_ne ? (eq ? 0 : 1) : (eq ? 1 : 0);
}
