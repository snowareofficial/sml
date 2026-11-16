/* =====================================================================
** 标识符白名单（门控 `lex.ident`）—— 与 Rust `sml-lex/src/identifiers.rs` 同判据
**
** 白名单：`A-Z` `a-z` `0-9` `_` `-` + 汉字 U+4E00–U+9FFF + 假名 U+3040–U+30FF
**        + 谚文音节 U+AC00–U+D7AF。
** 六类拒绝共用 `E-LEX-007`、靠 detail 区分：路径保留字符 / 空白与控制 /
** 不可见（零宽格式 Cf 与组合标记 Mn·Mc）/ 同形字母 / 全角兼容区 / 其余。
** 位置是**字符序号**（1 起算），不是字节偏移 —— 与错误文案一致。
** ===================================================================== */

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

static const char *const IDENT_SCRIPT_NAME[] = { "拉丁字母", "汉字/假名", "谚文" };

static int ident_in_ranges(unsigned int u, const unsigned int (*r)[2], size_t n) {
    size_t i;
    for (i = 0; i < n; i++)
        if (u >= r[i][0] && u <= r[i][1]) return 1;
    return 0;
}

/* UTF-8 解码一个码点：返回码点，*len 为字节数。
   非法字节按 U+FFFD、1 字节处理（它不在白名单内 ⇒ 会被拒）。 */
static unsigned int utf8_next(const unsigned char *s, size_t n, size_t *len) {
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

/* 标识符用的是什么文字系统：0 = 拉丁 / 1 = 汉字假名 / 2 = 谚文；-1 = 无。
   汉字与假名算**同一族**（日文里汉字与假名本来就混写），与 Rust `Script::Cjk` 同口径。 */
static int ident_script(const char *s) {
    const unsigned char *p = (const unsigned char *)s;
    size_t n = strlen(s), i = 0;
    while (i < n) {
        size_t l;
        unsigned int u = utf8_next(p + i, n - i, &l);
        if ((u >= 'A' && u <= 'Z') || (u >= 'a' && u <= 'z')) return 0;
        if ((u >= 0x4E00 && u <= 0x9FFF) || (u >= 0x3040 && u <= 0x30FF)) return 1;
        if (u >= 0xAC00 && u <= 0xD7AF) return 2;
        i += l;
    }
    return -1;
}

/* 校验一个标识符；不合法时写错误（含码）并返回 -1。 */
static int check_ident(parser *ps, const char *s) {
    const unsigned char *p = (const unsigned char *)s;
    size_t n, i = 0;
    int at = 0;
    char hex[16], chb[2];
    if (!s || !*s) {
        set_err(ps->lx->errbuf, ps->lx->errsz, SML_E_LEX_007 " 标识符为空");
        return -1;
    }
    n = strlen(s);
    while (i < n) {
        size_t l;
        unsigned int u = utf8_next(p + i, n - i, &l);
        at++;
        snprintf(hex, sizeof hex, "U+%04X", u);
        chb[0] = (u < 0x80) ? (char)u : '?';
        chb[1] = '\0';
        if ((u >= 'A' && u <= 'Z') || (u >= 'a' && u <= 'z') ||
            (u >= '0' && u <= '9') || u == '_' || u == '-') { i += l; continue; }
        if (u >= 0x4E00 && u <= 0x9FFF) { i += l; continue; }   /* 汉字 */
        if (u >= 0x3040 && u <= 0x30FF) { i += l; continue; }   /* 假名 */
        if (u >= 0xAC00 && u <= 0xD7AF) { i += l; continue; }   /* 谚文 */
        if (u == '.' || u == '[' || u == ']') {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_LEX_007 " 第 %d 个字符 `%s`（%s）是路径语法保留字符，"
                    "键名里不能出现 `.` `[` `]`", at, chb, hex);
            return -1;
        }
        if (u <= 0x20 || u == 0x7F || u == 0xA0 || u == 0x1680 || u == 0x205F ||
            (u >= 0x2000 && u <= 0x200A) || u == 0x2028 || u == 0x2029) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_LEX_007 " 第 %d 个字符是空白或控制字符（%s）", at, hex);
            return -1;
        }
        /* 组合标记 Mn/Mc（须排在同形字母块之前：「组合用西里尔字母」也在此区） */
        if (ident_in_ranges(u, IDENT_COMBINING, sizeof IDENT_COMBINING / sizeof IDENT_COMBINING[0])) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_LEX_007 " 第 %d 个字符是组合标记（Mn/Mc，非预组合 ⇒ 非 NFC）（%s）"
                    "——不可见，且能伪造看起来一样的名字", at, hex);
            return -1;
        }
        if (ident_in_ranges(u, IDENT_CF, sizeof IDENT_CF / sizeof IDENT_CF[0])) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_LEX_007 " 第 %d 个字符是零宽/格式字符（Cf）（%s）"
                    "——不可见，且能伪造看起来一样的名字", at, hex);
            return -1;
        }
        if (ident_in_ranges(u, IDENT_HOMOGLYPH, sizeof IDENT_HOMOGLYPH / sizeof IDENT_HOMOGLYPH[0])) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_LEX_007 " 第 %d 个字符 `%s`（%s）是与拉丁字母同形的西里尔 / 希腊 / "
                    "亚美尼亚字母；声明处只允许一种字形，请改用 ASCII", at, chb, hex);
            return -1;
        }
        if (ident_in_ranges(u, IDENT_WIDE, sizeof IDENT_WIDE / sizeof IDENT_WIDE[0])) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_LEX_007 " 第 %d 个字符 `%s`（%s）是全角 / 兼容区字符："
                    "与半角混用会「看起来一样、比较不等」", at, chb, hex);
            return -1;
        }
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_LEX_007 " 第 %d 个字符 `%s`（%s）不在白名单内"
                "（只允许字母数字、`_` `-`、汉字 / 假名 / 谚文）", at, chb, hex);
        return -1;
    }
    return 0;
}

/* 「每契约单一语种」：记录 / 比对文字系统；混用即报错。
   `seen` 存已见的第一种文字系统，`*first` 指向它的标识符（token 活到解析结束）。 */
static int ident_check_script(parser *ps, int *seen, const char **first,
                              const char *id) {
    int sc = ident_script(id);
    if (sc < 0) return 0;                 /* 纯数字 / `_` / `-` 不参与 */
    if (*seen < 0) { *seen = sc; *first = id; return 0; }
    if (*seen != sc) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_LEX_007 " `%s`（%s）与 `%s`（%s）混用了文字系统："
                "同一契约内的标识符只能有一种语种",
                id, IDENT_SCRIPT_NAME[sc], *first, IDENT_SCRIPT_NAME[*seen]);
        return -1;
    }
    return 0;
}

/* 标量识别 (裸词) */
static sml_value *coerce_word(const char *w, parser *ps) {
    /* 防御：`w` 为 NULL 只可能来自「游标已到 EOF 却又取 v」的调用点（那本身是 bug），
       但崩在库里的代价远大于返回 null —— 调用方会看到 failed / 空值，而不是段错误。 */
    if (!w) return sml_new_null();
    /* featag（B2）：`!` 只在块头的 featag 区里是记号；出现在**值位置**就是歧义
       （`a: !b` —— 字符串还是 featag？）。歧义一律报错，**开了 `bareword-string` 也不豁免**：
       靠特性绕过歧义等于把「猜」藏进配置里。要写字面量请加引号（`a: "!b"`）。
       门控 `type.featag`：未开启时 `!b` 仍是普通裸词，v1–v4 既有文档一字不变。 */
    if (w && (ps->features & (1u << 17)) && w[0] == '!') {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_027
                " 值位置不能写以 `!` 开头的裸词；若本意是字符串，请用引号包起来：`%s`", w);
        ps->failed = 1;
        return NULL;
    }
    if (strcmp(w, "true") == 0) return sml_new_bool(1);
    if (strcmp(w, "false") == 0) return sml_new_bool(0);
    if (strcmp(w, "null") == 0) return sml_new_null();
    /* $env.VAR（需 env 特性）。门控必须与引号串内插那条路径同判据：
       改前只拦 `"..."` 里的 `$env.`，裸词形态照旧读宿主环境变量 ⇒
       `@feature disable env` 在沙箱里形同虚设（与 Rust 的 coerce_word 不一致）。 */
    if (strncmp(w, "$env.", 5) == 0) {
        if (!(ps->features & (1u << 2))) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_FEATURE_002 " 裸词 `%s` 无法解析：未启用特性 `env`", w);
            ps->failed = 1;
            return NULL;
        }
        const char *ev = getenv(w + 5);
        return sml_new_str(ev ? ev : "");
    }
    /* 片段引用 &name */
    if (w[0] == '&') {
        struct frag *f;
        for (f = ps->frags; f; f = f->next) {
            if (strcmp(f->name, w + 1) == 0) {
                /* 返回深拷贝 */
                return sml_clone(f->val);
            }
        }
        /* 未定义的片段引用 ⇒ E-INCLUDE-006（W16，与 Rust/JS 同码）。
           改前这里 `return sml_new_str(w)`：拼错的片段名**静默退化**成普通字符串
           `"&nosuchfrag"`，下游取值取不到还查不出原因 —— 用户已裁决这类必须报错
           （"这种不应该出现，堪比 void"）。
           返回 NULL 是安全的：sml_obj_set 见到 NULL 会跳过该键，且 ps->failed
           会让各解析循环立刻收手（与 v2 裸词报错那条路径同一处理方式）。 */
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_INCLUDE_006 " 未定义的片段引用 `%s`", w);
        ps->failed = 1;
        return NULL;
    }
    /* 数字 */
    char *end = NULL;
    errno = 0;   /* ERANGE 必须「先清零再检查」，否则会读到之前调用留下的旧值 */
    long long iv = strtoll(w, &end, 10);
    if (end && *end == '\0' && end != w) {
        /* B10（对齐 Rust sml-lex 的 coerce_word）：**纯整数形态但超出 i64** 时，
           保留为字符串 —— 不能返回被夹住的 Int。Rust 在 `w.parse::<i64>()` 失败且
           looks_int 时 `return Ok(Value::Str(w.to_string()))`（round-trip 安全、零精度
           损失），且该 return **早于** bareword-string 检查，故 v2/v3 下同样不报
           E-FEATURE-005 —— 这里保持同一分支顺序。
           修之前 `99999999999999999999` 会变成 Int 9223372036854775807（静默错值，
           且能通过 `int` 契约校验）。 */
        if (errno == ERANGE) return sml_new_str(w);
        /* 前导零保留为字符串（与 Rust 同判据）：`007` / `0755` 转 Int 会把权限位、
           编号静默改成 7 / 755。`0x20` / `0b101` 本来就不进这一支（strtoll 停在 x/b）。 */
        {
            const char *dg = (w[0] == '+' || w[0] == '-') ? w + 1 : w;
            if (dg[0] == '0' && dg[1] != '\0' && strncmp(dg, "0x", 2) != 0
                && strncmp(dg, "0b", 2) != 0) {
                return sml_new_str(w);
            }
        }
        return sml_new_int(iv);
    }
    /* 浮点 (含 .5 / 1e3 等 strtod 可解析) */
    if (strchr(w, '.') || strchr(w, 'e') || strchr(w, 'E')) {
        char *fend = NULL;
        double fv = strtod(w, &fend);
        if (fend && *fend == '\0' && fend != w) return sml_new_float(fv);
    }
    /* V2/V3 严格模式：自由字符串必须加引号 */
    if (ps->version >= 2) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_FEATURE_005 " v2/v3 字符串必须加引号，裸词 `%s` 应写作 \"%s\"", w, w);
        ps->failed = 1;
        return NULL;
    }
    /* `bareword-string` 门控：关闭后纯字符串裸词必须加引号（与 Rust 同码 005）。 */
    if (!(ps->features & (1u << 0))) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_FEATURE_005 " 裸词 `%s` 应写作 `\"%s\"`（bareword-string 已禁用）", w, w);
        ps->failed = 1;
        return NULL;
    }
    return sml_new_str(w);
}

/* T9：判断 token 是否为「值型标量」（数字 / 引号串 / bool / null）。
 * 裸词标识符（如 `server`、`a`，作块类型名 / 块名）**不算**值型 —— 这是区分
 * T9 误报（`42 a { }`）与合法带类型块（`server web { }`）的关键。与 Rust/C++ 同判据。 */
static int tok_is_scalar_value(const token *t) {
    if (!t) return 0;
    if (t->t == T_STR) return 1;                       /* 引号串 */
    if (t->t != T_WORD) return 0;                     /* @ 指令 / 结构记号 */
    const char *w = t->v;
    if (w[0] == '\0') return 0;
    if (strcmp(w, "true") == 0 || strcmp(w, "false") == 0
        || strcmp(w, "null") == 0)
        return 1;
    /* 数字：整串可解析为浮点（与 Rust 的 parse::<f64>() 同口径） */
    char *end = NULL;
    errno = 0;
    strtod(w, &end);
    return (end != w && *end == '\0');
}

static void frag_put(parser *ps, const char *name, sml_value *v) {
    struct frag *f = (struct frag *)calloc(1, sizeof(struct frag));
    if (!f) return;
    f->name = strdup(name);
    f->val = v;
    f->next = ps->frags;
    ps->frags = f;
}

/* 诊断（warning）收集与释放。Diagnostics: append / free all. */
static void diag_push_at(parser *ps, const char *code, const char *msg, int line) {
    sml_diag *nd;
    size_t nc;
    if (!ps->want_diags) return;
    if (ps->n_diags == ps->cap_diags) {
        nc = ps->cap_diags ? ps->cap_diags * 2 : 8;
        nd = (sml_diag *)realloc(ps->diags, nc * sizeof(sml_diag));
        if (!nd) return;
        ps->diags = nd;
        ps->cap_diags = nc;
    }
    ps->diags[ps->n_diags].code = strdup(code);
    ps->diags[ps->n_diags].message = strdup(msg);
    ps->diags[ps->n_diags].line = line;
    ps->n_diags++;
}

/* 行号取「下一个未消费 token」的行 —— 对**消费完才发现**的诊断（如重复键）
   这是下一行的行号，故那类调用点必须走 `diag_push_at` 显式给行。 */
static void diag_push(parser *ps, const char *code, const char *msg) {
    int line = (ps->lx && ps->lx->pos < ps->lx->n)
        ? (int)ps->lx->toks[ps->lx->pos].line : 0;
    diag_push_at(ps, code, msg, line);
}

/* 释放 `sml_parse_ex` 交出的诊断数组（含每条的 code/message）。 */
void sml_diag_free(sml_diag *diags, size_t n) {
    size_t i;
    if (!diags) return;
    for (i = 0; i < n; i++) { free(diags[i].code); free(diags[i].message); }
    free(diags);
}

static void diag_free_all(parser *ps) {
    sml_diag_free(ps->diags, ps->n_diags);
    ps->diags = NULL;
    ps->n_diags = 0;
    ps->cap_diags = 0;
}

/* ---- 弱绑定旁路表（`docs/swsml/spec/weakbind.md` §5）---- */

/* 释放旁路表（含每条的 path/name）。Free a weak-binding side table. */
void sml_weakbind_free(sml_weakbind *binds, size_t n) {
    size_t i;
    if (!binds) return;
    for (i = 0; i < n; i++) {
        free(binds[i].path);
        free(binds[i].name);
    }
    free(binds);
}

static void path_push(parser *ps, const char *seg) {
    if (!seg || !*seg) return;
    if (ps->path_n == ps->path_cap) {
        size_t nc = ps->path_cap ? ps->path_cap * 2 : 8;
        char **np = (char **)realloc(ps->path, nc * sizeof(char *));
        if (!np) return;
        ps->path = np;
        ps->path_cap = nc;
    }
    ps->path[ps->path_n++] = strdup(seg);
}

static void path_pop(parser *ps) {
    if (ps->path_n == 0) return;
    free(ps->path[--ps->path_n]);
}

static void path_free_all(parser *ps) {
    while (ps->path_n) path_pop(ps);
    free(ps->path);
    ps->path = NULL;
    ps->path_cap = 0;
}

/* 拼出「祖先路径 + 本级键」：键名用 `.` 连接，`[n]` 段不加点。
   Build the dotted path of the new leaf; `[n]` segments take no dot. */
static char *path_join(parser *ps, const char *leaf) {
    size_t i, len = 1;
    char *out;
    for (i = 0; i < ps->path_n; i++) len += strlen(ps->path[i]) + 2;
    if (leaf) len += strlen(leaf) + 2;
    out = (char *)calloc(1, len);
    if (!out) return NULL;
    for (i = 0; i < ps->path_n; i++) {
        if (i && ps->path[i][0] != '[') strcat(out, ".");
        strcat(out, ps->path[i]);
    }
    if (leaf && *leaf) {
        if (out[0] && leaf[0] != '[') strcat(out, ".");
        strcat(out, leaf);
    }
    return out;
}

/* 记一条弱绑定：主树不动，只进旁路表。Record one `@weakbind` (side table only). */
static void weak_bind_push(parser *ps, const char *name, int line, const char *leaf) {
    if (ps->n_binds == ps->cap_binds) {
        size_t nc = ps->cap_binds ? ps->cap_binds * 2 : 4;
        sml_weakbind *nb = (sml_weakbind *)realloc(ps->binds, nc * sizeof(sml_weakbind));
        if (!nb) return;
        ps->binds = nb;
        ps->cap_binds = nc;
    }
    ps->binds[ps->n_binds].path = path_join(ps, leaf);
    ps->binds[ps->n_binds].name = strdup(name ? name : "");
    ps->binds[ps->n_binds].line = line;
    ps->n_binds++;
}

static void weak_check_block(parser *ps, sml_value *node, const char *cname);

/* 对象内同名键冲突 -> 提升为数组 */
static void obj_set_dup(parser *ps, sml_value *obj, const char *key, sml_value *val) {
    sml_value *ex = sml_obj_get(obj, key);
    if (!ex) {
        sml_obj_set(obj, key, val);
        return;
    }
    /* 重复键：值树照旧提升为数组，同时记一条 `W-PARSE-001`（不改变值）。 */
    if (ps) diag_push_at(ps, "W-PARSE-001", "键重复出现，已提升为数组", ps->key_line);
    if (ex->type == SML_ARRAY) {
        sml_arr_push(ex, val);
    } else {
        sml_value *arr = sml_new_array();
        sml_arr_push(arr, ex);
        sml_arr_push(arr, val);
        /* 替换 (需要先删再设) */
        sml_field *f;
        for (f = obj->u.obj.head; f; f = f->next) {
            if (strcmp(f->key, key) == 0) {
                f->value = arr;
                return;
            }
        }
    }
}

/* `@when` 只作用于紧邻的下一个字段/块：条件为假就丢弃并释放该项。
   `@when` applies to the immediately following field/block only; a false
   condition drops the item and frees its value. */
static void set_field_maybe_skip(parser *ps, sml_value *obj, const char *key,
                                 sml_value *val) {
    if (ps->pending_when == 2) {
        ps->pending_when = 0;
        sml_free(val);
        return;
    }
    ps->pending_when = 0;
    /* 弱绑定：`@weakbind X` 之后的那个块，用内置表做**只读**软校验（不填默认值、
       不删改键），并把「哪个块绑了哪个契约」记进旁路表。
       Weak-binding soft check (read-only) + side-table record for the next block. */
    /* 只在挂起声明的**同一层**消费：子块内部的字段（depth 更深）不属于
       `@weakbind` 的目标，不能在这里误判成「目标不是块」。 */
    if (ps->pending_weak && ps->depth == ps->pending_weak_depth) {
        if (val && val->type == SML_OBJECT) {
            const char *wname = ps->pending_weak;
            weak_check_block(ps, val, wname);
            weak_bind_push(ps, wname, ps->pending_weak_line, key);
            ps->pending_weak = NULL;
        } else {
            /* `@weakbind` 的目标不是块（标量 / 数组 / @for）⇒ E-PARSE-029。
               Rust value_step 同判据（parser/mod.rs:1698）。 */
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_029 " `@weakbind` 后面必须紧跟一个块（标量 / 数组 / `@for` 都不算）");
            ps->failed = 1;
            ps->pending_weak = NULL;
        }
    }
    obj_set_dup(ps, obj, key, val);
}

/* `@when` 条件求值（闭集：`$env.NAME` [+ `==`/`!=`/`=` + 值]）。
   与 Rust `cond.rs::eval_when_cond` 同语义：无运算符时做真值测试
   （非空且非 "0" / "false"）。
   Closed set, no expression evaluator — mirrors Rust `cond.rs::eval_when_cond`. */
static int eval_when(parser *ps) {
    token *lhs = next(ps);
    const char *var;
    token *op;
    if (!lhs || (lhs->t != T_WORD && lhs->t != T_STR)) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_014 " `@when` 后须条件（如 `$env.ENV == \"prod\"`）");
        ps->failed = 1;
        return 0;
    }
    if (strncmp(lhs->v, "$env.", 5) != 0 || lhs->v[5] == '\0') {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_014 " `@when` 的条件左侧只支持 `$env.NAME`，得 `%s`", lhs->v);
        ps->failed = 1;
        return 0;
    }
    var = lhs->v + 5;
    if (!(ps->features & (1u << 2))) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_FEATURE_002 " `@when $env.%s` 需要特性 `env`", var);
        ps->failed = 1;
        return 0;
    }
    {
        const char *act = getenv(var);
        if (!act) act = "";
        op = peek(ps);
        {
            int is_eq = 0, is_ne = 0;
            if (op && op->t == T_EQ) {
                /* `=` 是独立 token；`==` 被词法切成两个 T_EQ
                   （Rust 侧是一个 Word("==")）。 */
                is_eq = 1;
                next(ps);
                if (peek(ps)->t == T_EQ) next(ps);
            } else if (op && op->t == T_WORD && strcmp(op->v, "==") == 0) {
                is_eq = 1;
                next(ps);
            } else if (op && op->t == T_WORD && strcmp(op->v, "!=") == 0) {
                is_ne = 1;
                next(ps);
            }
            if (!is_eq && !is_ne) {
                /* 真值测试：空串 / "0" / "false" 视为假 ｜ truthiness test */
                return (act[0] != '\0' && strcmp(act, "0") != 0
                        && strcmp(act, "false") != 0) ? 1 : 0;
            }
            {
                token *rhs = peek(ps);
                const char *b;
                char buf[512];
                int eq;
                const char *opname = is_ne ? "!=" : "==";
                if (!rhs || (rhs->t != T_WORD && rhs->t != T_STR)) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_015 " `@when` 的 `%s` 后须比较值", opname);
                    ps->failed = 1;
                    return 0;
                }
                /* 漏写比较值时，下一个字段名会被当比较值吃掉 —— 看 rhs 之后是不是 `:` */
                if (peek_at(ps, 1)->t == T_COLON) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_015 " `@when` 的 `%s` 后缺少比较值"
                            "（写成了运算符后直接换行）", opname);
                    ps->failed = 1;
                    return 0;
                }
                b = rhs->v;
                next(ps);
                if (b[0] == '"') {
                    size_t len = strlen(b);
                    size_t n = (len >= 2 && b[len - 1] == '"') ? len - 2 : len - 1;
                    if (n >= sizeof(buf)) n = sizeof(buf) - 1;
                    memcpy(buf, b + 1, n);
                    buf[n] = '\0';
                    b = buf;
                }
                eq = (strcmp(act, b) == 0);
                if (is_ne) return eq ? 0 : 1;
                return eq;
            }
        }
    }
}

/* `${var}` 只读插值：未绑定则原样保留。返回 malloc 的串（调用方 free）。
   Read-only loop-variable interpolation; unknown names stay verbatim.
   用固定缓冲 + 截断，避免 realloc 路径带来的泄漏面（SML 值本身也有长度上限）。 */
static char *interp_loop(parser *ps, const char *s) {
    static char buf[8192];
    const char *p = s;
    size_t len = 0;
    if (!ps->loop_var || !strstr(s, "${")) return strdup(s);
    while (*p && len + 1 < sizeof(buf)) {
        const char *at = strstr(p, "${");
        const char *close;
        size_t seg, nlen;
        if (!at) break;
        close = strchr(at + 2, '}');
        if (!close) break;
        seg = (size_t)(at - p);
        if (len + seg + 1 >= sizeof(buf)) seg = sizeof(buf) - len - 1;
        memcpy(buf + len, p, seg);
        len += seg;
        nlen = (size_t)(close - at - 2);
        if (nlen == strlen(ps->loop_var) && strncmp(at + 2, ps->loop_var, nlen) == 0) {
            size_t vlen = ps->loop_val ? strlen(ps->loop_val) : 0;
            if (len + vlen + 1 >= sizeof(buf)) vlen = sizeof(buf) - len - 1;
            if (vlen) { memcpy(buf + len, ps->loop_val, vlen); len += vlen; }
        } else {
            size_t keep = (size_t)(close - at) + 1;
            if (len + keep + 1 >= sizeof(buf)) keep = sizeof(buf) - len - 1;
            memcpy(buf + len, at, keep);
            len += keep;
        }
        p = close + 1;
    }
    {
        size_t rest = strlen(p);
        if (len + rest + 1 > sizeof(buf)) rest = sizeof(buf) - len - 1;
        memcpy(buf + len, p, rest);
        len += rest;
    }
    buf[len] = '\0';
    return strdup(buf);
}

/* 声明侧软校验：`@weakbind X` 的可选体内每个字段，与内置表比对（只读）。
   Declaration-side soft check for an optional `{ field: type }` body. */
static void weak_check_decl(parser *ps, const char *cname, const char *fname,
                            const char *tyname) {
    ccontract *c;
    cfield *f;
    char msg[256];
    if (!ps->builtin) return;
    for (c = ps->builtin->contracts; c; c = c->next) {
        if (strcmp(c->name, cname) == 0) break;
    }
    if (!c) return;   /* 契约名不在表里：已在声明处报过「未知契约」 */
    for (f = c->fields; f; f = f->next) {
        if (strcmp(f->name, fname) == 0) break;
    }
    if (!f) {
        snprintf(msg, sizeof(msg), "契约 `%s` 没有字段 `%s`", cname, fname);
        diag_push(ps, "W-CONTRACT-001", msg);
        return;
    }
    if (tyname && strcmp(type_name(f->ty), tyname) != 0) {
        snprintf(msg, sizeof(msg), "字段 `%s.%s` 的类型与内置契约不同", cname, fname);
        diag_push(ps, "W-CONTRACT-001", msg);
    }
}

/* 弱绑定软校验：只用内置表做**只读**检查（不填默认值、不删改键）。
   与 `@is` 的 apply_contract 是两条路径：那条会改树，这条只出 warning。
   Weak-binding soft check: read-only, never mutates the value. */
static void weak_check_block(parser *ps, sml_value *node, const char *cname) {
    ccontract *c;
    cfield *f;
    if (!ps->builtin) return;
    for (c = ps->builtin->contracts; c; c = c->next) {
        if (strcmp(c->name, cname) == 0) break;
    }
    if (!c) return;   /* 内置表里没有 ⇒ 声明侧已处理，块侧不重复报 */
    for (f = c->fields; f; f = f->next) {
        sml_value *v = sml_obj_get(node, f->name);
        char msg[256];
        if (!v) {
            if (f->required) {
                snprintf(msg, sizeof(msg), "块里缺必填字段 `%s.%s`", cname, f->name);
                diag_push(ps, "W-CONTRACT-001", msg);
            }
            continue;
        }
        /* 类型 / 枚举：只读比对，**不调用 check_type**（后者会 set_err、可能中止解析）。 */
        {
            int ok = 1;
            if (f->ty == CT_STR)  ok = (v->type == SML_STR);
            else if (f->ty == CT_INT) ok = (v->type == SML_INT);
            else if (f->ty == CT_NUM) ok = (v->type == SML_INT || v->type == SML_FLOAT);
            else if (f->ty == CT_BOOL) ok = (v->type == SML_BOOL);
            else if (f->ty == CT_ENUM) {
                if (v->type != SML_STR) ok = 0;
                else {
                    size_t k;
                    int found = 0;
                    for (k = 0; k < f->enum_n; k++) {
                        if (strcmp(f->enum_vals[k], v->u.s) == 0) { found = 1; break; }
                    }
                    ok = found;
                }
            }
            if (!ok) {
                snprintf(msg, sizeof(msg), "字段 `%s.%s` 与内置契约类型不符", cname, f->name);
                diag_push(ps, "W-CONTRACT-001", msg);
                continue;
            }
        }
        /* 数值区间：只对数值值做（字符串不在这里报）。 */
        if (f->min_set || f->max_set) {
            double n = 0;
            int isnum = 0;
            if (v->type == SML_INT) { n = (double)v->u.i; isnum = 1; }
            else if (v->type == SML_FLOAT) { n = v->u.f; isnum = 1; }
            if (isnum && ((f->min_set && n < f->min) || (f->max_set && n > f->max))) {
                snprintf(msg, sizeof(msg), "字段 `%s.%s` 超出内置契约的区间", cname, f->name);
                diag_push(ps, "W-CONTRACT-001", msg);
            }
        }
    }
}

/* ---- 契约: 查找/校验/应用 ---- */

static ccontract *contract_find(parser *ps, const char *name) {
    for (ccontract *c = ps->contracts; c; c = c->next)
        if (strcmp(c->name, name) == 0) return c;
    return NULL;
}

/* 前向声明 (check_type 中递归调用) */
static int apply_contract_rec(parser *ps, ccontract *c, sml_value *node,
                              char *err, size_t errsz);

/* 校验值是否符合字段规格 (含 ContractRef 递归)。出错写 err 返回 -1 */
/* `check_type` 的前向声明：下面的 `check_contract_only` 要用它做类型校验。 */
static int check_type(parser *ps, const char *cname, const cfield *spec,
                      const sml_value *v, char *err, size_t errsz);

/* 只校验、**不回填默认值**：数组元素位置上的组合契约用它
   （与 Rust 的 `check_contract_only`、JS / Lua 的既有行为同口径：只报错、不改树）。
   严格性（strict ⇒ 未声明字段 ⇒ E-CONTRACT-004）与 `apply_contract_rec` 的第 1 步
   **同源**：同一个 `allow_extra` 标志、同一份 `__type` / `__featags` 豁免名单。 */
static int check_contract_only(parser *ps, ccontract *c, sml_value *node,
                               char *err, size_t errsz) {
    cfield *cf;
    /* 1) 严格性: 契约未写 `loose` ⇒ 对象里有契约未声明的键 ⇒ E-CONTRACT-004。
       与 `apply_contract_rec` 第 1 步同一判据，差别只在**只报错、不改树**
       （元素不是字段，回填会改用户的数据）。
       ⚠️ 本端没有 path 概念 ⇒ 文案只带字段名（Rust 侧带 `c.peers[0].port` 这样的路径）：
          **码是稳定契约、文案不是** ⇒ 两端同码即算一致（差异已登记在
          `tests/conformance/known_divergences.md`）。 */
    if (!c->allow_extra && node && node->type == SML_OBJECT) {
        size_t nk = sml_obj_len(node), i;
        for (i = 0; i < nk; i++) {
            const char *k = sml_obj_key_at(node, i);
            int found = 0;
            if (!k) continue;
            /* `__type` / `__featags` 不参与契约校验（与 `apply_contract_rec` 同一名单：
               语法元数据，不是数据字段）。 */
            if (!strcmp(k, "__type") || !strcmp(k, "__featags")) continue;
            for (cf = c->fields; cf; cf = cf->next)
                if (strcmp(cf->name, k) == 0) { found = 1; break; }
            if (!found) {
                set_err(err, errsz,
                        SML_E_CONTRACT_004 " 字段 `%s` 未在契约 `%s` 中声明（严格模式；如需允许额外字段请在契约名后写 `loose`）",
                        k, c->name);
                return -1;
            }
        }
    }
    /* 2) 逐字段: 必填缺失 + 类型校验 */
    for (cf = c->fields; cf; cf = cf->next) {
        sml_value *v = sml_obj_get(node, cf->name);
        if (!v) {
            /* 缺字段: `default` **不**放松必填 —— 教科书（`ch05-contract`）写明 `default`
               「同时自动视为可选」, 故有默认值的字段缺了不算违约（字段位置由
               `apply_contract_rec` 回填默认值, 那一层照旧回填）。
               只校验路径**不回填**, 所以「有 default」不等于「已存在」, 但也不算缺失。 */
            if (cf->required && !cf->def) {
                set_err(err, errsz,
                        SML_E_CONTRACT_003 " 字段 `%s` 必填但缺失（契约 `%s`）",
                        cf->name, c->name);
                return -1;
            }
            continue;   /* 缺字段但非必填：不报错，也**不**回填默认值 */
        }
        if (cf->ty == CT_CONTRACTREF) {
            if (v->type != SML_OBJECT) {
                set_err(err, errsz,
                        SML_E_CONTRACT_008
                        " 字段 `%s` 应为块并按契约 `%s` 校验，实际为 %s（契约 `%s`）",
                        cf->name, cf->ref_name, kind_name(v), c->name);
                return -1;
            }
            ccontract *tgt = contract_find(ps, cf->ref_name);
            if (!tgt) {
                set_err(err, errsz,
                        SML_E_CONTRACT_001
                        " 字段 `%s` 引用了未定义的契约 `%s`（契约 `%s`）",
                        cf->name, cf->ref_name, c->name);
                return -1;
            }
            if (check_contract_only(ps, tgt, v, err, errsz) != 0) return -1;
        } else {
            if (check_type(ps, c->name, cf, v, err, errsz) != 0) return -1;
        }
    }
    return 0;
}

static int check_type(parser *ps, const char *cname, const cfield *spec,
                      const sml_value *v, char *err, size_t errsz) {
    int ok = 0;
    /* 自定义模式类型（`@type`）：值须为**字符串**并匹配模式。
       非字符串必须**显式报错并提示加引号** —— `证件号: 221099 1988 0987 1211`
       这种裸写会被词法切成数字，最终只剩末段、还不报错，是本项目明确要消灭的
       「静默数据损坏」。故这一支**提前返回**，不走下面的通用 002。 */
    if (spec->ty == CT_PATTERN) {
        const char *pn = spec->pat_name ? spec->pat_name : "?";
        char perr[256];
        int m;
        if (v->type != SML_STR) {
            set_err(err, errsz,
                    SML_E_CONTRACT_009 " 字段 `%s` 类型 `%s` 要求字符串"
                    "（号码 / 编号 / 身份证请用引号包裹），实际为 %s（契约 `%s`）",
                    spec->name, pn, kind_name(v), cname);
            return -1;
        }
        perr[0] = '\0';
        m = sp_match(spec->pat, v->u.s, v->slen, perr, sizeof perr);
        if (m < 0) {
            set_err(err, errsz, SML_E_CONTRACT_009 " 类型 `%s` 匹配时出错：%s", pn, perr);
            return -1;
        }
        if (!m) {
            set_err(err, errsz,
                    SML_E_CONTRACT_009
                    " 字段 `%s` 的值 `%.*s` 不符合类型 `%s` 的格式要求（契约 `%s`）",
                    spec->name, (int)v->slen, v->u.s, pn, cname);
            return -1;
        }
        return 0;
    }
    switch (spec->ty) {
        case CT_ANY:  ok = 1; break;
        case CT_STR:  ok = (v->type == SML_STR); break;
        case CT_INT:  ok = (v->type == SML_INT); break;
        case CT_NUM:  ok = (v->type == SML_INT || v->type == SML_FLOAT); break;
        case CT_BOOL: ok = (v->type == SML_BOOL); break;
        /* 上面已提前返回；这里只为让 switch 覆盖全部枚举值（`-Wswitch`） */
        case CT_PATTERN: ok = 1; break;
        case CT_ENUM:
            ok = 0;
            for (size_t i = 0; i < spec->enum_n; i++)
                if (value_eq_str(v, spec->enum_vals[i])) { ok = 1; break; }
            break;
        case CT_ARRAY: {
            if (v->type != SML_ARRAY) { ok = 0; break; }
            ok = 1;
            for (size_t i = 0; i < v->u.arr.len; i++) {
                sml_value *el = v->u.arr.items[i];
                int eok = 0;
                switch (spec->arr_inner) {
                    case CT_STR:  eok = (el->type == SML_STR); break;
                    case CT_INT:  eok = (el->type == SML_INT); break;
                    case CT_NUM:  eok = (el->type == SML_INT || el->type == SML_FLOAT); break;
                    case CT_BOOL: eok = (el->type == SML_BOOL); break;
                    case CT_ANY:  eok = 1; break;
                    /* `[时刻]`：元素逐个匹配模式，错误**直接返回**（不等通用 002）——
                       与 Rust 同码 009，且带下标定位（`v[1]`）。 */
                    case CT_PATTERN: {
                        const char *pn = spec->pat_name ? spec->pat_name : "?";
                        char perr[256];
                        int m;
                        if (el->type != SML_STR) {
                            set_err(err, errsz, SML_E_CONTRACT_009
                                    " 字段 `%s[%zu]` 类型 `%s` 要求字符串"
                                    "（号码 / 编号 / 身份证请用引号包裹），实际为 %s（契约 `%s`）",
                                    spec->name, i, pn, kind_name(el), cname);
                            return -1;
                        }
                        perr[0] = '\0';
                        m = sp_match(spec->arr_pat, el->u.s, el->slen, perr, sizeof perr);
                        if (m < 0) {
                            set_err(err, errsz, SML_E_CONTRACT_009
                                    " 类型 `%s` 匹配时出错：%s", pn, perr);
                            return -1;
                        }
                        if (!m) {
                            set_err(err, errsz, SML_E_CONTRACT_009
                                    " 字段 `%s[%zu]` 的值 `%.*s` 不符合类型 `%s` 的格式要求（契约 `%s`）",
                                    spec->name, i, (int)el->slen, el->u.s, pn, cname);
                            return -1;
                        }
                        eok = 1;
                        break;
                    }
                    /* `[Endpoint]`：元素按**被引用契约**逐个校验（只校验、不回填）——
                       契约的**应用**只发生在字段位置，数组元素没人校验 ⇒ 元素缺必填字段
                       会被**静默放过**（与 Rust / JS / Lua 的口径对齐）。 */
                    case CT_CONTRACTREF: {
                        ccontract *tgt = contract_find(ps, spec->arr_ref_name);
                        if (!tgt) {
                            set_err(err, errsz,
                                    SML_E_CONTRACT_001
                                    " 字段 `%s[%zu]` 引用了未定义的契约 `%s`（契约 `%s`）",
                                    spec->name, i, spec->arr_ref_name, cname);
                            return -1;
                        }
                        if (el->type != SML_OBJECT) {
                            set_err(err, errsz,
                                    SML_E_CONTRACT_008
                                    " 字段 `%s[%zu]` 应为块并按契约 `%s` 校验，实际为 %s（契约 `%s`）",
                                    spec->name, i, spec->arr_ref_name, kind_name(el), cname);
                            return -1;
                        }
                        if (check_contract_only(ps, tgt, el, err, errsz) != 0) return -1;
                        eok = 1;
                        break;
                    }
                    default:      eok = 1; break;
                }
                if (!eok) { ok = 0; break; }
            }
            break;
        }
        case CT_EXT: {
            /* 外置类型：校验器由注册方提供；失败报 E-CONTRACT-007（与 Rust 同码）。 */
            char xerr[256];
            xerr[0] = '\0';
            if (!spec->ext_type || !spec->ext_type->check) { ok = 1; break; }
            if (spec->ext_type->check(spec->ext_type->ud, v, xerr, sizeof xerr)) {
                ok = 1;
            } else {
                set_err(err, errsz,
                         SML_E_CONTRACT_007 " 字段 `%s` 不符合外置类型 `%s` 的要求：%s（契约 `%s`）",
                         spec->name,
                         spec->ext_type->name ? spec->ext_type->name : "?",
                         xerr[0] ? xerr : "校验未通过", cname);
                return -1;
            }
            break;
        }
        case CT_CONTRACTREF: {
            if (v->type != SML_OBJECT) {
                /* 「组合字段应为块」是**独立条件**（E-CONTRACT-008），不能与「类型不符」
                   （E-CONTRACT-002）共用一个码 —— Rust / JS 侧同样区分，否则同一份文档
                   在 C 与它们之间会拿到不同的码，「同因同码」当场破功。 */
                set_err(err, errsz,
                         SML_E_CONTRACT_008 " 字段 `%s` 应为块并按契约 `%s` 校验，实际为 %s（契约 `%s`）",
                         spec->name, spec->ref_name, kind_name(v), cname);
                return -1;
            }
            ccontract *tgt = contract_find(ps, spec->ref_name);
            if (!tgt) {
                set_err(err, errsz,
                         SML_E_CONTRACT_001 " 字段 `%s` 引用了未定义的契约 `%s`（契约 `%s`）",
                         spec->name, spec->ref_name, cname);
                return -1;
            }
            if (apply_contract_rec(ps, tgt, (sml_value *)v, err, errsz) != 0) return -1;
            ok = 1;
            break;
        }
    }
    if (!ok) {
        /* 「取值不在枚举列表内」是**独立条件**（E-CONTRACT-006），不能与「类型不符」
           （E-CONTRACT-002）共用一个码 —— Rust / JS 侧同样区分（见 rust/sml-contract
           的 check_type：`if let TypeSpec::Enum(..)` 单独返回 E_CONTRACT_006）。 */
        if (spec->ty == CT_ENUM) {
            set_err(err, errsz,
                     SML_E_CONTRACT_006 " 字段 `%s` 类型应为 enum，实际取值不在列表内（契约 `%s`）",
                     spec->name, cname);
            return -1;
        }
        set_err(err, errsz,
                 SML_E_CONTRACT_002 " 字段 `%s` 类型应为 %s，实际为 %s（契约 `%s`）",
                 spec->name, type_name(spec->ty), kind_name(v), cname);
        return -1;
    }
    /* 数值区间 */
    if (spec->min_set || spec->max_set) {
        double n = 0;
        int isnum = 0;
        if (v->type == SML_INT)        { n = (double)v->u.i; isnum = 1; }
        else if (v->type == SML_FLOAT) { n = v->u.f;           isnum = 1; }
        if (isnum) {
            if (spec->min_set && n < spec->min) {
                set_err(err, errsz,
                         SML_E_CONTRACT_005 " 字段 `%s` 值 %g 小于下界 %g（契约 `%s`）",
                         spec->name, n, spec->min, cname);
                return -1;
            }
            if (spec->max_set && n > spec->max) {
                set_err(err, errsz,
                         SML_E_CONTRACT_005 " 字段 `%s` 值 %g 大于上界 %g（契约 `%s`）",
                         spec->name, n, spec->max, cname);
                return -1;
            }
        }
    }
    return 0;
}

/* 前向声明 (check_type 中用到) */
static int apply_contract_rec(parser *ps, ccontract *c, sml_value *node,
                              char *err, size_t errsz);

/* 对块应用契约: 严格性 + 默认值 + 逐字段校验/组合递归 */
static int apply_contract_rec(parser *ps, ccontract *c, sml_value *node,
                              char *err, size_t errsz) {
    if (node->type != SML_OBJECT) return 0;
    /* 1) 严格性: 未声明字段一律拒绝 (除非 loose) */
    if (!c->allow_extra) {
        sml_field *f;
        for (f = node->u.obj.head; f; f = f->next) {
            /* `__type` / `__featags` 不参与契约校验（Rust 实测：`S !t { }` 在严格契约下通过）；
           `__name` / `__args` **参与** —— `x { @is A addr Sub { } }` 在 Rust 上报
           「字段 `addr.__name` 未声明」。逐个列举是刻意的，别改成 `__` 前缀。 */
        if (!strcmp(f->key, "__type") || !strcmp(f->key, "__featags")) continue;
            int found = 0;
            for (cfield *cf = c->fields; cf; cf = cf->next)
                if (strcmp(cf->name, f->key) == 0) { found = 1; break; }
            if (!found) {
                set_err(err, errsz,
                         SML_E_CONTRACT_004 " 字段 `%s` 未在契约 `%s` 中声明（严格模式；如需允许额外字段请在契约名后写 `loose`）",
                         f->key, c->name);
                return -1;
            }
        }
    }
    /* 2) 逐字段: 填默认 + 类型/枚举/区间/组合校验 */
    for (cfield *cf = c->fields; cf; cf = cf->next) {
        sml_value *v = sml_obj_get(node, cf->name);
        if (!v) {
            if (cf->def) {
                sml_obj_set(node, cf->name, sml_clone(cf->def));
            } else if (cf->required) {
                set_err(err, errsz,
                         SML_E_CONTRACT_003 " 字段 `%s` 必填但缺失（契约 `%s`）",
                         cf->name, c->name);
                return -1;
            }
        } else {
            if (check_type(ps, c->name, cf, v, err, errsz) != 0) return -1;
        }
    }
    return 0;
}

/* ---------------------------------------------------------------------------
   featag S8：`any` 类型默认禁用，契约里写 `!any` 才放行（§10 Q7）

   - 门控 `type.featag`：没开时整段不查，v4 文档里的 `any` 照旧可用；
   - 报码复用 `E-FEATURE-001`（因相同：都是「这项能力没被放行」），detail 说清是契约级开关；
   - **递归**：`ContractRef` 字段按被引用契约继续查（内层自己有 `!any` 才算放行）；
   - `seen` 防递归契约绕圈。
   --------------------------------------------------------------------------- */
static int field_uses_any(const cfield *cf) {
    if (cf->ty == CT_ANY) return 1;
    if (cf->ty == CT_ARRAY && cf->arr_inner == CT_ANY) return 1;
    return 0;
}

static int check_any_allowed(parser *ps, ccontract *c, const char **seen, int n_seen) {
    cfield *cf;
    int i;
    if (!c) return 0;
    for (i = 0; i < n_seen; i++)
        if (strcmp(seen[i], c->name) == 0) return 0;   /* 递归契约绕圈 */
    if (n_seen >= 32) return 0;
    seen[n_seen++] = c->name;
    for (cf = c->fields; cf; cf = cf->next) {
        if (!c->allow_any && field_uses_any(cf)) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_FEATURE_001
                    " 契约 `%s` 的字段 `%s` 用了 `any`（等于放弃类型校验）；"
                    "若确实需要，请在该契约里写 `!any` 显式放开", c->name, cf->name);
            return -1;
        }
        if (cf->ty == CT_CONTRACTREF && cf->ref_name) {
            ccontract *inner = contract_find(ps, cf->ref_name);
            if (inner && check_any_allowed(ps, inner, seen, n_seen) != 0) return -1;
        }
    }
    return 0;
}

static int apply_contract_name(parser *ps, sml_value *node, const char *name,
                               char *err, size_t errsz) {
    ccontract *c = contract_find(ps, name);
    if (!c) {
        set_err(err, errsz, SML_E_CONTRACT_001 " 引用了未定义的契约 `%s`", name);
        return -1;
    }
    /* S8 必须在应用前查：这是「契约**声明**用了 any」的检查，与块里写了什么无关 */
    if (ps->features & (1u << 17)) {
        const char *seen[32];
        if (check_any_allowed(ps, c, seen, 0) != 0) return -1;
    }
    return apply_contract_rec(ps, c, node, err, errsz);
}

/* 应用契约；失败则置 parser.failed 标志 (不依赖 errbuf 是否有遗留内容)。
   `ps->failed` 已经是 1 时**直接返回**：块级 `@is` 是在块解析**结束处**应用的，
   而块体里可能已经报过更具体的错（词法 / 语法 / 未定义片段引用）—— 不加这道
   闸，后应用的契约错会把它覆盖掉，用户拿到的码就不是第一个错。 */
static void apply_or_fail(parser *ps, sml_value *node, const char *name) {
    if (ps->failed) return;
    if (apply_contract_name(ps, node, name, ps->lx->errbuf, ps->lx->errsz) != 0)
        ps->failed = 1;
}

/* ---------------------------------------------------------------------------
   多契约叠加（D3）：一个块应用多份契约 ⇒ **合并视图**

   语义逐条照 `rust/sml-contract/src/lib.rs::apply_contracts`：

   1. **有效严格度** = 块级覆盖 `unwrap_or` 契约级声明；多契约取**最严**一档
      ⇒ 任一份严格即整体严格（本端两档：`allow_extra` 取 AND）；
   2. **顺序无关**：字段按名取并集，与书写先后无关；
   3. **冲突即错**：同名字段但类型 / 默认值 / 区间任一不同 ⇒ `E-CONTRACT-016`，
      **不允许「后者覆盖」**（那会让结果依赖书写顺序）；
   4. 实现方式：构造一份临时契约，**复用** `apply_contract_rec` 做默认值回填 /
      类型校验 / 组合递归 —— 不另写一条校验路径，行为与单契约同源。
   --------------------------------------------------------------------------- */

/* 两个值是否相等（Rust `Value: PartialEq` 同口径：类型 + 内容）。
   只用于「多契约同名字段冲突」判定，故不做数值语义归一
   （`1` 与 `1.0` 记为不同，与 Rust 对 `Option<Value>` 的判等一致）。 */
static int cvalue_eq(const sml_value *a, const sml_value *b) {
    size_t i;
    if (a == b) return 1;
    if (!a || !b) return 0;
    if (a->type != b->type) return 0;
    switch (a->type) {
        case SML_NULL:  return 1;
        case SML_BOOL:  return (a->u.boolean != 0) == (b->u.boolean != 0);
        case SML_INT:   return a->u.i == b->u.i;
        case SML_FLOAT: return a->u.f == b->u.f;
        case SML_STR:
            return a->slen == b->slen && memcmp(a->u.s, b->u.s, a->slen) == 0;
        case SML_ARRAY:
            if (a->u.arr.len != b->u.arr.len) return 0;
            for (i = 0; i < a->u.arr.len; i++)
                if (!cvalue_eq(a->u.arr.items[i], b->u.arr.items[i])) return 0;
            return 1;
        case SML_OBJECT: {
            sml_field *fa = a->u.obj.head, *fb = b->u.obj.head;
            while (fa && fb) {
                if (strcmp(fa->key, fb->key) != 0) return 0;
                if (!cvalue_eq(fa->value, fb->value)) return 0;
                fa = fa->next;
                fb = fb->next;
            }
            return fa == NULL && fb == NULL;
        }
    }
    return 0;
}

/* 同名字段的**类型**是否不同（Rust 用 `TypeSpec::name()` 的规范串比较：
   数组比元素类型、枚举比取值表、组合比被引用契约名）。 */
static int cfield_type_differs(const cfield *a, const cfield *b) {
    size_t i;
    if (a->ty != b->ty) return 1;
    switch (a->ty) {
        case CT_ARRAY:
            if (a->arr_inner != b->arr_inner) return 1;
            /* `[时刻]` vs `[日期]`：元素类型名不同就是不同（Rust 用规范串比较） */
            if (a->arr_inner == CT_PATTERN) {
                if (!a->pat_name || !b->pat_name) return a->pat_name != b->pat_name;
                return strcmp(a->pat_name, b->pat_name) != 0;
            }
            return 0;
        case CT_ENUM:
            if (a->enum_n != b->enum_n) return 1;
            for (i = 0; i < a->enum_n; i++)
                if (strcmp(a->enum_vals[i], b->enum_vals[i]) != 0) return 1;
            return 0;
        case CT_CONTRACTREF:
            if (!a->ref_name || !b->ref_name) return a->ref_name != b->ref_name;
            return strcmp(a->ref_name, b->ref_name) != 0;
        case CT_PATTERN:
            /* 同名即同校验器（Rust `TypeSpec::name()` 对 Pattern 返回类型名） */
            if (!a->pat_name || !b->pat_name) return a->pat_name != b->pat_name;
            return strcmp(a->pat_name, b->pat_name) != 0;
        case CT_EXT:
            /* 同名的外置类型即同校验器（与 Rust `TypeSpec::Ext` 的 name 比较同口径）。 */
            if (!a->ext_type || !b->ext_type) return a->ext_type != b->ext_type;
            if (!a->ext_type->name || !b->ext_type->name)
                return a->ext_type->name != b->ext_type->name;
            return strcmp(a->ext_type->name, b->ext_type->name) != 0;
        default: return 0;
    }
}

/* 同名字段是否**冲突**（类型 / 默认值 / 区间任一不同）。 */
static int cfield_clash(const cfield *a, const cfield *b) {
    if (cfield_type_differs(a, b)) return 1;
    if (!cvalue_eq(a->def, b->def)) return 1;
    if (a->min_set != b->min_set) return 1;
    if (a->max_set != b->max_set) return 1;
    if (a->min_set && a->min != b->min) return 1;
    if (a->max_set && a->max != b->max) return 1;
    return 0;
}

static cfield *cfield_find(cfield *head, const char *name) {
    for (; head; head = head->next)
        if (strcmp(head->name, name) == 0) return head;
    return NULL;
}

/* 释放合并产物：名字 / 字段壳是**本次新分配**的，字段里的 `def` / `enum_vals` /
   `ref_name` 仍指向原契约（借用），故不能逐个深释放。 */
static void merged_free(ccontract *m) {
    cfield *f;
    if (!m) return;
    f = m->fields;
    while (f) {
        cfield *nx = f->next;
        free(f);
        f = nx;
    }
    free(m->name);
    free(m);
}

/* 把一组契约合并成一份临时契约。失败返回 NULL 并写 err（001 未定义 / 016 冲突）。 */
static ccontract *merge_contracts(parser *ps, const ccontracts *l,
                                  char *err, size_t errsz) {
    ccontract **list;
    ccontract *m;
    cfield **tail;
    size_t i, j, k;
    char *joined = NULL;
    size_t jlen = 0;

    list = (ccontract **)calloc(l->n ? l->n : 1, sizeof(*list));
    if (!list) { set_err(err, errsz, SML_E_INTERNAL_001 " 内存不足"); return NULL; }
    /* 1) 逐个解析（未定义 ⇒ 001，同单契约） */
    for (i = 0; i < l->n; i++) {
        list[i] = contract_find(ps, l->names[i]);
        if (!list[i]) {
            set_err(err, errsz, SML_E_CONTRACT_001 " 引用了未定义的契约 `%s`", l->names[i]);
            free(list);
            return NULL;
        }
    }
    /* 2) 冲突预检：按**字段名**两两比较，与书写顺序无关 */
    for (i = 0; i < l->n; i++) {
        for (j = i + 1; j < l->n; j++) {
            cfield *fa;
            for (fa = list[i]->fields; fa; fa = fa->next) {
                cfield *fb = cfield_find(list[j]->fields, fa->name);
                if (fb && cfield_clash(fa, fb)) {
                    set_err(err, errsz,
                            SML_E_CONTRACT_016
                            " 字段 `%s` 在 `%s` 与 `%s` 里类型 / 默认值 / 区间不一致",
                            fa->name, list[i]->name, list[j]->name);
                    free(list);
                    return NULL;
                }
            }
        }
    }
    m = (ccontract *)calloc(1, sizeof(ccontract));
    if (!m) { free(list); set_err(err, errsz, SML_E_INTERNAL_001 " 内存不足"); return NULL; }
    /* 3) 字段并集：同名字段的类型/默认值/区间已预检一致，取任一份即可；
          `required` **不在**预检内，取**或**（任一份要求即必填）才算顺序无关。 */
    tail = &m->fields;
    for (i = 0; i < l->n; i++) {
        cfield *fa;
        for (fa = list[i]->fields; fa; fa = fa->next) {
            cfield *dst = cfield_find(m->fields, fa->name);
            if (dst) {
                if (fa->required) dst->required = 1;
            } else {
                cfield *cp = (cfield *)malloc(sizeof(cfield));
                if (!cp) { free(list); merged_free(m); set_err(err, errsz, SML_E_INTERNAL_001 " 内存不足"); return NULL; }
                *cp = *fa;
                cp->next = NULL;
                *tail = cp;
                tail = &cp->next;
            }
        }
    }
    /* 4) 严格度：每条取「块级覆盖 unwrap_or 契约级」，多契约取**最严**
          （两档 ⇒ 任一份严格即整体严格 ⇒ `allow_extra` 取 AND）。 */
    m->allow_extra = 1;
    m->strict_paths = 0;
    m->allow_any = 0;
    for (i = 0; i < l->n; i++) {
        int eff = (l->ovs[i] < 0) ? list[i]->allow_extra
                                  : (l->ovs[i] == 0 ? 1 : 0);
        if (!eff) m->allow_extra = 0;
        if (list[i]->strict_paths) m->strict_paths = 1;
        if (list[i]->allow_any) m->allow_any = 1;
    }
    /* 名字只用于错误文案（`A+B`，与 Rust 一致） */
    for (i = 0; i < l->n; i++) jlen += strlen(l->names[i]) + 1;
    joined = (char *)malloc(jlen + 1);
    if (!joined) { free(list); merged_free(m); set_err(err, errsz, SML_E_INTERNAL_001 " 内存不足"); return NULL; }
    joined[0] = '\0';
    for (k = 0; k < l->n; k++) {
        if (k) strcat(joined, "+");
        strcat(joined, l->names[k]);
    }
    m->name = joined;
    free(list);
    return m;
}

/* 应用一组契约（n >= 1）。单契约且无块级覆盖时**逐字走原路径**（错误文案 / 行为不变）。 */
static int apply_contract_set(parser *ps, sml_value *node, const ccontracts *l,
                              char *err, size_t errsz) {
    ccontract *m;
    int rc;
    if (!l || l->n == 0) return 0;
    if (l->n == 1 && l->ovs[0] < 0)
        return apply_contract_name(ps, node, l->names[0], err, errsz);
    /* S8 逐份查（与 `@is` 单契约同一判据：契约**声明**用了 any 与块内容无关） */
    if (ps->features & (1u << 17)) {
        size_t i;
        for (i = 0; i < l->n; i++) {
            const char *seen[32];
            ccontract *c = contract_find(ps, l->names[i]);
            if (!c) {
                set_err(err, errsz, SML_E_CONTRACT_001 " 引用了未定义的契约 `%s`", l->names[i]);
                return -1;
            }
            if (check_any_allowed(ps, c, seen, 0) != 0) return -1;
        }
    }
    m = merge_contracts(ps, l, err, errsz);
    if (!m) return -1;
    rc = apply_contract_rec(ps, m, node, err, errsz);
    merged_free(m);
    return rc;
}

/* 同 apply_or_fail，但作用于一组契约。 */
static void apply_set_or_fail(parser *ps, sml_value *node, const ccontracts *l) {
    if (ps->failed) return;
    if (apply_contract_set(ps, node, l, ps->lx->errbuf, ps->lx->errsz) != 0)
        ps->failed = 1;
}

/* min/max 的边界取值：必须是十进制数字，且为**有限**值。
   非数字/非十进制写法 -> E-PARSE-022；非有限（nan / 1e400）-> E-CONTRACT-010。
   与 Rust 同码（rust/sml-parse/src/parser.rs 的 parse_spec_number：只收 Tok::Word 且
   f64 可解析；`!f.is_finite()` 时报 E_CONTRACT_010；其它一律 E_PARSE_022）。
   修之前这里是 `atof()` 且不看结果：`min abc` 静默变成下界 0，`max abc` 更糟 ——
   非法边界当 0 后把合法值判成 E-CONTRACT-005（**错码**）。 */
static int parse_bound_value(parser *ps, const char *kw, double *out) {
    token *nv = next(ps);
    char *end = NULL;
    double d;
    if (!nv || nv->t != T_WORD) {
        /* Rust 对 Tok::Str / 缺取值（EOF）同样报 E_PARSE_022（"期望数字"） */
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_022 " `%s` 的边界取值不是数字", kw);
        ps->failed = 1;
        return -1;
    }
    /* C99 的 strtod 还认 "0x10"（十六进制浮点，= 16.0），而 Rust 的 f64::from_str 不认。
       不收窄就会比 Rust 多接受一种写法；十进制浮点/科学计数/ inf / nan 都不含 x X p P。 */
    if (strpbrk(nv->v, "xXpP") != NULL) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_022 " `%s` 的边界取值 `%s` 不是数字", kw, nv->v);
        ps->failed = 1;
        return -1;
    }
    d = strtod(nv->v, &end);
    if (!end || end == nv->v || *end != '\0') {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_022 " `%s` 的边界取值 `%s` 不是数字", kw, nv->v);
        ps->failed = 1;
        return -1;
    }
    if (!isfinite(d)) {
        /* NaN 的所有比较均为 false，会令 min/max 校验被静默穿透（Rust 侧审计 #2 同因） */
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_CONTRACT_010 " `%s` 的边界取值 `%s` 非有限数，不能作为数值约束的取值", kw, nv->v);
        ps->failed = 1;
        return -1;
    }
    *out = d;
    return 0;
}

/* 解析契约体 (调用前已消费 '{'；负责消费 '}') */
static void parse_contract_body(parser *ps, ccontract *c) {
    /* 「每契约单一语种」（门控 `lex.ident`）：已见的第一种文字系统与其标识符。
       契约体解析不重入 ⇒ 局部变量足够（契约就是那个命名空间）。 */
    int id_script = -1;
    const char *id_script_first = NULL;
    if (peek(ps)->t != T_LBRACE) return;
    next(ps); /* consume { */
    while (1) {
        token *ft = peek(ps);
        if (ps->failed) break;   /* 出错后立刻收手：否则会继续吞 token 并可能覆盖已写的错误码 */
        /* —— 契约体里的 `@requires`：本端暂未实现，必须**响亮拒绝** ——
           参照 E-CONTRACT-017（契约叠加未实现）的既有先例。
           ⚠️ 改前 T_AT 落进下面「非 WORD/STR 就 next 丢弃」的兜底分支被**静默吞掉**，
           随后的 `requires` 反被当成字段名、`typed-block` 变成 CT_CONTRACTREF，
           最终 rc=0、契约里多出一个垃圾字段 —— 这正是要消灭的「静默给树」。 */
        if (ft->t == T_AT) {
            next(ps); /* 消费 '@' */
            token *dt = peek(ps);
            const char *dname = (dt->t == T_WORD || dt->t == T_STR) ? dt->v : "";
            if (strcmp(dname, "requires") == 0) {
                next(ps);   /* 消费 `requires` */
                /* 收集特性名并在**定义时**校验（Rust `directive.rs` 同判据）：
                   - 逗号 / 空格都可分隔，单词里的逗号（`a,b`）也拆开；
                   - 遇「词后跟冒号」⇒ 那是下一个字段名，列表到此结束（Rust 用 peek_at(1)）；
                   - 名字不在特性表里 ⇒ E-FEATURE-003；
                   - 名字有效但未开启 ⇒ E-CONTRACT-018。 */
                for (;;) {
                    token *rt = peek(ps);
                    if (rt->t == T_COMMA) { next(ps); continue; }
                    if (rt->t != T_WORD && rt->t != T_STR) break;
                    if (peek_at(ps, 1)->t == T_COLON) break;   /* 下一个字段 */
                    {
                        const char *rw = next(ps)->v;
                        const char *p = rw;
                        while (*p) {
                            const char *q = p;
                            char namebuf[64];
                            size_t len;
                            int fi;
                            while (*q && *q != ',') q++;
                            len = (size_t)(q - p);
                            while (len > 0 && (*p == ' ' || *p == '\t')) { p++; len--; }
                            while (len > 0 && (p[len - 1] == ' ' || p[len - 1] == '\t')) len--;
                            if (len > 0 && len < sizeof(namebuf)) {
                                memcpy(namebuf, p, len);
                                namebuf[len] = '\0';
                                fi = feature_index(namebuf);
                                if (fi < 0) {
                                    set_err(ps->lx->errbuf, ps->lx->errsz,
                                            SML_E_FEATURE_003
                                            " 契约 `%s` 的 `@requires` 引用了无效特性 `%s`",
                                            c ? c->name : "?", namebuf);
                                    ps->failed = 1;
                                    break;
                                }
                                if (!(ps->features & (1u << (unsigned)fi))) {
                                    set_err(ps->lx->errbuf, ps->lx->errsz,
                                            SML_E_CONTRACT_018
                                            " 契约 `%s` 要求特性 `%s` 已开启"
                                            "（请用 `@feature enable %s`）",
                                            c ? c->name : "?", namebuf, namebuf);
                                    ps->failed = 1;
                                    break;
                                }
                            }
                            p = (*q == ',') ? q + 1 : q;
                        }
                        if (ps->failed) break;
                    }
                }
                if (ps->failed) break;
                continue;
            }
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_005 " 契约体内不支持指令 `@%s`（契约体仅支持 `@requires`）", dname);
            ps->failed = 1;
            break;
        }
        /* featag 声明：`!name` / `!name: 类型` / `!name: 类型 -> 落点`。
           开了 `type.featag` 才是记号。**S5**：名字收进契约（使用处判「未声明即拒」）；
           **S5b**：`： 类型` 也收进来，使用处据此校验参数个数与标量类型。
           `-> 落点` 属 S6，仍整行跳过。 */
        if ((ps->features & (1u << 17)) && ft->t == T_WORD && ft->v[0] == '!') {
            int decl_line = ft->line;
            cty dty = CT_ANY, dinner = CT_ANY;
            int has_spec = 0;
            next(ps);   /* consume `!name` */
            /* B1：空名 / 连续 `!` ⇒ E-PARSE-026（Rust contract.rs:84 同判据）。 */
            if (ft->v[1] == '\0' || ft->v[1] == '!') {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_026 " featag 名为空或以 `!` 开头：`%s`", ft->v);
                ps->failed = 1;
                break;
            }
            /* 契约级开关 `!any` / `!strict-paths`：**不占** featag 名（Rust 同判据）——
               块头写 `C !any` 在 Rust 里报「未声明」，所以这里绝不能登记成 featag。
               两者也不接受 `: 类型` / `[…]` / `-> 落点`，写了就报 021。 */
            if (strcmp(ft->v + 1, "any") == 0 || strcmp(ft->v + 1, "strict-paths") == 0) {
                if (peek(ps)->t == T_COLON || peek(ps)->t == T_LBRACK) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_CONTRACT_021
                            " `!%s` 是契约级开关，不接受类型 / 参数 / 落点", ft->v + 1);
                    ps->failed = 1;
                    break;
                }
                if (c && strcmp(ft->v + 1, "strict-paths") == 0)
                    c->strict_paths = 1;   /* S6：落点父对象必须已存在 */
                if (c && strcmp(ft->v + 1, "any") == 0)
                    c->allow_any = 1;      /* S8：放行 `any` 类型 */
                while (peek(ps)->t != T_RBRACE && peek(ps)->t != T_EOF &&
                       peek(ps)->line == decl_line)
                    next(ps);
                continue;
            }
            if (peek(ps)->t == T_COLON) {
                next(ps);
                has_spec = 1;
                token *dt = peek(ps);
                if (dt->t == T_LBRACK) {
                    next(ps);
                    dty = CT_ARRAY;
                    if (peek(ps)->t == T_WORD) {
                        char *iw = next(ps)->v;
                        if      (strcmp(iw, "str")  == 0) dinner = CT_STR;
                        else if (strcmp(iw, "int")  == 0) dinner = CT_INT;
                        else if (strcmp(iw, "num")  == 0) dinner = CT_NUM;
                        else if (strcmp(iw, "bool") == 0) dinner = CT_BOOL;
                    }
                    if (peek(ps)->t == T_RBRACK) next(ps);
                } else if (dt->t == T_WORD) {
                    char *tw = next(ps)->v;
                    if      (strcmp(tw, "str")  == 0) dty = CT_STR;
                    else if (strcmp(tw, "int")  == 0) dty = CT_INT;
                    else if (strcmp(tw, "num")  == 0) dty = CT_NUM;
                    else if (strcmp(tw, "bool") == 0) dty = CT_BOOL;
                }
            }
            /* 修饰符：`default <值>` 与 `-> 落点`（可同时出现、顺序不定）。
               S6 的 `auto` 取的就是这里的 `default`；落点在声明期解析成段序列。 */
            {
                sml_value *ddef = NULL;
                int has_def = 0;
                char raw_path[512];
                size_t rp = 0;
                int has_path = 0;
                raw_path[0] = '\0';
                while (peek(ps)->t != T_RBRACE && peek(ps)->t != T_EOF &&
                       peek(ps)->line == decl_line) {
                    token *mt = peek(ps);
                    if (mt->t == T_WORD && strcmp(mt->v, "default") == 0) {
                        next(ps);
                        if (peek(ps)->t == T_WORD || peek(ps)->t == T_STR) {
                            token *dv = next(ps);
                            ddef = (dv->t == T_STR) ? sml_new_strn(dv->v, dv->len)
                                                    : coerce_word(dv->v, ps);
                            has_def = 1;
                        }
                        continue;
                    }
                    if (mt->t == T_WORD && strcmp(mt->v, "->") == 0) {
                        next(ps);
                        has_path = 1;
                        continue;
                    }
                    if (has_path) {
                        char one[2] = {0, 0};
                        const char *add = NULL;
                        if (mt->t == T_WORD || mt->t == T_STR) add = mt->v;
                        else if (mt->t == T_LBRACK) { one[0] = '['; add = one; }
                        else if (mt->t == T_RBRACK) { one[0] = ']'; add = one; }
                        if (add) {
                            size_t l = strlen(add);
                            if (rp + l < sizeof(raw_path)) {
                                memcpy(raw_path + rp, add, l);
                                rp += l;
                                raw_path[rp] = '\0';
                            }
                        }
                    }
                    next(ps);
                }
                if (c) {
                    cfeatag *df, *nf;
                    int dup = 0;
                    for (df = c->featags; df; df = df->next)
                        if (strcmp(df->name, ft->v + 1) == 0) { dup = 1; break; }
                    if (dup) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_CONTRACT_019 " `%s` 在本契约里出现了两次", ft->v + 1);
                        ps->failed = 1;
                        break;
                    }
                    /* 标识符白名单（门控 `lex.ident`）：featag 名同样是标识符 */
                    if (ps->features & (1u << 18)) {
                        if (check_ident(ps, ft->v + 1) != 0) { ps->failed = 1; break; }
                        if (ident_check_script(ps, &id_script, &id_script_first,
                                               ft->v + 1) != 0) {
                            ps->failed = 1; break;
                        }
                    }
                    nf = (cfeatag *)calloc(1, sizeof(cfeatag));
                    if (nf) {
                        nf->name = strdup(ft->v + 1);
                        nf->has_spec = has_spec;
                        nf->ty = dty;
                        nf->arr_inner = dinner;
                        nf->has_def = has_def;
                        nf->def = ddef;
                        if (has_path && cpath_parse(raw_path, &nf->path, &nf->n_path) != 0) {
                            set_err(ps->lx->errbuf, ps->lx->errsz,
                                    SML_E_CONTRACT_020 " `%s` 的落点 `%s` 写法非法",
                                    ft->v + 1, raw_path);
                            ps->failed = 1;
                            break;
                        }
                        if (nf->n_path > 0 && nf->path[0].is_idx) {
                            set_err(ps->lx->errbuf, ps->lx->errsz,
                                    SML_E_CONTRACT_020 " `%s` 的落点须以键名开头", ft->v + 1);
                            ps->failed = 1;
                            break;
                        }
                        nf->next = c->featags;
                        c->featags = nf;
                    }
                } else if (ddef) {
                    sml_free(ddef);
                }
            }
            continue;
        }
        if (ft->t == T_RBRACE) { next(ps); break; }
        if (ft->t == T_EOF) { err_unclosed(ps, "契约体"); break; }
        if (ft->t != T_WORD && ft->t != T_STR) { next(ps); continue; }
        char *fname = next(ps)->v;
        /* 标识符白名单（门控 `lex.ident`）：字段名同样是标识符 */
        if (ps->features & (1u << 18)) {
            if (check_ident(ps, fname) != 0) { ps->failed = 1; break; }
            if (ident_check_script(ps, &id_script, &id_script_first, fname) != 0) {
                ps->failed = 1; break;
            }
        }
        if (peek(ps)->t == T_COLON) next(ps);
        cfield *cf = (cfield *)calloc(1, sizeof(cfield));
        cf->name = strdup(fname);
        cf->required = 1;
        cf->arr_inner = CT_ANY;
        /* 类型 */
        token *tt = peek(ps);
        if (tt->t == T_LBRACK) {
            /* 数组: [ inner ] */
            next(ps); /* consume [ */
            cf->ty = CT_ARRAY;
            token *it = peek(ps);
            if (it->t == T_WORD) {
                char *iw = next(ps)->v;
                if      (strcmp(iw, "str")  == 0) cf->arr_inner = CT_STR;
                else if (strcmp(iw, "int")  == 0) cf->arr_inner = CT_INT;
                else if (strcmp(iw, "num")  == 0) cf->arr_inner = CT_NUM;
                else if (strcmp(iw, "bool") == 0) cf->arr_inner = CT_BOOL;
                else if (strcmp(iw, "any")  == 0) cf->arr_inner = CT_ANY;
                else if (strcmp(iw, "enum") == 0) {
                    /* 元素位置不许 enum（Rust 同码 012）：`[enum a b]` 没有确定语义 */
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_CONTRACT_012 " 数组元素类型不能是 `enum`");
                    ps->failed = 1;
                } else {
                    /* `@type` 声明的自定义类型优先（Rust `Some(Tok::LBrack)` 分支同一次序：
                       先查 types，再查外置类型，最后才回落成契约引用）。 */
                    sml_value *tdef = ps->types ? sml_obj_get(ps->types, iw) : NULL;
                    if (tdef) {
                        char perr[256];
                        perr[0] = '\0';
                        cf->arr_pat = sp_compile(tdef, ps->types, perr, sizeof perr);
                        if (!cf->arr_pat) {
                            set_err(ps->lx->errbuf, ps->lx->errsz,
                                    SML_E_CONTRACT_013 " 类型 `%s` 的模式编译失败：%s", iw, perr);
                            ps->failed = 1;
                        } else {
                            cf->arr_inner = CT_PATTERN;
                            cf->pat_name = strdup(iw);
                        }
                    } else {
                        /* 非 `@type`、非内置名 ⇒ **回落成契约引用**（Rust 同一次序：
                           `@type` → 外置类型 → 契约引用）。此前这里什么都不做 ⇒
                           `[Endpoint]` 的 `arr_inner` 留在 CT_ANY ⇒ 元素一律放行
                           （契约约束被静默跳过）。 */
                        cf->arr_inner = CT_CONTRACTREF;
                        cf->arr_ref_name = strdup(iw);
                    }
                }
            }
            if (peek(ps)->t == T_RBRACK) next(ps);
        } else if (tt->t == T_WORD) {
            char *tw = next(ps)->v;
            if      (strcmp(tw, "str")  == 0) cf->ty = CT_STR;
            else if (strcmp(tw, "int")  == 0) cf->ty = CT_INT;
            else if (strcmp(tw, "num")  == 0) cf->ty = CT_NUM;
            else if (strcmp(tw, "bool") == 0) cf->ty = CT_BOOL;
            else if (strcmp(tw, "any")  == 0) cf->ty = CT_ANY;
            else if (strcmp(tw, "enum") == 0) {
                cf->ty = CT_ENUM;
                if (peek(ps)->t == T_LBRACK) {
                    next(ps);
                    while (peek(ps)->t != T_RBRACK && peek(ps)->t != T_EOF) {
                        token *et = next(ps);
                        if (et->t == T_WORD || et->t == T_STR) {
                            cf->enum_vals = (char **)realloc(cf->enum_vals,
                                            (cf->enum_n + 1) * sizeof(char *));
                            cf->enum_vals[cf->enum_n++] = strdup(et->v);
                        }
                    }
                    if (peek(ps)->t == T_RBRACK) next(ps);
                } else {
                    /* `enum` 后必须是数组（Rust 同码 023）：`enum x y` 会被误读成类型名 x */
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_023 " `enum` 后须为数组，如 `enum [ a b ]`");
                    ps->failed = 1;
                }
            } else {
                /* 非内置类型名，按此顺序判定（与 Rust 完全相同）：
                   1. `@type` 声明的自定义类型（模式校验）
                   2. 外置类型（下游注册，如 image / link / time）
                   3. 回落为契约引用（组合）
                   次序不可颠倒：同名时 `@type` 更「显式」，外置其次。 */
                sml_value *tdef = ps->types ? sml_obj_get(ps->types, tw) : NULL;
                const sml_ext_type *xtype = sml_ext_find_type(ps->ext, tw);
                if (!tdef && xtype) {
                    cf->ty = CT_EXT;
                    cf->ext_type = xtype;
                } else if (tdef) {
                    char perr[256];
                    perr[0] = '\0';
                    cf->pat = sp_compile(tdef, ps->types, perr, sizeof perr);
                    if (!cf->pat) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_CONTRACT_013 " 类型 `%s` 的模式编译失败：%s", tw, perr);
                        ps->failed = 1;
                        break;
                    }
                    cf->ty = CT_PATTERN;
                    cf->pat_name = strdup(tw);
                } else {
                    /* 契约名 (组合 / ContractRef) */
                    cf->ty = CT_CONTRACTREF;
                    cf->ref_name = strdup(tw);
                }
            }
        } else if (tt->t == T_RBRACE || tt->t == T_EOF) {
            /* 类型位置直接是 `}` / EOF：`@contract C { a: }`。改前这里静默落成 any，
               契约照常「通过」——与 Rust 的 006 不一致，且用户以为写了约束。 */
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_006 " 契约字段 `%s` 缺少类型", fname);
            ps->failed = 1;
            break;
        } else {
            cf->ty = CT_ANY;
        }
        /* 修饰符：只保留单词形式 optional / default / min / max / loose。 */
        for (;;) {
            token *mt = peek(ps);
            if (mt->t == T_QMARK || mt->t == T_EQ) {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_006 " 契约字段修饰符只支持单词形式 `optional` / `default`");
                ps->failed = 1;
                break;
            }
            if (mt->t == T_WORD && strcmp(mt->v, "optional") == 0) {
                next(ps); cf->required = 0; continue;
            }
            if (mt->t == T_WORD && strcmp(mt->v, "default") == 0) {
                next(ps);
                token *dv = peek(ps);
                /* `default` 后没有取值（`}` / EOF / 标点）：**必须报 021**。
                   改前这里是 `dv->v` 交给 coerce_word，标点 token 的 v 是 NULL
                   ⇒ strcmp(NULL, ...) ⇒ 段错误（0xC0000005），是整个仓库唯一能让
                   宿主进程崩掉的输入。 */
                if (!dv || (dv->t != T_STR && dv->t != T_WORD)) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_021 " `default` 缺少取值（应与类型写在同一行）");
                    ps->failed = 1;
                    break;
                }
                next(ps);
                cf->def = (dv->t == T_STR) ? sml_new_strn(dv->v, dv->len)
                                           : coerce_word(dv->v, ps);
                continue;
            }
            if (mt->t != T_WORD) break;
            /* S7：来源修饰符 —— 该键只能由 featag 落点写入 */
            if (strcmp(mt->v, "from-featag") == 0) {
                next(ps); cf->from_featag = 1; continue;
            }
            if (strcmp(mt->v, "min") == 0) {
                next(ps);
                if (parse_bound_value(ps, "min", &cf->min) == 0) cf->min_set = 1;
                continue;
            }
            if (strcmp(mt->v, "max") == 0) {
                next(ps);
                if (parse_bound_value(ps, "max", &cf->max) == 0) cf->max_set = 1;
                continue;
            }
            if (strcmp(mt->v, "loose") == 0) {
                next(ps); c->allow_extra = 1; continue;
            }
            break;
        }
        cf->next = c->fields;
        c->fields = cf;
    }
}

/* parse_array 的守卫 wrapper：真正的实现在 parse_array_inner。
   与 parse_block **共用同一个 `ps->depth`**（Rust 侧同样块与数组共用一个 depth 计数器），
   故「128 层放行 / 第 129 层报 E-LIMIT-001」对块嵌套与数组嵌套是同一口径。
   ⚠️ 这层是 W17 补的：给 parse_array_inner 加嵌套数组递归时**必须同时**给闸 ——
   只加递归等于把「静默错解」换成「栈溢出」，而后者在 C 里是段错误，错误处理接不住。 */
static sml_value *parse_array(parser *ps) {
    if (ps->depth > SML_MAX_VALUE_DEPTH) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_LIMIT_001 " 嵌套过深（超过 %d 层），疑似递归或恶意输入",
                SML_MAX_VALUE_DEPTH);
        ps->failed = 1;
        return sml_new_null();
    }
    ps->depth++;
    sml_value *v = parse_array_inner(ps);
    if (ps->depth > 0) ps->depth--;
    return v;
}

/* 数组元素位置的裸块（W4 的 B 类对齐点）：`[ type [featag 区] [契约词…] [块名] { … } ]`。
   **实现在 `05_parse.inc`** —— 那里才有 S6 / S7 的 featag helper（本文件在它之前
   include）。两处（键位置 / 数组位置）共用同一对 `parse_bare_head` /
   `finish_bare_block`，与 Rust 两个入口共用 `bare_child_frame` 同构。 */
static sml_value *try_parse_array_bare_block(parser *ps);

static sml_value *parse_array_inner(parser *ps) {
    sml_value *arr = sml_new_array();
    for (;;) {
        token *t = peek(ps);
        if (ps->failed) break;   /* 出错/超限后立刻收手，让栈退掉（见 parse_block） */
        if (t->t == T_RBRACK) { next(ps); break; }
        /* 数组以 `[` 开场（本函数只在消费掉 `[` 之后被调用），故遇 EOF 即未闭合 */
        if (t->t == T_EOF) { err_unclosed(ps, "数组"); break; }
        if (t->t == T_COMMA) { next(ps); continue; }
        if (t->t == T_LBRACE) {
            char seg[32];
            next(ps);
            /* 旁路表路径：数组元素写成 `[n]`（与 Rust 的 PathSeg::Index 同表达）。 */
            snprintf(seg, sizeof seg, "[%zu]", sml_arr_len(arr));
            ps->enter_key = seg;
            sml_arr_push(arr, parse_block(ps, T_RBRACE));
        } else if (t->t == T_LBRACK) {
            /* 嵌套数组 `m: [ [a] b ]` / `m: [[1]]`（W17）。
               此前**没有这一支**，`[` 落到最后的 else 被 `next` 丢掉；后果不是"少一层"，
               而是**静默错解 + 凭空造键**（改前实测，与 Rust/JS 都不一致）：
                 `m: [ [ a ] ]`         → `{"m":["a"]}`         （期望 `{"m":[["a"]]}`）
                 `m: [ 1, [2, 3], 4 ]`  → `{"m":[1,2,3],"4":4}` （内层 `]` 被外层当结束符，
                                                                    剩下的 `4` 变成了键）
                 `m: [ [a], [b] ]`      → `{"m":["a"]}`         （`[b]` 整个丢掉）
                 `a: ` + 100 层 `[..]`  → `{"a":[]}`            （全被吞成空数组）
               与 Rust（parser.rs 的 `Tok::LBrack` → `self.parse_array()`）和 JS
               （同源 bug 早先已修）对齐。
               ⚠️ **必须走 wrapper `parse_array`**（不是直接调 inner）：深度守卫只在
               wrapper 里，直接递归 inner 会让深数组绕过上限 —— 那就是栈溢出。 */
            next(ps);
            sml_arr_push(arr, parse_array(ps));
        } else if (t->t == T_STR) {
            token *st = next(ps);
            sml_arr_push(arr, sml_new_strn(st->v, st->len));
        } else if (t->t == T_WORD) {
            /* 数组位置的裸块 `type [name…] { … }`（W4 的 B 类差异所在）。
               改前这里只有「当标量」一条路 ⇒ `[ section 情节 { … } ]` 被拆成
               `section`、`情节`、`{ … }` **三个**元素，而 Rust 是一个块对象
               （`{ __type: section, __name: 情节, … }`）。判据与写法照抄 Rust，
               见 try_parse_array_bare_block。 */
            sml_value *blk = try_parse_array_bare_block(ps);
            /* 该函数在「认出是裸块却拒绝」时返回 NULL 且置 failed（如多契约 017）——
               此时**不能**再走「当标量」那一步：游标已停在拒绝处，`next()` 会跨过它，
               而 `coerce_word` 又拿到 EOF 的 `v`（可能为 NULL）⇒ 段错误。 */
            if (ps->failed) break;
            if (blk) sml_arr_push(arr, blk);
            else     sml_arr_push(arr, coerce_word(next(ps)->v, ps));
        } else if (t->t == T_RBRACE) {
            /* 数组里多余的 `}` ⇒ E-PARSE-003（W16 落地；Rust 一直报它，JS 同批已改）。
               改前这一格落在下面的兜底 else 里**静默跳过**：`m: [ } ]` 得到 `{"m":[]}`，
               与输入形状不符却毫无提示 —— 属「静默改数据」，正是本批要堵的一类。 */
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_003 " 多余的结束符号 `}`，没有与之匹配的开始符号");
            ps->failed = 1;
            break;
        } else {
            /* 兜底：其余 token（孤立 `:` / `@` / `?` / `=` 等）仍**静默跳过**。
               ⚠️ 不在本批（W16 判定表）范围内，别顺手扩：判定表只把「数组里多余的 `}`」
               定为改成报错，其余留给后续逐条判定。 */
            next(ps);
        }
    }
    return arr;
}

