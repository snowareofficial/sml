/* =====================================================================
 * 2b. 模式语言（`@type name: X { … }`）—— 无回溯 L1 匹配器
 * =====================================================================
 *
 * 移植自 `rust/sml-pattern`（那里是权威实现，本文件逐条对齐）：
 *
 *   - 规则就是**普通 SML 数据**（序列 / 类 / 次 / 任一 …），不新增 token；
 *   - 编译成 Thompson NFA，模拟时所有线程并行推进 ⇒ **结构上不存在回溯**，
 *     因此没有 ReDoS，最坏时间 O(文本长 × 程序长)；
 *   - 字符类按 **Unicode** 判定（`数字` 不止 0-9，全角与罗马数字也算）——
 *     三端查的是同一张由 Rust 导出的区间表（`tools/gen/gen_unicode_tables.py`），
 *     否则「同一份文档一端通过、另一端报 009」这类差异最难查。
 *
 * 对外只有三个函数（无状态，不依赖 parser）：
 *   `sp_compile` 编译一条规则（`rules` 供 `用:` 引用其它类型）
 *   `sp_match`   全文匹配（不是搜索）
 *   `sp_free`    释放
 * ===================================================================== */

#include "../sml_pattern_unicode.h"

/* ---- 字符类 ---------------------------------------------------------- */

typedef enum {
    SPC_DIGIT, SPC_ALPHA, SPC_SPACE, SPC_WORD, SPC_ANY,
    SPC_ONE, SPC_RANGE, SPC_SET, SPC_NOT
} sp_class_kind;

typedef struct sp_class {
    sp_class_kind kind;
    unsigned long ch1, ch2;      /* ONE / RANGE */
    struct sp_class **set;       /* SET / NOT 的子类 */
    size_t n;
} sp_class;

/* ---- 模式 IR --------------------------------------------------------- */

typedef enum { SP_CLASS, SP_LIT, SP_SEQ, SP_ALT, SP_REPEAT, SP_NAMED, SP_UNTIL } sp_pat_kind;

typedef struct sp_pat {
    sp_pat_kind kind;
    /* SP_CLASS */
    sp_class *cls;
    /* SP_LIT */
    char *lit;
    size_t lit_len;
    /* SP_SEQ / SP_ALT */
    struct sp_pat **items;
    size_t n;
    /* SP_REPEAT / SP_NAMED / SP_UNTIL 的子节点 */
    struct sp_pat *sub;
    size_t min;
    long max;                    /* < 0 表示无上界（`+` / `*`） */
} sp_pat;

/* 单量词上限与程序规模上限：与 Rust `QUANT_MAX` / `MAX_PROG` 同数。
   前者挡「次: 999999999」，后者挡嵌套量词的相乘膨胀（`{1000}` 套 `{1000}`）。 */
#define SP_QUANT_MAX 1000
#define SP_MAX_PROG  100000
/* 匹配步数预算（Rust `DEFAULT_BUDGET`）：无回溯已保证多项式，这只是兜底。 */
#define SP_BUDGET 10000000L

/* ---- 关键字表（中英等价，与 Rust `BILINGUAL` 逐条同序）---------------- */

typedef enum {
    KC_NONE, KC_LIT, KC_CLASS, KC_ALT, KC_GROUP, KC_SEQ, KC_USE, KC_UNTIL,
    KC_TIMES, KC_OPTIONAL, KC_NAME, KC_REGEX, KC_MIN, KC_MAX,
    KC_DIGIT, KC_ALPHA, KC_SPACE, KC_WORD, KC_ANY, KC_EOL
} sp_concept;

typedef struct { const char *word; sp_concept c; } sp_kw;

static const sp_kw SP_KEYWORDS[] = {
    /* 中文 */
    {"字面", KC_LIT}, {"类", KC_CLASS}, {"任一", KC_ALT}, {"组", KC_GROUP},
    {"序列", KC_SEQ}, {"用", KC_USE}, {"直到", KC_UNTIL}, {"次", KC_TIMES},
    {"可选", KC_OPTIONAL}, {"名", KC_NAME}, {"正则", KC_REGEX},
    {"最小", KC_MIN}, {"最大", KC_MAX},
    {"数字", KC_DIGIT}, {"字母", KC_ALPHA}, {"空白", KC_SPACE},
    {"字", KC_WORD}, {"任意", KC_ANY}, {"行尾", KC_EOL}, {"末尾", KC_EOL},
    /* 英文 */
    {"lit", KC_LIT}, {"literal", KC_LIT}, {"class", KC_CLASS}, {"alt", KC_ALT},
    {"any-of", KC_ALT}, {"group", KC_GROUP}, {"seq", KC_SEQ}, {"use", KC_USE},
    {"until", KC_UNTIL}, {"times", KC_TIMES}, {"repeat", KC_TIMES},
    {"optional", KC_OPTIONAL}, {"name", KC_NAME}, {"regex", KC_REGEX},
    {"re", KC_REGEX}, {"min", KC_MIN}, {"max", KC_MAX},
    {"digit", KC_DIGIT}, {"alpha", KC_ALPHA}, {"space", KC_SPACE},
    {"word", KC_WORD}, {"any", KC_ANY}, {"eol", KC_EOL}, {"end", KC_EOL},
};

/* 线性扫描（表只有几十条）；同一个词重复出现时**先到先得**（Rust 同）。 */
static sp_concept sp_lookup(const char *w) {
    size_t i;
    for (i = 0; i < sizeof(SP_KEYWORDS) / sizeof(SP_KEYWORDS[0]); i++)
        if (strcmp(SP_KEYWORDS[i].word, w) == 0) return SP_KEYWORDS[i].c;
    return KC_NONE;
}

/* ---- UTF-8 解码 ------------------------------------------------------
   非法字节按「单字节码位」处理（Rust 侧 `&str` 保证合法 UTF-8，走不到这里）；
   这样解码永不失败、也永不越界，匹配函数不需要错误出口。 */
static unsigned long sp_next_cp(const char *s, size_t len, size_t *pos) {
    unsigned char b0 = (unsigned char)s[*pos];
    unsigned long cp;
    size_t need, i;
    if (b0 < 0x80) { (*pos)++; return b0; }
    if ((b0 & 0xE0) == 0xC0) { cp = b0 & 0x1F; need = 1; }
    else if ((b0 & 0xF0) == 0xE0) { cp = b0 & 0x0F; need = 2; }
    else if ((b0 & 0xF8) == 0xF0) { cp = b0 & 0x07; need = 3; }
    else { (*pos)++; return b0; }
    /* 续字节不够就越界（`*pos + need` 是最后一个续字节的下标，必须 < len） */
    if (*pos + need >= len) { (*pos)++; return b0; }
    for (i = 1; i <= need; i++) {
        unsigned char b = (unsigned char)s[*pos + i];
        if ((b & 0xC0) != 0x80) { (*pos)++; return b0; }
        cp = (cp << 6) | (b & 0x3F);
    }
    *pos += need + 1;
    return cp;
}

/* ---- 字符类判定 ------------------------------------------------------ */

static int sp_cp_numeric(unsigned long cp) {
    return sml_pat_cp_in((unsigned int)cp, SML_PAT_NUMERIC, SML_PAT_NUMERIC_N);
}
static int sp_cp_alpha(unsigned long cp) {
    return sml_pat_cp_in((unsigned int)cp, SML_PAT_ALPHABETIC, SML_PAT_ALPHABETIC_N);
}
static int sp_cp_space(unsigned long cp) {
    return sml_pat_cp_in((unsigned int)cp, SML_PAT_WHITESPACE, SML_PAT_WHITESPACE_N);
}

static int sp_class_matches(const sp_class *c, unsigned long cp) {
    size_t i;
    switch (c->kind) {
        case SPC_DIGIT: return sp_cp_numeric(cp);
        case SPC_ALPHA: return sp_cp_alpha(cp);
        case SPC_SPACE: return sp_cp_space(cp);
        /* Word = 字母数字 + 下划线（不含 `-`），与 Rust 同口径 */
        case SPC_WORD:  return sp_cp_numeric(cp) || sp_cp_alpha(cp) || cp == '_';
        case SPC_ANY:   return 1;
        case SPC_ONE:   return cp == c->ch1;
        case SPC_RANGE: return cp >= c->ch1 && cp <= c->ch2;
        case SPC_SET:
            for (i = 0; i < c->n; i++) if (sp_class_matches(c->set[i], cp)) return 1;
            return 0;
        case SPC_NOT:   return !sp_class_matches(c->set[0], cp);
    }
    return 0;
}

/* ---- 内存 ------------------------------------------------------------ */

static sp_class *sp_class_new(sp_class_kind k) {
    sp_class *c = (sp_class *)calloc(1, sizeof(sp_class));
    if (c) c->kind = k;
    return c;
}

static void sp_class_free(sp_class *c) {
    size_t i;
    if (!c) return;
    for (i = 0; i < c->n; i++) sp_class_free(c->set[i]);
    free(c->set);
    free(c);
}

static sp_pat *sp_pat_new(sp_pat_kind k) {
    sp_pat *p = (sp_pat *)calloc(1, sizeof(sp_pat));
    if (p) { p->kind = k; p->max = -1; }
    return p;
}

static void sp_free(sp_pat *p) {
    size_t i;
    if (!p) return;
    free(p->lit);
    for (i = 0; i < p->n; i++) sp_free(p->items[i]);
    free(p->items);
    sp_free(p->sub);
    sp_class_free(p->cls);
    free(p);
}

/* 追加子节点（失败返回 -1）。 */
static int sp_push_pat(sp_pat ***arr, size_t *n, size_t *cap, sp_pat *child) {
    if (*n == *cap) {
        size_t nc = *cap ? *cap * 2 : 4;
        sp_pat **na = (sp_pat **)realloc(*arr, nc * sizeof(sp_pat *));
        if (!na) return -1;
        *arr = na;
        *cap = nc;
    }
    (*arr)[(*n)++] = child;
    return 0;
}

static void sp_err(char *err, size_t errsz, const char *fmt, ...) {
    va_list ap;
    if (!err || errsz == 0) return;
    va_start(ap, fmt);
    vsnprintf(err, errsz, fmt, ap);
    va_end(ap);
}

/* ---- 取值辅助 -------------------------------------------------------- */

static const sml_value *sp_obj_get(const sml_value *o, const char *k) {
    return o && o->type == SML_OBJECT ? sml_obj_get(o, k) : NULL;
}

static const char *sp_type_name(const sml_value *v) {
    if (!v) return "缺失";
    switch (v->type) {
        case SML_STR:    return "字符串";
        case SML_INT:    return "整数";
        case SML_FLOAT:  return "浮点数";
        case SML_BOOL:   return "布尔";
        case SML_NULL:   return "null";
        case SML_ARRAY:  return "数组";
        case SML_OBJECT: return "对象";
    }
    return "?";
}

/* ---- 编译：SML 值 → 模式 IR ------------------------------------------
   `rules` 是「类型名 → 规则体」的对象（`@type` 表），供 `用:` 引用；
   `stack` 记录引用链，用来检出循环引用（L1 不支持递归）。 */
static sp_pat *sp_compile_value(const sml_value *v, const sml_value *rules,
                                const char **stack, size_t stack_n,
                                char *err, size_t errsz);

/* 字符类取值 → Class。非内置类按**单字符**处理（`类: "-"`）。 */
static sp_class *sp_parse_class(const char *s, char *err, size_t errsz) {
    sp_class *c;
    switch (sp_lookup(s)) {
        case KC_DIGIT: return sp_class_new(SPC_DIGIT);
        case KC_ALPHA: return sp_class_new(SPC_ALPHA);
        case KC_SPACE: return sp_class_new(SPC_SPACE);
        case KC_WORD:  return sp_class_new(SPC_WORD);
        case KC_ANY:   return sp_class_new(SPC_ANY);
        default: break;
    }
    /* 非内置类：单个字符按字面 */
    {
        size_t pos = 0, len = strlen(s);
        unsigned long cp1 = sp_next_cp(s, len, &pos);
        if (pos == len) {
            c = sp_class_new(SPC_ONE);
            if (c) c->ch1 = cp1;
            return c;
        }
    }
    sp_err(err, errsz, "sml: 未知字符类 `%s`", s);
    return NULL;
}

/* 量词：`次` 的取值（整数 / `+` `*` `?` / `a-b` / `{最小,最大}` 对象）。 */
static int sp_parse_times(const sml_value *v, size_t *min, long *max,
                          char *err, size_t errsz) {
    long mn = 0, mx = -1;
    int have_max = 0;
    if (v->type == SML_INT) {
        /* `次: 3` ⇒ 恰好 3 次。⚠️ 不能在这里提前 return —— 否则绕过下面的
           QUANT_MAX 闸门，`次: 5000` 会被当成合法量词（Rust 在同一处统一校验）。 */
        if (v->u.i < 0) { sp_err(err, errsz, "sml: 量词最小不能为负（得 %lld）", v->u.i); return -1; }
        mn = (long)v->u.i;
        mx = (long)v->u.i;
        have_max = 1;
    } else if (v->type == SML_STR) {
        const char *s = v->u.s;
        if (strcmp(s, "+") == 0) { *min = 1; *max = -1; return 0; }
        if (strcmp(s, "*") == 0) { *min = 0; *max = -1; return 0; }
        if (strcmp(s, "?") == 0) { *min = 0; *max = 1; return 0; }
        {
            const char *dash = strchr(s, '-');
            if (!dash) {
                sp_err(err, errsz, "sml: 未知量词 `%s`（可用 数字 / + / * / ? / a-b）", s);
                return -1;
            }
            mn = strtol(s, NULL, 10);
            mx = strtol(dash + 1, NULL, 10);
            have_max = 1;
        }
    } else if (v->type == SML_OBJECT) {
        const sml_field *fd;
        for (fd = v->u.obj.head; fd; fd = fd->next) {
            sp_concept kc = sp_lookup(fd->key);
            if (kc == KC_MIN) {
                if (fd->value->type != SML_INT) {
                    sp_err(err, errsz, "sml: 量词最小必须为整数");
                    return -1;
                }
                mn = (long)fd->value->u.i;
            } else if (kc == KC_MAX) {
                if (fd->value->type != SML_INT) {
                    sp_err(err, errsz, "sml: 量词最大必须为整数");
                    return -1;
                }
                mx = (long)fd->value->u.i;
                have_max = 1;
            }
        }
    } else {
        sp_err(err, errsz, "sml: 量词类型不支持: %s", sp_type_name(v));
        return -1;
    }
    if (mn < 0) { sp_err(err, errsz, "sml: 量词最小不能为负（得 %ld）", mn); return -1; }
    if (have_max && mx < mn) {
        sp_err(err, errsz, "sml: 量词最大(%ld)不能小于最小(%ld)", mx, mn);
        return -1;
    }
    if (mn > SP_QUANT_MAX || (have_max && mx > SP_QUANT_MAX)) {
        sp_err(err, errsz,
               "sml: 量词过大（> %d），拒绝编译（防病态规则；嵌套膨胀另有程序规模上限）",
               SP_QUANT_MAX);
        return -1;
    }
    *min = (size_t)mn;
    *max = have_max ? mx : -1;
    return 0;
}

/* ---- regex 逃生舱：把 regex 子集编译进同一个 IR ----------------------
 *
 * 与 Rust `sml-pattern/src/regex.rs` 逐条对齐：`正则/regex/re` 只是模式的
 * 另一种**前端**，解析后产出同一棵 `sp_pat`，仍交给无回溯 NFA 执行 ——
 * 因此逃生舱不逃掉线性时间保证。明确**拒绝**（报错，不静默降级）：
 *   - 反向引用 `\1`…`\9`（与自动机模型冲突，NP 完全）
 *   - lookaround / 其它 `(?...)` 扩展（破坏线性时间与流式处理）
 * 锚点 `^` `$` 是**隐含**的（全文匹配），出现时直接跳过。
 * ---------------------------------------------------------------------- */

typedef struct {
    unsigned long *cp;   /* 已解码的码位序列 */
    size_t n, i;
    char *err;
    size_t errsz;
} sp_rx;

static sp_pat *sp_rx_alt(sp_rx *p);

static unsigned long sp_rx_peek(const sp_rx *p) {
    return p->i < p->n ? p->cp[p->i] : 0UL;   /* 0 = EOF（C 字符串不含内嵌 NUL） */
}

static sp_class *sp_class_one(unsigned long cp) {
    sp_class *c = sp_class_new(SPC_ONE);
    if (c) c->ch1 = cp;
    return c;
}

static sp_class *sp_class_not(sp_class *inner) {
    sp_class *c;
    if (!inner) return NULL;
    c = sp_class_new(SPC_NOT);
    if (!c) { sp_class_free(inner); return NULL; }
    c->set = (sp_class **)malloc(sizeof(sp_class *));
    if (!c->set) { sp_class_free(inner); sp_class_free(c); return NULL; }
    c->set[0] = inner;
    c->n = 1;
    return c;
}

static sp_pat *sp_pat_from_class(sp_class *cl, char *err, size_t errsz) {
    sp_pat *pat;
    if (!cl) { sp_err(err, errsz, "sml: 内存不足"); return NULL; }
    pat = sp_pat_new(SP_CLASS);
    if (!pat) { sp_class_free(cl); sp_err(err, errsz, "sml: 内存不足"); return NULL; }
    pat->cls = cl;
    return pat;
}

/* 把码位编码成 UTF-8，供错误消息里显示那个无法解析的字符。 */
static int sp_utf8_encode(unsigned long cp, char *out) {
    if (cp < 0x80) { out[0] = (char)cp; return 1; }
    if (cp < 0x800) {
        out[0] = (char)(0xC0 | (cp >> 6));
        out[1] = (char)(0x80 | (cp & 0x3F));
        return 2;
    }
    if (cp < 0x10000) {
        out[0] = (char)(0xE0 | (cp >> 12));
        out[1] = (char)(0x80 | ((cp >> 6) & 0x3F));
        out[2] = (char)(0x80 | (cp & 0x3F));
        return 3;
    }
    out[0] = (char)(0xF0 | (cp >> 18));
    out[1] = (char)(0x80 | ((cp >> 12) & 0x3F));
    out[2] = (char)(0x80 | ((cp >> 6) & 0x3F));
    out[3] = (char)(0x80 | (cp & 0x3F));
    return 4;
}

/* 字符类内的一个成员：`\d` / `\w` / `\s`、`a-z` 范围、或单字符。 */
static sp_class *sp_rx_class_item(sp_rx *p) {
    unsigned long c;
    if (p->i >= p->n) { sp_err(p->err, p->errsz, "sml: 字符类意外结束"); return NULL; }
    c = p->cp[p->i];
    if (c == '\\') {
        unsigned long e;
        p->i++;
        if (p->i >= p->n) { sp_err(p->err, p->errsz, "sml: `\\` 后缺少字符"); return NULL; }
        e = p->cp[p->i++];
        switch (e) {
            case 'd': return sp_class_new(SPC_DIGIT);
            case 'w': return sp_class_new(SPC_WORD);
            case 's': return sp_class_new(SPC_SPACE);
            default:  return sp_class_one(e);
        }
    }
    p->i++;
    /* 范围 a-z（`-` 紧邻 `]` 时按字面 `-`，与 Rust 同） */
    if (sp_rx_peek(p) == '-' && p->i + 1 < p->n && p->cp[p->i + 1] != ']') {
        unsigned long hi;
        sp_class *r;
        p->i++;
        hi = p->cp[p->i++];
        r = sp_class_new(SPC_RANGE);
        if (r) { r->ch1 = c; r->ch2 = hi; }
        return r;
    }
    return sp_class_one(c);
}

/* `[ ... ]` / `[^ ... ]`。单成员直接那个类，多成员 SPC_SET（并集）。 */
static sp_class *sp_rx_class(sp_rx *p) {
    int negated = 0;
    sp_class **items = NULL;
    size_t ni = 0, ci = 0;
    sp_class *first;

    if (sp_rx_peek(p) != '[') { sp_err(p->err, p->errsz, "sml: 期望 `[`"); return NULL; }
    p->i++;
    if (sp_rx_peek(p) == '^') { negated = 1; p->i++; }

    for (;;) {
        unsigned long c = sp_rx_peek(p);
        if (p->i >= p->n) {
            size_t k;
            for (k = 0; k < ni; k++) sp_class_free(items[k]);
            free(items);
            sp_err(p->err, p->errsz, "sml: 字符类缺少 `]`");
            return NULL;
        }
        if (c == ']') { p->i++; break; }
        {
            sp_class *it = sp_rx_class_item(p);
            if (!it) {
                size_t k;
                for (k = 0; k < ni; k++) sp_class_free(items[k]);
                free(items);
                return NULL;
            }
            if (ni == ci) {
                size_t nc = ci ? ci * 2 : 4;
                sp_class **na = (sp_class **)realloc(items, nc * sizeof(sp_class *));
                if (!na) {
                    size_t k;
                    sp_class_free(it);
                    for (k = 0; k < ni; k++) sp_class_free(items[k]);
                    free(items);
                    sp_err(p->err, p->errsz, "sml: 内存不足");
                    return NULL;
                }
                items = na;
                ci = nc;
            }
            items[ni++] = it;
        }
    }

    if (ni == 1) {
        first = items[0];
        free(items);
    } else {
        first = sp_class_new(SPC_SET);
        if (!first) {
            size_t k;
            for (k = 0; k < ni; k++) sp_class_free(items[k]);
            free(items);
            sp_err(p->err, p->errsz, "sml: 内存不足");
            return NULL;
        }
        first->set = items;   /* 所有权转移给 SET */
        first->n = ni;
    }
    if (negated) return sp_class_not(first);
    return first;
}

/* `\d` `\D` `\w` `\W` `\s` `\S`；反向引用 `\1`…`\9` 拒绝；其余按字面。 */
static sp_pat *sp_rx_escape(sp_rx *p) {
    unsigned long c;
    if (sp_rx_peek(p) != '\\') { sp_err(p->err, p->errsz, "sml: 期望 `\\`"); return NULL; }
    p->i++;
    if (p->i >= p->n) { sp_err(p->err, p->errsz, "sml: `\\` 后缺少字符"); return NULL; }
    c = p->cp[p->i++];
    switch (c) {
        case 'd': return sp_pat_from_class(sp_class_new(SPC_DIGIT), p->err, p->errsz);
        case 'D': return sp_pat_from_class(sp_class_not(sp_class_new(SPC_DIGIT)), p->err, p->errsz);
        case 'w': return sp_pat_from_class(sp_class_new(SPC_WORD), p->err, p->errsz);
        case 'W': return sp_pat_from_class(sp_class_not(sp_class_new(SPC_WORD)), p->err, p->errsz);
        case 's': return sp_pat_from_class(sp_class_new(SPC_SPACE), p->err, p->errsz);
        case 'S': return sp_pat_from_class(sp_class_not(sp_class_new(SPC_SPACE)), p->err, p->errsz);
        default:
            if (c >= '1' && c <= '9') {
                sp_err(p->err, p->errsz,
                       "sml: regex 不支持反向引用 `\\%c`（与线性时间匹配根本冲突）；"
                       "请改用命名捕获 + 等值比较", (int)c);
                return NULL;
            }
            return sp_pat_from_class(sp_class_one(c), p->err, p->errsz);
    }
}

static sp_pat *sp_rx_atom(sp_rx *p) {
    unsigned long c;
    if (p->i >= p->n) { sp_err(p->err, p->errsz, "sml: regex 意外结束"); return NULL; }
    c = p->cp[p->i];
    if (c == '(') {
        sp_pat *inner;
        p->i++;
        if (sp_rx_peek(p) == '?') {
            sp_err(p->err, p->errsz,
                   "sml: regex 不支持 `(?...)` 扩展（lookaround / 命名组等）；"
                   "它们会破坏线性时间保证");
            return NULL;
        }
        inner = sp_rx_alt(p);
        if (!inner) return NULL;
        if (sp_rx_peek(p) != ')') {
            sp_free(inner);
            sp_err(p->err, p->errsz, "sml: regex 分组缺少 `)`");
            return NULL;
        }
        p->i++;
        return inner;
    }
    if (c == '[') return sp_pat_from_class(sp_rx_class(p), p->err, p->errsz);
    if (c == '.') { p->i++; return sp_pat_from_class(sp_class_new(SPC_ANY), p->err, p->errsz); }
    if (c == '\\') return sp_rx_escape(p);
    p->i++;
    return sp_pat_from_class(sp_class_one(c), p->err, p->errsz);
}

/* 读一串 ASCII 数字（`{n}` / `{n,m}` 的 n / m）。无数字返回 0。 */
static int sp_rx_read_uint(sp_rx *p, long *out) {
    long v = 0;
    size_t start = p->i;
    while (p->i < p->n && p->cp[p->i] >= '0' && p->cp[p->i] <= '9') {
        v = v * 10 + (long)(p->cp[p->i] - '0');
        if (v > 1000000000L) v = 1000000001L;   /* 夹住，交给 QUANT_MAX 闸门 */
        p->i++;
    }
    if (p->i == start) return 0;
    *out = v;
    return 1;
}

/* `{n}` / `{n,}` / `{n,m}`。max = -1 表示无上界（与 IR 同约定）。 */
static int sp_rx_counted(sp_rx *p, size_t *min, long *max) {
    long lo, hi;
    if (sp_rx_peek(p) != '{') { sp_err(p->err, p->errsz, "sml: 期望 `{`"); return -1; }
    p->i++;
    if (!sp_rx_read_uint(p, &lo)) {
        sp_err(p->err, p->errsz, "sml: `{n}` 量词缺少数字");
        return -1;
    }
    if (sp_rx_peek(p) == ',') {
        p->i++;
        if (!sp_rx_read_uint(p, &hi)) hi = -1;   /* {n,} 无上界 */
    } else {
        hi = lo;                                  /* {n} 恰好 n 次 */
    }
    if (sp_rx_peek(p) != '}') { sp_err(p->err, p->errsz, "sml: 量词缺少 `}`"); return -1; }
    p->i++;
    if (lo > SP_QUANT_MAX || (hi >= 0 && hi > SP_QUANT_MAX)) {
        sp_err(p->err, p->errsz,
               "sml: 量词过大（> %d），拒绝编译（防病态规则）", SP_QUANT_MAX);
        return -1;
    }
    *min = (size_t)lo;
    *max = hi;
    return 0;
}

static sp_pat *sp_rx_repeat(sp_rx *p) {
    sp_pat *atom = sp_rx_atom(p);
    unsigned long c;
    if (!atom) return NULL;
    c = sp_rx_peek(p);
    {
        size_t mn = 0;
        long mx = -1;
        int has = 0;
        if (c == '*') { p->i++; mn = 0; mx = -1; has = 1; }
        else if (c == '+') { p->i++; mn = 1; mx = -1; has = 1; }
        else if (c == '?') { p->i++; mn = 0; mx = 1; has = 1; }
        else if (c == '{') {
            if (sp_rx_counted(p, &mn, &mx) != 0) { sp_free(atom); return NULL; }
            has = 1;
        }
        if (has) {
            sp_pat *r;
            /* 无回溯引擎没有贪婪概念：`*?` 的 `?` 直接忽略（Rust 同）。 */
            if (sp_rx_peek(p) == '?') p->i++;
            r = sp_pat_new(SP_REPEAT);
            if (!r) { sp_free(atom); sp_err(p->err, p->errsz, "sml: 内存不足"); return NULL; }
            r->sub = atom;
            r->min = mn;
            r->max = mx;
            return r;
        }
    }
    return atom;
}

static sp_pat *sp_rx_seq(sp_rx *p) {
    sp_pat *seq = sp_pat_new(SP_SEQ);
    size_t cap = 0;
    if (!seq) { sp_err(p->err, p->errsz, "sml: 内存不足"); return NULL; }
    while (p->i < p->n) {
        unsigned long c = p->cp[p->i];
        if (c == '|' || c == ')') break;
        if (c == '^' || c == '$') { p->i++; continue; }   /* 锚点隐含 */
        {
            sp_pat *r = sp_rx_repeat(p);
            if (!r) { sp_free(seq); return NULL; }
            if (sp_push_pat(&seq->items, &seq->n, &cap, r) != 0) {
                sp_free(r);
                sp_free(seq);
                sp_err(p->err, p->errsz, "sml: 内存不足");
                return NULL;
            }
        }
    }
    return seq;
}

static sp_pat *sp_rx_alt(sp_rx *p) {
    sp_pat *first = sp_rx_seq(p);
    if (!first) return NULL;
    if (sp_rx_peek(p) != '|') return first;
    {
        sp_pat *alt = sp_pat_new(SP_ALT);
        size_t cap = 0;
        if (!alt) { sp_free(first); sp_err(p->err, p->errsz, "sml: 内存不足"); return NULL; }
        if (sp_push_pat(&alt->items, &alt->n, &cap, first) != 0) {
            sp_free(first);
            sp_free(alt);
            sp_err(p->err, p->errsz, "sml: 内存不足");
            return NULL;
        }
        while (sp_rx_peek(p) == '|') {
            sp_pat *s;
            p->i++;
            s = sp_rx_seq(p);
            if (!s) { sp_free(alt); return NULL; }
            if (sp_push_pat(&alt->items, &alt->n, &cap, s) != 0) {
                sp_free(s);
                sp_free(alt);
                sp_err(p->err, p->errsz, "sml: 内存不足");
                return NULL;
            }
        }
        return alt;
    }
}

/* 对外入口：把 regex 源码编译成 `sp_pat`（失败返回 NULL 并写 err）。 */
static sp_pat *sp_regex_parse(const char *src, char *err, size_t errsz) {
    sp_rx p;
    sp_pat *pat;
    size_t len = strlen(src), pos = 0, k = 0;
    unsigned long *cp = (unsigned long *)malloc((len + 1) * sizeof(unsigned long));
    if (!cp) { sp_err(err, errsz, "sml: 内存不足"); return NULL; }
    while (pos < len) cp[k++] = sp_next_cp(src, len, &pos);
    p.cp = cp;
    p.n = k;
    p.i = 0;
    p.err = err;
    p.errsz = errsz;
    pat = sp_rx_alt(&p);
    if (pat && p.i < p.n) {
        char ub[8];
        int ul = sp_utf8_encode(p.cp[p.i], ub);
        ub[ul] = '\0';
        sp_err(err, errsz,
               "sml: regex 在位置 %lu 处有无法解析的内容 `%s`", (unsigned long)p.i, ub);
        sp_free(pat);
        pat = NULL;
    }
    free(cp);
    return pat;
}

/* 一个模式元素（对象）→ Pat。键按**语义**归类，故换一张关键字表即换语言。 */
static sp_pat *sp_compile_elem(const sml_value *v, const sml_value *rules,
                               const char **stack, size_t stack_n,
                               char *err, size_t errsz) {
    const sml_value *lit = NULL, *cls = NULL, *alt = NULL, *grp = NULL, *seq = NULL;
    const sml_value *use = NULL, *until = NULL, *times = NULL, *optional = NULL;
    const sml_value *name = NULL, *regex_src = NULL, *fmin = NULL, *fmax = NULL;
    const sml_field *fd;
    sp_pat *base = NULL;

    for (fd = (v->type == SML_OBJECT ? v->u.obj.head : NULL); fd; fd = fd->next) {
        switch (sp_lookup(fd->key)) {
            case KC_LIT:      lit = fd->value; break;
            case KC_CLASS:    cls = fd->value; break;
            case KC_ALT:      alt = fd->value; break;
            case KC_GROUP:    grp = fd->value; break;
            case KC_SEQ:      seq = fd->value; break;
            case KC_USE:      use = fd->value; break;
            case KC_UNTIL:    until = fd->value; break;
            case KC_TIMES:    times = fd->value; break;
            case KC_OPTIONAL: optional = fd->value; break;
            case KC_MIN:      fmin = fd->value; break;
            case KC_MAX:      fmax = fd->value; break;
            case KC_NAME:     name = fd->value; break;
            case KC_REGEX:    regex_src = fd->value; break;
            /* 未知键容忍：规则里可以带说明性字段（`说明: "手机号"`），Rust 同。 */
            default: break;
        }
    }

    /* 1) 基底 */
    if (lit) {
        if (lit->type != SML_STR) {
            sp_err(err, errsz, "sml: `字面/lit` 须为字符串，实际 %s", sp_type_name(lit));
            return NULL;
        }
        base = sp_pat_new(SP_LIT);
        if (!base) goto oom;
        base->lit_len = lit->slen;
        base->lit = (char *)malloc(lit->slen + 1);
        if (!base->lit) goto oom;
        memcpy(base->lit, lit->u.s, lit->slen);
        base->lit[lit->slen] = '\0';
    } else if (cls) {
        if (cls->type != SML_STR) {
            sp_err(err, errsz, "sml: `类/class` 须为字符串，实际 %s", sp_type_name(cls));
            return NULL;
        }
        base = sp_pat_new(SP_CLASS);
        if (!base) goto oom;
        base->cls = sp_parse_class(cls->u.s, err, errsz);
        if (!base->cls) { sp_free(base); return NULL; }
    } else if (alt) {
        size_t i, cap = 0;
        if (alt->type != SML_ARRAY) {
            sp_err(err, errsz, "sml: `任一/alt` 须为数组，实际 %s", sp_type_name(alt));
            return NULL;
        }
        base = sp_pat_new(SP_ALT);
        if (!base) goto oom;
        for (i = 0; i < sml_arr_len(alt); i++) {
            sp_pat *child = sp_compile_value(sml_arr_get(alt, i), rules, stack, stack_n, err, errsz);
            if (!child) { sp_free(base); return NULL; }
            if (sp_push_pat(&base->items, &base->n, &cap, child) != 0) {
                sp_free(child);
                goto oom;
            }
        }
    } else if (grp) {
        base = sp_compile_value(grp, rules, stack, stack_n, err, errsz);
        if (!base) return NULL;
    } else if (seq) {
        base = sp_compile_value(seq, rules, stack, stack_n, err, errsz);
        if (!base) return NULL;
    } else if (use) {
        const sml_value *target;
        const char *rname;
        size_t i;
        if (use->type != SML_STR) {
            sp_err(err, errsz, "sml: `用/use` 须为字符串，实际 %s", sp_type_name(use));
            return NULL;
        }
        rname = use->u.s;
        for (i = 0; i < stack_n; i++) {
            if (strcmp(stack[i], rname) == 0) {
                sp_err(err, errsz,
                       "sml: 规则 `%s` 存在循环引用（L1 不支持递归；如需嵌套结构请先升级到 L2）",
                       rname);
                return NULL;
            }
        }
        target = sp_obj_get(rules, rname);
        if (!target) {
            sp_err(err, errsz, "sml: 未定义的规则 `%s`", rname);
            return NULL;
        }
        {
            const char **ns = (const char **)malloc((stack_n + 1) * sizeof(char *));
            if (!ns) goto oom;
            for (i = 0; i < stack_n; i++) ns[i] = stack[i];
            ns[stack_n] = rname;
            base = sp_compile_value(target, rules, ns, stack_n + 1, err, errsz);
            free(ns);
            if (!base) return NULL;
        }
    } else if (until) {
        if (until->type != SML_STR) {
            sp_err(err, errsz, "sml: `直到/until` 须为字符串，实际 %s", sp_type_name(until));
            return NULL;
        }
        base = sp_pat_new(SP_UNTIL);
        if (!base) goto oom;
        if (sp_lookup(until->u.s) == KC_EOL) {
            /* 「直到行尾」= 吃掉剩余全部字符（Rust 同样处理） */
            base->kind = SP_REPEAT;
            base->min = 0;
            base->max = -1;
            base->sub = sp_pat_new(SP_CLASS);
            if (!base->sub) { sp_free(base); goto oom; }
            base->sub->cls = sp_class_new(SPC_ANY);
            if (!base->sub->cls) { sp_free(base); goto oom; }
        } else {
            base->sub = sp_pat_new(SP_LIT);
            if (!base->sub) { sp_free(base); goto oom; }
            base->sub->lit_len = until->slen;
            base->sub->lit = (char *)malloc(until->slen + 1);
            if (!base->sub->lit) { sp_free(base); goto oom; }
            memcpy(base->sub->lit, until->u.s, until->slen);
            base->sub->lit[until->slen] = '\0';
        }
    } else if (regex_src) {
        /* `正则:` 逃生舱：解析成 regex 子集后编译进同一个 IR（Rust `regex.rs` 同构）。
           拒绝反向引用 / lookaround 时报错，不静默降级。 */
        if (regex_src->type != SML_STR) {
            sp_err(err, errsz, "sml: `正则/regex` 须为字符串，实际 %s", sp_type_name(regex_src));
            return NULL;
        }
        base = sp_regex_parse(regex_src->u.s, err, errsz);
        if (!base) {
            /* 与 Rust 同：外层再包一句「regex 编译失败」（内层文案已带 `sml:` 前缀）。 */
            char inner[512];
            inner[0] = '\0';
            if (err && errsz) {
                strncpy(inner, err, sizeof inner - 1);
                inner[sizeof inner - 1] = '\0';
            }
            sp_err(err, errsz, "sml: regex 编译失败：%s", inner);
            return NULL;
        }
    } else {
        sp_err(err, errsz,
               "sml: 无法识别的模式元素（需 字面/类/任一/组/序列/用/直到/正则 之一），实际键: `%s`",
               (v->type == SML_OBJECT && v->u.obj.head) ? v->u.obj.head->key : "?");
        return NULL;
    }

    /* 2) 量词：优先平铺 `最小` / `最大`，否则 `次` / `可选`。只包一层 Repeat。 */
    if (fmin || fmax) {
        long mn = 0, mx = -1;
        if (fmin) {
            if (fmin->type != SML_INT) { sp_err(err, errsz, "sml: 量词最小必须为整数"); sp_free(base); return NULL; }
            mn = (long)fmin->u.i;
        }
        if (fmax) {
            if (fmax->type != SML_INT) { sp_err(err, errsz, "sml: 量词最大必须为整数"); sp_free(base); return NULL; }
            mx = (long)fmax->u.i;
        }
        if (mn < 0) { sp_err(err, errsz, "sml: 量词最小不能为负（得 %ld）", mn); sp_free(base); return NULL; }
        if (fmax && mx < mn) {
            sp_err(err, errsz, "sml: 量词最大(%ld)不能小于最小(%ld)", mx, mn);
            sp_free(base);
            return NULL;
        }
        if (mn > SP_QUANT_MAX || (fmax && mx > SP_QUANT_MAX)) {
            sp_err(err, errsz, "sml: 量词过大（> %d），拒绝编译", SP_QUANT_MAX);
            sp_free(base);
            return NULL;
        }
        {
            sp_pat *r = sp_pat_new(SP_REPEAT);
            if (!r) { sp_free(base); goto oom; }
            r->sub = base;
            r->min = (size_t)mn;
            r->max = fmax ? mx : -1;
            base = r;
        }
    } else if (times) {
        size_t mn;
        long mx;
        sp_pat *r;
        if (sp_parse_times(times, &mn, &mx, err, errsz) != 0) { sp_free(base); return NULL; }
        r = sp_pat_new(SP_REPEAT);
        if (!r) { sp_free(base); goto oom; }
        r->sub = base;
        r->min = mn;
        r->max = mx;
        base = r;
    } else if (optional && optional->type == SML_BOOL && optional->u.boolean) {
        sp_pat *r = sp_pat_new(SP_REPEAT);
        if (!r) { sp_free(base); goto oom; }
        r->sub = base;
        r->min = 0;
        r->max = 1;
        base = r;
    }

    /* 3) 命名：IR 里保留名字（供将来「捕获」用），匹配不影响。 */
    if (name) {
        sp_pat *nm;
        if (name->type != SML_STR) {
            sp_err(err, errsz, "sml: `名/name` 须为字符串，实际 %s", sp_type_name(name));
            sp_free(base);
            return NULL;
        }
        nm = sp_pat_new(SP_NAMED);
        if (!nm) { sp_free(base); goto oom; }
        nm->sub = base;
        base = nm;
    }
    return base;

oom:
    sp_err(err, errsz, "sml: 内存不足");
    sp_free(base);
    return NULL;
}

static sp_pat *sp_compile_value(const sml_value *v, const sml_value *rules,
                                const char **stack, size_t stack_n,
                                char *err, size_t errsz) {
    if (!v) { sp_err(err, errsz, "sml: 模式元素缺失"); return NULL; }
    if (v->type == SML_ARRAY) {
        /* 数组 = 序列（保序，规则体的常见形态） */
        size_t i, cap = 0;
        sp_pat *p = sp_pat_new(SP_SEQ);
        if (!p) { sp_err(err, errsz, "sml: 内存不足"); return NULL; }
        for (i = 0; i < sml_arr_len(v); i++) {
            sp_pat *c = sp_compile_value(sml_arr_get(v, i), rules, stack, stack_n, err, errsz);
            if (!c) { sp_free(p); return NULL; }
            if (sp_push_pat(&p->items, &p->n, &cap, c) != 0) {
                sp_free(c);
                sp_free(p);
                sp_err(err, errsz, "sml: 内存不足");
                return NULL;
            }
        }
        return p;
    }
    if (v->type == SML_STR) {
        sp_pat *p = sp_pat_new(SP_LIT);
        if (!p) { sp_err(err, errsz, "sml: 内存不足"); return NULL; }
        p->lit_len = v->slen;
        p->lit = (char *)malloc(v->slen + 1);
        if (!p->lit) { sp_free(p); sp_err(err, errsz, "sml: 内存不足"); return NULL; }
        memcpy(p->lit, v->u.s, v->slen);
        p->lit[v->slen] = '\0';
        return p;
    }
    if (v->type == SML_OBJECT) {
        return sp_compile_elem(v, rules, stack, stack_n, err, errsz);
    }
    sp_err(err, errsz, "sml: 模式元素类型不支持: %s", sp_type_name(v));
    return NULL;
}

static sp_pat *sp_compile(const sml_value *rule, const sml_value *rules,
                          char *err, size_t errsz) {
    return sp_compile_value(rule, rules, NULL, 0, err, errsz);
}

/* ---- NFA：Thompson 构造 ----------------------------------------------
 *
 * `Repeat` 是安全性的关键（与 Rust 逐条同构）：
 *   - 有上界 ⇒ 展开成固定份数的可选副本（**不产生回边**）；
 *   - 无上界（`+` / `*`）⇒ 一条回边 Split，**不复制模式本体**
 *     —— 所以不会出现 `a+a+` 那种指数路径。
 * 指令总数上限 `SP_MAX_PROG` 在物化途中闸住嵌套量词的相乘膨胀。
 * ---------------------------------------------------------------------- */

typedef enum { SI_CHAR, SI_SPLIT, SI_JMP, SI_MATCH } sp_inst_kind;

typedef struct {
    sp_inst_kind kind;
    const sp_class *cls;   /* SI_CHAR */
    size_t x, y;           /* SI_SPLIT(x, y) / SI_JMP(x) */
} sp_inst;

typedef struct {
    sp_inst *code;
    size_t n, cap;
    /* emit 期间现造的临时字符类（`字面` 的每个字符、`直到行尾` 的 Any）：
       指令里存的是**借用指针**，故这些类必须活到匹配结束。 */
    sp_class **tmp;
    size_t ntmp, captmp;
} sp_prog;

static void sp_prog_free(sp_prog *pg) {
    size_t i;
    for (i = 0; i < pg->ntmp; i++) sp_class_free(pg->tmp[i]);
    free(pg->tmp);
    free(pg->code);
    pg->code = NULL;
    pg->tmp = NULL;
    pg->n = pg->cap = pg->ntmp = pg->captmp = 0;
}

static int sp_emit_push(sp_prog *pg, sp_inst_kind k, const sp_class *cls,
                        size_t x, size_t y) {
    if (pg->n == pg->cap) {
        size_t nc = pg->cap ? pg->cap * 2 : 32;
        sp_inst *na = (sp_inst *)realloc(pg->code, nc * sizeof(sp_inst));
        if (!na) return -1;
        pg->code = na;
        pg->cap = nc;
    }
    pg->code[pg->n].kind = k;
    pg->code[pg->n].cls = cls;
    pg->code[pg->n].x = x;
    pg->code[pg->n].y = y;
    pg->n++;
    return 0;
}

/* 现造一个临时类（`字面` 拆字符、`直到行尾` 的 Any）并登记进池子。 */
static const sp_class *sp_tmp_class(sp_prog *pg, sp_class_kind k, unsigned long ch) {
    sp_class *c = sp_class_new(k);
    if (!c) return NULL;
    c->ch1 = ch;
    if (pg->ntmp == pg->captmp) {
        size_t nc = pg->captmp ? pg->captmp * 2 : 16;
        sp_class **na = (sp_class **)realloc(pg->tmp, nc * sizeof(sp_class *));
        if (!na) { sp_class_free(c); return NULL; }
        pg->tmp = na;
        pg->captmp = nc;
    }
    pg->tmp[pg->ntmp++] = c;
    return c;
}

static int sp_emit(const sp_pat *p, sp_prog *pg, char *err, size_t errsz) {
    size_t i;
    if (pg->n > SP_MAX_PROG) {
        sp_err(err, errsz,
               "sml: 模式程序过大（超过 %d 条指令，疑似嵌套量词的病态膨胀）", SP_MAX_PROG);
        return -1;
    }
    switch (p->kind) {
        case SP_CLASS:
            return sp_emit_push(pg, SI_CHAR, p->cls, 0, 0);
        case SP_LIT: {
            /* 逐字符展开：这里按**码位**拆（Rust 同样是 `s.chars()`） */
            size_t pos = 0;
            while (pos < p->lit_len) {
                unsigned long cp = sp_next_cp(p->lit, p->lit_len, &pos);
                const sp_class *c = sp_tmp_class(pg, SPC_ONE, cp);
                if (!c) { sp_err(err, errsz, "sml: 内存不足"); return -1; }
                if (sp_emit_push(pg, SI_CHAR, c, 0, 0) != 0) {
                    sp_err(err, errsz, "sml: 内存不足");
                    return -1;
                }
            }
            return 0;
        }
        case SP_SEQ:
            for (i = 0; i < p->n; i++)
                if (sp_emit(p->items[i], pg, err, errsz) != 0) return -1;
            return 0;
        case SP_NAMED:
            return sp_emit(p->sub, pg, err, errsz);
        case SP_ALT: {
            size_t *jmps = NULL, njmps = 0, capj = 0;
            if (p->n == 0) return 0;
            for (i = 0; i < p->n; i++) {
                size_t split_pc = 0, start, jmp_pc;
                if (i + 1 < p->n) {
                    split_pc = pg->n;
                    if (sp_emit_push(pg, SI_SPLIT, NULL, 0, 0) != 0) goto oom;
                }
                start = pg->n;
                if (sp_emit(p->items[i], pg, err, errsz) != 0) { free(jmps); return -1; }
                if (i + 1 < p->n) {
                    jmp_pc = pg->n;
                    if (sp_emit_push(pg, SI_JMP, NULL, 0, 0) != 0) goto oom;
                    pg->code[split_pc].x = start;
                    pg->code[split_pc].y = pg->n;
                    if (njmps == capj) {
                        size_t nc = capj ? capj * 2 : 4;
                        size_t *na = (size_t *)realloc(jmps, nc * sizeof(size_t));
                        if (!na) goto oom;
                        jmps = na;
                        capj = nc;
                    }
                    jmps[njmps++] = jmp_pc;
                }
            }
            for (i = 0; i < njmps; i++) pg->code[jmps[i]].x = pg->n;
            free(jmps);
            return 0;
        oom:
            free(jmps);
            sp_err(err, errsz, "sml: 内存不足");
            return -1;
        }
        case SP_REPEAT: {
            size_t k;
            if (p->max >= 0 && (size_t)p->max >= p->min) {
                for (k = 0; k < p->min; k++)
                    if (sp_emit(p->sub, pg, err, errsz) != 0) return -1;
                for (k = 0; k < (size_t)p->max - p->min; k++) {
                    size_t split_pc = pg->n, start, end;
                    if (sp_emit_push(pg, SI_SPLIT, NULL, 0, 0) != 0) goto oom2;
                    start = pg->n;
                    if (sp_emit(p->sub, pg, err, errsz) != 0) return -1;
                    end = pg->n;
                    pg->code[split_pc].x = start;
                    pg->code[split_pc].y = end;
                }
                return 0;
            }
            if (p->max >= 0) {
                /* max < min：不可能匹配，退化为「至少 min 次」（Rust 同，便于诊断） */
                for (k = 0; k < p->min; k++)
                    if (sp_emit(p->sub, pg, err, errsz) != 0) return -1;
                return 0;
            }
            /* 无上界：min 份 + 一条回边（不复制本体） */
            for (k = 0; k < p->min; k++)
                if (sp_emit(p->sub, pg, err, errsz) != 0) return -1;
            {
                size_t split_pc = pg->n, start;
                if (sp_emit_push(pg, SI_SPLIT, NULL, 0, 0) != 0) goto oom2;
                start = pg->n;
                if (sp_emit(p->sub, pg, err, errsz) != 0) return -1;
                if (sp_emit_push(pg, SI_JMP, NULL, split_pc, 0) != 0) goto oom2;
                pg->code[split_pc].x = start;
                pg->code[split_pc].y = pg->n;
            }
            return 0;
        oom2:
            sp_err(err, errsz, "sml: 内存不足");
            return -1;
        }
        case SP_UNTIL: {
            size_t split_pc, try_end, jmp_pc, consume, end;
            const sp_class *any = sp_tmp_class(pg, SPC_ANY, 0);
            if (!any) { sp_err(err, errsz, "sml: 内存不足"); return -1; }
            split_pc = pg->n;
            if (sp_emit_push(pg, SI_SPLIT, NULL, 0, 0) != 0) goto oom3;
            try_end = pg->n;
            if (sp_emit(p->sub, pg, err, errsz) != 0) return -1;
            jmp_pc = pg->n;
            if (sp_emit_push(pg, SI_JMP, NULL, 0, 0) != 0) goto oom3;
            consume = pg->n;
            if (sp_emit_push(pg, SI_CHAR, any, 0, 0) != 0) goto oom3;
            if (sp_emit_push(pg, SI_JMP, NULL, split_pc, 0) != 0) goto oom3;
            end = pg->n;
            pg->code[split_pc].x = try_end;
            pg->code[split_pc].y = consume;
            pg->code[jmp_pc].x = end;
            return 0;
        oom3:
            sp_err(err, errsz, "sml: 内存不足");
            return -1;
        }
    }
    return 0;
}

/* ε 闭包：把 Split / Jmp 能到达的位置全部加入线程集合。
   `seen` 用来防 ε 环无限展开（与 Rust 的局部 visited 同）。 */
static int sp_add_thread(size_t *list, size_t *n, size_t cap,
                         const sp_prog *pg, size_t pc,
                         size_t *stack, size_t *seen, size_t *n_seen) {
    size_t sp = 0, i;
    stack[sp++] = pc;
    seen[(*n_seen)++] = pc;
    while (sp > 0) {
        size_t p = stack[--sp];
        if (pg->code[p].kind == SI_SPLIT) {
            size_t kids[2];
            kids[0] = pg->code[p].y;
            kids[1] = pg->code[p].x;
            for (i = 0; i < 2; i++) {
                size_t nx = kids[i], j;
                int dup = 0;
                for (j = 0; j < *n_seen; j++) if (seen[j] == nx) { dup = 1; break; }
                if (dup) continue;
                if (*n_seen >= cap) return -1;
                seen[(*n_seen)++] = nx;
                if (sp >= cap) return -1;
                stack[sp++] = nx;
            }
        } else if (pg->code[p].kind == SI_JMP) {
            size_t nx = pg->code[p].x, j;
            int dup = 0;
            for (j = 0; j < *n_seen; j++) if (seen[j] == nx) { dup = 1; break; }
            if (!dup) {
                if (*n_seen >= cap) return -1;
                seen[(*n_seen)++] = nx;
                if (sp >= cap) return -1;
                stack[sp++] = nx;
            }
        } else {
            /* SI_CHAR / SI_MATCH */
            int dup = 0;
            for (i = 0; i < *n; i++) if (list[i] == p) { dup = 1; break; }
            if (!dup) {
                if (*n >= cap) return -1;
                list[(*n)++] = p;
            }
        }
    }
    return 0;
}

/* 并行推进所有线程。返回 1 命中 / 0 不命中 / -1 超预算（写 err）。 */
static int sp_run(const sp_prog *pg, const char *text, size_t len,
                  char *err, size_t errsz) {
    size_t cap = pg->n + 1;
    size_t *clist, *nlist, *stack, *seen;
    size_t ncl = 0, nnl = 0, n_seen;
    size_t pos = 0;
    long steps = 0;
    size_t match_pc = pg->n - 1;
    int rc = 0;

    clist = (size_t *)calloc(cap, sizeof(size_t));
    nlist = (size_t *)calloc(cap, sizeof(size_t));
    stack = (size_t *)calloc(cap, sizeof(size_t));
    seen = (size_t *)calloc(cap, sizeof(size_t));
    if (!clist || !nlist || !stack || !seen) {
        sp_err(err, errsz, "sml: 内存不足");
        rc = -1;
        goto done;
    }
    n_seen = 0;
    if (sp_add_thread(clist, &ncl, cap, pg, 0, stack, seen, &n_seen) != 0) {
        sp_err(err, errsz, "sml: 内存不足");
        rc = -1;
        goto done;
    }
    while (pos < len) {
        unsigned long cp = sp_next_cp(text, len, &pos);
        size_t k;
        nnl = 0;
        for (k = 0; k < ncl; k++) {
            size_t pc = clist[k];
            if (++steps > SP_BUDGET) {
                sp_err(err, errsz,
                       "sml: 模式匹配超出步数预算 %ld（疑似病态规则或超长输入）", SP_BUDGET);
                rc = -1;
                goto done;
            }
            if (pg->code[pc].kind == SI_CHAR && sp_class_matches(pg->code[pc].cls, cp)) {
                n_seen = 0;
                if (sp_add_thread(nlist, &nnl, cap, pg, pc + 1, stack, seen, &n_seen) != 0) {
                    sp_err(err, errsz, "sml: 内存不足");
                    rc = -1;
                    goto done;
                }
            }
        }
        {
            size_t *t = clist;
            clist = nlist;
            nlist = t;
        }
        ncl = nnl;
        /* 没有线程活下来 ⇒ 后续字符不可能再救（与 Rust 提前返回同） */
        if (ncl == 0) { rc = 0; goto done; }
    }
    {
        size_t k;
        rc = 0;
        for (k = 0; k < ncl; k++) if (clist[k] == match_pc) { rc = 1; break; }
    }
done:
    free(clist);
    free(nlist);
    free(stack);
    free(seen);
    return rc;
}

/* 对外：全文匹配。`pat` 由 `sp_compile` 得到（或 NULL ⇒ 直接报错）。 */
static int sp_match(const sp_pat *pat, const char *text, size_t len,
                    char *err, size_t errsz) {
    sp_prog pg;
    int rc;
    memset(&pg, 0, sizeof pg);
    if (!pat) { sp_err(err, errsz, "sml: 模式未编译"); return -1; }
    if (sp_emit(pat, &pg, err, errsz) != 0) { sp_prog_free(&pg); return -1; }
    /* 末尾压 Match 指令（`match_pc = n - 1`）。与 Rust `compile` 同构。 */
    if (sp_emit_push(&pg, SI_MATCH, NULL, 0, 0) != 0) {
        sp_err(err, errsz, "sml: 内存不足");
        sp_prog_free(&pg);
        return -1;
    }
    rc = sp_run(&pg, text, len, err, errsz);
    sp_prog_free(&pg);
    return rc;
}
