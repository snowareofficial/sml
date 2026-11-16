/* 特性名表：与 Rust `FEATURES` 同序同名（跨端契约）。 */
static const char* const SML_FEATURE_NAMES[] = {
    "bareword-string", "include", "env", "contract", "fragment",
    "top-level-array", "namespace", "implicit-ns", "multi-include",
    "glob-include", "regex-include", "ext-rewrite", "when", "for",
    "typed-block", "case-insensitive", "block-name", "type.featag",
    "lex.ident", "type.weak",
};
static int feature_index(const std::string& name) {
    for (size_t k = 0; k < sizeof(SML_FEATURE_NAMES) / sizeof(SML_FEATURE_NAMES[0]); k++) {
        if (name == SML_FEATURE_NAMES[k]) return static_cast<int>(k);
    }
    return -1;
}

/* 版本字面量 → 主版本号（`docs/swsml/spec/version.md` §1）。
   1..4 = 本实现支持；-1 = 5.0 族（Dev，未启用）；0 = 无法识别。
   接受 `vN` / `N` / `N.0` / `N0`（连写）与代号 `camelia`（等价 50）。
   | 1..4 supported, -1 future (5.0 family), 0 unknown. */
static int version_lit_major(const std::string& raw) {
    std::string s = raw;
    if (s.size() >= 2 && s.front() == '"' && s.back() == '"') {
        s = s.substr(1, s.size() - 2);
    }
    if (s == "camelia") return -1;
    if (!s.empty() && (s[0] == 'v' || s[0] == 'V')) s.erase(0, 1);
    if (s.empty() || s[0] < '1' || s[0] > '9') return 0;
    const int major = s[0] - '0';
    const bool ok = (s.size() == 1)
                    || (s.size() == 3 && s[1] == '.' && s[2] == '0')
                    || (s.size() == 2 && s[1] == '0');
    if (!ok) return 0;
    return major <= 4 ? major : -1;
}

/* 预处理：扫行首的 `@feature enable|disable <名>[, <名>]`，算出初始特性集。
   必须在词法前调用（与 Rust `strip_features` 同定位）：顶层数组形态判定等
   都要看到 `@feature` 的效果。解析主循环仍会再处理（幂等）。
   简化：只看行首；字符串内部的 `@feature` 会被误判，属已知边界。 */
static unsigned scan_features(const std::string& text) {
    unsigned feats = SML_FEATURE_BASELINE;
    size_t pos = 0, n = text.size();
    while (pos < n) {
        size_t eol = text.find('\n', pos);
        size_t end = (eol == std::string::npos) ? n : eol;
        size_t i = pos;
        while (i < end && (text[i] == ' ' || text[i] == '\t')) i++;
        /* `@version <lit>`：版本决定**特性基集** —— v2 起裸词字符串必须加引号
           （与 Rust 的 `FeatureSet::for_edition` / JS 的 `scanVersionBase` 同构）。
           必须在词法前生效，否则第一行之后的裸词已经按 v1 语义取值了。
           非法 / 未启用的版本不在这里报错：留给解析层（同一处判据、同一条文案）。 */
        if (i + 8 <= end && text.compare(i, 8, "@version") == 0
            && (i + 8 == end || text[i + 8] == ' ' || text[i + 8] == '\t')) {
            size_t q = i + 8;
            while (q < end && (text[q] == ' ' || text[q] == '\t')) q++;
            std::string lit;
            while (q < end && text[q] != ' ' && text[q] != '\t') lit.push_back(text[q++]);
            const int major = version_lit_major(lit);
            if (major >= 2) feats &= ~1u;   /* 位 0 = bareword-string */
        }
        if (i + 8 <= end && text.compare(i, 8, "@feature") == 0
            && (i + 8 == end || text[i + 8] == ' ' || text[i + 8] == '\t')) {
            size_t q = i + 8;
            int on = -1;
            while (q < end && (text[q] == ' ' || text[q] == '\t')) q++;
            if (end - q >= 6 && text.compare(q, 6, "enable") == 0) { on = 1; q += 6; }
            else if (end - q >= 7 && text.compare(q, 7, "disable") == 0) { on = 0; q += 7; }
            if (on >= 0) {
                for (;;) {
                    std::string name;
                    while (q < end && (text[q] == ' ' || text[q] == '\t' || text[q] == ',')) q++;
                    if (q >= end) break;
                    while (q < end && text[q] != ' ' && text[q] != '\t' && text[q] != ',') {
                        name.push_back(text[q++]);
                    }
                    if (name.empty()) break;
                    int bit = feature_index(name);
                    if (bit < 0) break;   /* 未知名字留给解析层报 E-FEATURE-002 */
                    if (on) feats |= (1u << bit);
                    else    feats &= ~(1u << bit);
                }
            }
        }
        if (eol == std::string::npos) break;
        pos = eol + 1;
    }
    return feats;
}

struct PState {
    std::vector<Token> toks;
    size_t i = 0;
    std::map<std::string,ValuePtr> fragments;
    std::map<std::string,Contract> contracts;
    /* `@type name: X { … }` 声明的自定义类型（名 → 模式体）：跨块可见，
       同名后声明覆盖（与 Rust `types.insert` 同）。 */
    std::map<std::string, ValuePtr> types;
    std::string* err = nullptr;
    /* 原先这里还有 include_dir / include_stack 两个字段：include 是**边解析边插 token**
       处理的（W18 前的写法），所以状态得挂在解析器上。现在展开挪到解析前的
       `expand_includes` 里，链栈是那次递归的局部变量，解析器不必再知道 include 的存在。 */
    int depth = 0;              /* 当前块/数组嵌套深度（栈溢出防护） */
    unsigned features = SML_FEATURE_BASELINE;   /* `@feature enable/disable` 在版本基线上增减 */
    /* 一旦置位就**放弃解析**：各层循环立即 break，递归随之退栈。
       为什么需要它：光把 depth 复位并不够 —— 复位不会让栈帧退回去，外层循环接着又
       从 0 往下钻，于是「128 层一轮」地反复压栈，最终照样打穿（实测）。
       中止与 err 是否存在无关（err 允许为 NULL）。 */
    bool aborted = false;
    /* `@when` 的挂起条件：0 = 无，1 = 保留下一项，2 = 丢弃下一项。
       只作用于紧邻的下一个字段/块，消费后立即清 0。
       Pending `@when`: 0 = none, 1 = keep next item, 2 = drop next item. */
    int pending_when = 0;
    /* 诊断（warning）出口：只有调用方传了才收集（默认 nullptr = 零开销）。
       Diagnostics sink: filled only when the caller passes one. */
    std::vector<Diag>* diags = nullptr;
    /* 弱绑定：宿主注入的内置契约表 + 旁路表出口（都不进主树）。
       `@weakbind` 只作用于紧邻的下一个块，故挂起项带所在层（depth）。 */
    const BuiltinTable* builtins = nullptr;
    std::vector<WeakBind>* weak_binds = nullptr;
    std::string pending_weak;
    int pending_weak_line = 0;
    int pending_weak_depth = -1;
    /* 文档级 `@weakdoc` profile 名集合（判重 E-PARSE-030）。 */
    std::vector<std::string> weak_docs;
    /* 外置扩展表（nullptr = 零扩展）。 */
    const Ext* ext = nullptr;
    /* 当前块的路径栈：调用方在进 parse_block 前把键名放进 enter_key。
       Path stack: callers set `enter_key` right before entering a block. */
    std::string enter_key;
    std::vector<std::string> path;
};

// forward decls
static void set_field_local(const ValuePtr& node, const std::string& k, const ValuePtr& v);

static ValuePtr parse_value(PState& st);
static ValuePtr parse_value_inner(PState& st);
static ValuePtr parse_block_nested(PState& st);   // 定义在后（带深度守卫）
