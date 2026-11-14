
// 深度守卫：超限就报错并**置中止标志**。返回 true 表示「不该继续深入」，调用方立即
// 返回 null。
//
// 抽成函数是因为有**两个**入口共用同一套计数（parse_value 与 parse_block_nested）——
// 守卫逻辑复制一份迟早会漂移，而漂移的后果是「某一侧的上限悄悄变大」。
//
// ⚠️ 这里**不能**改成「把 depth 复位为 0」了事（最初就是这么写的，实测仍会崩）：
// 复位不会让已经压上去的栈帧退回来，外层循环随即又从 0 开始往下钻，于是「每 128 层
// 一轮」反复压栈，10 万层块嵌套照样打穿栈。真正的收手是置 aborted，由各层循环 break
// 让栈一层层退掉。
static bool depth_exceeded(PState& st) {
    if (st.aborted) return true;      // 已经决定放弃，别再往下走
    if (st.depth < SML_MAX_VALUE_DEPTH) return false;
    if (st.err)
        *st.err = code_prefix(SML_E_LIMIT_001, "sml: 嵌套过深（超过 "
                  + std::to_string(SML_MAX_VALUE_DEPTH)
                  + " 层），疑似递归或恶意输入");
    st.aborted = true;
    return true;
}

// parse a value: object / array / scalar
//
// 守卫 wrapper：真正的实现在 parse_value_inner。包一层而不是改每个 return 点，
// 是为了不漏任何出口（parse_value 有多个提前返回）。
static ValuePtr parse_value(PState& st) {
    if (depth_exceeded(st)) return Value::null();
    st.depth++;
    ValuePtr v = parse_value_inner(st);
    if (st.depth > 0) st.depth--;
    return v;
}

// 深入一层**块**：`key { ... }`、`key @is C { ... }`、片段体 `@f { ... }`、裸块。
//
// ⚠️ 这些位置原先**直接**递归 parse_block，绕过了 parse_value 上的守卫 —— 于是 depth
// 根本不增长，`a{b{c{...}}}` 这种纯块嵌套可以一路递归下去打穿栈（栈溢出在 C++ 里同样是
// 不可捕获的崩溃）。块嵌套必须与值嵌套共用同一个 depth 计数，否则「128 层上限」只对
// 数组生效，对块形同虚设 —— 这正是审计里记的那条。四类调用点全部改走这里。
/* ---------------- featag S6：落点解析与写入（照 Rust write_target / write_in_value） ---------------- */

static bool parse_target_path(const std::string& raw, std::vector<FeatagDecl::PathSeg>& out) {
    size_t p = 0;
    std::string cur;
    auto flush = [&]() {
        if (!cur.empty()) {
            FeatagDecl::PathSeg s;
            s.key = cur;
            out.push_back(s);
            cur.clear();
        }
    };
    while (p < raw.size()) {
        const char c = raw[p];
        if (c == '.') { flush(); p++; }
        else if (c == '[') {
            flush();
            const size_t j = raw.find(']', p);
            if (j == std::string::npos) return false;
            const std::string num = raw.substr(p + 1, j - p - 1);
            if (num.empty()) return false;
            for (char ch : num) if (ch < '0' || ch > '9') return false;
            FeatagDecl::PathSeg s;
            s.is_idx = true;
            s.idx = std::stol(num);
            out.push_back(s);
            p = j + 1;
        } else { cur += c; p++; }
    }
    flush();
    return true;
}

static bool write_target_in(PState& st, ValuePtr cur, const std::vector<FeatagDecl::PathSeg>& segs,
                            size_t idx, ValuePtr val, bool strict, const std::string& w);

static bool write_target(PState& st, ValuePtr node, const std::vector<FeatagDecl::PathSeg>& segs,
                         size_t idx, ValuePtr val, bool strict, const std::string& w) {
    if (idx >= segs.size()) return true;
    const auto& seg = segs[idx];
    const bool last = (idx + 1 == segs.size());
    if (seg.is_idx) {
        if (st.err && st.err->empty())
            *st.err = code_prefix(SML_E_CONTRACT_020, "sml: `" + w + "` 的落点须以键名开头");
        st.aborted = true;
        return false;
    }
    ValuePtr slot = node->get(seg.key);
    if (last) {
        if (slot) {
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_CONTRACT_022,
                    "sml: `" + w + "` 的落点 `" + seg.key + "` 已存在（featag 不能覆盖已有值）");
            st.aborted = true;
            return false;
        }
        node->obj.push_back({seg.key, val});
        return true;
    }
    if (!slot) {
        if (strict) {
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_CONTRACT_023,
                    "sml: `" + w + "` 的落点路径上父对象 `" + seg.key +
                    "` 不存在（本契约开了 `!strict-paths`）");
            st.aborted = true;
            return false;
        }
        if (segs[idx + 1].is_idx) {
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_CONTRACT_023,
                    "sml: `" + w + "` 的落点路径上数组 `" + seg.key +
                    "` 不存在（数组不会凭空创建：长度与中间元素无解）");
            st.aborted = true;
            return false;
        }
        slot = Value::object();
        node->obj.push_back({seg.key, slot});
    }
    return write_target_in(st, slot, segs, idx + 1, val, strict, w);
}

static bool write_target_in(PState& st, ValuePtr cur, const std::vector<FeatagDecl::PathSeg>& segs,
                            size_t idx, ValuePtr val, bool strict, const std::string& w) {
    if (idx >= segs.size()) return true;
    const auto& seg = segs[idx];
    const bool last = (idx + 1 == segs.size());
    if (!seg.is_idx) {
        if (!cur || cur->tag != Value::Tag::Obj) {
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_CONTRACT_023,
                    "sml: `" + w + "` 的落点路径上 `" + seg.key + "` 已是非对象，没法往里写键");
            st.aborted = true;
            return false;
        }
        return write_target(st, cur, segs, idx, val, strict, w);
    }
    if (!cur || cur->tag != Value::Tag::Arr) {
        if (st.err && st.err->empty())
            *st.err = code_prefix(SML_E_CONTRACT_023,
                "sml: `" + w + "` 的落点里下标 `[" + std::to_string(seg.idx) + "]` 作用在非数组上");
        st.aborted = true;
        return false;
    }
    const long n = seg.idx;
    const long len = (long)cur->arr.size();
    if (n < len) {
        if (last) {
            if (cur->arr[(size_t)n] && cur->arr[(size_t)n]->tag == Value::Tag::Null) {
                cur->arr[(size_t)n] = val;
                return true;
            }
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_CONTRACT_022,
                    "sml: `" + w + "` 的落点 `[" + std::to_string(n) +
                    "]` 已有非 null 值（featag 不能覆盖已有值）");
            st.aborted = true;
            return false;
        }
        if (cur->arr[(size_t)n] && cur->arr[(size_t)n]->tag == Value::Tag::Null)
            cur->arr[(size_t)n] = Value::object();
        return write_target_in(st, cur->arr[(size_t)n], segs, idx + 1, val, strict, w);
    }
    if (n == len) {
        if (strict) {
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_CONTRACT_023,
                    "sml: `" + w + "` 的落点要追加 `[" + std::to_string(n) +
                    "]`，但本契约开了 `!strict-paths`（只允许写已有元素）");
            st.aborted = true;
            return false;
        }
        if (last) { cur->arr.push_back(val); return true; }
        cur->arr.push_back(Value::object());
        return write_target_in(st, cur->arr[(size_t)n], segs, idx + 1, val, strict, w);
    }
    if (st.err && st.err->empty())
        *st.err = code_prefix(SML_E_CONTRACT_025,
            "sml: `" + w + "` 的落点 `[" + std::to_string(n) + "]` 越界：该数组只有 " +
            std::to_string(len) + " 个元素（越界不补 null）");
    st.aborted = true;
    return false;
}

static ValuePtr parse_block_nested(PState& st) {
    if (depth_exceeded(st)) return Value::null();
    st.depth++;
    /* 四个调用点（片段体、`key @is C {`、`key {`、裸块 `type name {`）都已消费 `{`，
       故一律要求闭合 —— 缺 `}` 到 EOF 时由 parse_block 报 E-PARSE-001。 */
    ValuePtr v = parse_block(st, false, true);
    if (st.depth > 0) st.depth--;
    return v;
}

/* 深入一层**数组**：`key: [ ... ]`。
   与 parse_block_nested 同一个理由 —— 这个位置原先**直接**调 parse_array，绕过了
   parse_value 上的守卫，于是 depth 不增长，`a: [[[ … ]]]` 的**第一层数组白送一层**。
   实测（闭合嵌套扫过五端）：块嵌套上限 128，数组却到 129 —— 本实现**自身两条路径
   就不一致**。这与 W13 修的「块嵌套直接递归绕过守卫」是同一个洞，当时只补了块、漏了数组。
   （`parse_value_inner` 里的 `[` 与顶层的 `[` 不在这里：前者本就在 parse_value 的守卫
    之内，后者是**文档根**，按各端口径根不计层。） */
static ValuePtr parse_array_nested(PState& st) {
    if (depth_exceeded(st)) return Value::null();
    st.depth++;
    ValuePtr v = parse_array(st);
    if (st.depth > 0) st.depth--;
    return v;
}

static ValuePtr parse_value_inner(PState& st) {
    if (st.i >= st.toks.size()) return Value::null();
    auto& t = st.toks[st.i];
    /* 必须**先消费 `{`** 再进 parse_block，并声明本块需要闭合的 `}`：
       - 不消费 `{` 的话，它会落在 parse_block 的「键位置」而被 E-PARSE-006 判为非法，
         但 `x: [ { a: 1 } ]` 这种「数组元素是对象」是合法写法；
       - 不声明需要闭合就发现不了 EOF 未闭合（E-PARSE-001）。 */
    if (t.t == Token::T::LBrace)  {
        st.i++;
        st.enter_key.clear();   /* 值位置块：无已知键名，路径段为空 */
        return parse_block(st, false, true);
    }
    if (t.t == Token::T::LBracket) { st.i++; return parse_array(st); }
    if (t.t == Token::T::Word) {
        ValuePtr v = coerce(t, st.fragments, st.err, st.features);
        st.i++;
        return v;
    }
    // unexpected token (e.g. '}' or ']') -> null and skip
    st.i++;
    return Value::null();
}

/* 路径栈的进出栈守卫：parse_block 有提前 return（契约字段解析失败），
   手写 pop 会漏；析构函数覆盖所有出口（含异常）。
   RAII guard for the path stack: covers every exit of parse_block. */
struct PathGuard {
    PState& st;
    size_t base;
    explicit PathGuard(PState& s) : st(s), base(s.path.size()) {
        if (!st.enter_key.empty()) st.path.push_back(st.enter_key);
        st.enter_key.clear();
    }
    ~PathGuard() { st.path.resize(base); }
};

static ValuePtr parse_block(PState& st, bool top, bool require_close) {
    auto node = Value::object();
    std::string pending_is;   // local: @is applies only to THIS block
    /* 路径栈：调用方已把本块的键名放进 `enter_key`（顶层为空）。 */
    PathGuard path_guard(st);
    /* 当前字段**键 token** 的行号：重复键诊断要用它，不能用「下一个 token」
       （那已在下一行甚至越过块尾，报出来的行号是错的）。
       Line of the key token being parsed (for the duplicate-key diagnostic). */
    int cur_key_line = 0;
    /* `@when` 只作用于紧邻的下一个字段/块：条件为假就丢弃该项。
       `@when` applies to the immediately following field/block only. */
    auto set_field = [&](const std::string& k, const ValuePtr& v) {
        if (st.pending_when == 2) { st.pending_when = 0; return; }
        st.pending_when = 0;
        /* 弱绑定：`@weakbind X` 之后的那个块，用内置表做**只读**软校验
           （不填默认值、不删改键），并把记录放进旁路表。 */
        if (!st.pending_weak.empty() && v && v->tag == Value::Tag::Obj) {
            const std::string wname = st.pending_weak;
            weak_check_block(st, v, wname);
            if (st.weak_binds)
                st.weak_binds->push_back(
                    WeakBind{weak_path(st, k), wname, st.pending_weak_line});
            st.pending_weak.clear();
        }
        // 重复键：值树照旧提升为数组，同时记一条 `W-PARSE-001`（不改变值）。
        if (st.diags) {
            bool dup = false;
            for (const auto& kv : node->obj) {
                if (kv.first == k) { dup = true; break; }
            }
            if (dup) {
                st.diags->push_back(Diag{
                    "W-PARSE-001",
                    "sml: 键 `" + k + "` 重复出现，已提升为数组",
                    cur_key_line});
            }
        }
        set_field_local(node, k, v);
    };
    /* require_close：本块是否由 `{` 开启（因而**必须**见到 `}`）。
       顶层由 `{` 开启的块、以及所有 parse_block_nested 的调用点都要求闭合；
       顶层裸块（`a: 1\nb: 2` 这种没有外层花括号的）到文件结尾收尾是合法的。
       缺 `}` 走到 EOF 要报 E-PARSE-001，而不是静默返回一棵残缺的树
       （原先 `a {` 会解析成 {a:{}}，探针实测 ok=1）。 */
    bool closed = !require_close;
    // support both { ... } and bare block; caller has consumed '{' if any
    // Here we are at the first token INSIDE a block (caller passed after '{' or at start).
    // For top-level, we are at token 0.
    while (st.i < st.toks.size()) {
        if (st.aborted) break;          // 出错/超限后立刻收手，别继续建树
        auto& tok = st.toks[st.i];
        if (tok.t == Token::T::RBrace) {
            if (require_close) { st.i++; closed = true; break; }
            // 顶层（require_close=false）遇到 `}` ⇒ 多余的（stray）→ E-PARSE-003（W16）
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_PARSE_003, "sml: 多余的 '}'（没有与之匹配的开始符号）");
            st.i++; st.aborted = true; break;
        }
        if (tok.t == Token::T::RBracket) {
            // 块内遇到 `]`（期望 `}`）⇒ 闭合符错配；顶层遇到 `]` ⇒ 多余的（stray）
            if (require_close) {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_PARSE_002, "sml: 闭合符错配（块期望 '}'，却遇到 ']'）");
            } else {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_PARSE_003, "sml: 多余的 ']'（没有与之匹配的开始符号）");
            }
            st.i++; st.aborted = true; break;
        }
        if (tok.t == Token::T::Comma) {
            // 逗号只用于对象字段分隔（其后须是 `key:`）；否则是裸词拆分或孤立逗号。
            const bool field_sep = (st.i + 2 < st.toks.size() &&
                st.toks[st.i + 1].t == Token::T::Word &&
                st.toks[st.i + 2].t == Token::T::Colon);
            if (!field_sep) {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_PARSE_007,
                        "sml: 非预期的逗号：裸词中不可含逗号，请改用数组或引号包裹");
                st.aborted = true;
                break;
            }
            st.i++; continue;
        }

        if (tok.t == Token::T::BareAt) {
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_PARSE_004, "sml: 孤立的 @：其后没有指令名或片段名");
            st.aborted = true;
            break;
        }

        if (tok.t == Token::T::At) {
            // directive keywords are handled by the directive branch below; do not
            // treat them as fragments here.
            bool is_directive = (st.i+1 < st.toks.size() && st.toks[st.i+1].t == Token::T::Word &&
                (st.toks[st.i+1].s == "is" || st.toks[st.i+1].s == "contract" ||
                 st.toks[st.i+1].s == "version" || st.toks[st.i+1].s == "feature" ||
                 st.toks[st.i+1].s == "when" ||
                 st.toks[st.i+1].s == "weakbind" || st.toks[st.i+1].s == "weakdata" ||
                 st.toks[st.i+1].s == "weakdoc" ||
                 st.toks[st.i+1].s == "include" ||
                 st.toks[st.i+1].s == "include!"));
            if (!is_directive) {
            // fragment @name { } or @name type args { }
            // (Rust: fragments parsed but ignored in tree; stored in snippets)
            st.i++;
            if (st.i >= st.toks.size()) break;
            std::string fname = st.toks[st.i].s; st.i++;
            // W16：未注册指令（`@foo ...`）只允许**显式** `type:` / `name:` 参数；
            //   位置参数（如 `@foo bar`）与缺少片段体 `{ ... }` ⇒ E-PARSE-005。
            //   `@foo { x: 1 }`（无参数）仍合法；`@foo type: X name: Y { ... }` 须成功。
            std::string ftype, fname_arg;
            while (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word &&
                   (st.toks[st.i].s == "type" || st.toks[st.i].s == "name") &&
                   st.i+1 < st.toks.size() && st.toks[st.i+1].t == Token::T::Colon) {
                bool is_type = (st.toks[st.i].s == "type");
                st.i += 2; // type/name + ':'
                const auto& vt = st.toks[st.i];
                if (vt.t != Token::T::Word) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_PARSE_005, "sml: 片段 `" + fname + "` 的参数缺少值");
                    st.aborted = true; st.i++; break;
                }
                if (is_type) {
                    if (!ftype.empty()) { if(st.err&&st.err->empty())*st.err=code_prefix(SML_E_PARSE_005,"sml: 片段 `"+fname+"` 的 type 参数重复"); st.aborted=true; break; }
                    ftype = vt.s;
                } else {
                    if (!fname_arg.empty()) { if(st.err&&st.err->empty())*st.err=code_prefix(SML_E_PARSE_005,"sml: 片段 `"+fname+"` 的 name 参数重复"); st.aborted=true; break; }
                    fname_arg = vt.s;
                }
                st.i++; // 消费参数值
            }
            if (!st.aborted) {
                if (!(st.i < st.toks.size() && st.toks[st.i].t == Token::T::LBrace)) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_PARSE_005,
                            "sml: `@" + fname + "` 不是合法指令且缺少片段体 { ... }；若本意是片段定义，参数须显式写作 `type: X` 与 `name: Y`（位置参数形式已废弃；不带参数时写作 `@" + fname + " { ... }`）");
                    st.aborted = true;
                    break;
                }
                st.i++; // consume {
                st.enter_key.clear();   // 片段体：不进主树，路径无意义
                auto sub = parse_block_nested(st);
                if (!ftype.empty()) {
                    sub->obj.push_back({"__type", Value::string(ftype)});
                    if (!fname_arg.empty()) sub->obj.push_back({"__name", Value::string(fname_arg)});
                }
                if (!(st.features & (1u << 4))) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_FEATURE_001,
                            "sml: `fragment`：当前文档已禁用该特性");
                    st.aborted = true;
                    break;
                }
                st.fragments[fname] = sub;
            }
            } // end if (!is_directive)
            if (!is_directive) continue;
        }

        // directive: @version / @include / @contract (tokenized as At + Word)
        if (tok.t == Token::T::At && st.i+1 < st.toks.size() &&
            st.toks[st.i+1].t == Token::T::Word) {
            std::string dir = st.toks[st.i+1].s;
            // 默认忽略：未开 `type.weak` 时 weak* 不解释、不校验、不产出（weakbind.md §0.3）。
            // 片段定义形态（后跟 `{`）不消费，落回原有语义；其余吞掉「指令 + 同行参数」。
            if (dir == "weakbind" || dir == "weakdata" || dir == "weakdoc") {
                if (!(st.features & (1u << 19))) {
                    // 未开 `type.weak`：响亮拒绝（与 Rust 同码 001）。
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_FEATURE_001,
                            "sml: `" + dir + "`：当前文档已禁用该特性"
                            "（请先写 `@feature enable type.weak`）");
                    st.aborted = true; break;
                }
                // 开了 `type.weak`：登记但不解释，值树零影响。
                const bool is_bind = (dir == "weakbind");
                const bool is_doc = (dir == "weakdoc");
                st.i += 2;   // @ weak*
                if (st.i >= st.toks.size() || st.toks[st.i].t != Token::T::Word) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_PARSE_019,
                            "sml: `@" + dir + "` 后须名字");
                    st.aborted = true; break;
                }
                const std::string wname = st.toks[st.i].s;
                const size_t wname_line = st.toks[st.i].line;
                st.i++;   // 名字
                if (is_bind) {
                    /* 挂起到「紧邻的下一个块」；行号与所在层供收尾时判悬空。 */
                    st.pending_weak = wname;
                    st.pending_weak_line = static_cast<int>(wname_line);
                    st.pending_weak_depth = st.depth;
                    if (st.builtins && !builtin_find(st, wname))
                        weak_warn(st, "sml: 内置契约表里没有 `" + wname + "`");
                    /* 可选字段体 `{ 字段: 类型 ... }`：判据必须看**同一行** ——
                       `@weakbind tls` 换行后的 `server { ... }` 是下一个块，不是体。 */
                    if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::LBrace
                        && st.toks[st.i].line == wname_line) {
                        st.i++;   // {
                        while (st.i < st.toks.size() && st.toks[st.i].t != Token::T::RBrace) {
                            if (st.toks[st.i].t != Token::T::Word) { st.i++; continue; }
                            const std::string fname = st.toks[st.i].s;
                            std::string tyname;
                            st.i++;
                            if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Colon) st.i++;
                            if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word) {
                                tyname = st.toks[st.i].s;
                                st.i++;
                            }
                            weak_check_decl(st, wname, fname, tyname);
                        }
                        if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::RBrace) st.i++;
                    }
                }
                if (is_doc && st.i < st.toks.size()
                    && st.toks[st.i].t == Token::T::LBrace) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_PARSE_019,
                            "sml: `@weakdoc` 只写 profile 名，不接受载荷块"
                            "（要内联数据请用 `@weakdata`）");
                    st.aborted = true; break;
                }
                if (!is_bind && !is_doc && st.i < st.toks.size()
                    && st.toks[st.i].t == Token::T::LBrace) {
                    int d = 1;
                    st.i++;
                    while (d > 0 && st.i < st.toks.size()) {
                        if (st.toks[st.i].t == Token::T::LBrace) d++;
                        else if (st.toks[st.i].t == Token::T::RBrace) d--;
                        st.i++;
                    }
                    if (d != 0) {
                        if (st.err && st.err->empty())
                            *st.err = code_prefix(SML_E_PARSE_001,
                                "sml: 未闭合的块（遇到文件结尾，缺少结束符号 }）");
                        st.aborted = true; break;
                    }
                }
                continue;
            }
            if (dir == "version") {
                st.i += 2; // consume @ version
                /* 校验字面量（与 Rust `Edition::parse` 同口径），并吃掉本行剩余
                   token —— `@version 50 "camelia" dev` 的代号与 dev 标记都不是值。 */
                if (st.i < st.toks.size() && st.toks[st.i].line == tok.line) {
                    const Token lit = st.toks[st.i];
                    const int major = version_lit_major(lit.s);
                    if (major < 0) {
                        if (st.err && st.err->empty())
                            *st.err = code_prefix(SML_E_FEATURE_004,
                                "sml: 版本 `" + lit.s + "` 尚未启用（5.0 仍为 Dev；本实现支持 v1..v4）");
                        st.aborted = true;
                        break;
                    }
                    if (major == 0) {
                        if (st.err && st.err->empty())
                            *st.err = code_prefix(SML_E_FEATURE_004,
                                "sml: 未知版本 `" + lit.s + "`；仅支持 v1..v4（v1–v4 / 1.0–4.0 / 10–40）");
                        st.aborted = true;
                        break;
                    }
                    while (st.i < st.toks.size() && st.toks[st.i].line == tok.line) st.i++;
                }
                continue;
            }
            if (dir == "feature") {
                // 与 C 端同一判据：只识别 enable/disable + 名字表，不门控。
                // 用行号区分「子命令 / 特性名」与下一行内容（`@feature\nk: 1` 不能把 k 当名字）。
                st.i += 2; // @ feature
                if (st.i >= st.toks.size() || st.toks[st.i].t != Token::T::Word) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_FEATURE_007,
                            "sml: `@feature` 缺少参数（enable / disable）");
                    st.aborted = true; break;
                }
                const Token sub = st.toks[st.i];
                if (sub.line != tok.line) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_FEATURE_007,
                            "sml: `@feature` 缺少参数（enable / disable）");
                    st.aborted = true; break;
                }
                if (sub.s == "enable" || sub.s == "disable") {
                    const bool on = (sub.s == "enable");
                    const size_t sub_line = sub.line;
                    int any = 0;
                    st.i++;
                    for (;;) {
                        if (st.i >= st.toks.size()) break;
                        const Token& nm = st.toks[st.i];
                        if (nm.t != Token::T::Word || nm.line != sub_line) break;
                        const int bit = feature_index(nm.s);
                        if (bit < 0) {
                            /* 未知特性名是 003（002 = 环境变量被禁用）。 */
                            if (st.err && st.err->empty())
                                *st.err = code_prefix(SML_E_FEATURE_003,
                                    "sml: 未知特性名 `" + nm.s + "`");
                            st.aborted = true; break;
                        }
                        if (on) st.features |= (1u << bit);
                        else    st.features &= ~(1u << bit);
                        any = 1;
                        st.i++;
                        if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Comma) {
                            st.i++; continue;
                        }
                        break;
                    }
                    if (st.aborted) break;
                    if (!any) {
                        if (st.err && st.err->empty())
                            *st.err = code_prefix(SML_E_FEATURE_007,
                                "sml: `@feature " + sub.s + "` 缺少特性名");
                        st.aborted = true; break;
                    }
                    continue;
                }
                /* `mode` 的参数校验属语法层：不是 whitelist / blacklist ⇒ 008（Rust 同码）；
                   参数合法但本端未实现 ⇒ 006（不把「没实现」谎报成「参数非法」）。 */
                if (sub.s == "mode") {
                    const bool arg_ok = st.i + 1 < st.toks.size()
                        && st.toks[st.i + 1].t == Token::T::Word
                        && st.toks[st.i + 1].line == sub.line
                        && (st.toks[st.i + 1].s == "whitelist"
                            || st.toks[st.i + 1].s == "blacklist");
                    if (st.err && st.err->empty())
                        *st.err = arg_ok
                            ? code_prefix(SML_E_FEATURE_006,
                                "sml: `@feature mode`：本端暂不支持该子命令")
                            : code_prefix(SML_E_FEATURE_008,
                                "sml: `@feature mode` 的参数须为 whitelist / blacklist");
                    st.aborted = true; break;
                }
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_FEATURE_006,
                        "sml: `@feature " + sub.s + "`：本端暂不支持该子命令");
                st.aborted = true; break;
            }
            if (dir == "when") {
                // 条件裁剪：只作用于紧邻的下一个字段/块。
                // `@when` applies to the immediately following field/block only.
                if (!(st.features & (1u << 12))) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_FEATURE_001,
                            "sml: `when`：当前文档已禁用该特性");
                    st.aborted = true; break;
                }
                if (st.pending_when != 0) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_PARSE_016,
                            "sml: `@when` 连续出现：它只作用于紧邻的下一个字段或块");
                    st.aborted = true; break;
                }
                st.i += 2;   // @ when
                st.pending_when = eval_when(st) ? 1 : 2;
                if (st.aborted) break;
                continue;
            }
            if (dir == "include" || dir == "include!") {
                /* 展开已在解析**之前**的 expand_includes 里做完（W18）。走到这里只剩两种
                   情形：① 调用方没传 include_dir（按 sml.hpp 的约定 = include 功能关闭）；
                   ② 这一句已在展开时被内联成子文件的 token 段。故此处只把三个 token
                   （`@`、`include`、路径）吃掉，不做文件工作。
                   ⚠️ 别在此处改回「边解析边插 token」：往 st.toks 中间插一段再靠 st.i++
                   找位置，正是 W18 那个 off-by-one 的温床 —— 先插到路径 token 之前、
                   再 st.i++ 吃掉路径，恰好跳过插入段的**首 token**，于是 `include`
                   退化成裸块键、把 includer 后续字段和被包含文件的字段一起吞掉。 */
                st.i += 2;
                if (st.i < st.toks.size()) st.i++;
                continue;
            }
            if (dir == "contract") {
                // 特性门控：与 Rust / C / Lua 同判据（Contract 位）。
                if (!(st.features & (1u << 3))) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_FEATURE_001,
                            "sml: `contract`：当前文档已禁用该特性");
                    st.aborted = true; break;
                }
                st.i += 2; // consume @ contract
            // parse: name { fields... }
            if (st.i >= st.toks.size()) break;
            std::string cname = st.toks[st.i].s; st.i++;
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Colon) st.i++;
            // contract-level `loose` (allow undeclared fields)
            Contract c; c.name = cname;
            /* 契约级修饰符：`loose`（允许未声明字段）与 `strict`（默认；显式写出也合法）。
               ⚠️ 改前**只认 `loose`**：`@contract 事项 strict { … }` 里的 `strict` 不被消费
               ⇒ 后面那个 `{` 就不再是「紧跟契约名」⇒ 整条契约声明被跳过、紧随的块体被当成
               普通键值解析（实测 rust/tests/fixtures/gov_demo.sml 报 E-PARSE-006）。
               与 Rust 对照：`@contract X strict` 两边都应成功且 allow_extra=false。 */
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Word &&
                (st.toks[st.i].s=="loose" || st.toks[st.i].s=="strict")) {
                c.allow_extra = (st.toks[st.i].s == "loose");
                st.i++;
            }
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::LBrace) {
                st.i++; // consume {
                bool closed = false;
                /* 加 `&& !st.aborted`：下面遇到 `@requires` 等不支持的指令时会就地把光标
                   停在 `@` 上并中止（不再前进），外层若无此判据就会原地打转 —— 同一份输入
                   挂死而不是报错，比改前的「静默吞掉」更糟。 */
                while (st.i < st.toks.size() && st.toks[st.i].t != Token::T::RBrace && !st.aborted) {
                    // each field is "name: Type mods..." until next field (newline not tokenized)
                    // We collect tokens until we see a Word followed by Colon that starts a new field,
                    // OR until RBrace. Simpler: accumulate raw token strings until next 'name:' pattern.
                    /* featag 声明：`!name` / `!name: 类型` / `!name: 类型 -> 落点`。
                       开了 `type.featag` 才是记号。**S5**：名字收进契约（块头判「未声明即拒」）；
                       **S5b**：`: 类型` 也收进来，使用处据此校验参数个数与标量类型。
                       `-> 落点` 属 S6，仍整行跳过。 */
                    if ((st.features & (1u << 17)) != 0 &&
                        st.toks[st.i].t == Token::T::Word &&
                        !st.toks[st.i].s.empty() && st.toks[st.i].s[0] == '!') {
                        size_t decl_line = st.toks[st.i].line;
                        FeatagDecl d;
                        d.name = st.toks[st.i].s.substr(1);
                        st.i++;
                        /* 契约级开关 `!any` / `!strict-paths`：**不占** featag 名（Rust 同判据）——
                           块头写 `C !any` 在 Rust 里报「未声明」。它们也不接受 `: 类型` / `[…]` /
                           `-> 落点`，写了就报 021。 */
                        if (d.name == "any" || d.name == "strict-paths") {
                            if (st.i < st.toks.size() &&
                                (st.toks[st.i].t == Token::T::Colon ||
                                 st.toks[st.i].t == Token::T::LBracket)) {
                                if (st.err && st.err->empty())
                                    *st.err = code_prefix(SML_E_CONTRACT_021,
                                        "sml: `!" + d.name +
                                        "` 是契约级开关，不接受类型 / 参数 / 落点");
                                st.aborted = true;
                                break;
                            }
                            if (d.name == "strict-paths") c.strict_paths = true;  // S6 开关
                            while (st.i < st.toks.size() &&
                                   st.toks[st.i].t != Token::T::RBrace &&
                                   st.toks[st.i].line == decl_line) st.i++;
                            continue;
                        }
                        if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Colon) {
                            st.i++;
                            d.has_spec = true;
                            if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::LBracket) {
                                st.i++;
                                d.is_array = true;
                                if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word)
                                    d.inner = st.toks[st.i++].s;
                                if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::RBracket)
                                    st.i++;
                            } else if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::Word) {
                                d.ty = st.toks[st.i++].s;
                            }
                        }
                        /* 修饰符：`default <值>` 与 `-> 落点`（顺序不定）。
                           S6 的 `auto` 取这里的 default；落点在声明期解析成段序列。 */
                        {
                            std::string raw;
                            bool has_path = false;
                            while (st.i < st.toks.size() &&
                                   st.toks[st.i].t != Token::T::RBrace &&
                                   st.toks[st.i].line == decl_line) {
                                const Token& mt = st.toks[st.i];
                                if (mt.t == Token::T::Word && mt.s == "default") {
                                    st.i++;
                                    if (st.i < st.toks.size() &&
                                        (st.toks[st.i].t == Token::T::Word)) {
                                        d.def = coerce(st.toks[st.i], st.fragments, st.err,
                                                       st.features);
                                        d.has_default = true;
                                        st.i++;
                                    }
                                    continue;
                                }
                                if (mt.t == Token::T::Word && mt.s == "->") {
                                    st.i++;
                                    has_path = true;
                                    continue;
                                }
                                if (has_path) {
                                    if (mt.t == Token::T::Word) raw += mt.s;
                                    else if (mt.t == Token::T::LBracket) raw += "[";
                                    else if (mt.t == Token::T::RBracket) raw += "]";
                                }
                                st.i++;
                            }
                            if (has_path) {
                                if (!parse_target_path(raw, d.path)) {
                                    if (st.err && st.err->empty())
                                        *st.err = code_prefix(SML_E_CONTRACT_020,
                                            "sml: `!" + d.name + "` 的落点 `" + raw + "` 写法非法");
                                    st.aborted = true;
                                    break;
                                }
                                if (!d.path.empty() && d.path[0].is_idx) {
                                    if (st.err && st.err->empty())
                                        *st.err = code_prefix(SML_E_CONTRACT_020,
                                            "sml: `!" + d.name + "` 的落点须以键名开头");
                                    st.aborted = true;
                                    break;
                                }
                            }
                        }
                        c.featags.push_back(d);
                        continue;
                    }
                    std::string fieldbuf;
                    while (st.i < st.toks.size()) {
                        auto& ft = st.toks[st.i];
                        if (ft.t == Token::T::RBrace) break;
                        /* 词法器不保留换行 ⇒ `port: int default 8080` 之后的 `!critical`
                           会被贪心收进同一个 fieldbuf，最后当成类型串（实测报
                           `unknown contract '!critical'`）。遇到 `!` 词先收笔，
                           交给外层循环整行跳过。 */
                        if ((st.features & (1u << 17)) != 0 &&
                            ft.t == Token::T::Word && !ft.s.empty() && ft.s[0] == '!')
                            break;
                        /* —— 契约体里的 `@requires`：本端暂未实现，必须**响亮拒绝** ——
                           参照 E-CONTRACT-017（契约叠加未实现）的既有先例。
                           ⚠️ 改前 At 会被拼进 fieldbuf，`parse_field` 拿 "@" 当字段名、
                           其余当类型串交给 `parse_type`，而后者的兜底分支对**任何**未知串
                           都返回 ContractRef —— 于是契约里多出一个垃圾字段且**全程无错**。
                           故必须在此拦截，并同时写 err 与 aborted（只写 err 仍会继续建树）。 */
                        if (ft.t == Token::T::At) {
                            /* 本词法器没有独立的 Str token：引号串以 **Word（含两侧引号）**
                               的形态出现（见 tokenize 的 `Token::T::Word, "\"" + buf + "\""`），
                               故比较前要把两侧引号剥掉，免得 `@"requires"` 漏判。 */
                            std::string dname;
                            if (st.i + 1 < st.toks.size() && st.toks[st.i + 1].t == Token::T::Word) {
                                dname = st.toks[st.i + 1].s;
                                if (dname.size() >= 2 && dname.front() == '"' && dname.back() == '"')
                                    dname = dname.substr(1, dname.size() - 2);
                            }
                            if (st.err && st.err->empty()) {
                                if (dname == "requires")
                                    *st.err = code_prefix(SML_E_CONTRACT_018,
                                        "sml: 本实现暂不支持契约的 `@requires` 指令（它要求先开启所列特性）");
                                else
                                    *st.err = code_prefix(SML_E_PARSE_005,
                                        "sml: 契约体内不支持指令 `@" + dname + "`（契约体仅支持 `@requires`）");
                            }
                            st.aborted = true;
                            break;
                        }
                        // new field detection: Word ':' where previous was not part of type
                        // Use heuristic: a Word immediately followed by Colon and not inside [ ]
                        if (ft.t == Token::T::Word && st.i+1 < st.toks.size() &&
                            st.toks[st.i+1].t == Token::T::Colon && !fieldbuf.empty()) {
                            break;
                        }
                        if (!fieldbuf.empty()) fieldbuf += " ";
                        fieldbuf += ft.s;
                        st.i++;
                    }
                    if (!fieldbuf.empty()) {
                        FieldSpec fs;
                        std::string e;
                        if (!parse_field(fieldbuf, fs, &e)) {
                            if (st.err) *st.err = e;
                            return nullptr;
                        }
                        c.fields.push_back(fs);
                    }
                }
                if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::RBrace) { st.i++; closed = true; }
                /* 契约体未闭合必须报错（Rust 同格：E-PARSE-001「契约体未闭合（缺少结束符号 ）」）。
                   改前这里是 `if (…) st.i++;` —— **有 `}` 才吃、没有就算了**，于是
                   `@contract C { a: str`（缺 `}`）整份解析**成功**，与「未闭合块必报错」
                   的既有口径相反。这处松口一直存在，只是改前 strict 契约进不了本分支
                   才没被看见（探针：前缀 `@contract 事项 strict {` HEAD 报 E-PARSE-001、
                   只修 strict 的第一版反而变成 rc=0 —— 就是它）。 */
                if (!closed && !st.aborted) {
                    if (st.err && st.err->empty())
                        *st.err = code_prefix(SML_E_PARSE_001,
                                              "sml: 契约体未闭合（缺少结束符号 ）");
                    st.aborted = true;
                }
            }
            st.contracts[cname] = c;
            continue;
        }
        if (tok.t == Token::T::At && st.i+1 < st.toks.size() &&
            st.toks[st.i+1].t == Token::T::Word && st.toks[st.i+1].s == "is") {
            // 特性门控：与 `@contract` 同一判据。
            if (!(st.features & (1u << 3))) {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_FEATURE_001,
                        "sml: `contract`：当前文档已禁用该特性");
                st.aborted = true; break;
            }
            st.i += 2;
            // @is ContractName  -> applied after this block (Rust applies to current block)
            if (st.i < st.toks.size()) {
                std::string cname = st.toks[st.i].s;
                size_t cname_line = st.toks[st.i].line;
                st.i++;
                // 任务 D：多契约「响亮拒绝」—— `@is` 后出现第 2 个裸词（同一行、
                //   且该词不是「后跟 : 的键」）即报 E-CONTRACT-017，绝不静默给树。
                //   与 Rust D1 同口径：遇 : / { / ] / 换行 / 逗号（可省）即停。
                {
                    size_t j = st.i;
                    if (j < st.toks.size() && st.toks[j].t == Token::T::Comma) j++;
                    if (j < st.toks.size() && st.toks[j].t == Token::T::Word) {
                        const Token& nx = st.toks[j];
                        if (nx.line != cname_line) {
                            // 换行即停：下一行的键/值不是契约名
                        } else if (j+1 < st.toks.size() && st.toks[j+1].t == Token::T::Colon) {
                            // 后跟 : 是键，非契约名
                        } else if (nx.s != "loose" && nx.s != "warn" && nx.s != "strict" &&
                                   st.contracts.find(nx.s) == st.contracts.end()) {
                            /* 第二个词不是严格度关键字、也不是已注册契约 ⇒ Rust 报
                               「未定义的契约」001；只有「两个都注册了」才是真·多契约缺口 017。 */
                            if (st.err && st.err->empty())
                                *st.err = code_prefix(SML_E_CONTRACT_001,
                                    "sml: 应用未定义契约 `" + nx.s + "`");
                            st.aborted = true;
                        } else {
                            if (st.err && st.err->empty())
                                *st.err = code_prefix(SML_E_CONTRACT_017,
                                    "sml: 该实现暂不支持一个块应用多份契约（@is 多契约写法）");
                            st.aborted = true;
                        }
                    }
                }
                if (!st.aborted) pending_is = cname;  // store pending; applied after block completes
            }
            continue;
        }
        } // close `if (tok.t == At ...)`

        // normal key
        if (tok.t != Token::T::Word) {
            /* 键位置只接受裸词/引号串（引号串在 tokenizer 里也是 Word）。
               其余**结构记号是语法错误**，不能静默跳过 —— Rust 在同一位置报
               E-PARSE-006（parser.rs:967-970 的 `_ => … 期望键, 得 {:?}`）。
               原先这里是 `st.i++; continue;`，于是 `a { { x } }` 会静默解析成功
               （探针实测 ok=1）。注意 `{` 落在这里只可能是**多余的**：所有合法
               的 `{` 都由调用方先消费（见 parse_value_inner 与 parse_block_nested）。
               ⚠️ `$` 例外，见下。 */
            const bool bad_token = (tok.t == Token::T::LBrace ||
                                    tok.t == Token::T::LBracket ||
                                    tok.t == Token::T::Colon);
            if (bad_token) {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_PARSE_006,
                                          "sml: 期望键或标识符，得结构记号");
                st.aborted = true;
                break;
            }
            /* 其余记号（`]` / `,` / `@`）仍**静默跳过** —— 这条兜底与上面 bad_token
               的三格是有意分开的（W16 只把那三格判成错误）。`$` 已不再是独立 token
               （见 tokenizer 里的说明），故本兜底不再有它那一格。 */
            st.i++; continue;
        }
        std::string key = tok.s;
        cur_key_line = static_cast<int>(tok.line);
        // 键名位置不支持 `${...}` 插值（循环变量只可用于值位置）。
        // No `$` interpolation in key position (E-PARSE-013).
        if (key.find("${") != std::string::npos) {
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_PARSE_013,
                    "sml: 键名位置不支持 `$` 插值：循环变量只可用于值位置");
            st.aborted = true;
            break;
        }
        st.i++;
        bool colon = false;
        if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::Colon) { colon=true; st.i++; }
        /* 值位置的游离记号（Rust 兜底码 E-PARSE-012）：`a: :` 的 `:`、
           `a: @ b` 的孤立 `@`（词法给 BareAt）。改前它们会落到下一轮的
           「键位置结构记号」判 006 / 「成员级孤立 @」判 004 —— 都是拒绝但码不对。
           成员位置（无冒号）的 BareAt 仍是 004，不动。 */
        if (colon && st.i < st.toks.size() &&
            (st.toks[st.i].t == Token::T::Colon || st.toks[st.i].t == Token::T::BareAt)) {
            if (st.err && st.err->empty())
                *st.err = code_prefix(SML_E_PARSE_012,
                    "sml: 值位置出现游离的 `:` 或孤立的 `@`；含这些记号的标量请加引号");
            st.aborted = true;
            break;
        }
        if (st.i >= st.toks.size()) {
            // key with nothing after -> coerce key itself as value (Rust: bare block name case)
            set_field(key, coerce_word(key, st.fragments, st.err, st.features));
            break;
        }
        auto& nxt = st.toks[st.i];
        if (nxt.t == Token::T::At && st.i+1 < st.toks.size() &&
            st.toks[st.i+1].t == Token::T::Word && st.toks[st.i+1].s == "is") {
            // key @is Contract { ... }   -> set key = block, apply contract
            st.i += 2; // consume @ is
            if (st.i < st.toks.size()) {
                std::string cname = st.toks[st.i].s;
                size_t cname_line = st.toks[st.i].line;
                st.i++;
                // 任务 D：字段级 @is 同样「响亮拒绝」第 2 个裸词（同一行、非键）。
                {
                    size_t j = st.i;
                    if (j < st.toks.size() && st.toks[j].t == Token::T::Comma) j++;
                    if (j < st.toks.size() && st.toks[j].t == Token::T::Word) {
                        const Token& nx = st.toks[j];
                        if (nx.line != cname_line) {
                            // 换行即停
                        } else if (j+1 < st.toks.size() && st.toks[j+1].t == Token::T::Colon) {
                            // 后跟 : 是键，非契约名
                        } else if (nx.s != "loose" && nx.s != "warn" && nx.s != "strict" &&
                                   st.contracts.find(nx.s) == st.contracts.end()) {
                            /* 第二个词不是严格度关键字、也不是已注册契约 ⇒ Rust 报
                               「未定义的契约」001；只有「两个都注册了」才是真·多契约缺口 017。 */
                            if (st.err && st.err->empty())
                                *st.err = code_prefix(SML_E_CONTRACT_001,
                                    "sml: 应用未定义契约 `" + nx.s + "`");
                            st.aborted = true;
                        } else {
                            if (st.err && st.err->empty())
                                *st.err = code_prefix(SML_E_CONTRACT_017,
                                    "sml: 该实现暂不支持一个块应用多份契约（@is 多契约写法）");
                            st.aborted = true;
                        }
                    }
                }
                if (!st.aborted && st.i < st.toks.size() && st.toks[st.i].t == Token::T::LBrace) {
                    st.i++;
                    st.enter_key = key;   /* 旁路表路径：本块的键名 */
                    auto sub = parse_block_nested(st);
                    std::string e;
                    Parser::apply_contract(sub, st.contracts, cname, &e);
                    if (!e.empty() && st.err) *st.err = e;
                    set_field(key, sub);
                } else if (!st.aborted) {
                    set_field(key, Value::null());
                }
            }
            continue;
        }
        if (nxt.t == Token::T::At && st.i + 1 < st.toks.size()
            && st.toks[st.i + 1].t == Token::T::Word
            && st.toks[st.i + 1].s == "for") {
            // 值位置 `@for`：有界循环展开成数组（opt-in）。
            if (!(st.features & (1u << 13))) {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_FEATURE_001,
                        "sml: `for`：当前文档已禁用该特性");
                st.aborted = true; break;
            }
            auto arr = parse_for(st);
            if (st.aborted) break;
            set_field(key, arr);
            continue;
        }
        if (nxt.t == Token::T::LBrace) {
            st.i++;
            st.enter_key = key;   /* 旁路表路径：本块的键名 */
            auto sub = parse_block_nested(st);
            set_field(key, sub);
        } else if (nxt.t == Token::T::LBracket) {
            /* ⚠️ 必须走**带守卫**的 parse_array_nested，不能直接调 parse_array ——
               直接调会绕过 depth 计数，让 `key: [ ... ]` 的第一层数组白送一层
               （实测：数组到 129 层而块只到 128 层）。理由见 parse_array_nested。 */
            st.i++;
            auto arr = parse_array_nested(st);
            set_field(key, arr);
        } else if (!colon && bare_block_ahead(st)) {
            // bare block: key is type, subsequent tokens until '{' are args
            /* 判据 = Rust 键位置那一支（parser.rs:1180：`if !colon && self.bare_block_ahead()`）。
               ⚠️ 改前这里只要求「后继是个词」，而且参数是**贪心**收的（一直吃到
               `{` / `}` / `,`，中间任何 token 都算参数）。那个过宽的判据会**吞掉**
               本实现后面几处语法缺陷的痕迹 —— 实测：单独收紧它会让
               `examples/secrets.sml`（`$` 独立 token）、`slint/login.sml`（反引号串）、
               `gov_demo.sml`（`@contract X strict {`）从「静默错解」变成
               `E-PARSE-006` **硬失败**。**故顺序是「先修那三处、再收紧判据」**
               （反过来做等于把能解析的文件变成不能解析）。三处修完后收紧的收益：
               `examples/common.sml` 不再把注释闭合符那一串当成裸块参数
               （改前 C++ 14 行 vs Rust 1 行）。
               参数**不再静默丢弃**：首个 ⇒ `__name`、其余 ⇒ `__args`；
               参数经 coerce（与 Rust `parse_bare_block`、C 侧一致）。 */
            std::vector<ValuePtr> args;
            ValuePtr featags = nullptr;
            bool key_is_contract = (st.contracts.find(key) != st.contracts.end());
            const bool feat_on = (st.features & (1u << 17)) != 0 && key_is_contract;
            bool arg_has_contract = false;
            while (st.i < st.toks.size() && st.toks[st.i].t != Token::T::LBrace) {
                const Token& tk = st.toks[st.i];
                if (feat_on && tk.t == Token::T::Word && !tk.s.empty() && tk.s[0] == '!') {
                    const std::string tag = tk.s.substr(1);
                    /* **S5 使用处校验**：featag 契约私有 —— 块头写契约里没声明的名字就拒
                       （Rust 同码 E-CONTRACT-021）。改前只收集不校验。 */
                    const FeatagDecl* decl = nullptr;
                    {
                        auto owner = st.contracts.find(key);
                        if (owner != st.contracts.end()) {
                            for (const auto& d : owner->second.featags)
                                if (d.name == tag) { decl = &d; break; }
                            if (!decl) {
                                if (st.err && st.err->empty())
                                    *st.err = code_prefix(SML_E_CONTRACT_021,
                                        "sml: featag `!" + tag + "` 未在契约 `" + key +
                                        "` 中声明（契约私有的作用域，请先在契约体内写 `!" + tag + "`）");
                                st.aborted = true;
                                break;
                            }
                        }
                    }
                    st.i++;
                    ValuePtr payload = nullptr;
                    if (st.i < st.toks.size() && st.toks[st.i].t == Token::T::LBracket) {
                        st.i++;
                        payload = parse_array(st);
                    }
                    /* **S5b**：个数与标量类型（判据照 Rust `check_featags`）。
                       类型不符报 002 —— Rust 在那里复用 `check_value`，产出的是字段类型码。 */
                    if (decl) {
                        auto arg_ok = [](const std::string& ty, const ValuePtr& v) -> bool {
                            if (!v) return false;
                            if (ty.empty() || ty == "any") return true;
                            if (ty == "str")  return v->tag == Value::Tag::Str;
                            if (ty == "int")  return v->tag == Value::Tag::Int;
                            if (ty == "num")  return v->tag == Value::Tag::Int ||
                                                     v->tag == Value::Tag::Float;
                            if (ty == "bool") return v->tag == Value::Tag::Bool;
                            return true;
                        };
                        const bool has_args = payload && payload->tag == Value::Tag::Arr;
                        const size_t nargs = has_args ? payload->arr.size() : 0;
                        const bool is_auto = has_args && nargs == 1 &&
                                             payload->arr[0] && payload->arr[0]->tag == Value::Tag::Str &&
                                             payload->arr[0]->s == "auto";
                        std::string e;
                        if (!decl->has_spec && has_args)
                            e = "sml: featag `!" + tag + "` 声明为无参（纯标签），使用处不能带参数";
                        else if (decl->has_spec && !has_args)
                            e = "sml: featag `!" + tag + "` 声明为带参，使用处必须写成 `" + tag + "[…]`";
                        else if (decl->has_spec && has_args && !is_auto) {
                            if (decl->is_array) {
                                for (size_t k = 0; k < nargs; k++)
                                    if (!arg_ok(decl->inner, payload->arr[k])) {
                                        e = code_prefix(SML_E_CONTRACT_002,
                                            "sml: featag `!" + tag + "` 的参数类型不符（第 " +
                                            std::to_string(k + 1) + " 个参数）");
                                        break;
                                    }
                            } else if (nargs != 1)
                                e = "sml: featag `!" + tag + "` 的参数类型不是数组，只接受 1 个参数（实际给了 " +
                                    std::to_string(nargs) + " 个）";
                            else if (!arg_ok(decl->ty, payload->arr[0]))
                                e = code_prefix(SML_E_CONTRACT_002,
                                    "sml: featag `!" + tag + "` 的参数类型不符");
                        }
                        if (!e.empty()) {
                            if (e.find("E-") == std::string::npos)
                                e = code_prefix(SML_E_CONTRACT_021, e);
                            if (st.err && st.err->empty()) *st.err = e;
                            st.aborted = true;
                            break;
                        }
                    }
                    if (!featags) featags = Value::array();
                    ValuePtr item = Value::object();
                    item->obj.push_back({"name", Value::string(tag)});
                    if (payload) item->obj.push_back({"args", payload});
                    featags->arr.push_back(item);
                    continue;
                }
                if (feat_on && tk.t == Token::T::Comma) { st.i++; continue; }
                if (tk.t != Token::T::Word) break;
                if (key_is_contract && st.contracts.find(tk.s) != st.contracts.end())
                    arg_has_contract = true;
                args.push_back(coerce(tk, st.fragments, st.err, st.features));
                st.i++;
            }
            // 任务 D：块级多契约「响亮拒绝」—— head 词命中契约表且 args 里出现第 2 个
            //   已注册契约词 ⇒ E-CONTRACT-017（单契约 `server web prod` 的 web/prod 不是契约，不受影响）。
            if (key_is_contract && arg_has_contract) {
                if (st.err && st.err->empty())
                    *st.err = code_prefix(SML_E_CONTRACT_017,
                        "sml: 该实现暂不支持一个块应用多份契约（块级多契约写法）");
                st.aborted = true; break;
            }
            if (st.i < st.toks.size() && st.toks[st.i].t==Token::T::LBrace) {
                st.i++;
                st.enter_key = key;   /* 裸块：路径段用块类型名 */
                auto sub = parse_block_nested(st);
                sub->obj.push_back({"__type", Value::string(key)});
                if (featags) sub->obj.push_back({"__featags", featags});
                /* **S6 落点填充**：必须在契约应用**之前**（Rust `finish_bare_block` 同序）。
                   契约先跑会看不到这些键：严格契约会把它们报成 004，
                   `auto` 也取不到声明自己的 `default`。 */
                if (featags && st.contracts.find(key) != st.contracts.end()) {
                    const Contract& cc = st.contracts[key];
                    for (const auto& item : featags->arr) {
                        ValuePtr namev = item->get("name");
                        ValuePtr argsv = item->get("args");
                        if (!namev || namev->tag != Value::Tag::Str || !argsv) continue;
                        const FeatagDecl* d2 = nullptr;
                        for (const auto& dd : cc.featags)
                            if (dd.name == namev->s) { d2 = &dd; break; }
                        if (!d2 || d2->path.empty()) continue;
                        const std::string w = key + "." + namev->s;
                        const bool is_auto = argsv->arr.size() == 1 && argsv->arr[0] &&
                                             argsv->arr[0]->tag == Value::Tag::Str &&
                                             argsv->arr[0]->s == "auto";
                        ValuePtr val;
                        if (is_auto) {
                            if (!d2->has_default || !d2->def) {
                                if (st.err && st.err->empty())
                                    *st.err = code_prefix(SML_E_CONTRACT_022,
                                        "sml: `" + w + "` 的参数是 `auto`（取声明里的默认值），"
                                        "但该声明没有 `default`");
                                st.aborted = true;
                                break;
                            }
                            val = d2->def;
                        } else if (d2->has_spec && d2->is_array) {
                            val = argsv;
                        } else {
                            val = argsv->arr.empty() ? Value::null() : argsv->arr[0];
                        }
                        if (!write_target(st, sub, d2->path, 0, val, cc.strict_paths, w)) break;
                    }
                }
                /* typed-block：块头首词命中契约 ⇒ 等价块内首行 `@is`。
                   必须在 `__name` 之前应用：Rust 的块头契约校验发生在命名参数落位之前。 */
                if ((st.features & (1u << 14)) != 0 &&
                    st.contracts.find(key) != st.contracts.end()) {
                    std::string ce;
                    Parser::apply_contract(sub, st.contracts, key, &ce);
                    if (!ce.empty()) {
                        if (st.err && st.err->empty()) *st.err = ce;
                        st.aborted = true;
                    }
                }
                if (!args.empty()) {
                    sub->obj.push_back({"__name", args[0]});
                    if (args.size() > 1) {
                        auto extra = Value::array();
                        for (size_t k = 1; k < args.size(); k++) extra->arr.push_back(args[k]);
                        sub->obj.push_back({"__args", extra});
                    }
                }
                set_field(key, sub);
            } else {
                // no body -> treat as scalar string
                set_field(key, coerce_word(key, st.fragments, st.err, st.features));
            }
        } else if (nxt.t == Token::T::Word) {
            ValuePtr v = coerce(nxt, st.fragments, st.err, st.features);
            st.i++;
            set_field(key, v);
        } else if (nxt.t == Token::T::RBrace || nxt.t==Token::T::RBracket) {
            /* key }  -> key itself is the value (Rust: coerce key).
               例外：块内独立一行的 `&frag` 是**字段展开**（splice）——把片段的字段并进
               当前块，而不是造一个 `&frag` 键（conformance 11；C/JS/Lua 已是此行为）。
               ｜ A bare `&frag` splices the fragment's fields into this block. */
            if (!key.empty() && key[0] == '&') {
                auto it = st.fragments.find(key.substr(1));
                if (it != st.fragments.end() && it->second->tag == Value::Tag::Obj) {
                    for (auto& kv : it->second->obj) {
                        set_field_local(node, kv.first, kv.second->clone());
                    }
                } else {
                    set_field(key, coerce_word(key, st.fragments, st.err, st.features));
                }
            } else {
                set_field(key, coerce_word(key, st.fragments, st.err, st.features));
            }
        } else {
            // nxt is some structural token with no value
            set_field(key, Value::null());
            if (nxt.t==Token::T::Comma) st.i++;
        }
    }

    /* 由 `{` 开启却走到文件结尾 → E-PARSE-001（原先静默返回残缺的树）。
       置 aborted 与 err 是否存在无关：调用方可以不传 err，但「解析已失败」这件事
       照样要让整体失败（与深度守卫同一约定）。 */
    if (!closed && !st.aborted) {
        if (st.err && st.err->empty())
            *st.err = code_prefix(SML_E_PARSE_001,
                                  "sml: 未闭合的块（遇到文件结尾，缺少结束符号 }）");
        st.aborted = true;
    }

    // apply pending @is for this block
    /* ⚠️ 已中止时**不再应用** @is：否则契约错误会覆盖掉上面刚写好的
       E-PARSE-001/E-LIMIT-001，把码报成别的（码是契约，不许被后来者改写）。 */
    if (!st.aborted && !pending_is.empty()) {
        std::string cname = pending_is;
        pending_is.clear();
        std::string e;
        Parser::apply_contract(node, st.contracts, cname, &e);
        if (!e.empty() && st.err) *st.err = e;
    }
    /* 悬空 `@when`：块结束时条件还没被任何字段/块消费。
       Dangling `@when`: the block ended before the condition consumed an item. */
    if (!st.aborted && st.pending_when != 0) {
        if (st.err && st.err->empty())
            *st.err = code_prefix(SML_E_PARSE_017,
                "sml: `@when` 后未跟随任何字段或块（悬空的条件）");
        st.aborted = true;
    }
    /* 悬空的 `@weakbind`（本块内没等到块）：随本块丢弃，不落到外层的下一个块上。
       软校验发生在 set_field 消费挂起项的那一刻。 */
    if (!st.pending_weak.empty() && st.pending_weak_depth >= st.depth)
        st.pending_weak.clear();
    return node;
}

// standalone set_field used inside parse_block (promote same-name to array)
static void set_field_local(const ValuePtr& node, const std::string& k, const ValuePtr& v) {
    for (auto& kv : node->obj) {
        if (kv.first == k) {
            if (kv.second->tag == Value::Tag::Arr) {
                kv.second->arr.push_back(v);
            } else {
                auto arr = Value::array();
                arr->arr.push_back(kv.second);
                arr->arr.push_back(v);
                kv.second = arr;
            }
            return;
        }
    }
    node->obj.push_back({k, v});
}

// Value::array_with helper
ValuePtr Value::array_with(const std::vector<ValuePtr>& elems) {
    auto a = Value::array();
    a->arr = elems;
    return a;
}

// ===========================================================================
// include 展开（W18）
// ===========================================================================
/* 把 `@include "f"` 三个 token 原地换掉，换成 f 的 token 段（递归展开）。
 *
 * 为什么不继续「边解析边插 token」（W18 前的写法）：那种写法每次插入都要让 `st.i`
 * 恰好落在插入段的首 token 上，而「先插到路径 token 之前、再 st.i++ 吃掉路径」的顺序
 * 会让 st.i **跳过插入段的首 token**。后果是 `include` 退化成裸块键，把 includer 后面
 * 的字段和被包含文件的字段一起吞掉 —— 实测 a.sml = `@include "b.sml"` + `from_a: 1`
 * 解析完只剩一个垃圾键 `include = "include"`。
 *
 * ⚠️ 也**不能只改索引**：那个 off-by-one 恰好压住了无限展开（首 token 被跳过 ⇒ 嵌套的
 *    `@include` 永远不会被当成指令），而环检测用的栈当时是 push 完立刻 pop、**永远为空**，
 *    于是自包含/互包含根本拦不住。只修索引会把「被压住的无限展开」放出来变成真死循环。
 *    环检测必须同时给出：本函数用**链栈**（只装根到当前这条路径），故菱形包含仍合法。
 *
 * 语义与 Rust 的 sml-include::expand_includes / C 的 resolve_includes 对齐：
 *   - 环检测按**链栈**（只判当前路径）→ 菱形包含合法，自包含/互包含报 E-INCLUDE-002；
 *   - 嵌套深度上限 32（E-INCLUDE-004，与 Rust 的 MAX_INCLUDE_DEPTH 同值）；
 *   - 展开次数是**全局**计数（E-LIMIT-003）：深度上限挡不住菱形包含的 2^N 膨胀；
 *   - 被包含文件的基准目录换成**它自己的所在目录**（嵌套 include 相对父文件解析）；
 *   - 越界即拒绝（E-INCLUDE-003）；基准目录本身不可解析也拒绝（E-INCLUDE-010，
 *     fail-closed：不比一个比不了的对象就放行，等于把越界校验静默关掉）。
 */
#define SML_MAX_INCLUDE_DEPTH 32
#define SML_MAX_INCLUDE_EXPANSIONS 10000
