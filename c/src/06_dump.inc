/* =====================================================================
** 4. 序列化
** ===================================================================== */

typedef struct { char *buf; size_t len, cap; } sbuf;

static void sb_ensure(sbuf *b, size_t extra) {
    if (b->len + extra + 1 > b->cap) {
        size_t ncap = b->cap ? b->cap * 2 : 256;
        while (b->len + extra + 1 > ncap) ncap *= 2;
        char *nb = (char *)realloc(b->buf, ncap);
        if (nb) { b->buf = nb; b->cap = ncap; }
    }
}

static void sb_add(sbuf *b, const char *s) {
    size_t n = strlen(s);
    sb_ensure(b, n);
    memcpy(b->buf + b->len, s, n);
    b->len += n;
    b->buf[b->len] = '\0';
}

static void sb_addc(sbuf *b, char c) {
    sb_ensure(b, 1);
    b->buf[b->len++] = c;
    b->buf[b->len] = '\0';
}

/* 纯数字形态（会被 coerce_word 字面量化成 Int/Float）。`0x` / `0b` 前缀不算
   —— 与 Rust `f64::parse` 不认十六进制对齐（C 的 strtod 会认 `0x20`，不挡住就会
   把 `"0x20"` 写成带引号的形式，与 Rust 的裸词写法分叉）。 */
static int looks_number(const char *s) {
    const char *q = s;
    char *e = NULL;
    if (*q == '+' || *q == '-') q++;
    if (q[0] == '0' && (q[1] == 'x' || q[1] == 'X' || q[1] == 'b' || q[1] == 'B')) return 0;
    if (!((*q >= '0' && *q <= '9') || *q == '.')) return 0;
    (void)strtoll(s, &e, 10);
    if (e && *e == '\0' && e != s) return 1;
    e = NULL;
    (void)strtod(s, &e);
    if (e && *e == '\0' && e != s) return 1;
    return 0;
}

/* 是否需要给字符串加引号 —— 与 Rust `sml-value::text_rules::needs_quote` 同判据：
   空串 / true·false·null·inf·nan / 纯数字 / 注释前缀 / 破坏语法的字符 /
   首字符非字母下划线 / 含 [A-Za-z0-9_.-] 之外的字符。
   改前只判空白与 `:#{}"\\` ⇒ `"42"`、`"true"`、`"007"` 全被写成裸词，
   自己 dump 的文本自己读回来就换了类型（台账 S-05）。 */
static int needs_quote(const char *s) {
    const unsigned char *p;
    if (!*s) return 1;
    if (strcmp(s, "true") == 0 || strcmp(s, "false") == 0 || strcmp(s, "null") == 0
        || strcmp(s, "inf") == 0 || strcmp(s, "nan") == 0)
        return 1;
    if (looks_number(s)) return 1;
    if (strncmp(s, "--", 2) == 0 || strncmp(s, "//", 2) == 0
        || strncmp(s, "/*", 2) == 0 || strncmp(s, "*/", 2) == 0
        || s[0] == '*' || strncmp(s, "_*", 2) == 0)
        return 1;
    for (p = (const unsigned char *)s; *p; p++) {
        if (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r' ||
            *p == ':' || *p == '#' || *p == '{' || *p == '}' ||
            *p == ',' || *p == '[' || *p == ']' ||
            *p == '"' || *p == '\\' || *p == '/' || *p == '*')
            return 1;
    }
    /* 首字符须字母 / 下划线（UTF-8 多字节首字节按字母算） */
    if (!((s[0] >= 'A' && s[0] <= 'Z') || (s[0] >= 'a' && s[0] <= 'z')
          || s[0] == '_' || (unsigned char)s[0] >= 0x80))
        return 1;
    for (p = (const unsigned char *)s; *p; p++) {
        int ok = (*p >= 'A' && *p <= 'Z') || (*p >= 'a' && *p <= 'z')
              || (*p >= '0' && *p <= '9') || *p == '_' || *p == '-' || *p == '.'
              || *p >= 0x80;
        if (!ok) return 1;
    }
    return 0;
}

/* 输出一个字符串：能裸就裸，否则加引号并转义。
   转义集与 Rust `quote_if_needed` 一致：`\\ \" \n \t \r \0`，其余控制字符走 `\u00XX`。
   改前只转义 `"` 与 `\`，真换行 / 真 NUL 会直接落进文本（台账 S-06）。 */
/* 含 NUL / 控制字符的字符串必须加引号（否则这些字节会裸写进文本）。
   无内嵌 NUL 时退回 needs_quote(s)：那里的 C 串函数才安全。 */
static int needs_quote_n(const char *s, size_t n) {
    size_t i;
    if (n == 0) return 1;
    for (i = 0; i < n; i++) {
        unsigned char c = (const unsigned char)s[i];
        if (c == 0 || c < 0x20 || c == 0x7f) return 1;
    }
    return needs_quote(s);
}

static void dump_quote_str(sbuf *b, const char *s, size_t n) {
    size_t i;
    if (!needs_quote_n(s, n)) {
        for (i = 0; i < n; i++) sb_addc(b, s[i]);
        return;
    }
    sb_addc(b, '"');
    for (i = 0; i < n; i++) {
        unsigned char c = (const unsigned char)s[i];
        switch (c) {
            case '"':  sb_add(b, "\\\""); break;
            case '\\': sb_add(b, "\\\\"); break;
            case '\n': sb_add(b, "\\n"); break;
            case '\t': sb_add(b, "\\t"); break;
            case '\r': sb_add(b, "\\r"); break;
            case 0:    sb_add(b, "\\0"); break;
            default:
                if (c < 0x20 || c == 0x7f) {
                    char u[8];
                    snprintf(u, sizeof(u), "\\u%04X", c);
                    sb_add(b, u);
                } else {
                    sb_addc(b, (char)c);
                }
        }
    }
    sb_addc(b, '"');
}

static void dump_value(sbuf *b, const sml_value *v, int indent);
static void dump_inline(sbuf *b, const sml_value *v);
static void dump_element(sbuf *b, const sml_value *v, int indent);
static void dump_object_body(sbuf *b, const sml_value *v, int indent);
static void dump_array_body(sbuf *b, const sml_value *v, int indent);

/* 该对象是否为**非空**对象 —— 它决定两处「形态选择」，两处判据都必须与 Rust 一致：
   ① `key:` 之后**要不要**补那个空格（W4 ①）：Rust `starts_inline(v)` 的定义是
      「**空**对象才同行渲染」，非空对象会另起一行写 `{ … }` ⇒ 键后**不留**行尾空格。
   ② `dump_value` 里写 `{}` 还是 `\n{ … }`：Rust `dump_block` 同样是「空 ⇒ `{}`」。
   ⚠️ 原先这里**排除** `__type` / `__name`（把它们当内部标记键），那正是 A 类差异的根源：
   这两个键在 Rust 侧是**要往返的数据键**（原样序列化），于是「只有元数据的块」在 C 里被
   判成「空体」⇒ 输成 `{}`，**元数据被静默丢掉**。现在按「有没有键」判。 */
static int obj_has_body(const sml_value *v) {
    if (!v || v->type != SML_OBJECT) return 0;
    return v->u.obj.head != NULL;
}

/* 容器是否「扁平」：**直接子项全是标量**（不再嵌对象 / 数组）。
   与 Rust `dump.rs::is_flat` 同一判据，是「一行写完」还是「展开多行」的唯一开关：
     `{ type: home }`                      扁平 → `[ { type: home } ]` 仍是一行
     `[ { type: home } { type: office } ]` 扁平 → 一行（子项全是标量）
     `{ children: [ … ] }`                 非扁平 → `\n{ … }` 展开

   ⚠️ **只看一层，不递归**。递归版（「子孙全是标量」）是恒真判据 —— 任何对象的
   子孙最终都会落到标量，于是所有东西都被判成扁平、排版分毫不改。Rust 侧写这段时
   真踩过这个坑（编译与测试全绿，只是完全没生效），这里按同一规则实现，别改回递归。
   `__type` / `__name` 在 C 侧存为字符串，天然算标量，不影响判定（与 Rust 一致）。 */
static int is_flat(const sml_value *v) {
    if (!v) return 1;
    if (v->type == SML_OBJECT) {
        sml_field *f;
        for (f = v->u.obj.head; f; f = f->next) {
            const sml_value *fv = f->value;
            if (fv && (fv->type == SML_OBJECT || fv->type == SML_ARRAY)) return 0;
        }
        return 1;
    }
    if (v->type == SML_ARRAY) {
        size_t i;
        for (i = 0; i < v->u.arr.len; i++) {
            const sml_value *iv = v->u.arr.items[i];
            if (iv && (iv->type == SML_OBJECT || iv->type == SML_ARRAY)) return 0;
        }
        return 1;
    }
    return 1; /* 标量一律扁平 */
}

/* 写对象体：**不含**开头的 `{`（由调用方写），负责逐键换行与收尾 `}`。
   抽出来是为了让「键后面接的块」（先写 `\n{`，见 dump_value）与
   「数组元素里的对象」（`{` 跟在同一行，见 dump_element）共用同一套键渲染 ——
   与 Rust `dump.rs` 里 `dump_object_body` / `dump_block` / `dump_element` 的关系同构。
   ⚠️ **所有键都写**，包括 `__type` / `__name`：Rust `dump_object_body` 是
   `for (k, val) in m`（不筛键），因为裸块 `type [name] { … }` 解析后就长成这两个键，
   序列化时必须能写回去（否则元数据不往返）。这是 A 类差异的修复点。 */
static void dump_object_body(sbuf *b, const sml_value *v, int indent) {
    sml_field *f;
    int j;
    for (f = v->u.obj.head; f; f = f->next) {
        sb_add(b, "\n");
        for (j = 0; j < indent + 1; j++) sb_add(b, "  ");
        sb_add(b, f->key);
        /* W4 ①：「键: 后接块」**不留行尾空格**。
           对象体的 dump 是从 `\n{ … }` 起的一整块，这里若写 ": "，
           那个空格就落在行尾（Rust `to_sml` 早就不留了，见 CHANGELOG
           「to_sml 不再在『键: 后接块』时于行尾留空格」）。
           标量 / 空容器（同行渲染）照旧用 ": "。 */
        if (obj_has_body(f->value)) sb_add(b, ":");
        else                        sb_add(b, ": ");
        dump_value(b, f->value, indent + 1);
    }
    sb_add(b, "\n");
    for (j = 0; j < indent; j++) sb_add(b, "  ");
    sb_addc(b, '}');
}

/* 写数组体：**不含**开头的 `[`（由调用方写）。每个元素先换行 + 缩进，
   再由 dump_element 决定「一行写完」还是「就地展开」。 */
static void dump_array_body(sbuf *b, const sml_value *v, int indent) {
    size_t i;
    int j;
    sb_addc(b, '[');
    for (i = 0; i < v->u.arr.len; i++) {
        sb_add(b, "\n");
        for (j = 0; j < indent + 1; j++) sb_add(b, "  ");
        dump_element(b, v->u.arr.items[i], indent + 1);
    }
    sb_add(b, "\n");
    for (j = 0; j < indent; j++) sb_add(b, "  ");
    sb_addc(b, ']');
}

/* 浮点输出：整数值**必须保留小数点** —— `1.0` 若写成 `1`，回读就变成 Int，
   类型被静默改掉（Rust 侧 `format_float` 自 2026-10-28 起同一口径，见 TODO 雷 5 B）。
   NaN / inf 没有合法的 SML 字面量，写成**引号字符串**（与 Rust 一致），
   否则裸 `nan` 回读会变成 Str `nan`（看起来成功、其实类型变了）。 */
static void add_float(sbuf *b, double f) {
    char num[64];
    if (isnan(f) || isinf(f)) {
        snprintf(num, sizeof(num), "\"%g\"", f);
        sb_add(b, num);
        return;
    }
    snprintf(num, sizeof(num), "%g", f);
    sb_add(b, num);
    /* `%g` 对整数值吞掉小数点 ⇒ 补 `.0`；科学计数形式（含 e/E）本来就是浮点形态，不动。 */
    if (floor(f) == f && !strpbrk(num, "eE")) sb_add(b, ".0");
}

static void dump_value(sbuf *b, const sml_value *v, int indent) {
    char num[64];
    if (!v) { sb_add(b, "null"); return; }
    switch (v->type) {
        case SML_NULL: sb_add(b, "null"); break;
        case SML_BOOL: sb_add(b, v->u.boolean ? "true" : "false"); break;
        case SML_INT: snprintf(num, sizeof(num), "%lld", v->u.i); sb_add(b, num); break;
        case SML_FLOAT: add_float(b, v->u.f); break;
        case SML_STR: dump_quote_str(b, v->u.s, v->slen); break;
        case SML_ARRAY: {
            if (v->u.arr.len == 0) { sb_add(b, "[]"); break; }
            dump_array_body(b, v, indent);
            break;
        }
        case SML_OBJECT: {
            /* 空对象写 `{}`、非空写 `\n{ … }` —— 与 Rust `dump_block` 同判据。
               （A 类：这里原先排除 __type/__name，于是「只有元数据的块」被误判成空对象、
               元数据被静默丢掉。） */
            if (!obj_has_body(v)) { sb_add(b, "{}"); break; }
            /* `key:` 之后另起一行写 `{ … }`，与 Rust `dump_block` 同形。 */
            sb_add(b, "\n");
            int j;
            for (j = 0; j < indent; j++) sb_add(b, "  ");
            sb_addc(b, '{');
            dump_object_body(b, v, indent);
            break;
        }
    }
}

/* 把值压成**一行**写。契约：只对「扁平」值调用（见 is_flat）——
   因此它内部的递归永远不会撞上容器，压出来的行里不会再有换行。
   W4 ② 之前这里是**无条件**压行（数组项一律走它），于是整棵子树被塞进一行；
   现在由 dump_element 按 is_flat 决定是否走这里。 */
static void dump_inline(sbuf *b, const sml_value *v) {
    char num[64];
    if (!v) { sb_add(b, "null"); return; }
    switch (v->type) {
        case SML_NULL: sb_add(b, "null"); break;
        case SML_BOOL: sb_add(b, v->u.boolean ? "true" : "false"); break;
        case SML_INT: snprintf(num, sizeof(num), "%lld", v->u.i); sb_add(b, num); break;
        case SML_FLOAT: add_float(b, v->u.f); break;
        case SML_STR: dump_quote_str(b, v->u.s, v->slen); break;
        case SML_ARRAY: {
            sb_add(b, "[ ");
            size_t i;
            for (i = 0; i < v->u.arr.len; i++) {
                if (i) sb_add(b, ", ");
                dump_inline(b, v->u.arr.items[i]);
            }
            sb_add(b, " ]");
            break;
        }
        case SML_OBJECT: {
            sb_add(b, "{ ");
            int first = 1;
            sml_field *f;
            /* 同 dump_object_body：**不筛键**（含 __type / __name），与 Rust
               `dump_inline` 的 `m.iter()` 一致。 */
            for (f = v->u.obj.head; f; f = f->next) {
                if (!first) sb_add(b, ", ");
                first = 0;
                sb_add(b, f->key);
                sb_add(b, ": ");
                dump_inline(b, f->value);
            }
            sb_add(b, " }");
            break;
        }
    }
}

/* 写一个「元素」：扁平的走单行（dump_inline），含结构的**就地展开**成多行。
   调用方负责**已**写好本元素开头的换行与缩进（`indent` 即该缩进级别），
   因此这里不在开头补缩进；展开出来的续行由各自递归负责对齐。
   对应 Rust `dump.rs::dump_element`（W4 ② 的对齐点）。 */
static void dump_element(sbuf *b, const sml_value *v, int indent) {
    if (!v) { sb_add(b, "null"); return; }
    if (is_flat(v)) { dump_inline(b, v); return; }
    switch (v->type) {
        case SML_OBJECT:
            /* 与 dump_value 的 OBJECT 分支的差别就在这一行：这里是数组元素位置，
               `{` 要跟在**当前行**（缩进已由调用方写好），不能再另起一行。 */
            sb_addc(b, '{');
            dump_object_body(b, v, indent);
            break;
        case SML_ARRAY:
            dump_array_body(b, v, indent);
            break;
        default:
            /* is_flat 为假只可能是容器；标量一律扁平。真到这儿也退化成单行。 */
            dump_inline(b, v);
            break;
    }
}

char *sml_dump(const sml_value *v) {
    if (!v) return NULL;
    sbuf b;
    memset(&b, 0, sizeof(b));
    if (v->type == SML_OBJECT) {
        sml_field *f;
        /* 顶层分叉与 Rust `to_sml` 逐字对应：带 `__type` 的对象是**裸块的树形**
           （`type [name] { … }` 解析出来的），按 `dump_block(0,0)` 渲染 —— 先换行再
           `{`、逐键、收尾 `}`；空对象写 `{}`。不带 `__type` 的才是「顶层逐键成行」。
           （A 类：原先这里无条件逐键、并把 __type/__name 跳过。） */
        if (sml_obj_get(v, "__type") != NULL) {
            if (obj_has_body(v)) {
                sb_add(&b, "\n{");
                dump_object_body(&b, v, 0);
            } else {
                sb_add(&b, "{}");
            }
        } else {
            for (f = v->u.obj.head; f; f = f->next) {
                sb_add(&b, f->key);
                /* 同 dump_value：值是有体的对象时（另起一行渲染）**不留**行尾空格（W4 ①）。
                   顶层这里是绝大多数 `key: ` 尾随空格的来源。 */
                if (obj_has_body(f->value)) sb_add(&b, ":");
                else                        sb_add(&b, ": ");
                dump_value(&b, f->value, 0);
                sb_addc(&b, '\n');
            }
        }
    } else {
        /* 顶层非对象：与数组元素同一套规则（扁平单行 / 含结构展开），
           Rust 侧 to_sml 走的也是 dump_element。W4 ② 之前这里无脑 dump_inline，
           顶层一个嵌套数组会被整坨压进一行。 */
        dump_element(&b, v, 0);
    }
    return b.buf ? b.buf : strdup("");
}

