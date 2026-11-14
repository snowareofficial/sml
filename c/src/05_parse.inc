/* 解析块/对象。closing=T_RBRACE 或 T_EOF(顶层) */
static sml_value *parse_block_inner(parser *ps, tok_type closing) {
    sml_value *obj = sml_new_object();
    char *block_is = NULL;  /* 块级 @is 契约名 (作用于本块) */
    for (;;) {
        token *t = peek(ps);
        if (ps->failed) break;   /* 出错/超限后立刻收手，让栈退掉（见 parse_block） */
        if (t->t == T_EOF) {
            /* 只在「本层期望 `}`」时才算未闭合；顶层（closing==T_EOF）遇 EOF 是正常结束 */
            if (closing == T_RBRACE) err_unclosed(ps, "块");
            break;
        }
        if (t->t == T_RBRACE || t->t == T_RBRACK) {
            if (closing == t->t) { next(ps); break; }
            if (closing == T_EOF) {
                /* 顶层（closing==T_EOF）遇到结束符号：没有与之匹配的开始符号 ⇒
                   E-PARSE-003（W16）。改前这里直接 break：`a: 1` + `}` 静默成功，
                   而 Rust / JS 都报 003。 */
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_003 " 多余的结束符号 `%s`，没有与之匹配的开始符号",
                        t->t == T_RBRACE ? "}" : "]");
                ps->failed = 1;
                break;
            }
            /* 本层期望 `}`，却遇到 `]` ⇒ 闭合符错配 E-PARSE-002（W16）。
               改前这一格落到下面的 key 分支，`kt->t` 既不是 WORD 也不是 STR 就
               直接 break —— `a { ] }` 静默得到 `{"a":{}}`（Rust 报 002，JS 同批已改）。 */
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_002 " 闭合符错配：期望 `}`，实得 `%s`",
                    t->t == T_RBRACE ? "}" : "]");
            ps->failed = 1;
            break;
        }
        if (t->t == T_COMMA) {
            /* 逗号只用于对象字段分隔：其后须是 `key:`；否则是裸词拆分
               （`a: x,y`）或孤立逗号 —— 报错而非静默吞掉（与 Rust 同判据）。 */
            token *c1 = peek_at(ps, 1);
            token *c2 = peek_at(ps, 2);
            if (!((c1->t == T_WORD || c1->t == T_STR) && c2->t == T_COLON)) {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_007 " 非预期的逗号：裸词中不可含逗号，请改用数组或引号包裹");
                ps->failed = 1;
                break;
            }
            next(ps); continue;
        }
        if (t->t == T_BAREAT) {
            /* 孤立 `@`（其后是空白 / 行尾）：报错，不把下一行的键当指令名，
               也不把紧随的块当片段体吞掉（Rust 的 Tok::BareAt 同判据）。 */
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_004 " 孤立的 @：其后没有指令名或片段名");
            ps->failed = 1;
            break;
        }
        if (t->t == T_AT) {
            token *nxt = peek_at(ps, 1);
            /* 默认忽略：未开 `type.weak` 时 `@weakbind` / `@weakdata` / `@weakdoc`
               不解释、不校验、不产出（`docs/swsml/spec/weakbind.md` §0.3）。
               片段定义形态（后跟 `{`）**不消费**，落回原有语义，不抢占名字；
               其余形态把「指令 + 同一行的参数」吞掉，值树零影响。
               Default-ignore: swallow weak* directives unless `type.weak` is on. */
            if (nxt && nxt->t == T_WORD
                && (strcmp(nxt->v, "weakbind") == 0
                    || strcmp(nxt->v, "weakdata") == 0
                    || strcmp(nxt->v, "weakdoc") == 0)
                && !(ps->features & (1u << 19))) {
                /* 2026-11-13 裁决：未开 `type.weak` 时**响亮拒绝**（与 Rust 同码 001），
                   不做「默认忽略」—— 少一个静默路径，也少一处四端与 Rust 的对照缺口。
                   Loudly reject weak* when `type.weak` is off (same code as Rust). */
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_FEATURE_001 " `%s`：当前文档已禁用该特性"
                        "（请先写 `@feature enable type.weak`）", nxt->v);
                ps->failed = 1;
                break;
            }
            /* 开了 `type.weak`：weak* 三条指令**登记但不解释**，值树零影响。
               With `type.weak` on: acknowledge the directives, never touch the value. */
            if (nxt && nxt->t == T_WORD
                && (strcmp(nxt->v, "weakbind") == 0
                    || strcmp(nxt->v, "weakdoc") == 0
                    || strcmp(nxt->v, "weakdata") == 0)) {
                int is_bind = (strcmp(nxt->v, "weakbind") == 0);
                int is_doc = (strcmp(nxt->v, "weakdoc") == 0);
                token *nm;
                next(ps);   /* @ */
                next(ps);   /* weak* */
                nm = peek(ps);
                if (!nm || (nm->t != T_WORD && nm->t != T_STR)) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_019 " `@%s` 后须名字", nxt->v);
                    ps->failed = 1;
                    break;
                }
                next(ps);   /* 契约名 / profile 名 */
                if (is_bind) {
                    /* 挂起到「紧邻的下一个块」；行号与所在层供收尾时判悬空。 */
                    ps->pending_weak = nm->v;
                    ps->pending_weak_line = nm->line;
                    ps->pending_weak_depth = ps->depth;
                }
                if (is_bind && ps->builtin) {
                    /* 声明侧：契约名在内置表里吗？ */
                    ccontract *bc;
                    for (bc = ps->builtin->contracts; bc; bc = bc->next) {
                        if (strcmp(bc->name, nm->v) == 0) break;
                    }
                    if (!bc) {
                        char dmsg[256];
                        snprintf(dmsg, sizeof(dmsg), "内置契约表里没有 `%s`", nm->v);
                        diag_push(ps, "W-CONTRACT-001", dmsg);
                    }
                }
                /* 可选体 `{ 字段: 类型 ... }`：判据必须看**同一行** ——
                   `@weakbind tls` 换行后的 `server { ... }` 是下一个块，不是体。 */
                if (is_bind && peek(ps)->t == T_LBRACE && peek(ps)->line == nm->line) {
                    token *fld;
                    next(ps);   /* { */
                    while ((fld = peek(ps)) && fld->t != T_RBRACE) {
                        token *ty;
                        if (fld->t != T_WORD && fld->t != T_STR) break;
                        next(ps);
                        if (peek(ps)->t == T_COLON) next(ps);
                        ty = peek(ps);
                        if (ty && (ty->t == T_WORD || ty->t == T_STR)) next(ps);
                        weak_check_decl(ps, nm->v, fld->v, ty ? ty->v : NULL);
                    }
                    if (peek(ps)->t == T_RBRACE) next(ps);
                }
                if (is_doc && peek(ps)->t == T_LBRACE) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_019
                            " `@weakdoc` 只写 profile 名，不接受载荷块"
                            "（要内联数据请用 `@weakdata`）");
                    ps->failed = 1;
                    break;
                }
                if (!is_bind && !is_doc && peek(ps)->t == T_LBRACE) {
                    /* `@weakdata` 的载荷块：跳过配对花括号，不解释。 */
                    int d = 1;
                    next(ps);
                    while (d > 0 && peek(ps)->t != T_EOF) {
                        if (peek(ps)->t == T_LBRACE) d++;
                        else if (peek(ps)->t == T_RBRACE) d--;
                        next(ps);
                    }
                    if (d != 0) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_PARSE_001 " 未闭合的块（遇到文件结尾，缺少结束符号 }）");
                        ps->failed = 1;
                        break;
                    }
                }
                continue;
            }
            /* `@version vN` 是版本声明指令，不是片段定义（与 Rust/Lua/JS 对齐）。 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "version") == 0) {
                next(ps); next(ps); /* @ version */
                token *lit = peek(ps);
                if (lit->t == T_WORD || lit->t == T_STR) {
                    const int ver = version_lit_major(lit->v);
                    if (ver < 0) {
                        /* 5.0 族：写法合法，但该代尚未定稿（Dev）⇒ 与 Rust 同码。 */
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_004
                                " 版本 `%s` 尚未启用（5.0 仍为 Dev；本实现支持 v1..v4）",
                                lit->v);
                        ps->failed = 1;
                    } else if (ver == 0) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_004
                                " 未知版本 `%s`；仅支持 v1..v4（v1–v4 / 1.0–4.0 / 10–40）",
                                lit->v);
                        ps->failed = 1;
                    } else {
                        ps->version = ver;
                    }
                    next(ps);
                }
                continue;
            }
            /* `@feature enable|disable <名>[, <名>]` —— 记录开关位。
               名字表与 Rust 的 FEATURES 同序同名；本端**只识别、不门控**（见
               SML_FEATURE_NAMES 的注释）。`mode` / `base` 子命令本端未实现，
               明确拒绝而不是静默忽略（登记见 crossend-sync.md 的 P3）。 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "feature") == 0) {
                token *sub;
                next(ps); /* @ */
                next(ps); /* feature */
                sub = peek(ps);
                if (!sub || sub->t != T_WORD || sub->line != t->line) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_007 " `@feature` 缺少参数（enable / disable）");
                    ps->failed = 1;
                    break;
                }
                if (strcmp(sub->v, "enable") == 0 || strcmp(sub->v, "disable") == 0) {
                    int on = (sub->v[0] == 'e');
                    int any = 0;
                    int sub_line = sub->line;
                    next(ps);
                    for (;;) {
                        token *nm = peek(ps);
                        int bit;
                        if (!nm || nm->t != T_WORD || nm->line != sub_line) break;
                        bit = feature_index(nm->v);
                        if (bit < 0) {
                            /* 未知特性名是 003（002 的定义是「环境变量被禁用」）。
                               改前借用 002，编辑器与调用方会判错类别。 */
                            set_err(ps->lx->errbuf, ps->lx->errsz,
                                    SML_E_FEATURE_003 " 未知特性名 `%s`", nm->v);
                            ps->failed = 1;
                            break;
                        }
                        next(ps);
                        if (on) ps->features |= (1u << bit);
                        else    ps->features &= ~(1u << bit);
                        any = 1;
                        if (peek(ps)->t == T_COMMA) { next(ps); continue; }
                        break;
                    }
                    if (ps->failed) break;
                    if (!any) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_007 " `@feature %s` 缺少特性名", sub->v);
                        ps->failed = 1;
                        break;
                    }
                    continue;
                }
                /* `mode` 的参数校验是**语法层**的：参数不是 whitelist / blacklist 时报 008
                   （Rust 同码）。参数合法但本端未实现 mode，仍按「不支持该子命令」报 006
                   —— 不能把「没实现」谎报成「参数非法」。 */
                if (strcmp(sub->v, "mode") == 0) {
                    token *arg = peek(ps);
                    if (arg && arg->t == T_WORD && arg->line == sub->line
                        && (strcmp(arg->v, "whitelist") == 0
                            || strcmp(arg->v, "blacklist") == 0)) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_006 " `@feature mode`：本端暂不支持该子命令");
                    } else {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_FEATURE_008 " `@feature mode` 的参数须为 whitelist / blacklist");
                    }
                    ps->failed = 1;
                    break;
                }
                /* base 等其它子命令：本端暂不支持（Rust 端已实现，登记为跨端差异） */
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_FEATURE_006 " `@feature %s`：本端暂不支持该子命令", sub->v);
                ps->failed = 1;
                break;
            }
            /* `@when <条件>`：条件裁剪，只作用于紧邻的下一个字段/块。
               `@when <cond>` — trims the immediately following field/block. */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "when") == 0) {
                if (!(ps->features & (1u << 12))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `when`：当前文档已禁用该特性");
                    ps->failed = 1;
                    break;
                }
                if (ps->pending_when != 0) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_016 " `@when` 连续出现："
                            "它只作用于紧邻的下一个字段或块");
                    ps->failed = 1;
                    break;
                }
                next(ps); /* @ */
                next(ps); /* when */
                ps->pending_when = eval_when(ps) ? 1 : 2;
                if (ps->failed) break;
                continue;
            }
            /* `@contract Name [loose] { ... }` 定义契约 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "contract") == 0) {
                /* 特性门控：`@feature disable contract` 后必须拒绝（与 Rust 的
                   `!features.has(Feature::Contract)` ⇒ E-FEATURE-001 同判据）。 */
                if (!(ps->features & (1u << 3))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `contract`：当前文档已禁用该特性");
                    ps->failed = 1;
                    break;
                }
                next(ps); next(ps); /* @ contract */
                char *cname = next(ps)->v;
                int loose = 0;
                if (peek(ps)->t == T_WORD && strcmp(peek(ps)->v, "loose") == 0) {
                    loose = 1; next(ps);
                } else if (peek(ps)->t == T_WORD && strcmp(peek(ps)->v, "strict") == 0) {
                    /* 显式严格（与默认等价，写出来只为可读性/团队规范）—— Rust 与 JS
                       都接受它（rust/sml-parse/src/parser.rs、js/sml.mjs）。
                       此前 C **只认 loose**：`@contract X strict { ... }` 会让
                       parse_contract_body 见不到 `{` 而直接返回 —— 契约字段为空、
                       紧跟的 `{ ... }` 变成名为 `strict` 的数据块（静默给错树）。
                       W16 全仓扫描抓到（`_gov_demo.sml`）。这里只做**对齐**，不改语义：
                       strict ≡ 默认严格。 */
                    next(ps);
                }
                ccontract *c = (ccontract *)calloc(1, sizeof(ccontract));
                c->name = strdup(cname);
                c->allow_extra = loose;
                parse_contract_body(ps, c);
                c->next = ps->contracts;
                ps->contracts = c;
                continue;
            }
            /* `@is Name` 块级契约应用 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "is") == 0) {
                /* 特性门控：与 `@contract` 同一判据（Rust 的 `@is` 也查 Contract 位）。 */
                if (!(ps->features & (1u << 3))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `contract`：当前文档已禁用该特性");
                    ps->failed = 1;
                    break;
                }
                next(ps); next(ps); /* @ is */
                token *cn = next(ps);
                if (cn && (cn->t == T_WORD || cn->t == T_STR)) {
                    char *cname_is = cn->v;
                    /* 任务 D：多契约「响亮拒绝」—— `@is` 后出现第 2 个裸词（同一行、
                       且该词不是「后跟 : 的键」）即报 E-CONTRACT-017，绝不静默给树。
                       与 Rust D1 同口径：遇 : / { / ] / 换行 / 逗号（可省）即停。 */
                    {
                        token *nx = peek(ps);
                        if (nx && nx->t == T_COMMA) nx = peek_at(ps, 1);  /* 跳过可省逗号 */
                        if (nx && (nx->t == T_WORD || nx->t == T_STR)) {
                            if (nx->line != cn->line) {
                                /* 换行即停：下一行的键/值不是契约名 */
                            } else if (peek_at(ps, (nx == peek(ps)) ? 1 : 2)
                                       && peek_at(ps, (nx == peek(ps)) ? 1 : 2)->t == T_COLON) {
                                /* 后跟 : 是键，非契约名 */
                            } else if (strcmp(nx->v, "loose") != 0 &&
                                       strcmp(nx->v, "warn") != 0 &&
                                       strcmp(nx->v, "strict") != 0 &&
                                       contract_find(ps, nx->v) == NULL) {
                                /* 第二个词既不是严格度关键字、也不是已注册契约 ⇒
                                   Rust 报「未定义的契约」001（它把 @is 后的每个词都当契约名）。
                                   用 017 顶替会把「名字写错」说成「本端不支持多契约」。 */
                                set_err(ps->lx->errbuf, ps->lx->errsz,
                                        SML_E_CONTRACT_001 " 应用未定义契约 `%s`", nx->v);
                                ps->failed = 1;
                            } else {
                                set_err(ps->lx->errbuf, ps->lx->errsz,
                                        SML_E_CONTRACT_017 " 该实现暂不支持一个块应用多份契约（@is 多契约写法）");
                                ps->failed = 1;
                            }
                        }
                    }
                    if (peek(ps)->t == T_LBRACE) {
                        /* `@is Name { ... }` 匿名块/当前块应用契约:
                        ** 解析块体、应用契约、合并字段回当前对象 */
                        next(ps);
                        ps->enter_key = NULL;   /* 匿名块：路径与所在块相同 */
                        sml_value *sub = parse_block(ps, T_RBRACE);
                        apply_or_fail(ps, sub, cname_is);
                        sml_field *f;
                        for (f = sub->u.obj.head; f; f = f->next)
                            sml_obj_set(obj, f->key, sml_clone(f->value));
                        sml_free(sub);
                    } else {
                        block_is = cname_is;
                    }
                }
                continue;
            }
            /* 片段定义: `@name { ... }`；参数只认**显式**写法 `type: X` / `name: Y`。
               ⚠️ **位置参数形式**（`@name X [Y] { ... }`）自 v4 起已废弃 —— 它与
               「拼错的指令」在 token 流上完全同形、无法判别，故一律报 E-PARSE-005
               （与 Rust 同判据），而不是猜。
               改前 C 的行为（两端都不报错）：`@foo bar { x: 1 }` 被**静默**当片段收下
               （拼错的指令名于是变成数据）；`@foo bar`（无体）**静默丢掉整行**。
               判据逐字对齐 Rust（`parser.rs` 的 is_param）：
                 · 只有 `type`/`name` **紧跟冒号**才算参数（`@type { .. }` 仍可定义）；
                 · 参数读完后既不是 `{` 也不是文件末尾 ⇒ 位置参数形式 ⇒ 005；
                 · 没有片段体（后面不是 `{`）⇒ 005。 */
            next(ps);                 /* @ */
            token *ft = next(ps);
            if (ft->t != T_WORD && ft->t != T_STR) {
                /* `@` 后不是名字（如孤立 `@`、`@ {`）：Rust 报 E-PARSE-011，
                   不属本批（W16 判定表）范围，保持既有的「停止解析本层」行为。 */
                break;
            }
            char *fname = ft->v;
            char *ftype = NULL, *farg = NULL;
            int dir_bad = 0;
            while (peek(ps)->t == T_WORD &&
                   (strcmp(peek(ps)->v, "type") == 0 || strcmp(peek(ps)->v, "name") == 0) &&
                   peek_at(ps, 1)->t == T_COLON) {
                int is_type = (strcmp(peek(ps)->v, "type") == 0);
                const char *kw = is_type ? "type" : "name";
                next(ps);   /* type / name */
                next(ps);   /* : */
                token *vt = peek(ps);
                if (vt->t != T_WORD && vt->t != T_STR) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_020 " 片段 `@%s` 的参数 `%s:` 后须值", fname, kw);
                    ps->failed = 1;
                    dir_bad = 1;
                    break;
                }
                if ((is_type ? ftype : farg) != NULL) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_PARSE_020 " 片段 `@%s` 的 `%s:` 参数重复", fname, kw);
                    ps->failed = 1;
                    dir_bad = 1;
                    break;
                }
                if (is_type) ftype = next(ps)->v;
                else         farg  = next(ps)->v;
            }
            if (dir_bad) break;
            if (peek(ps)->t != T_LBRACE) {
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_005 " `@%s` 不是合法指令且缺少片段体 { ... }；"
                        "若本意是「片段定义」，参数须显式写作 `type: X` 与 `name: Y`"
                        "（位置参数形式自 v4 起已废弃；不带参数时写作 `@%s { ... }`）；"
                        "若本意是「指令」，请检查拼写（合法指令：contract / is / version）",
                        fname, fname);
                ps->failed = 1;
                break;
            }
            next(ps);                 /* { */
            {
                ps->enter_key = NULL;   /* 片段体：不进主树，路径无意义 */
                sml_value *sub = parse_block(ps, T_RBRACE);
                if (ftype) {
                    sml_obj_set(sub, "__type", sml_new_str(ftype));
                    if (farg) sml_obj_set(sub, "__name", sml_new_str(farg));
                }
                if (!(ps->features & (1u << 4))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `fragment`：当前文档已禁用该特性");
                    ps->failed = 1;
                    sml_free(sub);
                    break;
                }
                frag_put(ps, fname, sub);
            }
            continue;
        }
        /* key */
        token *kt = next(ps);
        /* 键位置上的裸 `:`（P1 欠账，与 Rust / JS 对齐）：`::` 被词法切成两个 `:`，
           第二个 `:` 落到键位置。改前这里对「非 WORD/STR 的键」直接 break ⇒
           `X { a: B::C }` 静默得 {"X":{"a":"B"}}（第二个字段被丢、**还不报错**）。
           Rust / JS 均对同一输入报 E-PARSE-006，此处补齐同码；文案点明 `::` 与「加引号」。 */
        if (kt->t == T_COLON) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_006 " 键位置出现裸冒号 `:`（`::` 会被切成两个 `:`）；"
                    "含 `:` 的标量请加引号，如 a: \"B::C\"");
            ps->failed = 1;
            break;
        }
        if (kt->t != T_WORD && kt->t != T_STR) break;
        char *key = kt->v;
        ps->key_line = kt->line;   /* 重复键诊断要用**键所在行**，不是下一个 token */
        /* 键名位置不支持 `${...}` 插值（循环变量只可用于值位置）。
           No `$` interpolation in key position (E-PARSE-013). */
        if (strstr(key, "${")) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_013 " 键名位置不支持 `$` 插值：循环变量只可用于值位置");
            ps->failed = 1;
            break;
        }
        /* 行级 &frag 展开（splice），与 Rust / JS 对齐：块内独立一行、其后不是 `:` 的
           `&name` 若命中已定义片段，则把该片段的字段**合并进当前块**，而不是造一个
           `"&name"` 键。改前 C 让 `&name` 落到下面的 else 分支走 coerce_word，展开成
           片段值后存到字面键 `"&name"` 下 ⇒ `w { c: 3\n&base }` 得
           {"w":{"c":3,"&base":{...}}}，严格契约据此**误报** E-CONTRACT-004。
           语义与 JS 片段分支一致：按出现顺序合并、同名后者覆盖（sml_obj_set 覆盖）、
           **深拷贝**（片段在解析末尾单独释放，不能借指针）。C 无原型链，故无危险键概念；
           未命中的 `&name` 继续下落到 coerce_word，按 E-INCLUDE-006 报未定义片段引用。 */
        if (key[0] == '&' && !(peek(ps) && peek(ps)->t == T_COLON)) {
            struct frag *fr = NULL;
            for (struct frag *g = ps->frags; g; g = g->next)
                if (strcmp(g->name, key + 1) == 0) { fr = g; break; }
            if (fr && fr->val && fr->val->type == SML_OBJECT) {
                sml_field *ff;
                for (ff = fr->val->u.obj.head; ff; ff = ff->next)
                    sml_obj_set(obj, ff->key, sml_clone(ff->value));
                continue;   /* &name 已作为展开消费，不落入下面的键值处理 */
            }
        }
        /* 字段级 @is: key 后紧跟 @is Name */
        char *field_is = NULL;
        if (peek(ps)->t == T_AT) {
            token *nn = peek_at(ps, 1);
            if (nn && nn->t == T_WORD && strcmp(nn->v, "is") == 0) {
                next(ps); next(ps); /* @ is */
                token *cn = next(ps);
                if (cn && (cn->t == T_WORD || cn->t == T_STR)) {
                    field_is = cn->v;
                    /* 任务 D：字段级 @is 同样「响亮拒绝」第 2 个裸词（同一行、非键）。 */
                    {
                        token *nx = peek(ps);
                        if (nx && nx->t == T_COMMA) nx = peek_at(ps, 1);
                        if (nx && (nx->t == T_WORD || nx->t == T_STR)) {
                            if (nx->line != cn->line) {
                                /* 换行即停 */
                            } else if (peek_at(ps, (nx == peek(ps)) ? 1 : 2)
                                       && peek_at(ps, (nx == peek(ps)) ? 1 : 2)->t == T_COLON) {
                                /* 后跟 : 是键，非契约名 */
                            } else if (strcmp(nx->v, "loose") != 0 &&
                                       strcmp(nx->v, "warn") != 0 &&
                                       strcmp(nx->v, "strict") != 0 &&
                                       contract_find(ps, nx->v) == NULL) {
                                /* 第二个词既不是严格度关键字、也不是已注册契约 ⇒
                                   Rust 报「未定义的契约」001（它把 @is 后的每个词都当契约名）。
                                   用 017 顶替会把「名字写错」说成「本端不支持多契约」。 */
                                set_err(ps->lx->errbuf, ps->lx->errsz,
                                        SML_E_CONTRACT_001 " 应用未定义契约 `%s`", nx->v);
                                ps->failed = 1;
                            } else {
                                set_err(ps->lx->errbuf, ps->lx->errsz,
                                        SML_E_CONTRACT_017 " 该实现暂不支持一个块应用多份契约（@is 多契约写法）");
                                ps->failed = 1;
                            }
                        }
                    }
                }
            }
        }
        int colon = 0;
        if (peek(ps)->t == T_COLON) { colon = 1; next(ps); }
        /* 值位置的游离记号（Rust 兜底码 E-PARSE-012）：
           `a: :` 的 `:`、`a: @ b` 的孤立 `@`（词法给 T_BAREAT）。
           改前 `a: :` 会落到下一轮的「键位置裸冒号」判 006，`a: @ b` 落到
           成员级「孤立 @」判 004 —— 都是「拒绝但码不对」。
           注意：行首/成员位置的孤立 `@` 仍是 004（见上面的 T_BAREAT 分支）。 */
        if (colon && (peek(ps)->t == T_COLON || peek(ps)->t == T_BAREAT)) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_PARSE_012 " 值位置出现游离的 `:` 或孤立的 `@`；含这些记号的标量请加引号");
            ps->failed = 1;
            break;
        }
        token *nt = peek(ps);
        /* 裸块预扫描: 无冒号且后继是词, 可能 `type name { }`。
           开了 `type.featag` 且首词命中契约时，块头还可能有 `!tag [args]` 区与逗号分隔的块名
           （`Service !critical !obey[4], prod { }`）—— 这两类记号要算进「到 `{` 为止」的窗口，
           否则预扫描判定失败、整行落到值位置报错。 */
        if (!colon && nt->t == T_WORD) {
            size_t probe = ps->lx->pos;
            int found = 0;
            const int feat_scan = (ps->features & (1u << 17)) != 0 &&
                                  contract_find(ps, key) != NULL;
            int seen_tag = 0;
            while (probe < ps->lx->n) {
                tok_type pt = ps->lx->toks[probe].t;
                if (pt == T_WORD || pt == T_STR) {
                    if (feat_scan && pt == T_WORD && ps->lx->toks[probe].v[0] == '!')
                        seen_tag = 1;
                    probe++;
                } else if (feat_scan && pt == T_LBRACK) {
                    int depth = 0;
                    while (probe < ps->lx->n) {
                        if (ps->lx->toks[probe].t == T_LBRACK) depth++;
                        else if (ps->lx->toks[probe].t == T_RBRACK) {
                            if (--depth == 0) { probe++; break; }
                        }
                        probe++;
                    }
                } else if (feat_scan && seen_tag && pt == T_COMMA) {
                    probe++;
                } else if (pt == T_LBRACE) { found = 1; break; }
                else break;
            }
            if (found) {
                sml_value *args = sml_new_array();
                sml_value *featags = NULL;
                /* 任务 D：块级多契约「响亮拒绝」—— head 词命中契约表且 args 里出现第 2 个
                   已注册契约词 ⇒ E-CONTRACT-017（单契约 `server web prod` 的 web/prod 不是
                   契约，不受影响）。与 Rust D2 同口径：从首词起连续命中契约表的词才计数。 */
                int key_is_contract = (contract_find(ps, key) != NULL);
                const int feat_on = (ps->features & (1u << 17)) != 0 && key_is_contract;
                int arg_has_contract = 0;
                while (1) {
                    token *tk = peek(ps);
                    if (tk->t == T_LBRACE) break;
                    /* `!tag` / `!tag[args]`：只在契约块头有效（Rust 同判据）。
                       记进 `__featags` 保序数组；无参项只有 name，带参项多一个 args。 */
                    if (feat_on && tk->t == T_WORD && tk->v[0] == '!') {
                        char *tag = tk->v + 1;
                        /* **S5 使用处校验**：featag 的作用域是契约私有 —— 块头写了契约里
                           没声明的名字就拒（Rust 同码 E-CONTRACT-021）。改前只收集不校验，
                           拼错的 `!critcal` 会被静默记进 `__featags`。 */
                        ccontract *owner = contract_find(ps, key);
                        cfeatag *decl = contract_featag_find(owner, tag);
                        if (owner && !decl) {
                            set_err(ps->lx->errbuf, ps->lx->errsz,
                                    SML_E_CONTRACT_021
                                    " featag `!%s` 未在契约 `%s` 中声明（契约私有的作用域，"
                                    "请先在契约体内写 `!%s`）", tag, key, tag);
                            ps->failed = 1;
                            break;
                        }
                        next(ps);
                        {
                            sml_value *payload = NULL;
                            if (peek(ps)->t == T_LBRACK) {
                                next(ps);
                                payload = parse_array(ps);
                            }
                            /* **S5b 使用处校验**：个数与标量类型。判据照 Rust
                               `check_featags`：无参声明不许带参数、带参声明必须写 `[…]`；
                               数组类型按「参数表整体」校验，其它类型只收 1 个参数。
                               单个 `auto` 是「取声明里的默认值」，不校验（S6 写入时才取）。 */
                            if (decl) {
                                int nargs = payload ? (int)sml_arr_len(payload) : -1;
                                int is_auto = (nargs == 1 &&
                                               sml_arr_get(payload, 0) != NULL &&
                                               sml_arr_get(payload, 0)->type == SML_STR &&
                                               strcmp(sml_arr_get(payload, 0)->u.s, "auto") == 0);
                                if (!decl->has_spec && payload) {
                                    set_err(ps->lx->errbuf, ps->lx->errsz,
                                            SML_E_CONTRACT_021
                                            " featag `!%s` 声明为无参（纯标签），使用处不能带参数", tag);
                                    ps->failed = 1;
                                } else if (decl->has_spec && !payload) {
                                    set_err(ps->lx->errbuf, ps->lx->errsz,
                                            SML_E_CONTRACT_021
                                            " featag `!%s` 声明为带参，使用处必须写成 `%s[…]`", tag, tag);
                                    ps->failed = 1;
                                } else if (decl->has_spec && payload && !is_auto) {
                                    if (decl->ty == CT_ARRAY) {
                                        size_t k;
                                        for (k = 0; k < sml_arr_len(payload); k++) {
                                            if (!featag_arg_matches(decl->arr_inner,
                                                                    sml_arr_get(payload, k))) {
                                                /* 类型不符报 002：Rust 在这里复用
                                                   `check_value`，产出的是字段类型码，
                                                   不是 021（021 只表示「声明/用法不符」）。 */
                                                set_err(ps->lx->errbuf, ps->lx->errsz,
                                                        SML_E_CONTRACT_002
                                                        " featag `!%s` 的参数类型不符（第 %d 个参数）",
                                                        tag, (int)k + 1);
                                                ps->failed = 1;
                                                break;
                                            }
                                        }
                                    } else if (nargs != 1) {
                                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                                SML_E_CONTRACT_021
                                                " featag `!%s` 的参数类型不是数组，只接受 1 个参数"
                                                "（实际给了 %d 个）", tag, nargs);
                                        ps->failed = 1;
                                    } else if (!featag_arg_matches(decl->ty,
                                                                   sml_arr_get(payload, 0))) {
                                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                                SML_E_CONTRACT_002
                                                " featag `!%s` 的参数类型不符", tag);
                                        ps->failed = 1;
                                    }
                                }
                                if (ps->failed) {
                                    if (payload) sml_free(payload);
                                    break;
                                }
                            }
                            if (!featags) featags = sml_new_array();
                            {
                                sml_value *item = sml_new_object();
                                sml_obj_set(item, "name", sml_new_str(tag));
                                if (payload) sml_obj_set(item, "args", payload);
                                sml_arr_push(featags, item);
                            }
                        }
                        continue;
                    }
                    if (feat_on && tk->t == T_COMMA) { next(ps); continue; }
                    if (tk->t == T_WORD || tk->t == T_STR) {
                        token *at = next(ps);
                        if (key_is_contract && contract_find(ps, at->v)) arg_has_contract = 1;
                        sml_arr_push(args, at->t == T_STR ? sml_new_strn(at->v, at->len)
                                                          : coerce_word(at->v, ps));
                        continue;
                    }
                    break;
                }
                if (key_is_contract && arg_has_contract) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_CONTRACT_017 " 该实现暂不支持一个块应用多份契约（块级多契约写法）");
                    ps->failed = 1;
                    sml_free(args);
                    if (featags) sml_free(featags);
                    continue;
                }
                if (peek(ps)->t == T_LBRACE) {
                    next(ps);
                    ps->enter_key = key;   /* 裸块：路径段用块类型名 */
                    sml_value *sub = parse_block(ps, T_RBRACE);
                    sml_obj_set(sub, "__type", sml_new_str(key));
                    if (featags) sml_obj_set(sub, "__featags", featags);
                    /* 裸块参数**不再丢**：首个 ⇒ `__name`、其余 ⇒ `__args`，与 Rust
                       `parse_bare_block` 一致（数组位置是同一个写法，见
                       try_parse_array_bare_block；两处必须同步改，否则同一份输入在
                       键位置与数组位置会得到不同的树）。
                       改前只在「恰好一个参数」时写 `__name`，两个以上参数**直接丢弃**
                       —— 与 Rust 的 `server web prod {}`（⇒ `__name: web` +
                       `__args: [prod]`）不符，属静默改数据。
                       必须克隆再存：args 下一行就被 sml_free，直接借用 args[0] 会让
                       `__name` / `__args` 变成悬垂指针，根节点释放时二次释放 ——
                       即已知的「带名块堆损坏」。 */
                    /* 契约应用**先于** `__name` / `__args` 写入：Rust 里块头契约的校验发生在
                       命名参数落位之前，所以 `Service !t, prod { }` 在严格契约下能通过；
                       而「字段值是带名裸块」（`x { @is A addr Sub { } }`）里 `__name` 已存在，
                       会照常报 E-CONTRACT-004 —— 两条实测行为都要留住。 */
                    if (field_is) {
                        apply_or_fail(ps, sub, field_is);
                    } else if ((ps->features & (1u << 14)) && key_is_contract) {
                        /* typed-block：块头首词命中契约 ⇒ 等价于块内首行 `@is`。
                           改前本端不应用契约，契约的 default / 必填 / 类型校验全部静默失效
                           —— 只要契约恰好没有默认值，树看起来还一样（探针原切片因此漏判）。 */
                        apply_or_fail(ps, sub, key);
                    }
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
                    set_field_maybe_skip(ps, obj, key, sub);
                    continue;
                }
                sml_free(args);
                if (featags) sml_free(featags);   /* 没走到 `{` ⇒ 未挂到树上 */
            }
        }
        nt = peek(ps);
        if (nt->t == T_AT) {
            token *nx = peek_at(ps, 1);
            if (nx && nx->t == T_WORD && strcmp(nx->v, "for") == 0) {
                sml_value *arr;
                if (!(ps->features & (1u << 13))) {
                    set_err(ps->lx->errbuf, ps->lx->errsz,
                            SML_E_FEATURE_001 " `for`：当前文档已禁用该特性");
                    ps->failed = 1;
                    break;
                }
                arr = parse_for(ps);
                if (ps->failed) { if (arr) sml_free(arr); break; }
                if (field_is) apply_or_fail(ps, arr, field_is);
                set_field_maybe_skip(ps, obj, key, arr);
                continue;
            }
        }
        if (nt->t == T_LBRACE) {
            next(ps);
            ps->enter_key = key;   /* 旁路表路径：本块的键名 */
            sml_value *sub = parse_block(ps, T_RBRACE);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (nt->t == T_LBRACK) {
            next(ps);
            sml_value *sub = parse_array(ps);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (nt->t == T_STR) {
            token *st = next(ps);
            sml_value *sub;
            if (memchr(st->v, '$', st->len) != NULL) {
                /* `${}` 插值走既有路径（该路径按 C 串处理，插值串不含 NUL）。 */
                char *iv = interp_loop(ps, st->v);
                sub = sml_new_str(iv ? iv : "");
                free(iv);
            } else {
                /* 按字节长度建值：串内的 `\0` 与之后的字节都保留下来。 */
                sub = sml_new_strn(st->v, st->len);
            }
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (nt->t == T_WORD) {
            sml_value *sub = coerce_word(next(ps)->v, ps);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (colon) {
            sml_value *sub = sml_new_null();
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else {
            /* key 本身即值 (片段引用/裸词) */
            sml_value *sub = coerce_word(key, ps);
            if (field_is)
                apply_or_fail(ps, sub, field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        }
    }
    /* `@when` 悬空：块结束时条件还没被任何字段/块消费。
       Dangling `@when`: the block ended before the condition consumed an item. */
    /* 只在没有更早的错误时补 017，避免把 016（连续出现）覆盖掉。 */
    if (!ps->failed && ps->pending_when != 0) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_017 " `@when` 后未跟随任何字段或块（悬空的条件）");
        ps->failed = 1;
    }
    /* 悬空的 `@weakbind`（本块内没等到块）：丢弃，不让它落到外层的下一个块上。
       软校验发生在 `set_field_maybe_skip` 消费挂起项的那一刻。
       A dangling `@weakbind` dies with its own block; the soft check happens where
       the pending bind is consumed (see set_field_maybe_skip). */
    if (ps->pending_weak && ps->pending_weak_depth >= ps->depth) {
        ps->pending_weak = NULL;
    }
    /* 块级 @is 应用 */
    if (block_is)
        apply_or_fail(ps, obj, block_is);
    return obj;
}

static sml_value *parse_core(const char *text, char *err, size_t errsz,
                             sml_diag **diags_out, size_t *n_diags_out,
                             const sml_builtin *builtin,
                             sml_weakbind **binds_out, size_t *n_binds_out) {
    if (diags_out) *diags_out = NULL;
    if (n_diags_out) *n_diags_out = 0;
    if (binds_out) *binds_out = NULL;
    if (n_binds_out) *n_binds_out = 0;
    if (!text) {
        if (err && errsz) set_err(err, errsz, SML_E_INTERNAL_001 " 空指针入参（text 为 NULL）");
        return NULL;
    }
    if (err && errsz) err[0] = '\0';   /* 成功时保持为空，避免误判 */
    /* 文件级规范化：跳过开头的 UTF-8 BOM（EF BB BF）。
       BOM **不是空白** ⇒ 会被词法器吞进第一个单词 ⇒ 第一个键名变成 "\xEF\xBB\xBFx"
       （最坏是**静默**改掉键名）。来源很常见：Windows 记事本「另存为 UTF-8」。
       与 Rust（sml-parse::scan::strip_version）、JS（parse）、C++/Lua 对齐 ——
       仓库里 smltools 的 YAML/XML 导入器早就各自去过 BOM，只有 native SML 漏了。 */
    if (text && strlen(text) >= 3 && (unsigned char)text[0] == 0xEF
        && (unsigned char)text[1] == 0xBB && (unsigned char)text[2] == 0xBF) {
        text += 3;
    }
    lexer lx;
    memset(&lx, 0, sizeof(lx));
    lx.errbuf = err;
    lx.errsz = errsz;
    /* 预处理：先扫 `@feature` 行定下特性集，词法与顶层形态判定才能看到它
       （与 Rust 的 strip_features 同一定位）。 */
    lx.features = scan_features(text);
    lex_run(&lx, text);
    /* 词法错误**先收手**：词法与语法共用一个 err 缓冲，若带着词法错误继续解析，
       解析器写的后续错误会覆盖它，用户拿到的码就不是**第一个**错的码。
       与 Rust 的 `tokenize()` 返回 Err 即短路同义。 */
    if (lx.failed) {
        lex_free(&lx);
        return NULL;
    }
    /* 顶层标量不可往返 ⇒ E-PARSE-008（与 Rust 同判据：**顶层第一个结构是值型标量、且其后
       紧跟块/数组/键值**，T9 收紧）。
       改前这里是**静默造键**：`42` 走 parse_block(T_EOF) 的「键即值」分支，被解析成
       `{"42": 42}`，重新序列化得到 `"42": 42` ≠ `42` —— 数据形状被悄悄改掉
       （与 W17 的 C 嵌套数组同族：不报错，但数据错）。
       判据与 Rust 逐字一致：
         - `42`                → 仅一个顶层结构（n == 2，含末尾 T_EOF）⇒ 报；
         - `42\na { b: 1 }`    → 值型标量 `42` 后跟块 `a` ⇒ 报（T9：42 被重解释成块类型名）；
         - `42\nb: 1` / `42\n[1,2]` → 后跟键值/数组 ⇒ 报；
         - `server web { }`    → `server` 是裸词标识符（块类型名）**非值型** ⇒ 不报（合法块）；
         - `hello world`       → 两 token 裸键对，值可往返 ⇒ 不报；
         - `@version v1` + `42` → 首结构是 @ 指令（非裸标量）⇒ 不报（保留正对照）；
         - `42 { a: 1 }`       → 名为 42 的块（紧跟 `{`）⇒ 不报。
       注：C 的 token 流末尾固定有一个 T_EOF，故「仅一个标量」= n == 2。 */
    {
        int scalar0 = (lx.toks[0].t == T_WORD || lx.toks[0].t == T_STR);
        int bare0 = scalar0 && lx.n >= 2
                    && lx.toks[1].t != T_LBRACE && lx.toks[1].t != T_COLON;
        if (bare0) {
            int only = (lx.n == 2);
            int followed = (lx.n >= 3 && lx.toks[1].t == T_LBRACK)
                || (lx.n >= 3
                    && (lx.toks[1].t == T_WORD || lx.toks[1].t == T_STR)
                    && (lx.toks[2].t == T_LBRACE || lx.toks[2].t == T_COLON));
            /* T9：仅当首个 token 是「值型标量」时，其后跟块/数组/键值才报；
               裸词标识符（块类型名/块名）不算值型，`server web { }` 合法。 */
            int first_is_val = tok_is_scalar_value(&lx.toks[0]);
            if (only || (first_is_val && followed)) {
                if (err && errsz)
                    set_err(err, errsz,
                            SML_E_PARSE_008 " 顶层须为容器（键值块、对象块或数组），单独的标量无法往返");
                lex_free(&lx);
                return NULL;
            }
        }
    }
    parser ps;
    memset(&ps, 0, sizeof(ps));
    ps.lx = &lx;
    ps.frags = NULL;
    ps.contracts = NULL;
    ps.features = lx.features;   /* 预处理算出的初始特性集（解析期 @feature 再增减，幂等） */
    ps.want_diags = (diags_out != NULL);
    ps.builtin = builtin;
    /* 旁路表恒收集（与 Rust 的 `weak_binds` 同为恒收集的 Vec），没要就最后释放。 */
    ps.enter_key = NULL;
    /* 顶层支持三种形态，与 sml_dump 的输出对称：
    **   - `[ ... ]` 数组
    **   - `{ ... }` 顶层对象块
    **   - 键值块（传统形态） */
    sml_value *v;
    /* 顶层数组门控：跳过开头的 `@version` / `@feature` 行，看**实质**第一个 token。
       `@feature` 行在前时 `peek()` 是 `@`，直接判 `first == T_LBRACK` 永远不成立，
       故必须先跳行（Rust 靠 strip_features 剥行，本端用只读跳过达到同一判定）。 */
    {
        size_t k = 0;
        while (k < lx.n && lx.toks[k].t == T_AT) {
            int li = lx.toks[k].line;
            k++;
            while (k < lx.n && lx.toks[k].line == li) k++;
        }
        if (k < lx.n && lx.toks[k].t == T_LBRACK && !(ps.features & (1u << 5))) {
            if (err && errsz)
                set_err(err, errsz,
                        SML_E_FEATURE_001 " `top-level-array`：当前文档已禁用该特性");
            diag_free_all(&ps);
            lex_free(&lx);
            return NULL;
        }
    }
    tok_type first = peek(&ps)->t;
    if (first == T_LBRACK) { next(&ps); v = parse_array(&ps); }
    else if (first == T_LBRACE) { next(&ps); v = parse_block(&ps, T_RBRACE); }
    else v = parse_block(&ps, T_EOF);
    /* 契约校验错误 -> 整体解析失败 (与 Rust 一致) */
    if (ps.failed) {
        sml_free(v);
        v = NULL;
    }
    /* 清理片段 (共享引用, 不 double free: 只释放链本身) */
    struct frag *f = ps.frags;
    while (f) { struct frag *nx = f->next; free(f->name); sml_free(f->val); f = nx; }
    lex_free(&lx);
    path_free_all(&ps);
    if (!v) {
        diag_free_all(&ps);
        sml_weakbind_free(ps.binds, ps.n_binds);
        if (err && errsz && err[0] == '\0')
            set_err(err, errsz, SML_E_PARSE_012 " 解析失败：未能给出更具体的原因");
        return NULL;
    }
    /* 成功：把诊断交给调用方（没开收集就 free 掉，避免泄漏）。 */
    if (diags_out) {
        *diags_out = ps.diags;
        if (n_diags_out) *n_diags_out = ps.n_diags;
    } else {
        diag_free_all(&ps);
    }
    /* 旁路表：只有调用方要才交出，否则释放（空表时两个出参为 NULL / 0）。 */
    if (binds_out) {
        *binds_out = ps.n_binds ? ps.binds : NULL;
        if (n_binds_out) *n_binds_out = ps.n_binds;
    } else {
        sml_weakbind_free(ps.binds, ps.n_binds);
    }
    return v;
}

/* 兼容入口：不收集诊断（与改动前逐字节一致）。 */
sml_value *sml_parse(const char *text, char *err, size_t errsz) {
    return parse_core(text, err, errsz, NULL, NULL, NULL, NULL, NULL);
}

sml_value *sml_parse_ex(const char *text, char *err, size_t errsz,
                        sml_diag **diags_out, size_t *n_diags_out) {
    return parse_core(text, err, errsz, diags_out, n_diags_out, NULL, NULL, NULL);
}

sml_value *sml_parse_with_builtins(const char *text, char *err, size_t errsz,
                                   const sml_builtin *b,
                                   sml_diag **diags, size_t *n_diags,
                                   sml_weakbind **binds, size_t *n_binds) {
    return parse_core(text, err, errsz, diags, n_diags, b, binds, n_binds);
}

