/* ---------------------------------------------------------------------------
   featag S7：`from-featag` 的键不许文档手写（`E-CONTRACT-024`）

   **必须在落点填充之前调用**：填充之后 featag 自己写的键与文档手写的键长得一模一样，
   就分不出「来源合法」还是「伪造」了 —— 而来源正是这条规则的全部意义。
   递归：`ContractRef` 字段且值确实是块时，按被引用契约继续查（路径为 `parent.child`）。
   --------------------------------------------------------------------------- */
static int check_from_featag(parser *ps, ccontract *c, sml_value *node, const char *path) {
    cfield *cf;
    if (!c || !node || node->type != SML_OBJECT) return 0;
    for (cf = c->fields; cf; cf = cf->next) {
        char full[256];
        sml_value *child;
        if (path != NULL && path[0] != '\0')
            snprintf(full, sizeof(full), "%s.%s", path, cf->name);
        else
            snprintf(full, sizeof(full), "%s", cf->name);
        if (cf->from_featag && sml_obj_get(node, cf->name) != NULL) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_CONTRACT_024
                    " 字段 `%s` 声明为 `from-featag`：只能由 featag 落点写入，文档里不能直接赋值",
                    full);
            return -1;
        }
        if (cf->ty != CT_CONTRACTREF || !cf->ref_name) continue;
        child = sml_obj_get(node, cf->name);
        if (!child || child->type != SML_OBJECT) continue;
        {
            ccontract *inner = contract_find(ps, cf->ref_name);
            if (inner && check_from_featag(ps, inner, child, full) != 0) return -1;
        }
    }
    return 0;
}

/* ---------------------------------------------------------------------------
   featag S6：按落点写值（判据逐条照 Rust `write_target` / `write_in_value`）

   - 落点已存在 ⇒ `E-CONTRACT-022`（**禁止覆盖**；静默跳过会让「以为写进去了」变成最难查的错）
   - 父对象缺失 ⇒ 默认自动创建（那是「插入」不是「覆盖」）；`!strict-paths` 时 ⇒ `023`
   - 路径上某键已是非对象 ⇒ `023`；下标作用在非数组 ⇒ `023`
   - 数组下标：`n < len` 且元素为 `null` ⇒ 补缺；元素非 null ⇒ `022`；
     `n == len` ⇒ 追加（`strict` 下 `023`）；`n > len` ⇒ `025`（**绝不补 null**）
   - 数组本身不存在时不自动创建（造多长、中间填什么没有无歧义答案）
   --------------------------------------------------------------------------- */

/* 把 `val` 放进数组槽位（旧槽必是 null，无需释放其内容）。 */
static void arr_put_null_slot(sml_value *slot, sml_value *val) {
    sml_value tmp = *val;   /* 浅移动：val 是独立分配的外壳 */
    free(val);
    *slot = tmp;
}

static int write_in_value(parser *ps, sml_value *cur, cpathseg *segs, size_t n,
                          sml_value *val, int strict, const char *where_);

static int write_target(parser *ps, sml_value *node, cpathseg *segs, size_t n,
                        sml_value *val, int strict, const char *where_) {
    sml_value *slot;
    if (n == 0) { sml_free(val); return 0; }
    if (segs[0].is_idx) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_CONTRACT_020 " `%s` 的落点须以键名开头", where_);
        sml_free(val);
        return -1;
    }
    slot = sml_obj_get(node, segs[0].key);
    if (n == 1) {
        if (slot) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_CONTRACT_022 " `%s` 的落点 `%s` 已存在（featag 不能覆盖已有值）",
                    where_, segs[0].key);
            sml_free(val);
            return -1;
        }
        sml_obj_set(node, segs[0].key, val);
        return 0;
    }
    if (!slot) {
        if (strict) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_CONTRACT_023
                    " `%s` 的落点路径上父对象 `%s` 不存在（本契约开了 `!strict-paths`）",
                    where_, segs[0].key);
            sml_free(val);
            return -1;
        }
        if (segs[1].is_idx) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_CONTRACT_023
                    " `%s` 的落点路径上数组 `%s` 不存在（数组不会凭空创建：长度与中间元素无解）",
                    where_, segs[0].key);
            sml_free(val);
            return -1;
        }
        slot = sml_new_object();
        sml_obj_set(node, segs[0].key, slot);
    }
    return write_in_value(ps, slot, segs + 1, n - 1, val, strict, where_);
}

static int write_in_value(parser *ps, sml_value *cur, cpathseg *segs, size_t n,
                          sml_value *val, int strict, const char *where_) {
    size_t len;
    long idx;
    if (n == 0) { sml_free(val); return 0; }
    if (!segs[0].is_idx) {
        if (!cur || cur->type != SML_OBJECT) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_CONTRACT_023 " `%s` 的落点路径上 `%s` 已是非对象，没法往里写键",
                    where_, segs[0].key);
            sml_free(val);
            return -1;
        }
        return write_target(ps, cur, segs, n, val, strict, where_);
    }
    if (!cur || cur->type != SML_ARRAY) {
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_CONTRACT_023 " `%s` 的落点里下标 `[%ld]` 作用在非数组上",
                where_, segs[0].idx);
        sml_free(val);
        return -1;
    }
    len = sml_arr_len(cur);
    idx = segs[0].idx;
    if (idx < (long)len) {
        sml_value *el = sml_arr_get(cur, (size_t)idx);
        if (n == 1) {
            if (el && el->type == SML_NULL) {
                arr_put_null_slot(el, val);
                return 0;
            }
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_CONTRACT_022
                    " `%s` 的落点 `[%ld]` 已有非 null 值（featag 不能覆盖已有值）", where_, idx);
            sml_free(val);
            return -1;
        }
        if (el && el->type == SML_NULL) {
            sml_value *o = sml_new_object();
            arr_put_null_slot(el, o);
        }
        return write_in_value(ps, el, segs + 1, n - 1, val, strict, where_);
    }
    if (idx == (long)len) {
        if (strict) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_CONTRACT_023
                    " `%s` 的落点要追加 `[%ld]`，但本契约开了 `!strict-paths`（只允许写已有元素）",
                    where_, idx);
            sml_free(val);
            return -1;
        }
        if (n == 1) { sml_arr_push(cur, val); return 0; }
        {
            sml_value *o = sml_new_object();
            sml_arr_push(cur, o);
            return write_in_value(ps, o, segs + 1, n - 1, val, strict, where_);
        }
    }
    set_err(ps->lx->errbuf, ps->lx->errsz,
            SML_E_CONTRACT_025 " `%s` 的落点 `[%ld]` 越界：该数组只有 %zu 个元素（越界不补 null）",
            where_, idx, len);
    sml_free(val);
    return -1;
}

/* ---------------------------------------------------------------------------
   多契约：收集 `@is` 之后的契约名列表（D1）
   ---------------------------------------------------------------------------
   连续取裸词，遇 `:` / `{` / `]` / **换行**即停；逗号作分隔（**可省**：
   `@is a, b` ≡ `@is a b`）。两条停机判据缺一不可：

   - 「换行即停」：`@is X` 独占一行、字段写在下一行是常见写法，无脑收词会把
     下一行的键吞进契约名；
   - 「裸词后跟 `:` 的是键」：`@is A host: h1` 的 host 是键值，不是契约名 ——
     否则同行的键值永远写不了。

   严格度关键字（`loose` / `warn` / `strict`）**紧跟某个契约名**时不是契约名，
   而是那个契约的**块级严格度覆盖**（D4①）：`@is A warn B` ⇒ A=warn、B=契约级默认。
   --------------------------------------------------------------------------- */
static void parse_is_names(parser *ps, ccontracts *out) {
    token *first = peek(ps);
    int line;
    int have_name = 0;   /* 刚收了一个契约名、还没被严格度关键字覆盖 */
    if (!first || (first->t != T_WORD && first->t != T_STR)) return;
    line = first->line;
    for (;;) {
        token *tk = peek(ps);
        if (tk->t == T_COMMA) {
            if (tk->line != line) break;
            next(ps);
            continue;
        }
        if (tk->t != T_WORD && tk->t != T_STR) break;
        if (tk->line != line) break;
        if (have_name && (strcmp(tk->v, "loose") == 0 || strcmp(tk->v, "warn") == 0 ||
                          strcmp(tk->v, "strict") == 0)) {
            /* 绑定到**上一个**契约名：strict ⇒ 1，loose / warn ⇒ 0 */
            out->ovs[out->n - 1] = (strcmp(tk->v, "strict") == 0) ? 1 : 0;
            have_name = 0;
            next(ps);
            continue;
        }
        /* 后跟 `:` 且又不是契约名 ⇒ 它是键，停 */
        if (peek_at(ps, 1)->t == T_COLON && contract_find(ps, tk->v) == NULL) break;
        {
            const char *name = next(ps)->v;
            if (ccontracts_push(out, name, -1) != 0) {
                set_err(ps->lx->errbuf, ps->lx->errsz, SML_E_INTERNAL_001 " 内存不足");
                ps->failed = 1;
                return;
            }
        }
        have_name = 1;
    }
}

/* `from-featag` 的键不许文档手写：多契约时逐份查（Rust 同样逐份调 `check_from_featag`）。 */
static int check_from_featag_set(parser *ps, const ccontracts *l, sml_value *node) {
    size_t i;
    for (i = 0; i < l->n; i++) {
        ccontract *c = contract_find(ps, l->names[i]);
        if (c && check_from_featag(ps, c, node, "") != 0) return -1;
    }
    return 0;
}

/* 在契约集合里找一个 featag 声明（Rust：`contract_names` 的声明并集，取首个命中）。 */
static cfeatag *featag_find_set(parser *ps, const ccontracts *l, const char *name) {
    size_t i;
    for (i = 0; i < l->n; i++) {
        ccontract *c = contract_find(ps, l->names[i]);
        cfeatag *d = c ? contract_featag_find(c, name) : NULL;
        if (d) return d;
    }
    return NULL;
}

/* 集合里任一份契约开了 `!strict-paths` ⇒ 叠加后就得守这条（约束取并）。 */
static int set_strict_paths(parser *ps, const ccontracts *l) {
    size_t i;
    for (i = 0; i < l->n; i++) {
        ccontract *c = contract_find(ps, l->names[i]);
        if (c && c->strict_paths) return 1;
    }
    return 0;
}

/* ---------------------------------------------------------------------------
   裸块块头：`type [featag 区] [契约词…] [块名] { … }`

   键位置（`A B item { }`）与**数组位置**（`xs: [ A B item { } ]`）共用这两个函数。
   Rust 侧两处走的是**同一个** `bare_child_frame`（`parse_block` 与 `parse_array_inner`
   都经 `bare_block_ahead` 进去），所以两侧语义必须逐字一致 —— 抽出来就是为了不再
   出现「键位置改了、数组位置忘了」。
   --------------------------------------------------------------------------- */

/* 判据：从当前位置起「词 / `!tag[…]` 区 / 区的终止逗号」之后是否紧跟 `{`。
   **只看 `type.featag` 特性位**（不看首词是不是契约），`[` 按括号配平跳过。
   —— 与 Rust `bare_block_ahead` 同。 */
static int bare_ahead(parser *ps) {
    size_t probe = ps->lx->pos;
    const int feat_scan = (ps->features & (1u << 17)) != 0;
    while (probe < ps->lx->n) {
        tok_type pt = ps->lx->toks[probe].t;
        if (pt == T_WORD || pt == T_STR) {
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
        } else if (feat_scan && pt == T_COMMA) {
            probe++;
        } else if (pt == T_LBRACE) {
            return 1;
        } else break;
    }
    return 0;
}

/* 块头（`key` 已由调用方消费）。返回：
   1  = 是裸块：`*args_out` / `*featags_out` / `ty` 已填好，游标停在「参数之后」；
   0  = 不是裸块：**一个 token 都不消费**，调用方走标量路径；
   -1 = 出错（`ps->failed` 已置），调用方只需清理自己的东西。 */
static int parse_bare_head(parser *ps, const char *key,
                           ccontracts *ty, size_t *ty_consumed,
                           sml_value **args_out, sml_value **featags_out) {
    sml_value *args;
    sml_value *featags = NULL;
    int key_is_contract;
    int feat_on;

    *args_out = NULL;
    *featags_out = NULL;
    if (!bare_ahead(ps)) return 0;

    args = sml_new_array();
    key_is_contract = (contract_find(ps, key) != NULL);
    feat_on = (ps->features & (1u << 17)) != 0 && key_is_contract;

    /* ① featag 区：**只在这里**（紧跟契约首词），且**每一项都以 `!` 开头**。
       `!` 是 featag 的**唯一**标记。若允许带参项省掉 `!`（靠紧跟的 `[` 区分），
       它就会与「块头裸词命中契约表即契约词」争用同一个裸词 —— `A B rep[3]` 里
       `rep` 是契约词还是 featag，只能靠先后次序猜；三类词（featag / 契约词 /
       块名）互不重叠，比省下这一个字符重要。 */
    if (feat_on && !ps->failed) {
        for (;;) {
            token *tk = peek(ps);
            char *tag;
            if (tk->t != T_WORD || tk->v[0] != '!') break;
            tag = tk->v + 1;
            /* **S5 使用处校验**：featag 的作用域是契约私有 —— 块头写了契约里
               没声明的名字就拒（Rust 同码 E-CONTRACT-021）。改前只收集不校验，
               拼错的 `!critcal` 会被静默记进 `__featags`。 */
            {
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
                                    " featag `!%s` 声明为带参，使用处必须写成 `!%s[…]`", tag, tag);
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
            }
        }
        /* 区的终止逗号（可省：`wea !critical { }` 直接进块体也成立） */
        if (!ps->failed && featags && peek(ps)->t == T_COMMA) next(ps);
        /* 本块没有应用任何契约却写了 featag ⇒ 021（Rust 的兜底同码）：
           featag 的作用域是契约私有的，没有契约就谈不上「声明过」。 */
        if (!ps->failed && featags && !key_is_contract) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_CONTRACT_021
                    " 本块没有应用任何契约 —— featag 的作用域是契约私有的，"
                    "先 `@contract` 声明它");
            ps->failed = 1;
        }
    }
    /* ② 之后才是契约词 / 块名。这里的 `!` 词落在**参数位置**，由
       `coerce_word` 按 B2 拒（Rust 同码 E-PARSE-027）——
       `A B !t` 于是与 Rust 一样是错误，不会静默当成 featag。 */
    if (!ps->failed) {
        while (peek(ps)->t == T_WORD || peek(ps)->t == T_STR) {
            token *at = next(ps);
            sml_arr_push(args, at->t == T_STR ? sml_new_strn(at->v, at->len)
                                              : coerce_word(at->v, ps));
        }
    }
    /* 块级多契约（D2）：从**首词**起，连续命中契约表的词都是契约；
       第一个未命中的词是块名（`server web prod { }` 的 web/prod 不是契约，
       不受影响）。只有开了 `typed-block` 才这么认 —— 否则块头参数照旧只是
       块名 / `__args`，既不算契约词也不改 `__name` 的起点。 */
    if (key_is_contract && (ps->features & (1u << 14)) && !ps->failed) {
        size_t ai;
        if (ccontracts_push(ty, key, -1) != 0) {
            set_err(ps->lx->errbuf, ps->lx->errsz, SML_E_INTERNAL_001 " 内存不足");
            ps->failed = 1;
        }
        for (ai = 0; !ps->failed && ai < sml_arr_len(args); ai++) {
            sml_value *a = sml_arr_get(args, ai);
            if (!a || a->type != SML_STR) break;
            if (!contract_find(ps, a->u.s)) break;
            if (ccontracts_push(ty, a->u.s, -1) != 0) {
                set_err(ps->lx->errbuf, ps->lx->errsz, SML_E_INTERNAL_001 " 内存不足");
                ps->failed = 1;
                break;
            }
            (*ty_consumed)++;
        }
    }
    if (ps->failed) {
        sml_free(args);
        if (featags) sml_free(featags);
        return -1;
    }
    *args_out = args;
    *featags_out = featags;
    return 1;
}

/* 块体已解析成 `sub`：拆雷 → S7 → S6 → 契约应用 → `__name`/`__args`。
   —— 对应 Rust `finish_bare_block`。返回 0 成功 / -1 失败（调用方负责 free sub）。 */
static int finish_bare_block(parser *ps, const char *key, sml_value *sub,
                             const ccontracts *ty, size_t ty_consumed,
                             const ccontracts *field_is,
                             sml_value *featags, sml_value *args) {
    /* —— 拆雷：契约词之后还剩一个词、但 `block-name` 没开 ⇒ 只能是块名 ——
       未开 `block-name` 时块名没有合法位置：`A B item { }` 里的 `item` 会被
       静默当成 `__name`，读者以为写的是契约、实际写成了块名。Rust 在
       `finish_bare_block` 里报 001 并给出「先 @feature enable block-name」
       的出口 —— 同样在**块体解析之后**，故 `S !t rep[3]` 那种块头语法错
       会先以 012 报出来，不被这条抢走。 */
    if (!ps->failed && ty->n > 0 && sml_arr_len(args) > ty_consumed &&
        !(ps->features & (1u << 16))) {
        sml_value *w = sml_arr_get(args, ty_consumed);
        if (w && w->type == SML_STR) {
            set_err(ps->lx->errbuf, ps->lx->errsz,
                    SML_E_CONTRACT_001
                    " 块级标注 `%s` 后的 `%s` 不是已定义的契约；"
                    "若本意是把它作为块名，请先 `@feature enable block-name`",
                    key, w->u.s);
            ps->failed = 1;
            return -1;
        }
    }
    /* 契约应用**先于** `__name` / `__args` 写入：Rust 里块头契约的校验发生在
       命名参数落位之前，所以 `Service !t, prod { }` 在严格契约下能通过；
       而「字段值是带名裸块」（`x { @is A addr Sub { } }`）里 `__name` 已存在，
       会照常报 E-CONTRACT-004 —— 两条实测行为都要留住。 */
    /* **S7**：`from-featag` 的键不许文档手写 —— 必须在**填充之前**，
       否则填进去的键与手写的长得一样，来源就分不出来了。 */
    if (ps->features & (1u << 17)) {
        const ccontracts *s7 = (field_is && field_is->n > 0) ? field_is : ty;
        if (s7->n > 0 && check_from_featag_set(ps, s7, sub) != 0) return -1;
    }
    /* **S6 落点填充**：必须在**契约应用之前** —— 与 Rust
       `finish_bare_block` 同序（源码注释：`from-featag` 那类「只能由 featag
       提供」的必填字段，契约先跑会被误判成 E-CONTRACT-003）。
       副作用有两处可见：严格契约能看到填充出来的键（报 004）；
       `auto` 取的是**声明自己的 default**（此时契约默认值还没落位）。 */
    if (featags && ty->n > 0 && !ps->failed) {
        const int strict6 = set_strict_paths(ps, ty);
        size_t fi;
        for (fi = 0; fi < sml_arr_len(featags); fi++) {
            sml_value *item = sml_arr_get(featags, fi);
            sml_value *nm = sml_obj_get(item, "name");
            sml_value *argsv = sml_obj_get(item, "args");
            cfeatag *d6;
            sml_value *val;
            char where6[160];
            if (!nm || nm->type != SML_STR || !argsv) continue;
            d6 = featag_find_set(ps, ty, nm->u.s);
            if (!d6 || d6->n_path == 0) continue;
            snprintf(where6, sizeof(where6), "%s.%s", key, nm->u.s);
            if (sml_arr_len(argsv) == 1) {
                sml_value *only = sml_arr_get(argsv, 0);
                if (only && only->type == SML_STR && strcmp(only->u.s, "auto") == 0) {
                    if (!d6->has_def || !d6->def) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_CONTRACT_022
                                " `%s` 的参数是 `auto`（取声明里的默认值），"
                                "但该声明没有 `default`", where6);
                        ps->failed = 1;
                        break;
                    }
                    val = sml_clone(d6->def);
                } else if (d6->has_spec && d6->ty == CT_ARRAY) {
                    val = sml_clone(argsv);
                } else {
                    val = sml_clone(only);
                }
            } else if (d6->has_spec && d6->ty == CT_ARRAY) {
                val = sml_clone(argsv);
            } else {
                val = sml_clone(sml_arr_get(argsv, 0));
            }
            if (write_target(ps, sub, d6->path, d6->n_path, val,
                             strict6, where6) != 0) {
                ps->failed = 1;
                break;
            }
        }
        if (ps->failed) return -1;
    }
    if (field_is && field_is->n > 0) {
        apply_set_or_fail(ps, sub, field_is);
    } else if (ty->n > 0) {
        /* typed-block：块头首词命中契约 ⇒ 等价于块内首行 `@is`（可多份）。
           改前本端不应用契约，契约的 default / 必填 / 类型校验全部静默失效
           —— 只要契约恰好没有默认值，树看起来还一样（探针原切片因此漏判）。 */
        apply_set_or_fail(ps, sub, ty);
    }
    if (ps->failed) return -1;
    /* `__name` 的起点跳过被算作契约词的参数（首个非契约词才是块名）；
       单契约 / 无契约时 `ty_consumed == 0`，与旧行为逐字一致。
       必须克隆再存：args 随后就被 sml_free，直接借用会让 `__name` / `__args`
       变成悬垂指针（已知的「带名块堆损坏」）。 */
    if (sml_arr_len(args) > ty_consumed) {
        size_t i;
        sml_obj_set(sub, "__name", sml_clone(sml_arr_get(args, ty_consumed)));
        if (sml_arr_len(args) > ty_consumed + 1) {
            sml_value *extra = sml_new_array();
            for (i = ty_consumed + 1; i < sml_arr_len(args); i++)
                sml_arr_push(extra, sml_clone(sml_arr_get(args, i)));
            sml_obj_set(sub, "__args", extra);
        }
    }
    return 0;
}

/* 数组元素位置的裸块（W4 的 B 类对齐点）：`[ type [featag 区] [契约词…] [块名] { … } ]`。
   与键位置**共用** `parse_bare_head` / `finish_bare_block`（Rust 两处也是同一个
   `bare_child_frame`）。返回块对象；不是裸块返回 NULL 且**不消费任何 token**。 */
static sml_value *try_parse_array_bare_block(parser *ps) {
    ccontracts ty = {0};
    size_t ty_consumed = 0;
    sml_value *args = NULL;
    sml_value *featags = NULL;
    sml_value *sub;
    token *first;
    const char *type_word;
    int r;

    if (peek(ps)->t != T_WORD) return NULL;
    /* 先用**同一判据**确认是裸块头，避免消费首词之后无法回退。 */
    if (!bare_ahead(ps)) return NULL;
    first = next(ps);
    type_word = first->v;   /* 原词（token 存活到 lex_free） */
    r = parse_bare_head(ps, type_word, &ty, &ty_consumed, &args, &featags);
    if (r != 1) {
        ccontracts_free(&ty);
        if (args) sml_free(args);
        if (featags) sml_free(featags);
        return NULL;
    }
    if (peek(ps)->t != T_LBRACE) {
        /* 预扫描已确认「参数之后是 `{`」，参数却停在了别的记号上 ⇒ 与键位置
           同一个兜底 012（Rust `bare_child_frame` 同位置同码），不回退标量路径。 */
        set_err(ps->lx->errbuf, ps->lx->errsz,
                SML_E_PARSE_012 " 裸块块头的参数之后未能接上 `{`");
        ps->failed = 1;
        ccontracts_free(&ty);
        sml_free(args);
        if (featags) sml_free(featags);
        return NULL;
    }
    next(ps);   /* `{` */
    ps->enter_key = type_word;
    sub = parse_block(ps, T_RBRACE);
    sml_obj_set(sub, "__type", sml_new_str(type_word));
    if (featags) sml_obj_set(sub, "__featags", featags);
    if (finish_bare_block(ps, type_word, sub, &ty, ty_consumed, NULL, featags, args) != 0) {
        sml_free(args);
        ccontracts_free(&ty);
        sml_free(sub);   /* featags 已挂在 sub 上，随之释放 */
        return NULL;
    }
    sml_free(args);
    ccontracts_free(&ty);
    return sub;
}

/* 解析块/对象。closing=T_RBRACE 或 T_EOF(顶层) */
static sml_value *parse_block_inner(parser *ps, tok_type closing) {
    sml_value *obj = sml_new_object();
    ccontracts block_is = {0};   /* 块级 @is（累积，可多份） */
    /* 下面两项按**轮**复用，故在循环顶部统一回收（各 `continue` 出口都会经过那里）。 */
    ccontracts field_is = {0};   /* 字段级 `@is` */
    ccontracts ty = {0};         /* 块头条的契约（typed-block，可多份） */
    size_t ty_consumed = 0;      /* args 里被算作契约词的数量 */
    for (;;) {
        ccontracts_free(&field_is);
        ccontracts_free(&ty);
        ty_consumed = 0;
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
            /* `@type name: X { 模式 }` —— 自定义类型（字段引用时按模式校验）。
               判据**必须是「后跟 `name:`」**：`@type { … }` 是**名为 type 的片段定义**，
               不能被抢占（Rust 侧 `fragment_named_type_or_name_still_works` 守着这条）。
               类型体是普通 SML 数据，存进 `ps->types`（跨块可见，同名后声明覆盖）。
               编译留到字段引用时做 —— 与 Rust 同（那里 `TypeSpec::Pattern` 才 compile_rule），
               故**没被引用过的坏模式不报错**。 */
            if (nxt && nxt->t == T_WORD && strcmp(nxt->v, "type") == 0) {
                token *a1 = peek_at(ps, 2);
                token *a2 = peek_at(ps, 3);
                if (a1 && a1->t == T_WORD && strcmp(a1->v, "name") == 0 &&
                    a2 && a2->t == T_COLON) {
                    token *tn;
                    char *tname;
                    sml_value *body;
                    next(ps); next(ps); next(ps); next(ps);   /* @ type name : */
                    tn = peek(ps);
                    if (!tn || (tn->t != T_WORD && tn->t != T_STR)) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_PARSE_019 " `@type name:` 后须类型名");
                        ps->failed = 1;
                        break;
                    }
                    tname = next(ps)->v;
                    if (peek(ps)->t != T_LBRACE) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_PARSE_019 " `@type %s` 后须 `{ ... }` 模式体", tname);
                        ps->failed = 1;
                        break;
                    }
                    next(ps);
                    ps->enter_key = NULL;    /* 类型体不进旁路表路径 */
                    body = parse_block(ps, T_RBRACE);
                    if (ps->failed) { if (body) sml_free(body); break; }
                    if (!ps->types) ps->types = sml_new_object();
                    sml_obj_set(ps->types, tname, body);
                    continue;
                }
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
                {
                    ccontracts set = {0};
                    parse_is_names(ps, &set);
                    /* `@is` 后不是契约名（`@is {` / `@is` 到结尾）：Rust 报 E-PARSE-019。
                       改前本端**什么也不做** —— 静默忽略整行，方向恰好相反。 */
                    if (!ps->failed && set.n == 0) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_PARSE_019 " `@is` 后须契约名");
                        ps->failed = 1;
                    }
                    if (!ps->failed && set.n > 0) {
                        if (peek(ps)->t == T_LBRACE) {
                            /* `@is Name { ... }` 匿名块/当前块应用契约:
                            ** 解析块体、应用契约、合并字段回当前对象 */
                            next(ps);
                            ps->enter_key = NULL;   /* 匿名块：路径与所在块相同 */
                            sml_value *sub = parse_block(ps, T_RBRACE);
                            apply_set_or_fail(ps, sub, &set);
                            {
                                sml_field *f;
                                for (f = sub->u.obj.head; f; f = f->next)
                                    sml_obj_set(obj, f->key, sml_clone(f->value));
                            }
                            sml_free(sub);
                        } else {
                            /* 累积（Rust 的 `applied_contracts` 同样是累加 + 按名去重）。 */
                            size_t k;
                            for (k = 0; k < set.n && !ps->failed; k++) {
                                if (ccontracts_find(&block_is, set.names[k]) >= 0) continue;
                                if (ccontracts_push(&block_is, set.names[k], set.ovs[k]) != 0) {
                                    set_err(ps->lx->errbuf, ps->lx->errsz,
                                            SML_E_INTERNAL_001 " 内存不足");
                                    ps->failed = 1;
                                }
                            }
                        }
                    }
                    ccontracts_free(&set);
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
        /* 字段级 @is: key 后紧跟 @is Name（可多份：`k @is A, B { }`） */
        if (peek(ps)->t == T_AT) {
            token *nn = peek_at(ps, 1);
            if (nn && nn->t == T_WORD && strcmp(nn->v, "is") == 0) {
                next(ps); next(ps); /* @ is */
                parse_is_names(ps, &field_is);
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
           开了 `type.featag` 时，窗口里还要跨过 `!tag [args]` 区与它的终止逗号
           （`Service !critical !obey[4], prod { }`）—— 判据照 Rust `bare_block_ahead`：
           **只看特性位**（不看首词是不是契约），`[` 按括号配平跳过、`,` 与词一样放行。 */
        if (!colon && nt->t == T_WORD) {
            size_t probe = ps->lx->pos;
            int found = 0;
            const int feat_scan = (ps->features & (1u << 17)) != 0;
            while (probe < ps->lx->n) {
                tok_type pt = ps->lx->toks[probe].t;
                if (pt == T_WORD || pt == T_STR) {
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
                } else if (feat_scan && pt == T_COMMA) {
                    probe++;
                } else if (pt == T_LBRACE) { found = 1; break; }
                else break;
            }
            if (found) {
                sml_value *args = sml_new_array();
                sml_value *featags = NULL;
                int key_is_contract = (contract_find(ps, key) != NULL);
                const int feat_on = (ps->features & (1u << 17)) != 0 && key_is_contract;
                /* ① featag 区：**只在这里**（紧跟契约首词），且**每一项都以 `!` 开头**。
                   `!` 是 featag 的**唯一**标记。若允许带参项省掉 `!`（靠紧跟的 `[` 区分），
                   它就会与「块头裸词命中契约表即契约词」争用同一个裸词 —— `A B rep[3]` 里
                   `rep` 是契约词还是 featag，只能靠先后次序猜；三类词（featag / 契约词 /
                   块名）互不重叠，比省下这一个字符重要。 */
                if (feat_on && !ps->failed) {
                    for (;;) {
                        token *tk = peek(ps);
                        char *tag;
                        if (tk->t != T_WORD || tk->v[0] != '!') break;
                        tag = tk->v + 1;
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
                    }
                    /* 区的终止逗号（可省：`wea !critical { }` 直接进块体也成立） */
                    if (!ps->failed && featags && peek(ps)->t == T_COMMA) next(ps);
                    /* 本块没有应用任何契约却写了 featag ⇒ 021（Rust 的兜底同码）：
                       featag 的作用域是契约私有的，没有契约就谈不上「声明过」。 */
                    if (!ps->failed && featags && !key_is_contract) {
                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                SML_E_CONTRACT_021
                                " 本块没有应用任何契约 —— featag 的作用域是契约私有的，"
                                "先 `@contract` 声明它");
                        ps->failed = 1;
                    }
                }
                /* ② 之后才是契约词 / 块名。这里的 `!` 词落在**参数位置**，由
                   `coerce_word` 按 B2 拒（Rust 同码 E-PARSE-027）——
                   `A B !t` 于是与 Rust 一样是错误，不会静默当成 featag。 */
                if (!ps->failed) {
                    while (peek(ps)->t == T_WORD || peek(ps)->t == T_STR) {
                        token *at = next(ps);
                        sml_arr_push(args, at->t == T_STR ? sml_new_strn(at->v, at->len)
                                                          : coerce_word(at->v, ps));
                    }
                }
                /* 块级多契约（D2）：从**首词**起，连续命中契约表的词都是契约；
                   第一个未命中的词是块名（`server web prod { }` 的 web/prod 不是契约，
                   不受影响）。只有开了 `typed-block` 才这么认 —— 否则块头参数照旧只是
                   块名 / `__args`，既不算契约词也不改 `__name` 的起点。 */
                if (key_is_contract && (ps->features & (1u << 14)) && !ps->failed) {
                    size_t ai;
                    if (ccontracts_push(&ty, key, -1) != 0) {
                        set_err(ps->lx->errbuf, ps->lx->errsz, SML_E_INTERNAL_001 " 内存不足");
                        ps->failed = 1;
                    }
                    for (ai = 0; !ps->failed && ai < sml_arr_len(args); ai++) {
                        sml_value *a = sml_arr_get(args, ai);
                        if (!a || a->type != SML_STR) break;
                        if (!contract_find(ps, a->u.s)) break;
                        if (ccontracts_push(&ty, a->u.s, -1) != 0) {
                            set_err(ps->lx->errbuf, ps->lx->errsz,
                                    SML_E_INTERNAL_001 " 内存不足");
                            ps->failed = 1;
                            break;
                        }
                        ty_consumed++;
                    }
                }
                if (ps->failed) {
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
                    /* —— 拆雷：契约词之后还剩一个词、但 `block-name` 没开 ⇒ 只能是块名 ——
                       未开 `block-name` 时块名没有合法位置：`A B item { }` 里的 `item` 会被
                       静默当成 `__name`，读者以为写的是契约、实际写成了块名。Rust 在
                       `finish_bare_block` 里报 001 并给出「先 @feature enable block-name」
                       的出口 —— 同样在**块体解析之后**，故 `S !t rep[3]` 那种块头语法错
                       会先以 012 报出来，不被这条抢走。 */
                    if (!ps->failed && ty.n > 0 && sml_arr_len(args) > ty_consumed &&
                        !(ps->features & (1u << 16))) {
                        sml_value *w = sml_arr_get(args, ty_consumed);
                        if (w && w->type == SML_STR) {
                            set_err(ps->lx->errbuf, ps->lx->errsz,
                                    SML_E_CONTRACT_001
                                    " 块级标注 `%s` 后的 `%s` 不是已定义的契约；"
                                    "若本意是把它作为块名，请先 `@feature enable block-name`",
                                    key, w->u.s);
                            ps->failed = 1;
                            sml_free(args);
                            sml_free(sub);
                            continue;
                        }
                    }
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
                    /* **S7**：`from-featag` 的键不许文档手写 —— 必须在**填充之前**，
                       否则填进去的键与手写的长得一样，来源就分不出来了。 */
                    if (ps->features & (1u << 17)) {
                        const ccontracts *s7 = field_is.n > 0 ? &field_is : &ty;
                        if (s7->n > 0 && check_from_featag_set(ps, s7, sub) != 0) {
                            ps->failed = 1;
                            sml_free(args);
                            sml_free(sub);
                            continue;
                        }
                    }
                    /* **S6 落点填充**：必须在**契约应用之前** —— 与 Rust
                       `finish_bare_block` 同序（源码注释：`from-featag` 那类「只能由 featag
                       提供」的必填字段，契约先跑会被误判成 E-CONTRACT-003）。
                       副作用有两处可见：严格契约能看到填充出来的键（报 004）；
                       `auto` 取的是**声明自己的 default**（此时契约默认值还没落位）。 */
                    if (featags && ty.n > 0 && !ps->failed) {
                        const int strict6 = set_strict_paths(ps, &ty);
                        size_t fi;
                        for (fi = 0; fi < sml_arr_len(featags); fi++) {
                            sml_value *item = sml_arr_get(featags, fi);
                            sml_value *nm = sml_obj_get(item, "name");
                            sml_value *argsv = sml_obj_get(item, "args");
                            cfeatag *d6;
                            sml_value *val;
                            char where6[160];
                            if (!nm || nm->type != SML_STR || !argsv) continue;
                            d6 = featag_find_set(ps, &ty, nm->u.s);
                            if (!d6 || d6->n_path == 0) continue;
                            snprintf(where6, sizeof(where6), "%s.%s", key, nm->u.s);
                            if (sml_arr_len(argsv) == 1) {
                                sml_value *only = sml_arr_get(argsv, 0);
                                if (only && only->type == SML_STR &&
                                    strcmp(only->u.s, "auto") == 0) {
                                    if (!d6->has_def || !d6->def) {
                                        set_err(ps->lx->errbuf, ps->lx->errsz,
                                                SML_E_CONTRACT_022
                                                " `%s` 的参数是 `auto`（取声明里的默认值），"
                                                "但该声明没有 `default`", where6);
                                        ps->failed = 1;
                                        break;
                                    }
                                    val = sml_clone(d6->def);
                                } else if (d6->has_spec && d6->ty == CT_ARRAY) {
                                    val = sml_clone(argsv);
                                } else {
                                    val = sml_clone(only);
                                }
                            } else if (d6->has_spec && d6->ty == CT_ARRAY) {
                                val = sml_clone(argsv);
                            } else {
                                val = sml_clone(sml_arr_get(argsv, 0));
                            }
                            if (write_target(ps, sub, d6->path, d6->n_path, val,
                                             strict6, where6) != 0) {
                                ps->failed = 1;
                                break;
                            }
                        }
                        if (ps->failed) { sml_free(args); sml_free(sub); continue; }
                    }
                    if (field_is.n > 0) {
                        apply_set_or_fail(ps, sub, &field_is);
                    } else if (ty.n > 0) {
                        /* typed-block：块头首词命中契约 ⇒ 等价于块内首行 `@is`（可多份）。
                           改前本端不应用契约，契约的 default / 必填 / 类型校验全部静默失效
                           —— 只要契约恰好没有默认值，树看起来还一样（探针原切片因此漏判）。 */
                        apply_set_or_fail(ps, sub, &ty);
                    }
                    /* `__name` 的起点跳过被算作契约词的参数（首个非契约词才是块名）；
                       单契约 / 无契约时 `ty_consumed == 0`，与旧行为逐字一致。 */
                    if (sml_arr_len(args) > ty_consumed) {
                        size_t i;
                        sml_obj_set(sub, "__name", sml_clone(sml_arr_get(args, ty_consumed)));
                        if (sml_arr_len(args) > ty_consumed + 1) {
                            sml_value *extra = sml_new_array();
                            for (i = ty_consumed + 1; i < sml_arr_len(args); i++)
                                sml_arr_push(extra, sml_clone(sml_arr_get(args, i)));
                            sml_obj_set(sub, "__args", extra);
                        }
                    }
                    sml_free(args);
                    set_field_maybe_skip(ps, obj, key, sub);
                    continue;
                }
                /* 预扫描已确认「参数之后是 `{`」，参数却停在了别的记号上
                   （`A B rep[3]` 的 `[`、区后多余的 `,` 等）。Rust 在 `bare_child_frame`
                   里同一位置报兜底 012 且**不回退**旧路径；这里照抄，别静默改判。 */
                set_err(ps->lx->errbuf, ps->lx->errsz,
                        SML_E_PARSE_012 " 裸块块头的参数之后未能接上 `{`");
                ps->failed = 1;
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
                if (field_is.n > 0) apply_set_or_fail(ps, arr, &field_is);
                set_field_maybe_skip(ps, obj, key, arr);
                continue;
            }
        }
        if (nt->t == T_LBRACE) {
            next(ps);
            ps->enter_key = key;   /* 旁路表路径：本块的键名 */
            sml_value *sub = parse_block(ps, T_RBRACE);
            if (field_is.n > 0)
                apply_set_or_fail(ps, sub, &field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (nt->t == T_LBRACK) {
            next(ps);
            sml_value *sub = parse_array(ps);
            if (field_is.n > 0)
                apply_set_or_fail(ps, sub, &field_is);
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
            if (field_is.n > 0)
                apply_set_or_fail(ps, sub, &field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (nt->t == T_WORD) {
            sml_value *sub = coerce_word(next(ps)->v, ps);
            if (field_is.n > 0)
                apply_set_or_fail(ps, sub, &field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else if (colon) {
            sml_value *sub = sml_new_null();
            if (field_is.n > 0)
                apply_set_or_fail(ps, sub, &field_is);
            set_field_maybe_skip(ps, obj, key, sub);
        } else {
            /* key 本身即值 (片段引用/裸词) */
            sml_value *sub = coerce_word(key, ps);
            if (field_is.n > 0)
                apply_set_or_fail(ps, sub, &field_is);
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
    /* 块级 @is 应用（单契约 / 多契约叠加走同一入口） */
    if (block_is.n > 0)
        apply_set_or_fail(ps, obj, &block_is);
    ccontracts_free(&block_is);
    ccontracts_free(&field_is);
    ccontracts_free(&ty);
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
    /* `@type` 声明的模式体：只在本端解析期用，解析结束即释放。 */
    if (ps.types) sml_free(ps.types);
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

