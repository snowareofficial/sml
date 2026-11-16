/* =====================================================================
** 6. include / @include 文本内联 (对齐 Rust resolve_includes)
** ===================================================================== */

#define MAX_INC_DEPTH 32
/* 全局 include 展开次数上限：防「菱形包含」（A 含 B、C，B 与 C 又各含 D…）
   造成指数级文件读取。与 Rust 侧 MAX_INCLUDE_EXPANSIONS 同口径。 */
#define MAX_INC_EXPANSIONS 256

static void path_dir(const char *path, char *out, size_t outsz) {
    const char *slash = strrchr(path, '/');
    const char *bslash = strrchr(path, '\\');
    const char *last = (slash > bslash) ? slash : bslash;
    if (!last) { strncpy(out, ".", outsz - 1); out[outsz - 1] = '\0'; return; }
    size_t n = (size_t)(last - path);
    if (n >= outsz) n = outsz - 1;
    memcpy(out, path, n);
    out[n] = '\0';
}

/* 若行是 include 指令, 返回目标路径 (调用方 free); 否则返回 NULL */
static char *try_include_target(const char *line, char *err, size_t errsz) {
    lexer lx;
    memset(&lx, 0, sizeof(lx));
    /* 给词法器一个错误缓冲：下面要区分「引号未闭合」（`E-LEX-001`）与其它词法错，
       所以这次**不能**像以前那样把 errbuf 留空。 */
    char lexbuf[256];
    lexbuf[0] = '\0';
    lx.errbuf = lexbuf;
    lx.errsz = sizeof(lexbuf);
    lex_run(&lx, line);
    char *res = NULL;
    if (lx.n >= 2) {
        token *a = &lx.toks[0];
        token *b = &lx.toks[1];
        if (a->t == T_AT && lx.n >= 3) { a = &lx.toks[1]; b = &lx.toks[2]; }
        if (a->t == T_WORD && strcmp(a->v, "import") == 0) {
            /* `import { keys }` 是部分引用（本端不实现）：显式拒绝，别静默当普通内容。
               裸 `import "x"` 保持原状（本端不展开 import），故不在此处理。 */
            if (b->t == T_LBRACE)
                set_err(err, errsz, SML_E_FEATURE_001 " 部分引用（{ keys }）本实现未启用");
        } else if (a->t == T_WORD && strcmp(a->v, "include") == 0) {
            size_t bi = (size_t)(b - lx.toks);
            if (b->t == T_LBRACE) {
                /* `include { keys }`：部分引用，本端不实现 ⇒ 显式拒绝。 */
                set_err(err, errsz, SML_E_FEATURE_001 " 部分引用（{ keys }）本实现未启用");
            } else if (b->t == T_STR && bi + 1 < lx.n && lx.toks[bi + 1].t == T_LBRACE) {
                /* `include "x" { keys }`：改前会展开整个文件、`{ keys }` 静默丢失。 */
                set_err(err, errsz, SML_E_FEATURE_001 " 部分引用（{ keys }）本实现未启用");
            } else if (b->t == T_STR) {
                /* 引号未闭合时词法器**仍会**推进一个 T_STR（内容是残缺路径），于是旧实现拿它去
                   fopen ⇒ 报 `E-INCLUDE-001`（"读取失败"），把**写法**错误导成"文件不存在"。
                   与 Rust / Lua 对齐：引号未闭合归 `E-INCLUDE-012`。
                   ⚠️ 只认"未闭合"这一种词法错（比对码），其它词法错（如未知转义）保持旧行为。 */
                if (lx.failed && strncmp(lexbuf, SML_E_LEX_001, strlen(SML_E_LEX_001)) == 0) {
                    set_err(err, errsz,
                            SML_E_INCLUDE_012 " include 路径写法非法（引号未闭合）");
                } else {
                    res = strdup(b->v);
                }
            } else if (b->t == T_WORD) {
                /* **未加引号**：旧实现这里返回 NULL ⇒ 整行被当普通内容写回，指令意图被**静默**
                   丢弃（连错都不报）。与 Rust / Lua 对齐：明确报 `E-INCLUDE-012`。
                   注：`include:`（T_COLON）/ `include {`（T_LBRACE）/ 只有 `include`（lx.n<2）
                   都不会命中此分支 —— 那些不是"路径写法错"。 */
                set_err(err, errsz, SML_E_INCLUDE_012 " include 路径写法非法（未加引号）");
            }
        }
    }
    lex_free(&lx);
    return res;
}

/* 递归展开 include。stack 为已展开文件规范路径 (防环) */
static int resolve_includes(const char *text, const char *base,
                            sbuf *out, char (*stack)[1024], int depth,
                            long *expansions,
                            char *err, size_t errsz, unsigned features) {
    if (depth >= MAX_INC_DEPTH) {
        set_err(err, errsz, SML_E_INCLUDE_004 " include 嵌套超过 %d 层", MAX_INC_DEPTH);
        return -1;
    }
    /* 主文件行首的 UTF-8 BOM：本函数按行切分后判断「行首是不是 include」，
       BOM 会粘在首词上 ⇒ `\xEF\xBB\xBFinclude "x"` 认不出来，整行被当普通内容写回，
       最终变成字段 `include: "x"`（静默错树）。`sml_parse` 里那处去 BOM 管不到这里
       （它在 include 展开**之后**才跑）。子文件的 BOM 在读取处已就地移除。 */
    if (depth == 0 && text[0] && text[1] && text[2]
        && (unsigned char)text[0] == 0xEF
        && (unsigned char)text[1] == 0xBB && (unsigned char)text[2] == 0xBF) {
        text += 3;
    }
    /* 展开次数是**全局**计数（跨整棵包含树），嵌套深度限制挡不住菱形包含 */
    if (++(*expansions) > MAX_INC_EXPANSIONS) {
        set_err(err, errsz, SML_E_LIMIT_003 " include 展开次数超过 %d 次上限", MAX_INC_EXPANSIONS);
        return -1;
    }
    const char *p = text;
    while (*p) {
        const char *nl = strchr(p, '\n');
        size_t linelen = nl ? (size_t)(nl - p) : strlen(p);
        char *line = (char *)malloc(linelen + 1);
        if (!line) { set_err(err, errsz, SML_E_LIMIT_010 " 内存分配失败（include 行缓冲）"); return -1; }
        memcpy(line, p, linelen);
        line[linelen] = '\0';

        /* 先查「写法非法」：是 include 指令、但路径没加引号（或引号未闭合）⇒ 明确报码，
           而不是像旧实现那样当普通内容**静默**放过。 */
        char terr[512];
        terr[0] = '\0';
        char *inc = try_include_target(line, terr, sizeof(terr));
        if (!inc && terr[0]) {
            set_err(err, errsz, "%s", terr);
            free(line);
            return -1;
        }
        if (inc) {
            /* include 门控：`@feature disable include` 后必须拒绝（与 Rust 同码 001）。 */
            if (!(features & (1u << 1))) {
                set_err(err, errsz, SML_E_FEATURE_001 " include 特性未启用");
                free(line); free(inc);
                return -1;
            }
            char path[1024];
            snprintf(path, sizeof(path), "%s/%s", base, inc);
            FILE *f = fopen(path, "rb");
            if (!f) {
                set_err(err, errsz, SML_E_INCLUDE_001 " include 读取失败 %s", path);
                free(line); free(inc);
                return -1;
            }
            fseek(f, 0, SEEK_END);
            long sz = ftell(f);
            fseek(f, 0, SEEK_SET);
            char *content = (char *)malloc((size_t)sz + 1);
            if (!content) { fclose(f); set_err(err, errsz, SML_E_LIMIT_010 " 内存分配失败（include 文件内容）"); free(line); free(inc); return -1; }
            fread(content, 1, (size_t)sz, f);
            content[sz] = '\0';
            /* 子文件同样可能带 BOM：它在**拼接后的文本中间**，`sml_parse` 那处的开头检查救不了，
               而且不去掉的话连它的第一行 `include "x"` 都认不出来（BOM 会粘在首词上）。
               就地把 BOM 移掉（含结尾 NUL 一起搬）。 */
            if ((size_t)sz >= 3 && (unsigned char)content[0] == 0xEF
                && (unsigned char)content[1] == 0xBB && (unsigned char)content[2] == 0xBF) {
                memmove(content, content + 3, (size_t)sz - 2);
            }
            fclose(f);

            /* 规范化为绝对路径。改用**动态分配**：固定 1024 缓冲在长路径下
               会规范化失败，若那时回落到未规范化的 path，下面的越界校验与
               循环检测就等于失效（Rust 侧口径是 canonicalize + starts_with）。 */
            char *canon = NULL;
#ifdef _WIN32
            canon = _fullpath(NULL, path, 0);
#else
            canon = realpath(path, NULL);
#endif
            if (!canon) {
                set_err(err, errsz, SML_E_INCLUDE_001 " include 路径无法解析: %s", path);
                free(content); free(line); free(inc);
                return -1;
            }
            /* 路径穿越防护：被包含文件必须仍在基准目录之内。
               缺这道校验时 `include "../../etc/passwd"` 会把任意文件内容
               内联进解析结果。 */
            char *basec = NULL;
#ifdef _WIN32
            basec = _fullpath(NULL, base, 0);
#else
            basec = realpath(base, NULL);
#endif
            if (basec) {
                size_t bl = strlen(basec);
                int inside = strncmp(canon, basec, bl) == 0 &&
                             (canon[bl] == '/' || canon[bl] == '\\' || canon[bl] == '\0');
                free(basec);
                if (!inside) {
                    set_err(err, errsz, SML_E_INCLUDE_003 " include 目标越出基准目录: %s", inc);
                    free(canon); free(content); free(line); free(inc);
                    return -1;
                }
            } else {
                /* 基准目录无法规范化 → **拒绝**，不能因为"没法比"就放行
                   （fail-open 会让越界读取在校验失败时静默通过）。 */
                set_err(err, errsz, SML_E_INCLUDE_010 " include 基准目录不可解析，已拒绝: %s", base);
                free(canon); free(content); free(line); free(inc);
                return -1;
            }
            int cyc = 0;
            for (int i = 0; i < depth; i++)
                if (strcmp(stack[i], canon) == 0) { cyc = 1; break; }
            if (cyc) {
                set_err(err, errsz, SML_E_INCLUDE_002 " include 循环引用: %s", canon);
                free(canon); free(content); free(line); free(inc);
                return -1;
            }
            strncpy(stack[depth], canon, 1023);
            stack[depth][1023] = '\0';

            /* 子基准用**规范化后的**路径，避免把未规范化的路径继续传下去 */
            char childbase[1024];
            path_dir(canon, childbase, sizeof(childbase));
            free(canon);
            if (resolve_includes(content, childbase, out, stack, depth + 1, expansions, err, errsz, features) != 0) {
                free(content); free(line); free(inc);
                return -1;
            }
            free(content);
        } else {
            sb_add(out, line);
            /* 末行补换行是历史行为，但源文本**以反斜杠结尾**时不能补：补了就把
               「转义符后遇文件结束」（E-LEX-006）改写成「未知转义」（E-LEX-004）——
               那是按行重建的实现副作用，不是语义差别。与 Rust 的
               `needs_trailing_newline` 同判据（`sml-parse/src/scan.rs`）。 */
            if (nl || !(linelen > 0 && line[linelen - 1] == '\\')) sb_addc(out, '\n');
        }
        free(line);
        free(inc);
        p = nl ? nl + 1 : p + strlen(p);
    }
    return 0;
}

/* 文件入口的核心：读文件 → 展开 include → 解析，并按需交出诊断 / 弱表。
   | Core of the file entries: read, expand includes, parse, collect outputs. */
static sml_value *parse_file_core(const char *path, char *err, size_t errsz,
                                  sml_diag **diags_out, size_t *n_diags_out,
                                  const sml_builtin *b,
                                  sml_weakbind **binds_out, size_t *n_binds_out) {
    if (!path) {
        if (err && errsz) set_err(err, errsz, SML_E_INTERNAL_001 " 空指针入参（path 为 NULL）");
        return NULL;
    }
    FILE *f = fopen(path, "rb");
    if (!f) {
        if (err && errsz) set_err(err, errsz, SML_E_IO_001 " 读取失败 %s", path);
        return NULL;
    }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *text = (char *)malloc((size_t)sz + 1);
    if (!text) { fclose(f); set_err(err, errsz, SML_E_LIMIT_010 " 内存分配失败（入口文件内容）"); return NULL; }
    fread(text, 1, (size_t)sz, f);
    text[sz] = '\0';
    fclose(f);

    char base[1024];
    path_dir(path, base, sizeof(base));
    sbuf out;
    memset(&out, 0, sizeof(out));
    /* include 环检测栈：`char stack[33][1024]` 曾直接放在栈上 ⇒ 本函数帧 35 KB。
       本函数虽然只进一次，但入口帧过大对**小栈线程**（8 KB~64 KB 的宿主 / WASM）是硬伤，
       且这份数据天生属于「按需分配的临时表」，故改到堆上。 */
    char (*stack)[1024] = (char (*)[1024])calloc(MAX_INC_DEPTH + 1, 1024);
    if (!stack) {
        free(text);
        set_err(err, errsz, SML_E_LIMIT_010 " 内存分配失败（include 环检测栈）");
        return NULL;
    }
    long expansions = 0;
    if (resolve_includes(text, base, &out, stack, 0, &expansions, err, errsz,
                         scan_features(text)) != 0) {
        free(text);
        free(stack);
        free(out.buf);
        return NULL;
    }
    free(stack);
    free(text);
    sml_value *v = parse_core(out.buf ? out.buf : "", err, errsz,
                              diags_out, n_diags_out, b, binds_out, n_binds_out);
    free(out.buf);
    return v;
}

sml_value *sml_parse_file(const char *path, char *err, size_t errsz) {
    return parse_file_core(path, err, errsz, NULL, NULL, NULL, NULL, NULL);
}

sml_value *sml_parse_file_ex(const char *path, char *err, size_t errsz,
                             sml_diag **diags, size_t *n_diags) {
    return parse_file_core(path, err, errsz, diags, n_diags, NULL, NULL, NULL);
}

sml_value *sml_parse_file_full(const char *path, char *err, size_t errsz,
                               const sml_builtin *b,
                               sml_diag **diags, size_t *n_diags,
                               sml_weakbind **binds, size_t *n_binds) {
    return parse_file_core(path, err, errsz, diags, n_diags, b, binds, n_binds);
}
