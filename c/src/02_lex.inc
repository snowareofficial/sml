/* =====================================================================
** 2. 词法
** ===================================================================== */

typedef enum {
    T_LBRACE, T_RBRACE, T_LBRACK, T_RBRACK, T_COMMA, T_COLON, T_AT, T_BAREAT,
    T_QMARK, T_EQ,   /* 契约修饰符 ? 与默认值 = (独立 token, 确保 str? / = 不并入裸词) */
    T_STR,   /* 引号串 (已解码, 含 \u 转义) */
    T_WORD,  /* 裸词 */
    T_EOF,
} tok_type;

typedef struct {
    tok_type t;
    char *v;      /* T_STR / T_WORD 的内容（NUL 结尾，兼容 C 串用法） */
    size_t len;   /* 字节长度：引号串解码后可能含 `\0`，此时 strlen 不够用 */
    int line;     /* 该 token 起始行（1 起）；任务 D 的 @is 同行判据用 */
} token;

typedef struct {
    token *toks;
    size_t n, cap, pos;
    int cur_line;    /* 当前行（lex_run 维护；token.line 的来源） */
    char *errbuf;
    size_t errsz;
    int failed;      /* 词法错误已记（只记第一条）；sml_parse 见到它立即返回 NULL */
    unsigned features;  /* 预处理扫出的特性集（供引号串 `$env.` 内插等处判定） */
} lexer;

/* 词法错误写入：**只记第一条**（与 Rust 的 `tokenize()` 返回 Err 即短路同义），
   码写在消息最前（口径同 set_err）。为什么必须短路：词法与语法共用一个 err 缓冲，
   若带着词法错误继续解析，解析器写的后续错误会**覆盖**它 —— 用户拿到的码就不是
   第一个错的码了（W16 之前 C 的 LEX 层根本不报，这个"谁覆盖谁"的问题因此没暴露）。

   复用的 `errbuf`/`errsz` 与 set_err 同一约定：缓冲区为空或大小为 0 时一个字节都不写。 */
static void lex_err(lexer *lx, const char *fmt, ...) {
    va_list ap;
    if (lx->failed) return;
    lx->failed = 1;
    if (!lx->errbuf || lx->errsz == 0) return;
    va_start(ap, fmt);
    vsnprintf(lx->errbuf, lx->errsz, fmt, ap);
    va_end(ap);
}

static void lex_push(lexer *lx, tok_type t, char *v) {
    if (lx->n >= lx->cap) {
        size_t ncap = lx->cap ? lx->cap * 2 : 64;
        token *nt = (token *)realloc(lx->toks, ncap * sizeof(token));
        if (!nt) return;
        lx->toks = nt;
        lx->cap = ncap;
    }
    lx->toks[lx->n].t = t;
    lx->toks[lx->n].v = v;
    lx->toks[lx->n].len = v ? strlen(v) : 0;
    lx->toks[lx->n].line = lx->cur_line;
    lx->n++;
}

/* 同 lex_push，但**显式给长度**：引号串里 `\0` 之后的字节不能丢。
   `v` 不必以 NUL 结尾；这里复制 n 字节并自己补终止符。 */
static void lex_pushn(lexer *lx, tok_type t, const char *v, size_t n) {
    char *copy = (char *)malloc(n + 1);
    if (!copy) return;
    memcpy(copy, v, n);
    copy[n] = '\0';
    if (lx->n >= lx->cap) {
        size_t ncap = lx->cap ? lx->cap * 2 : 64;
        token *nt = (token *)realloc(lx->toks, ncap * sizeof(token));
        if (!nt) { free(copy); return; }
        lx->toks = nt;
        lx->cap = ncap;
    }
    lx->toks[lx->n].t = t;
    lx->toks[lx->n].v = copy;
    lx->toks[lx->n].len = n;
    lx->toks[lx->n].line = lx->cur_line;
    lx->n++;
}

static void lex_free(lexer *lx) {
    size_t i;
    for (i = 0; i < lx->n; i++) free(lx->toks[i].v);
    free(lx->toks);
    lx->toks = NULL;
    lx->n = lx->cap = 0;
}

static void lex_run(lexer *lx, const char *text) {
    const char *p = text;
    char buf[1024];
    size_t blen = 0;
    lx->cur_line = 1;
#define FLUSH() do { if (blen) { buf[blen] = '\0'; lex_push(lx, T_WORD, strdup(buf)); blen = 0; } } while (0)
    while (*p) {
        char c = *p;
        if (c == '#') {
            /* 单行注释到行尾 */
            while (*p && *p != '\n') p++;
        } else if (c == '-' && p[1] == '-') {
            /* `--` 单行注释到行尾 */
            while (*p && *p != '\n') p++;
        } else if (c == '/' && p[1] == '/') {
            /* 斜杠斜杠 单行注释到行尾 */
            while (*p && *p != '\n') p++;
        } else if (c == '/' && p[1] == '*') {
            /* 斜杠星 多行注释，直到 星斜杠。**EOF 未闭合 ⇒ E-LEX-002**
               （W16：改前静默吃到文件结尾 —— 后面整篇内容凭空消失，一个错都不报）。 */
            p += 2;
            int closed = 0;
            while (*p) {
                if (*p == '\n') lx->cur_line++;
                if (*p == '*' && p[1] == '/') { p += 2; closed = 1; break; }
                p++;
            }
            if (!closed)
                lex_err(lx, SML_E_LEX_002 " 未闭合的块注释 /* ... */（遇到文件结尾）");
        } else if (c == '_' && p[1] == '*') {
            /* `_*` 多行注释，直到 `*_`。**EOF 未闭合 ⇒ E-LEX-003**（同上）。 */
            p += 2;
            int closed = 0;
            while (*p) {
                if (*p == '\n') lx->cur_line++;
                if (*p == '*' && p[1] == '_') { p += 2; closed = 1; break; }
                p++;
            }
            if (!closed)
                lex_err(lx, SML_E_LEX_003 " 未闭合的块注释 _* ... *_（遇到文件结尾）");
        } else if (c == '"') {
            FLUSH();
            p++;
            char sb[4096];
            size_t slen = 0;
            int closed = 0;   /* 见过结束引号；否则 EOF ⇒ E-LEX-001 */
            while (*p) {
                if (*p == '"') { p++; closed = 1; break; }
                /* 转义符后**直接**遇文件结尾 ⇒ E-LEX-006（与 Rust 词法层同码）。
                   语义上不是「少了结束引号」，而是「少了一个被转义的字符」——
                   修复动作不同（补字符 vs 补引号），故不能并进 001。
                   T21 裁决（2026-11-06）：五端统一到 006。 */
                if (*p == '\\' && !p[1]) {
                    lex_err(lx, SML_E_LEX_006 " 字符串中的转义符后遇到文件结束");
                    break;
                }
                if (*p == '\\' && p[1]) {
                    p++;
                    char e = *p;
                    switch (e) {
                        case 'n': sb[slen++] = '\n'; break;
                        case 't': sb[slen++] = '\t'; break;
                        case 'r': sb[slen++] = '\r'; break;
                        case '0': sb[slen++] = '\0'; break;
                        case '"': sb[slen++] = '"'; break;
                        case '\\': sb[slen++] = '\\'; break;
                        case 'u': {
                            /* \uXXXX（定长四位）或 \u{...}。两类失败都报 E-LEX-005：
                               ① 定长形式**不足 4 位**（改前照收：`"\u12"` 静默变成控制字符
                                  U+0012，Rust/JS 都报 005，C 是唯一的异类）；
                               ② 非十六进制、花括号未闭合、空 hex；
                               ③ 非法码点（代理区 D800-DFFF / 超出 U+10FFFF）——
                                  put_utf8 会把它们编成非法 UTF-8，正是 Rust
                                  `char::from_u32` 拒绝的那两类。 */
                            int has_brace = 0;
                            if (*(p + 1) == '{') { has_brace = 1; p++; }
                            unsigned long cp = 0;
                            int cnt = 0, bad = 0;
                            while (1) {
                                if (has_brace) {
                                    if (*(p + 1) == '}') { p++; break; }
                                    if (!*(p + 1)) { bad = 1; break; }
                                } else {
                                    if (cnt >= 4) break;
                                }
                                int h = hexdigit((unsigned char)*(p + 1));
                                if (h < 0) { bad = 1; break; }
                                cp = cp * 16 + (unsigned long)h;
                                cnt++;
                                p++;
                            }
                            if (!bad && !has_brace && cnt != 4) bad = 1;
                            if (!bad && cnt == 0) bad = 1;
                            if (!bad && (cp > 0x10FFFFUL ||
                                         (cp >= 0xD800UL && cp <= 0xDFFFUL))) bad = 1;
                            if (bad)
                                lex_err(lx, SML_E_LEX_005
                                        " Unicode 转义非法（位数不足、非十六进制、或非法码点）");
                            else
                                put_utf8(sb, &slen, cp);
                            break;
                        }
                        default:
                            /* 未知转义 ⇒ E-LEX-004。改前 default 只把该字符原样收下、
                               **反斜杠直接丢掉**：`"C:\Users"` 静默变成 `C:Users`
                               （路径/正则被悄悄改坏）。Rust 与 C++ 同为严格策略。 */
                            lex_err(lx, SML_E_LEX_004
                                    " 字符串含未知转义符 \\%c（仅支持 \\n \\t \\r \\0 \\\" \\\\ \\uXXXX）", e);
                            break;
                    }
                    p++;
                } else {
                    sb[slen++] = *p++;
                }
                if (slen >= sizeof(sb) - 5) break;
            }
            sb[slen] = '\0';
            /* 未闭合字符串 ⇒ E-LEX-001（改前把余下全文当串内容，静默吞掉整篇）。
               注意只在**真的遇到文件结尾**时报：上面那个缓冲区保护 break 不属于此条件。 */
            if (!closed && *p == '\0')
                lex_err(lx, SML_E_LEX_001 " 字符串未闭合（缺少结束引号）");
            /* 内插 `$env.NAME`（与 Rust `interp_str` 同语义）：NAME 取 [字母数字_ .]，
               取到第一个其它字符为止；无 `$env.` 或名字为空时原样保留。
               原先只做「整串恰为 `$env.NAME`」，内嵌形式不替换 —— 那是 conformance 26
               的分歧；四端统一为内插后该用例转 AGREE。
               ｜ Inline `$env.NAME`, same semantics as the Rust reference. */
            /* env 门控：引号串含 `$env.` 且未开 env ⇒ E-FEATURE-002（与 Rust 同判据）。
               `lx->features` 由预处理扫出，词法阶段可用。 */
            if (strstr(sb, "$env.") != NULL && !(lx->features & (1u << 2))) {
                lex_err(lx, SML_E_FEATURE_002
                        " 字符串 `\"%s\"` 无法解析：未启用特性 `env`", sb);
                break;
            }
            {
                char tmp[sizeof(sb)];
                const char *src = sb;
                size_t ti = 0;
                int changed = 0;
                while (*src && ti + 1 < sizeof(tmp)) {
                    if (strncmp(src, "$env.", 5) == 0) {
                        const char *name = src + 5;
                        const char *q = name;
                        while (*q && ((*q >= 'a' && *q <= 'z') || (*q >= 'A' && *q <= 'Z')
                                      || (*q >= '0' && *q <= '9') || *q == '_' || *q == '.')) q++;
                        if (q > name) {
                            char var[256];
                            size_t nl = (size_t)(q - name);
                            if (nl >= sizeof(var)) nl = sizeof(var) - 1;
                            memcpy(var, name, nl);
                            var[nl] = '\0';
                            const char *ev = getenv(var);
                            const char *val = ev ? ev : "";
                            size_t vl = strlen(val);
                            if (ti + vl + 1 >= sizeof(tmp)) vl = sizeof(tmp) - ti - 2;
                            memcpy(tmp + ti, val, vl);
                            ti += vl;
                            src = q;
                            changed = 1;
                            continue;
                        }
                    }
                    tmp[ti++] = *src++;
                }
                tmp[ti] = '\0';
                if (changed) { memcpy(sb, tmp, ti + 1); slen = ti; }
            }
            /* 按**字节长度**入队：`\0` 转义解出的真 NUL 之后还有内容，
               strdup 会在 NUL 处截断（台账 S-06 的 C 半）。 */
            lex_pushn(lx, T_STR, sb, slen);
        } else if (c == '{') { FLUSH(); lex_push(lx, T_LBRACE, NULL); p++; }
        else if (c == '}') { FLUSH(); lex_push(lx, T_RBRACE, NULL); p++; }
        else if (c == '[') { FLUSH(); lex_push(lx, T_LBRACK, NULL); p++; }
        else if (c == ']') { FLUSH(); lex_push(lx, T_RBRACK, NULL); p++; }
        else if (c == ',') { FLUSH(); lex_push(lx, T_COMMA, NULL); p++; }
        else if (c == ':') { FLUSH(); lex_push(lx, T_COLON, NULL); p++; }
        else if (c == '?') { FLUSH(); lex_push(lx, T_QMARK, NULL); p++; }
        else if (c == '=') { FLUSH(); lex_push(lx, T_EQ, NULL); p++; }
        else if (c == '$' && p[1] == '{') {
            /* `${var}` 整体作为一个 WORD（`@for` 插值）：否则 `{` / `}` 会被当结构记号切开，
               键名位置也认不出 `${`（`${h}_port` 会散成 `${h`、`}`、`_port`）。
               Keep `${var}` as one word so it survives structural splitting. */
            const char *close = strchr(p + 2, '}');
            if (close) {
                size_t len = (size_t)(close - p) + 1;
                if (blen + len < sizeof(buf)) {
                    memcpy(buf + blen, p, len);
                    blen += len;
                }
                p = close + 1;
            } else {
                if (blen < sizeof(buf) - 1) buf[blen++] = c;
                p++;
            }
        }
        /* `@` 仅当位于**词首**时才是片段定义标记（`@base { ... }`）。
        ** 出现在词中间时（典型如邮箱 `a@b.c`）必须作为普通字符保留：
        ** 否则 `a@b.c` 会被切成 WORD("a") + AT + WORD("b.c")，
        ** 后半段在解析时被丢弃，导致邮箱静默损坏为 `a`。 */
        else if (c == '@') {
            if (blen == 0) {
                /* 孤立 `@`：其后紧跟空白 / 行尾 ⇒ 没有指令名或片段名。
                   与 Rust 的 `Tok::BareAt` 同判据（sml-lex/src/lib.rs 的 `'@'` 分支）。
                   不区分的话，`@` + 下一行的 `k` 会被拼成 `@k` 未注册指令（E-PARSE-005）。 */
                char n = p[1];
                FLUSH();
                lex_push(lx, (n == '\0' || n == ' ' || n == '\t' || n == '\n' || n == '\r')
                             ? T_BAREAT : T_AT, NULL);
            }
            else { if (blen < sizeof(buf) - 1) buf[blen++] = c; }
            p++;
        }
        else if (c == ' ' || c == '\t' || c == '\n' || c == '\r') { FLUSH(); if (c == '\n') lx->cur_line++; p++; }
        else {
            if (blen < sizeof(buf) - 1) buf[blen++] = c;
            p++;
        }
    }
    FLUSH();
    lex_push(lx, T_EOF, NULL);
#undef FLUSH
}

