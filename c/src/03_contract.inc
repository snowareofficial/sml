/* =====================================================================
** 3. 契约系统 (与 Rust sml-rs 对齐)
** ===================================================================== */

typedef enum {
    CT_ANY, CT_STR, CT_INT, CT_NUM, CT_BOOL, CT_ENUM, CT_ARRAY, CT_CONTRACTREF
} cty;

typedef struct cfield {
    char *name;
    cty ty;
    int required;            /* 默认 1; '?' 修饰符置 0 */
    sml_value *def;          /* 默认值 (own) 或 NULL */
    int min_set, max_set;
    double min, max;
    char **enum_vals; size_t enum_n;   /* CT_ENUM */
    char *ref_name;          /* CT_CONTRACTREF */
    cty arr_inner;           /* CT_ARRAY 元素类型 */
    /* S7：`from-featag` —— 该键只能由 featag 落点写入，文档直接赋值 ⇒ E-CONTRACT-024。 */
    int from_featag;
    struct cfield *next;
} cfield;

/* 落点的一段：键名或数组下标（`tls.cert` 两段键、`rules[0].name` 键+下标+键）。 */
typedef struct cpathseg {
    char *key;   /* is_idx == 0 时有效 */
    long idx;    /* is_idx == 1 时有效 */
    int is_idx;
} cpathseg;

/* featag 声明（S2–S5b）：名字 + 可选参数规格。
   `has_spec` 区分「无参纯标签」与「带参」—— 使用处的个数校验靠它（Rust 同判据：
   无参带参数 ⇒ 021，带参不写 `[…]` ⇒ 021）。`-> 落点` 属 S6，尚未接，故这里不存。 */
typedef struct cfeatag {
    char *name;
    int has_spec;
    cty ty;            /* 单参类型；CT_ARRAY 时看 arr_inner */
    cty arr_inner;
    /* S6：`-> 落点` 的段序列；`n_path == 0` 表示纯标签（不写任何键）。 */
    cpathseg *path;
    size_t n_path;
    /* S6：`auto` 取的就是这里（**声明自己的 default**，不是契约应用填的）。 */
    int has_def;
    sml_value *def;
    struct cfeatag *next;
} cfeatag;

typedef struct ccontract {
    char *name;
    int allow_extra;         /* loose */
    int strict_paths;        /* S6：`!strict-paths` 开关（落点父对象必须已存在） */
    int allow_any;           /* S8：`!any` 开关（放行 `any` 类型；默认禁用） */
    cfield *fields;          /* 头插链表 */
    cfeatag *featags;        /* 头插链表（featag 的作用域是契约私有） */
    struct ccontract *next;
} ccontract;

/* 一组待应用的契约（`@is A, B` / `@is A loose B` / 块头 `A B item { }`）。
   名字借用 token 串（词法表活到解析结束），故这里不释放。
   `ov` 是**块级严格度覆盖**（D4①）：-1 = 无覆盖（用契约级声明）/ 0 = loose / 1 = strict。
   `warn` 在本端按 `loose` 记：本端没有「放行 + 告警」通道，而 warn 与 loose 的**值树**
   逐字相同（Rust 只多一条诊断），故树对齐。 */
typedef struct {
    const char **names;
    int *ovs;
    size_t n, cap;
} ccontracts;

/* 追加一条；返回 0 成功、-1 失败（内存不足）。 */
static int ccontracts_push(ccontracts *l, const char *name, int ov) {
    if (l->n == l->cap) {
        size_t cap = l->cap ? l->cap * 2 : 4;
        const char **nn = (const char **)realloc(l->names, cap * sizeof(*nn));
        int *no;
        if (!nn) return -1;
        l->names = nn;
        no = (int *)realloc(l->ovs, cap * sizeof(*no));
        if (!no) return -1;
        l->ovs = no;
        l->cap = cap;
    }
    l->names[l->n] = name;
    l->ovs[l->n] = ov;
    l->n++;
    return 0;
}

/* 按名字查已有条目；未命中返回 -1。 */
static long ccontracts_find(const ccontracts *l, const char *name) {
    size_t i;
    for (i = 0; i < l->n; i++)
        if (strcmp(l->names[i], name) == 0) return (long)i;
    return -1;
}

static void ccontracts_free(ccontracts *l) {
    free(l->names);
    free(l->ovs);
    l->names = NULL;
    l->ovs = NULL;
    l->n = l->cap = 0;
}

/* 找 featag 声明；未声明返回 NULL（使用处据此报 E-CONTRACT-021）。 */
static cfeatag *contract_featag_find(const ccontract *c, const char *name) {
    cfeatag *f;
    if (!c) return NULL;
    for (f = c->featags; f; f = f->next)
        if (strcmp(f->name, name) == 0) return f;
    return NULL;
}

/* 一个参数值是否满足声明的类型（S5b）。
   只做标量这一层：枚举 / 区间 / 外置类型仍只在 Rust 侧校验 —— 四端既有的字段校验
   没有可复用的公共入口，硬塞进来会与 `check_field` 的两套判据分叉。 */
/* 把 `a.b[2].c` 解析成段序列（S6）。返回 0 成功；-1 表示写法非法。 */
static int cpath_parse(const char *raw, cpathseg **out, size_t *n_out) {
    cpathseg *segs = NULL;
    size_t n = 0, cap = 0;
    const char *p = raw;
    while (*p) {
        if (*p == '.') { p++; continue; }
        if (*p == '[') {
            const char *q = p + 1;
            long idx = 0;
            int digits = 0;
            while (*q >= '0' && *q <= '9') { idx = idx * 10 + (*q - '0'); q++; digits = 1; }
            if (!digits || *q != ']') goto bad;
            if (n == cap) {
                cap = cap ? cap * 2 : 4;
                segs = (cpathseg *)realloc(segs, cap * sizeof(*segs));
                if (!segs) return -1;
            }
            segs[n].is_idx = 1; segs[n].key = NULL; segs[n].idx = idx; n++;
            p = q + 1;
            continue;
        }
        {
            const char *q = p;
            while (*q && *q != '.' && *q != '[') q++;
            if (q == p) goto bad;
            if (n == cap) {
                cap = cap ? cap * 2 : 4;
                segs = (cpathseg *)realloc(segs, cap * sizeof(*segs));
                if (!segs) return -1;
            }
            segs[n].is_idx = 0;
            segs[n].idx = 0;
            segs[n].key = (char *)malloc((size_t)(q - p) + 1);
            if (!segs[n].key) goto bad;
            memcpy(segs[n].key, p, (size_t)(q - p));
            segs[n].key[q - p] = '\0';
            n++;
            p = q;
        }
    }
    *out = segs;
    *n_out = n;
    return 0;
bad:
    free(segs);
    return -1;
}

static int featag_arg_matches(cty ty, const sml_value *v) {
    if (!v) return 0;
    switch (ty) {
        case CT_STR:  return v->type == SML_STR;
        case CT_INT:  return v->type == SML_INT;
        case CT_NUM:  return v->type == SML_INT || v->type == SML_FLOAT;
        case CT_BOOL: return v->type == SML_BOOL;
        default:      return 1;   /* CT_ANY / 未写具体类型 */
    }
}

/* 内置契约表：宿主注入，供 `@weakbind` 软校验（与文档内 `@contract` 互不相干）。 */
struct sml_builtin {
    ccontract *contracts;
};

static cty type_from_name(const char *t) {
    if (!t) return CT_ANY;
    if (strcmp(t, "str") == 0) return CT_STR;
    if (strcmp(t, "int") == 0) return CT_INT;
    if (strcmp(t, "num") == 0) return CT_NUM;
    if (strcmp(t, "bool") == 0) return CT_BOOL;
    return CT_ANY;
}

sml_builtin *sml_builtin_new(void) {
    sml_builtin *b = (sml_builtin *)calloc(1, sizeof(sml_builtin));
    return b;
}

void sml_builtin_add_field(sml_builtin *b, const char *contract,
                           const char *field, const char *type, int required) {
    ccontract *c;
    cfield *f;
    if (!b || !contract || !field) return;
    for (c = b->contracts; c; c = c->next) {
        if (strcmp(c->name, contract) == 0) break;
    }
    if (!c) {
        c = (ccontract *)calloc(1, sizeof(ccontract));
        if (!c) return;
        c->name = strdup(contract);
        c->next = b->contracts;
        b->contracts = c;
    }
    f = (cfield *)calloc(1, sizeof(cfield));
    if (!f) return;
    f->name = strdup(field);
    f->ty = type_from_name(type);
    f->required = required ? 1 : 0;
    f->next = c->fields;
    c->fields = f;
}

static cfield *builtin_find_field(sml_builtin *b, const char *contract, const char *field) {
    ccontract *c;
    cfield *f;
    if (!b || !contract || !field) return NULL;
    for (c = b->contracts; c; c = c->next) {
        if (strcmp(c->name, contract) == 0) break;
    }
    if (!c) return NULL;
    for (f = c->fields; f; f = f->next) {
        if (strcmp(f->name, field) == 0) return f;
    }
    return NULL;
}

void sml_builtin_set_range(sml_builtin *b, const char *contract, const char *field,
                           int has_min, double min, int has_max, double max) {
    cfield *f = builtin_find_field(b, contract, field);
    if (!f) return;
    f->min_set = has_min ? 1 : 0;
    f->min = min;
    f->max_set = has_max ? 1 : 0;
    f->max = max;
}

void sml_builtin_add_enum(sml_builtin *b, const char *contract, const char *field,
                          const char *value) {
    cfield *f = builtin_find_field(b, contract, field);
    char **nv;
    if (!f || !value) return;
    nv = (char **)realloc(f->enum_vals, (f->enum_n + 1) * sizeof(char *));
    if (!nv) return;
    f->enum_vals = nv;
    f->enum_vals[f->enum_n++] = strdup(value);
    f->ty = CT_ENUM;
}

void sml_builtin_free(sml_builtin *b) {
    ccontract *c;
    if (!b) return;
    c = b->contracts;
    while (c) {
        ccontract *cn = c->next;
        cfield *f = c->fields;
        while (f) {
            cfield *fn = f->next;
            free(f->name);
            free(f->ref_name);
            if (f->enum_vals) {
                size_t k;
                for (k = 0; k < f->enum_n; k++) free(f->enum_vals[k]);
                free(f->enum_vals);
            }
            free(f);
            f = fn;
        }
        free(c->name);
        free(c);
        c = cn;
    }
    free(b);
}

static const char *kind_name(const sml_value *v) {
    switch (v->type) {
        case SML_NULL:   return "null";
        case SML_BOOL:   return "bool";
        case SML_INT:    return "int";
        case SML_FLOAT:  return "float";
        case SML_STR:    return "str";
        case SML_ARRAY:  return "array";
        case SML_OBJECT: return "object";
    }
    return "?";
}

static const char *type_name(cty t) {
    switch (t) {
        case CT_ANY:         return "any";
        case CT_STR:         return "str";
        case CT_INT:         return "int";
        case CT_NUM:         return "num";
        case CT_BOOL:        return "bool";
        case CT_ENUM:        return "enum";
        case CT_ARRAY:       return "array";
        case CT_CONTRACTREF: return "contract-ref";
    }
    return "?";
}

static int value_eq_str(const sml_value *v, const char *s) {
    if (!v) return 0;
    if (v->type == SML_STR) return strcmp(v->u.s, s) == 0;
    if (v->type == SML_INT) {
        char buf[32];
        snprintf(buf, sizeof buf, "%lld", v->u.i);
        return strcmp(buf, s) == 0;
    }
    return 0;
}

/* =====================================================================
** 解析 (递归下降)
** ===================================================================== */

/* 片段表条目 */
struct frag {
    char *name;
    sml_value *val;
    struct frag *next;
};

typedef struct {
    lexer *lx;
    struct frag *frags;
    ccontract *contracts;   /* 全局契约表 (跨块可见) */
    int failed;             /* 契约校验失败标志 (不依赖 errbuf 内容) */
    int version;            /* 语法版本: 1=V1(裸词即字符串) 2=V2 3=V3(字符串须引号) */
    int depth;              /* 当前块/数组嵌套深度（栈溢出防护，见 parse_block） */
    unsigned features;      /* `@feature enable/disable` 记录的开关位；0 = 全关（C 端暂无版本基线） */
    /* `@when` 的挂起条件：0 = 无，1 = 保留下一项，2 = 丢弃下一项。
       只作用于紧邻的下一个字段/块，消费后立即清 0。
       Pending `@when`: 0 = none, 1 = keep next item, 2 = drop next item. */
    int pending_when;
    /* `@for` 的循环变量绑定（展开时临时设置，展开后恢复）。
       `@for` loop-variable binding: set during expansion, restored afterwards. */
    const char *loop_var;
    const char *loop_val;
    /* 诊断（warning）收集：只有 `sml_parse_ex` 传了出口才开启（默认零开销）。
       Diagnostics sink: enabled only when `sml_parse_ex` asks for it. */
    int want_diags;
    sml_diag *diags;
    size_t n_diags;
    size_t cap_diags;
    /* 弱绑定：宿主注入的内置契约表 + 「`@weakbind` 作用于下一个块」的挂起名。 */
    const sml_builtin *builtin;
    const char *pending_weak;
    /* 挂起项所在层（`ps->depth`）：块收尾时据此判断「是本块内的悬空项」还是
       「外层留给下一个兄弟块的」。Depth at which the pending bind was declared. */
    int pending_weak_depth;
    int pending_weak_line;
    /* 旁路表：`@weakbind` 的记录（不进主树）。Weak-binding side table. */
    sml_weakbind *binds;
    size_t n_binds, cap_binds;
    /* 当前块的路径栈（键名；数组元素段写成 `[n]`）。
       上层的 `enter_key` 由调用方在 parse_block 之前设置，进入时压栈、退出时弹栈。
       Path stack: callers set `enter_key` right before parse_block. */
    const char *enter_key;
    char **path;
    size_t path_n, path_cap;
    /* 当前字段**键 token** 的行号：重复键诊断要用它，不能用「下一个 token」
       （那个 token 已在下一行甚至越过块尾，报出来的行号是错的）。
       Line of the key token currently being parsed. */
    int key_line;
} parser;

/* 值嵌套深度上限：与 Rust 侧 MAX_VALUE_DEPTH 同口径。
   `a{a{a{ ... }}}` 这类输入会让递归下降一路压栈；栈溢出在 C 里是段错误，
   **无法被错误处理接住**，只能在递归入口主动限深。 */
#define SML_MAX_VALUE_DEPTH 128

/* 特性名表：与 Rust 的 `FEATURES` 同序同名（跨端契约，见
   `rust/sml-feature/src/lib.rs`）。C 端目前**只识别、不门控** ——
   位记录下来供后续 when / for / typed-block 使用，现有语法的门控随 P3 逐项接。 */
static const char *const SML_FEATURE_NAMES[] = {
    "bareword-string", "include", "env", "contract", "fragment",
    "top-level-array", "namespace", "implicit-ns", "multi-include",
    "glob-include", "regex-include", "ext-rewrite", "when", "for",
    "typed-block", "case-insensitive", "block-name", "type.featag",
    "lex.ident", "type.weak",
};

static int feature_index(const char *name) {
    size_t i;
    for (i = 0; i < sizeof(SML_FEATURE_NAMES) / sizeof(SML_FEATURE_NAMES[0]); i++) {
        if (strcmp(SML_FEATURE_NAMES[i], name) == 0) return (int)i;
    }
    return -1;
}

/* 版本基线：与 Rust `FeatureSet::baseline()` 一致的 8 位核心特性
   （bareword-string / include / env / contract / fragment / top-level-array /
   namespace / implicit-ns）。位序见 SML_FEATURE_NAMES。 */
#define SML_FEATURE_BASELINE \
    ((1u << 0) | (1u << 1) | (1u << 2) | (1u << 3) | \
     (1u << 4) | (1u << 5) | (1u << 6) | (1u << 7))

/* 预处理：扫行首的 `@feature enable|disable <名>[, <名>]`，算出初始特性集。
   必须在词法前调用 —— 引号串 `$env.` 内插、顶层数组形态判定都要看到最终特性集
   （与 Rust `sml-parse/src/scan.rs` 的 `strip_features` 同一定位）。
   解析主循环仍会再处理 `@feature`（幂等：重复置同一位结果不变）。
   简化：只看行首（前导空白后）；字符串内部的 `@feature` 会被误判，属已知边界。 */
static unsigned scan_features(const char *text) {
    unsigned feats = SML_FEATURE_BASELINE;
    const char *p = text;
    while (*p) {
        const char *line = p;
        const char *nl = strchr(p, '\n');
        size_t len = nl ? (size_t)(nl - p) : strlen(p);
        size_t i = 0;
        while (i < len && (line[i] == ' ' || line[i] == '\t')) i++;
        if (i + 8 <= len && strncmp(line + i, "@feature", 8) == 0
            && (i + 8 == len || line[i + 8] == ' ' || line[i + 8] == '\t')) {
            const char *q = line + i + 8;
            const char *end = line + len;
            int on = -1;
            while (q < end && (*q == ' ' || *q == '\t')) q++;
            if (end - q >= 6 && strncmp(q, "enable", 6) == 0) { on = 1; q += 6; }
            else if (end - q >= 7 && strncmp(q, "disable", 7) == 0) { on = 0; q += 7; }
            if (on >= 0) {
                for (;;) {
                    char name[64];
                    size_t nlen = 0;
                    int bit;
                    while (q < end && (*q == ' ' || *q == '\t' || *q == ',')) q++;
                    if (q >= end) break;
                    while (q < end && *q != ' ' && *q != '\t' && *q != ','
                           && nlen + 1 < sizeof(name)) {
                        name[nlen++] = *q++;
                    }
                    name[nlen] = '\0';
                    if (nlen == 0) break;
                    bit = feature_index(name);
                    if (bit < 0) break;   /* 未知名字留给解析层报 E-FEATURE-002 */
                    if (on) feats |= (1u << bit);
                    else    feats &= ~(1u << bit);
                }
            }
        }
        if (!nl) break;
        p = nl + 1;
    }
    return feats;
}

static sml_value *parse_block(parser *ps, tok_type closing);
static sml_value *parse_block_inner(parser *ps, tok_type closing);
static sml_value *parse_array(parser *ps);
/* 旁路表的路径栈（定义在 diag_free_all 之后，parse_block 先用到）。 */
static void path_push(parser *ps, const char *seg);
static void path_pop(parser *ps);
static sml_value *parse_array_inner(parser *ps);
/* 游标原语（定义在下方）：`parse_for` 需要提前可见。 */
static token *peek(parser *ps);
static token *peek_at(parser *ps, size_t off);
static token *next(parser *ps);

/* `@for var in a b c { ... }` —— 有界循环展开，返回数组。
   与 Rust `cond.rs::eval_for_header` + `Parser::parse_for` 同语义：
   只遍历有限枚举、变量只读、无 `while` / 无递归。
   Bounded loop only: finite enumeration, read-only variable, no recursion.
   ⚠️ 子 token 流通过**临时替换** lx->toks / n / pos 实现；所有出口都经 `done`
   统一恢复，避免失败路径把后续文档留在错的流上。 */
static sml_value *parse_for(parser *ps) {
    lexer *lx = ps->lx;
    token *saved_toks = lx->toks;
    size_t saved_n = lx->n, saved_pos = lx->pos;
    const char *saved_var = ps->loop_var, *saved_val = ps->loop_val;
    sml_value *arr = NULL;
    const char *var = NULL;
    const char **items = NULL;
    size_t n_items = 0, cap = 8, i, start = 0, end = 0, j;
    int depth_b = 1;

    next(ps); next(ps);   /* @ for */
    if (!peek(ps) || peek(ps)->t != T_WORD) {
        set_err(lx->errbuf, lx->errsz, SML_E_PARSE_018 " `@for` 缺少循环变量");
        ps->failed = 1;
        goto done;
    }
    var = peek(ps)->v;
    next(ps);
    if (!peek(ps) || peek(ps)->t != T_WORD || strcmp(peek(ps)->v, "in") != 0) {
        set_err(lx->errbuf, lx->errsz, SML_E_PARSE_018 " `@for` 缺少 `in`");
        ps->failed = 1;
        goto done;
    }
    next(ps);
    items = (const char **)malloc(cap * sizeof(char *));
    if (!items) { ps->failed = 1; goto done; }
    while (peek(ps) && peek(ps)->t == T_WORD) {
        if (n_items == cap) {
            const char **ni;
            cap *= 2;
            ni = (const char **)realloc(items, cap * sizeof(char *));
            if (!ni) { ps->failed = 1; goto done; }
            items = ni;
        }
        items[n_items++] = peek(ps)->v;
        next(ps);
    }
    if (n_items == 0) {
        set_err(lx->errbuf, lx->errsz,
                SML_E_PARSE_018 " `@for` 的 `in` 之后至少须有一个枚举项");
        ps->failed = 1;
        goto done;
    }
    if (!peek(ps) || peek(ps)->t != T_LBRACE) {
        set_err(lx->errbuf, lx->errsz, SML_E_PARSE_018 " `@for` 缺少循环体 `{ ... }`");
        ps->failed = 1;
        goto done;
    }
    start = lx->pos + 1;
    j = start;
    while (j < lx->n) {
        if (lx->toks[j].t == T_LBRACE) depth_b++;
        else if (lx->toks[j].t == T_RBRACE) { depth_b--; if (depth_b == 0) break; }
        j++;
    }
    if (depth_b != 0) {
        set_err(lx->errbuf, lx->errsz,
                SML_E_PARSE_001 " 未闭合的块（遇到文件结尾，缺少结束符号 }）");
        ps->failed = 1;
        goto done;
    }
    end = j;
    arr = sml_new_array();
    if (!arr) { ps->failed = 1; goto done; }
    for (i = 0; i < n_items && !ps->failed; i++) {
        size_t len = end - start;
        token *sub = (token *)calloc(len + 1, sizeof(token));
        sml_value *node;
        if (!sub) { ps->failed = 1; break; }
        memcpy(sub, lx->toks + start, len * sizeof(token));
        sub[len].t = T_EOF;
        sub[len].v = NULL;
        lx->toks = sub;
        lx->n = len + 1;
        lx->pos = 0;
        ps->loop_var = var;
        ps->loop_val = items[i];
        node = parse_block(ps, T_EOF);
        lx->toks = saved_toks;
        lx->n = saved_n;
        free(sub);
        if (ps->failed || !node) {
            if (node) sml_free(node);
            break;
        }
        sml_arr_push(arr, node);
    }

done:
    lx->toks = saved_toks;
    lx->n = saved_n;
    ps->loop_var = saved_var;
    ps->loop_val = saved_val;
    free(items);
    if (ps->failed) {
        if (arr) sml_free(arr);
        lx->pos = saved_pos;
        return NULL;
    }
    lx->pos = end + 1;
    return arr;
}

/* parse_block 的守卫 wrapper：真正的实现在 parse_block_inner。
   包一层而不是改每个 return 点，是为了不遗漏任何出口（错误路径同样要收尾）。
   真正让递归**收手**的不是这一层的返回值，而是 ps->failed + 各解析循环的 break。 */
static sml_value *parse_block(parser *ps, tok_type closing) {
    /* 口径（各端一致，**已实测钉住**）：`ps->depth` 是「进入本块**前**已进入的层数」
       （根块为 0），故允许 `depth == SML_MAX_VALUE_DEPTH` —— 第 128 层块放行、
       第 129 层报此码。
       ⚠️ 这里原先是 `>=`：128 层就被拒，而下面的文案写的是「**超过** 128 层」，
       行为与文案自相矛盾；且与 C++ / JS / Lua 实测边界（128 放行 / 129 报）差一格。
       改动前用闭合嵌套 `("a { "):rep(N) + ("} "):rep(N)` 实测量过五端：
       Rust 127 / C 127 / C++ 128 / JS 128 / Lua 128。
       本文件有**两处**同一守卫、共用 `ps->depth`：`parse_block` 与 `parse_array`
       （后者是 W17 补的 —— 嵌套数组递归落地时**必须同时**给闸，理由见那里的注释）。
       Rust 侧同样块与数组共用一个 `depth` 计数器，故两处口径一致。 */
    if (ps->depth > SML_MAX_VALUE_DEPTH) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_LIMIT_001 " 嵌套过深（超过 %d 层），疑似递归或恶意输入",
                SML_MAX_VALUE_DEPTH);
        ps->failed = 1;
        /* ⚠️ 这里**不能**只把 depth 复位为 0 就了事（最初就是这么写的，实测仍崩）：
           复位不会让已经压上去的栈帧退回来，外层循环随即又从 0 开始往下钻，于是
           「每 128 层一轮」反复压栈，10 万层块嵌套照样打穿栈。真正收手靠 ps->failed：
           parse_array / parse_block_inner 的循环见到它就 break，栈才会一层层退掉。 */
        return sml_new_null();
    }
    ps->depth++;
    /* 路径栈：调用方在调本函数前把本块的键名放进 `enter_key`（顶层为 NULL）。
       Path stack: the caller stores this block's key in `enter_key` first. */
    {
        const char *k = ps->enter_key;
        ps->enter_key = NULL;
        path_push(ps, k);
    }
    sml_value *v = parse_block_inner(ps, closing);
    path_pop(ps);
    if (ps->depth > 0) ps->depth--;
    return v;
}

/* 未闭合的块/数组/契约体（遇到文件结尾就收尾了）：E-PARSE-001。
   与 Rust 同码：rust/sml-parse/src/parser.rs 在「本层期望闭合符号（closing 非 None）
   却遇到文件结尾」时报 E_PARSE_001，契约体未闭合也归同一条。
   注：顶层（closing==T_EOF）遇到文件结尾是**正常**结束，不算未闭合，不报。 */
static void err_unclosed(parser *ps, const char *what) {
    if (ps->failed) return;   /* **第一条错误为准**：别覆盖已经写在 err 里的码 */
    set_err(ps->lx->errbuf, ps->lx->errsz,
            SML_E_PARSE_001 " 未闭合的%s（遇到文件结尾，缺少结束符号）", what);
    ps->failed = 1;
}

static token *peek(parser *ps) {
    return &ps->lx->toks[ps->lx->pos];
}

static token *peek_at(parser *ps, size_t off) {
    size_t idx = ps->lx->pos + off;
    if (idx >= ps->lx->n) return &ps->lx->toks[ps->lx->n - 1];
    return &ps->lx->toks[idx];
}

static token *next(parser *ps) {
    token *t = &ps->lx->toks[ps->lx->pos];
    if (t->t != T_EOF) ps->lx->pos++;
    return t;
}

