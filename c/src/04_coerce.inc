/* 标量识别 (裸词) */
static sml_value *coerce_word(const char *w, parser *ps) {
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
    if (ps->pending_weak && val && val->type == SML_OBJECT) {
        const char *wname = ps->pending_weak;
        weak_check_block(ps, val, wname);
        weak_bind_push(ps, wname, ps->pending_weak_line, key);
        ps->pending_weak = NULL;
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
static int check_type(parser *ps, const char *cname, const cfield *spec,
                      const sml_value *v, char *err, size_t errsz) {
    int ok = 0;
    switch (spec->ty) {
        case CT_ANY:  ok = 1; break;
        case CT_STR:  ok = (v->type == SML_STR); break;
        case CT_INT:  ok = (v->type == SML_INT); break;
        case CT_NUM:  ok = (v->type == SML_INT || v->type == SML_FLOAT); break;
        case CT_BOOL: ok = (v->type == SML_BOOL); break;
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
                    default:      eok = 1; break;
                }
                if (!eok) { ok = 0; break; }
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

static int apply_contract_name(parser *ps, sml_value *node, const char *name,
                               char *err, size_t errsz) {
    ccontract *c = contract_find(ps, name);
    if (!c) {
        set_err(err, errsz, SML_E_CONTRACT_001 " 引用了未定义的契约 `%s`", name);
        return -1;
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
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_CONTRACT_018 " 本实现暂不支持契约的 `@requires` 指令（它要求先开启所列特性）");
            } else {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_005 " 契约体内不支持指令 `@%s`（契约体仅支持 `@requires`）", dname);
            }
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
                    cfeatag *nf = (cfeatag *)calloc(1, sizeof(cfeatag));
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
                /* 契约名 (组合 / ContractRef) */
                cf->ty = CT_CONTRACTREF;
                cf->ref_name = strdup(tw);
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

/* 数组元素位置的裸块 `type [name…] { … }`（W4 的 B 类对齐点）。
   与 Rust 的 `bare_block_ahead()` + `parse_bare_block()` **逐字对应**：

   * 入口判据：当前 token 必须是 **T_WORD**。Rust 的 `Some(Tok::Str(_))` 分支直接当字符串
     元素、不试裸块 —— 所以 `[ "sec" { } ]` 在两端都**不是**块，别顺手给 T_STR 也开。
   * 预扫描：从当前位置起，连续的词/串之后紧跟 `{` 才算裸块；撞上别的 token（含 `]`、
     EOF）即「不是」。因此 `[ hello world ]` 仍是两个标量元素（Rust 同）。
   * 识别成功：消费掉「类型词 [参数…] { … }」并返回块对象，块内写
     `__type` = 类型词（**原词**，不经 coerce）、`__name` = 首个参数、多余参数进 `__args`
     —— 与 Rust `parse_bare_block` 一致（参数不再静默丢弃）。
   * 不识别：返回 NULL 且**一个 token 都不消费**，调用方按原来的标量路径处理。

   为什么参数要 clone 后存：`args` 这个临时数组下面就被 sml_free，直接借用其中的值会让
   `__name` / `__args` 变成悬垂指针（与 parse_block 里「带名块堆损坏」那条注释同因）。 */
static sml_value *try_parse_array_bare_block(parser *ps) {
    if (peek(ps)->t != T_WORD) return NULL;
    size_t probe = ps->lx->pos;
    int found = 0;
    while (probe < ps->lx->n) {
        tok_type pt = ps->lx->toks[probe].t;
        if (pt == T_WORD || pt == T_STR) probe++;
        else if (pt == T_LBRACE) { found = 1; break; }
        else break;
    }
    if (!found) return NULL;
    /* ⚠️ 先**消费类型名**再收参数（Rust：`self.next(); // 消费类型名` 然后
       `parse_bare_block(&w)`）。漏掉这一次消费，类型词就会被当成第一个参数 ⇒
       `section 情节 { }` 得到 `__name: section` + `__args: [情节]`（实测踩过）。 */
    token *first = next(ps);
    const char *type_word = first->v;   /* 原词（token 存活到 lex_free） */
    sml_value *args = sml_new_array();
    /* 任务 D：数组位置的块级多契约同样「响亮拒绝」（head 命中契约表且参数里出现
       第 2 个已注册契约词 ⇒ E-CONTRACT-017）。 */
    int key_is_contract = (contract_find(ps, type_word) != NULL);
    int arg_has_contract = 0;
    while (peek(ps)->t == T_WORD || peek(ps)->t == T_STR) {
        token *at = next(ps);
        if (key_is_contract && contract_find(ps, at->v)) arg_has_contract = 1;
        sml_arr_push(args, at->t == T_STR ? sml_new_strn(at->v, at->len)
                                          : coerce_word(at->v, ps));
    }
    if (key_is_contract && arg_has_contract) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_CONTRACT_017 " 该实现暂不支持一个块应用多份契约（块级多契约写法）");
        ps->failed = 1;
        sml_free(args);
        return NULL;
    }
    next(ps);   /* `{`（预扫描已保证在此） */
    ps->enter_key = type_word;   /* 旁路表路径用块类型名当段 */
    sml_value *sub = parse_block(ps, T_RBRACE);
    sml_obj_set(sub, "__type", sml_new_str(type_word));
    if (sml_arr_len(args) > 0) {
        size_t i;
        sml_obj_set(sub, "__name", sml_clone(sml_arr_get(args, 0)));
        if (sml_arr_len(args) > 1) {
            sml_value *extra = sml_new_array();
            for (i = 1; i < sml_arr_len(args); i++)
                sml_arr_push(extra, sml_clone(sml_arr_get(args, i)));
            sml_obj_set(sub, "__args", extra);
        }
    }
    sml_free(args);
    return sub;
}

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

