
// ---------------------------------------------------------------------------
// 标识符白名单（门控 `lex.ident`）—— 与 Rust `sml-lex/src/identifiers.rs` 同判据
//
// 白名单：`A-Z` `a-z` `0-9` `_` `-` + 汉字 U+4E00–U+9FFF + 假名 U+3040–U+30FF
//        + 谚文音节 U+AC00–U+D7AF。
// 六类拒绝共用 `E-LEX-007`、靠 detail 区分：路径保留字符 / 空白与控制 /
// 不可见（零宽格式 Cf 与组合标记 Mn·Mc）/ 同形字母 / 全角兼容区 / 其余。
// 位置是**字符序号**（1 起算），不是字节偏移 —— 与错误文案一致。
// ---------------------------------------------------------------------------

static const unsigned int IDENT_COMBINING[][2] = {
    {0x0300,0x036F},{0x0483,0x0489},{0x1AB0,0x1AFF},
    {0x1DC0,0x1DFF},{0x20D0,0x20FF},{0xFE20,0xFE2F}};
static const unsigned int IDENT_CF[][2] = {
    {0x00AD,0x00AD},{0x0600,0x0605},{0x061C,0x061C},{0x06DD,0x06DD},
    {0x070F,0x070F},{0x180E,0x180E},{0x200B,0x200F},{0x202A,0x202E},
    {0x2060,0x2064},{0x2066,0x206F},{0xFEFF,0xFEFF},{0xFFF9,0xFFFB},
    {0x1D173,0x1D17A},{0xE0001,0xE0001},{0xE0020,0xE007F}};
static const unsigned int IDENT_HOMOGLYPH[][2] = {
    {0x0370,0x03FF},{0x0400,0x04FF},{0x0500,0x052F},{0x0530,0x058F},
    {0x1F00,0x1FFF}};
static const unsigned int IDENT_WIDE[][2] = {
    {0x2E80,0x2EFF},{0x3000,0x303F},{0x31C0,0x31EF},{0xFE30,0xFE4F},
    {0xFF01,0xFF60},{0xFFE0,0xFFE6}};

static const char* const IDENT_SCRIPT_NAME[3] = { "拉丁字母", "汉字/假名", "谚文" };

static bool ident_in_ranges(unsigned int u, const unsigned int (*r)[2], size_t n) {
    for (size_t i = 0; i < n; i++)
        if (u >= r[i][0] && u <= r[i][1]) return true;
    return false;
}

// UTF-8 解码一个码点：返回码点，*len 为字节数。
// 非法字节按 U+FFFD、1 字节处理（它不在白名单内 ⇒ 会被拒）。
static unsigned int utf8_next(const unsigned char* s, size_t n, size_t* len) {
    unsigned char c = s[0];
    unsigned int cp;
    size_t l, k;
    if (c < 0x80) { cp = c; l = 1; }
    else if (c < 0xC0) { cp = 0xFFFD; l = 1; }
    else if (c < 0xE0) { cp = (unsigned int)(c & 0x1Fu); l = 2; }
    else if (c < 0xF0) { cp = (unsigned int)(c & 0x0Fu); l = 3; }
    else if (c < 0xF8) { cp = (unsigned int)(c & 0x07u); l = 4; }
    else { cp = 0xFFFD; l = 1; }
    for (k = 1; k < l; k++) {
        if (k >= n || (s[k] & 0xC0) != 0x80) { cp = 0xFFFD; l = 1; break; }
        cp = (cp << 6) | (unsigned int)(s[k] & 0x3Fu);
    }
    *len = l;
    return cp;
}

// 去引号：本端引号串也存成 Word（带引号），白名单要判的是**内容**。
static std::string ident_unquote(const std::string& s) {
    if (s.size() >= 2 && s.front() == '"' && s.back() == '"')
        return s.substr(1, s.size() - 2);
    return s;
}

// 标识符用的是什么文字系统：0 = 拉丁 / 1 = 汉字假名 / 2 = 谚文；-1 = 无。
// 汉字与假名算**同一族**（日文里汉字与假名本来就混写），与 Rust `Script::Cjk` 同口径。
static int ident_script(const std::string& s) {
    size_t i = 0, n = s.size();
    while (i < n) {
        size_t l;
        unsigned int u = utf8_next((const unsigned char*)s.data() + i, n - i, &l);
        if ((u >= 'A' && u <= 'Z') || (u >= 'a' && u <= 'z')) return 0;
        if ((u >= 0x4E00 && u <= 0x9FFF) || (u >= 0x3040 && u <= 0x30FF)) return 1;
        if (u >= 0xAC00 && u <= 0xD7AF) return 2;
        i += l;
    }
    return -1;
}

// 校验一个标识符；不合法时写错误（含码）并返回 false。
static bool ident_check(const std::string& raw, std::string* err) {
    const std::string s = ident_unquote(raw);
    if (s.empty()) {
        if (err && err->empty()) *err = code_prefix(SML_E_LEX_007, "sml: 标识符为空");
        return false;
    }
    size_t i = 0, n = s.size();
    int at = 0;
    while (i < n) {
        size_t l;
        unsigned int u = utf8_next((const unsigned char*)s.data() + i, n - i, &l);
        at++;
        char hex[16];
        snprintf(hex, sizeof hex, "U+%04X", u);
        const std::string ch = (u < 0x80) ? std::string(1, (char)u) : std::string("?");
        if ((u >= 'A' && u <= 'Z') || (u >= 'a' && u <= 'z') ||
            (u >= '0' && u <= '9') || u == '_' || u == '-') { i += l; continue; }
        if (u >= 0x4E00 && u <= 0x9FFF) { i += l; continue; }   // 汉字
        if (u >= 0x3040 && u <= 0x30FF) { i += l; continue; }   // 假名
        if (u >= 0xAC00 && u <= 0xD7AF) { i += l; continue; }   // 谚文
        if (u == '.' || u == '[' || u == ']') {
            if (err && err->empty())
                *err = code_prefix(SML_E_LEX_007, "sml: 第 " + std::to_string(at) +
                    " 个字符 `" + ch + "`（" + hex + "）是路径语法保留字符，键名里不能出现 `.` `[` `]`");
            return false;
        }
        if (u <= 0x20 || u == 0x7F || u == 0xA0 || u == 0x1680 || u == 0x205F ||
            (u >= 0x2000 && u <= 0x200A) || u == 0x2028 || u == 0x2029) {
            if (err && err->empty())
                *err = code_prefix(SML_E_LEX_007, "sml: 第 " + std::to_string(at) +
                    " 个字符是空白或控制字符（" + hex + "）");
            return false;
        }
        // 组合标记 Mn/Mc（须排在同形字母块之前：「组合用西里尔字母」也在此区）
        if (ident_in_ranges(u, IDENT_COMBINING, sizeof IDENT_COMBINING / sizeof IDENT_COMBINING[0])) {
            if (err && err->empty())
                *err = code_prefix(SML_E_LEX_007, "sml: 第 " + std::to_string(at) +
                    " 个字符是组合标记（Mn/Mc，非预组合 ⇒ 非 NFC）（" + hex +
                    "）——不可见，且能伪造看起来一样的名字");
            return false;
        }
        if (ident_in_ranges(u, IDENT_CF, sizeof IDENT_CF / sizeof IDENT_CF[0])) {
            if (err && err->empty())
                *err = code_prefix(SML_E_LEX_007, "sml: 第 " + std::to_string(at) +
                    " 个字符是零宽/格式字符（Cf）（" + hex + "）——不可见，且能伪造看起来一样的名字");
            return false;
        }
        if (ident_in_ranges(u, IDENT_HOMOGLYPH, sizeof IDENT_HOMOGLYPH / sizeof IDENT_HOMOGLYPH[0])) {
            if (err && err->empty())
                *err = code_prefix(SML_E_LEX_007, "sml: 第 " + std::to_string(at) +
                    " 个字符 `" + ch + "`（" + hex +
                    "）是与拉丁字母同形的西里尔 / 希腊 / 亚美尼亚字母；声明处只允许一种字形，请改用 ASCII");
            return false;
        }
        if (ident_in_ranges(u, IDENT_WIDE, sizeof IDENT_WIDE / sizeof IDENT_WIDE[0])) {
            if (err && err->empty())
                *err = code_prefix(SML_E_LEX_007, "sml: 第 " + std::to_string(at) +
                    " 个字符 `" + ch + "`（" + hex +
                    "）是全角 / 兼容区字符：与半角混用会「看起来一样、比较不等」");
            return false;
        }
        if (err && err->empty())
            *err = code_prefix(SML_E_LEX_007, "sml: 第 " + std::to_string(at) +
                " 个字符 `" + ch + "`（" + hex +
                "）不在白名单内（只允许字母数字、`_` `-`、汉字 / 假名 / 谚文）");
        return false;
    }
    return true;
}

// 「每契约单一语种」：记录 / 比对文字系统；混用即报错。
static bool ident_check_script(const std::string& raw, int& seen, std::string& first,
                               std::string* err) {
    const std::string s = ident_unquote(raw);
    int sc = ident_script(s);
    if (sc < 0) return true;                 // 纯数字 / `_` / `-` 不参与
    if (seen < 0) { seen = sc; first = s; return true; }
    if (seen != sc) {
        if (err && err->empty())
            *err = code_prefix(SML_E_LEX_007, "sml: `" + s + "`（" + IDENT_SCRIPT_NAME[sc] +
                "）与 `" + first + "`（" + IDENT_SCRIPT_NAME[seen] +
                "）混用了文字系统：同一契约内的标识符只能有一种语种");
        return false;
    }
    return true;
}

static ValuePtr coerce_word(const std::string& raw,
                            const std::map<std::string,ValuePtr>& fragments,
                            std::string* err,
                            unsigned features) {
    std::string t = raw;
    // featag（B2）：`!` 只在块头的 featag 区里是记号；出现在**值位置**就是歧义
    // （`a: !b` —— 字符串还是 featag？）。歧义一律报错，**开了 `bareword-string`
    // 也不豁免**：靠特性绕过歧义等于把「猜」藏进配置里。要写字面量请加引号。
    // 门控 `type.featag`：未开启时 `!b` 仍是普通裸词，v1–v4 既有文档一字不变。
    if ((features & (1u << 17)) && !t.empty() && t[0] == '!') {
        if (err && err->empty())
            *err = code_prefix(SML_E_PARSE_027,
                "sml: 值位置不能写以 `!` 开头的裸词；若本意是字符串，请用引号包起来：" + t);
        return Value::null();
    }
    // $env.VAR inline (bareword)。门控与引号串内插同判据：
    // 改前只拦 `"..."` 里的 `$env.`，裸词照旧读宿主环境变量 ⇒
    // `@feature disable env` 在沙箱里形同虚设。
    if (t.rfind("$env.", 0) == 0) {
        if (!(features & (1u << 2))) {
            if (err && err->empty())
                *err = code_prefix(SML_E_FEATURE_002,
                    "sml: 裸词 `" + t + "` 无法解析：未启用特性 `env`");
            return Value::null();
        }
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
            /* 前导零保留为字符串（与 Rust 同判据）：`007` / `0755` 转 Int 会把
               权限位、编号静默改成 7 / 755。 */
            {
                const size_t s0 = (t[0] == '+' || t[0] == '-') ? 1 : 0;
                if (t.size() > s0 + 1 && t[s0] == '0'
                    && t.compare(s0, 2, "0x") != 0 && t.compare(s0, 2, "0b") != 0)
                    return Value::string(t);
            }
            return Value::integer(v);
        } catch(...) {}
    }
    /* `bareword-string` 门控：关闭后纯字符串裸词必须加引号（与 Rust 同码 005）。 */
    if (!(features & (1u << 0))) {
        if (err && err->empty())
            *err = code_prefix(SML_E_FEATURE_005,
                "sml: 裸词 `" + t + "` 应写作 `\"" + t + "\"`（bareword-string 已禁用）");
        return Value::null();
    }
    return Value::string(t);
}

/* 内插 `$env.NAME`（与 Rust `interp_str` 同语义）：NAME 取 [字母数字_ .]，取到第一个
   其它字符为止；名字为空则原样保留 `$env.`。改前只认「整串恰为 `$env.VAR`」，
   内嵌形式不替换 —— conformance 26 的分歧，四端已统一为内插。
   ｜ Inline `$env.NAME`, same semantics as the Rust reference. */
/* `@for` 的循环变量绑定（单线程解析，展开时临时设置、展开完清空）。
   `@for` loop-variable binding: set during expansion, cleared afterwards. */
static std::string g_loop_var;
static std::string g_loop_val;

/* `${var}` 只读插值：未绑定则原样保留。
   Read-only loop-variable interpolation; unknown names stay verbatim. */
static std::string interp_loop(const std::string& s) {
    if (g_loop_var.empty() || s.find("${") == std::string::npos) return s;
    std::string out;
    size_t pos = 0;
    for (;;) {
        size_t at = s.find("${", pos);
        if (at == std::string::npos) { out.append(s, pos, std::string::npos); break; }
        size_t close = s.find('}', at + 2);
        if (close == std::string::npos) { out.append(s, pos, std::string::npos); break; }
        out.append(s, pos, at - pos);
        const std::string name = s.substr(at + 2, close - at - 2);
        if (name == g_loop_var) out += g_loop_val;
        else out += "${" + name + "}";
        pos = close + 1;
    }
    return out;
}

static std::string interp_env(const std::string& s) {
    if (s.find("$env.") == std::string::npos) return s;
    std::string out;
    size_t pos = 0;
    for (;;) {
        size_t at = s.find("$env.", pos);
        if (at == std::string::npos) { out.append(s, pos, std::string::npos); break; }
        out.append(s, pos, at - pos);
        size_t j = at + 5, k = j;
        while (k < s.size()) {
            char c = s[k];
            if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
                  (c >= '0' && c <= '9') || c == '_' || c == '.')) break;
            k++;
        }
        if (k > j) {
            std::string name = s.substr(j, k - j);
            const char* ev = std::getenv(name.c_str());
            out += (ev ? ev : "");
            pos = k;
        } else {
            out += "$env.";
            pos = at + 5;
        }
    }
    return out;
}

/* 版本基线：与 Rust `FeatureSet::baseline()` 一致的 8 位核心特性。
   必须在 `coerce` 之前定义（默认参数用到它）。 */
#define SML_FEATURE_BASELINE \
    ((1u << 0) | (1u << 1) | (1u << 2) | (1u << 3) | \
     (1u << 4) | (1u << 5) | (1u << 6) | (1u << 7))

static ValuePtr coerce(const Token& tok,
                       const std::map<std::string,ValuePtr>& fragments,
                       std::string* err,
                       unsigned features = SML_FEATURE_BASELINE) {
    if (tok.t != Token::T::Word) return Value::null();
    const std::string& t = tok.s;
    if (!t.empty() && t[0] == '"' && t.back() == '"') {
        std::string inner = t.substr(1, t.size()-2);
        std::string ue = unescape_unicode(inner, err);
        /* env 门控：引号串含 `$env.` 且未开 env ⇒ E-FEATURE-002（与 Rust 同判据）。 */
        if (!(features & (1u << 2)) && ue.find("$env.") != std::string::npos) {
            if (err && err->empty())
                *err = code_prefix(SML_E_FEATURE_002,
                    "sml: 字符串无法解析：未启用特性 `env`");
            return Value::null();
        }
        return Value::string(interp_env(interp_loop(ue)));
    }
    return coerce_word(t, fragments, err, features);
}

// ===========================================================================
// Contract parsing  (mirrors Rust parse_contract / parse_type)
// ===========================================================================
static bool parse_type(const std::string& raw, TypeSpec& out, std::string* err,
                       const std::map<std::string, ValuePtr>* types = nullptr,
                       const Ext* ext = nullptr) {
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
        /* 元素位置不许 enum（Rust 同码 012）：改前递归进 enum 分支、因为没有 `[`
           报成 023（「enum 后不是数组」），把码指错了地方。 */
        {
            std::string el = lower(elem);
            while (!el.empty() && (el[0]==' '||el[0]=='\t')) el.erase(0,1);
            if (el.rfind("enum", 0)==0) {
                if (err)*err=code_prefix(SML_E_CONTRACT_012, "sml: 数组元素类型不能是 enum");
                return false;
            }
        }
        if (elem.empty()) e.kind = TypeSpec::Kind::Any;
        else if (!parse_type(elem, e, err, types, ext)) return false;
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
        /* 非内置类型名，按此顺序判定（与 Rust 相同）：
           1. `@type` 声明的自定义类型（模式校验）
           2. 回落为契约引用（组合）
           次序不可颠倒：同名时 `@type` 更「显式」。 */
        if (types) {
            auto it = types->find(s);
            if (it != types->end()) {
                std::string perr;
                auto pat = pat_compile(it->second, types, &perr);
                if (!pat) {
                    if (err) *err = code_prefix(SML_E_CONTRACT_013,
                        "sml: 类型 `" + s + "` 的模式编译失败：" + perr);
                    return false;
                }
                TypeSpec sp; sp.kind = TypeSpec::Kind::Pattern;
                sp.pat_name = s;
                sp.pat = pat;
                out = sp;
                return true;
            }
        }
        /* 2. 外置类型（下游注册，如 image / link / time）—— 与 C / Rust 同序：
              `@type` > 外置 > 契约引用。 */
        if (ext) {
            for (const auto& t : ext->types) {
                if (t.name == s) {
                    TypeSpec sp; sp.kind = TypeSpec::Kind::Ext; sp.ext_type = &t;
                    out = sp;
                    return true;
                }
            }
        }
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

// 契约字段词法切分（仅供 parse_field 使用；不影响顶层键值行）。
//
// 背景：遗留符号修饰符 `?`(optional) 与 `=`(default) 已废弃，五端一律报
// E-PARSE-006。但原实现用 `while (iss >> p)` 只按空白切词，于是 `str?` 这类
// **粘连**写法被整体当成一个 token，下面的 `if (w=="?" || w=="=")` 永远匹配不上，
// 只有带空格的 `str ?` 才拦得住 —— 导致粘连形式静默通过、被当普通类型串
// （实测 C++ 端 `@contract S { x: str? }` 反而报 E-CONTRACT-003 missing field）。
//
// 修法（对齐 C 侧词法层切独立 token 的做法）：在空白切分之外，额外把 `?` 与 `=`
// 切成独立 token，同时把双引号串当原子整体（含内部空白 / `?` / `=`）—— 既修掉粘连
// 缺陷，又不误伤下列合法情形：
//   · 类型串 / 枚举：`enum [ a b ]`、`[int]`、`ContractRef`（本就不含 ?/=）；
//   · `default` 取值里的引号串：`default "a=b"`、`default "str?"` 不会被拆出 ?/=；
//   · 粘连边界：
//       str?x    ->  str, ?, x        （? 命中）
//       a=b      ->  a, =, b          （= 命中）
//       int=5432 ->  int, =, 5432     （= 命中）
//   引号串内部出现的 ?/= 一律保留，绝不外溢到类型/修饰符判定。
static void tokenize_field(const std::string& line, std::vector<std::string>& parts) {
    const size_t n = line.size();
    size_t i = 0;
    std::string cur;
    auto flush = [&]() { if (!cur.empty()) { parts.push_back(cur); cur.clear(); } };
    while (i < n) {
        char c = line[i];
        if (c == ' ' || c == '\t' || c == '\r' || c == '\n') { flush(); i++; continue; }
        if (c == '"') {
            // 引号串当原子：内部空白 / ? / = 全部保留，不再切分（含 \" 转义）。
            cur += c; i++;
            while (i < n) {
                if (line[i] == '\\' && i + 1 < n) { cur += line[i]; cur += line[i+1]; i += 2; continue; }
                char d = line[i];
                cur += d; i++;
                if (d == '"') break;   // 含闭合引号，停止
            }
            flush();
            continue;
        }
        if (c == '?' || c == '=') {     // 粘连的遗留修饰符：切成独立 token
            flush();
            parts.push_back(std::string(1, c));
            i++;
            continue;
        }
        cur += c; i++;
    }
    flush();
}

static bool parse_field(const std::string& line, FieldSpec& out, std::string* err,
                        const std::map<std::string, ValuePtr>* types = nullptr,
                        const Ext* ext = nullptr) {
    // line like:  name: Type optional default "x" min 0 max 10 loose
    std::vector<std::string> parts;
    tokenize_field(line, parts);
    if (parts.empty()){ if(err)*err=code_prefix(SML_E_PARSE_024, "sml: empty field"); return false; }
    /* `name:` 后面什么都没有：类型缺失 ⇒ E-PARSE-006（Rust 同码）。
       改前这里静默落成 Any，契约照常「通过」——用户以为写了约束。 */
    {
        std::string tline = line;
        while (!tline.empty() && (tline.back() == ' ' || tline.back() == '\t')) tline.pop_back();
        if (!tline.empty() && tline.back() == ':') {
            if (err) *err = code_prefix(SML_E_PARSE_006, "sml: 契约字段缺少类型");
            return false;
        }
    }

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
        // 符号形式 `?` / `=` 已废弃（2026-10-30）：统一到单词形式 optional / default，
        // 报 E-PARSE-006 与 Rust / Lua / C 同码。
        if (w=="?" || w=="=") {
            if (err) *err = code_prefix(SML_E_PARSE_006,
                std::string("sml: contract modifier `")+w+"` removed (use `optional` / `default`)");
            return false;
        }
        // S7：来源修饰符 —— 该键只能由 featag 落点写入
        if (w=="from-featag") { mods.from_featag=true; k++; continue; }
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
            if (m=="optional"||m=="loose"||m=="default"||m=="min"||m=="max"||m=="from-featag"||m=="?"||m=="=") break;
            if (!rest.empty()) rest += " ";
            rest += m;
            k++;
        }
        if (!parse_type(rest, type, err, types, ext)) return false;
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
    /* 自定义模式类型（`@type`）：值须为**字符串**并匹配模式。
       非字符串必须显式报错并提示加引号 —— 裸写的号码会被词法切成数字，
       最终只剩末段、还不报错（本项目明确要消灭的「静默数据损坏」）。 */
    if (spec.kind == TypeSpec::Kind::Pattern) {
        if (v->tag != Value::Tag::Str) {
            if (err) *err = code_prefix(SML_E_CONTRACT_009,
                "sml: 类型 `" + spec.pat_name +
                "` 要求字符串（号码 / 编号 / 身份证请用引号包裹），实际为 " + pat_kind_name(v));
            return false;
        }
        {
            std::string perr;
            int m = pat_match(spec.pat, v->s, &perr);
            if (m < 0) {
                if (err) *err = code_prefix(SML_E_CONTRACT_009,
                    "sml: 类型 `" + spec.pat_name + "` 匹配时出错：" + perr);
                return false;
            }
            if (!m) {
                if (err) *err = code_prefix(SML_E_CONTRACT_009,
                    "sml: 值 `" + v->s + "` 不符合类型 `" + spec.pat_name + "` 的格式要求");
                return false;
            }
        }
        return true;
    }
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
        case TypeSpec::Kind::Ext: {
            /* 外置类型：校验器由注册方提供；失败报 E-CONTRACT-007（与 Rust 同码）。 */
            if (!spec.ext_type || !spec.ext_type->check) return true;
            std::string xerr;
            if (spec.ext_type->check(v, xerr)) return true;
            if (err) *err = code_prefix(SML_E_CONTRACT_007,
                "sml: 字段不符合外置类型 `" + spec.ext_type->name + "` 的要求：" +
                (xerr.empty() ? std::string("校验未通过") : xerr));
            return false;
        }
        case TypeSpec::Kind::ContractRef:
            // not resolved here (top-level @is handles refs); accept
            return true;
        case TypeSpec::Kind::Pattern:
            // 上面已提前返回；这里只为让 switch 覆盖全部枚举值
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
// 多契约叠加（D3）：一个块应用多份契约 —— **合并视图**
//
// 语义照 `rust/sml-contract/src/lib.rs::apply_contracts`：
//   1. 有效严格度 = 块级覆盖 `unwrap_or` 契约级声明；多契约取**最严**一档
//      ⇒ 任一份严格即整体严格（本端两档：`allow_extra` 取 AND）；
//   2. **顺序无关**：字段按名取并集；
//   3. **冲突即错**：同名字段但类型 / 默认值 / 区间任一不同 ⇒ `E-CONTRACT-016`
//      （不允许「后者覆盖」——那会让结果依赖书写顺序）；
//   4. 构造一份临时 `Contract` 后**复用** `apply_contract` 的既有校验，
//      不另写一条校验路径。
// ===========================================================================

/* 一组待应用的契约：名字 + 块级严格度覆盖。
   `ov`：-1 = 无覆盖（用契约级声明）/ 0 = loose / 1 = strict。
   `warn` 在本端按 `loose` 记：本端没有「放行 + 告警」通道，而两者的**值树**逐字相同
   （Rust 只多一条诊断），故树对齐。 */
struct ContractSet {
    std::vector<std::pair<std::string, int> > items;
    bool empty() const { return items.empty(); }
    void clear() { items.clear(); }
    bool has(const std::string& n) const {
        for (size_t i = 0; i < items.size(); i++) if (items[i].first == n) return true;
        return false;
    }
    void push(const std::string& n, int ov) {
        if (has(n)) return;   /* 按名去重（Rust `applied_contracts` 同口径，首个覆盖胜） */
        items.push_back(std::make_pair(n, ov));
    }
};

/* 类型的**规范串**（Rust `TypeSpec::name()` 同口径）：数组比元素类型、
   枚举比取值表、组合比被引用契约名。 */
static std::string type_key(const TypeSpec& t) {
    switch (t.kind) {
        case TypeSpec::Kind::Any:  return "any";
        case TypeSpec::Kind::Str:  return "str";
        case TypeSpec::Kind::Int:  return "int";
        case TypeSpec::Kind::Num:  return "num";
        case TypeSpec::Kind::Bool: return "bool";
        case TypeSpec::Kind::Array:
            return "[" + (t.elem ? type_key(*t.elem) : std::string("?")) + "]";
        case TypeSpec::Kind::Enum: {
            std::string s = "enum [";
            for (size_t i = 0; i < t.enum_values.size(); i++) {
                if (i) s += " ";
                s += t.enum_values[i];
            }
            return s + "]";
        }
        case TypeSpec::Kind::ContractRef: return t.contract_ref;
        // 同名即同校验器（Rust `TypeSpec::name()` 对 Pattern 返回类型名）
        case TypeSpec::Kind::Pattern:     return t.pat_name;
        // 外置类型同样按名比较（与 Rust `TypeSpec::Ext` 的 name 同口径）
        case TypeSpec::Kind::Ext:         return t.ext_type ? t.ext_type->name : std::string();
    }
    return "?";
}

/* 同名字段是否**冲突**（类型 / 默认值 / 区间任一不同）。 */
static bool field_clash(const FieldSpec& a, const FieldSpec& b) {
    if (type_key(a.type) != type_key(b.type)) return true;
    if (a.mods.default_value != b.mods.default_value) return true;
    if (a.mods.min != b.mods.min) return true;
    if (a.mods.max != b.mods.max) return true;
    return false;
}

/* 应用一组契约（定义在 06_include_api.inc：那里才有 `apply_contract` 的实现）。
   单契约且无块级覆盖时逐字走原路径（错误文案 / 行为不变）。 */
static bool apply_contract_set(const ValuePtr& val,
                               const std::map<std::string, Contract>& contracts,
                               const ContractSet& set,
                               std::string* err);

// ===========================================================================
// Parser internals
// ===========================================================================
/* 值嵌套深度上限：与 C / Rust 侧同口径（Rust 为 MAX_VALUE_DEPTH=128）。
   `a{a{a{ … }}}` 会把递归下降一路压栈，栈溢出在 C++ 里同样接不住。 */
#define SML_MAX_VALUE_DEPTH 128

