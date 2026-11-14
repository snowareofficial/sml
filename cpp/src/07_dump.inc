static bool starts_inline(const ValuePtr& v) {
    return !(v && v->tag == Value::Tag::Obj && !v->obj.empty());
}

/* 容器是否「扁平」：**直接子项全是标量**（不再嵌对象 / 数组）。
   与 Rust `dump.rs::is_flat`、C `sml.c::is_flat` 同一判据，是「一行写完」还是
   「展开多行」的唯一开关：
     `{ type: home }`                      扁平 → `[ { type: home } ]` 仍是一行
     `{ children: [ … ] }`                 非扁平 → `\n{ … }` 展开
   ⚠️ **只看一层，不递归**：递归版（「子孙全是标量」）是**恒真判据** —— 任何对象的
   子孙最终都会落到标量，于是所有东西都被判成扁平、排版分毫不改。Rust 侧当初写这段时
   真踩过这个坑（编译与测试全绿，只是完全没生效）。 */
static bool is_flat(const ValuePtr& v) {
    if (!v) return true;
    if (v->tag == Value::Tag::Obj) {
        for (auto& kv : v->obj) {
            const ValuePtr& fv = kv.second;
            if (fv && (fv->tag == Value::Tag::Obj || fv->tag == Value::Tag::Arr)) return false;
        }
        return true;
    }
    if (v->tag == Value::Tag::Arr) {
        for (auto& iv : v->arr)
            if (iv && (iv->tag == Value::Tag::Obj || iv->tag == Value::Tag::Arr)) return false;
        return true;
    }
    return true;   // 标量一律扁平
}

/* 写对象体：**不含**开头的 `{`，逐键换行 + 收尾 `}`。
   ⚠️ **所有键都写**，包括 `__type` / `__name`：裸块 `type [name] { … }` 解析后就长成
   这两个键，序列化必须能写回去（W4 ③ A 类）。改前这几处把它们当「内部标记」跳过，
   后果不只是每个块少两行 —— **只有元数据的块会被判成空体 ⇒ 输成 `{}`**，元数据静默消失。 */
static void dump_object_body(const ValuePtr& v, int indent, std::string& out) {
    for (auto& kv : v->obj) {
        out += "\n";
        out += std::string((size_t)(indent + 1) * 2, ' ');
        out += kv.first;
        out += starts_inline(kv.second) ? ": " : ":";
        dump_value(kv.second, indent + 1, out);
    }
    out += "\n";
    out += std::string((size_t)indent * 2, ' ');
    out += "}";
}

/* 写数组体：**不含**开头的 `[`。每元素先换行 + 缩进，再由 dump_element 决定
   「一行写完」还是「就地展开」。 */
static void dump_array_body(const ValuePtr& v, int indent, std::string& out) {
    out += "[";
    for (auto& e : v->arr) {
        out += "\n";
        out += std::string((size_t)(indent + 1) * 2, ' ');
        dump_element(e, indent + 1, out);
    }
    out += "\n";
    out += std::string((size_t)indent * 2, ' ');
    out += "]";
}

/* 纯数字形态（会被 coerce_word 字面量化）。`0x` / `0b` 前缀不算 —— 与 Rust
   `f64::parse` 不认十六进制对齐（strtod 会认 `0x20`）。 */
static bool dump_looks_number(const std::string& s) {
    if (s.empty()) return false;
    const size_t i = (s[0] == '+' || s[0] == '-') ? 1 : 0;
    if (i >= s.size()) return false;
    if (s.size() > i + 1 && s[i] == '0'
        && (s[i+1] == 'x' || s[i+1] == 'X' || s[i+1] == 'b' || s[i+1] == 'B'))
        return false;
    if (!((s[i] >= '0' && s[i] <= '9') || s[i] == '.')) return false;
    char* e = nullptr;
    std::strtoll(s.c_str(), &e, 10);
    if (e && *e == '\0' && e != s.c_str()) return true;
    e = nullptr;
    std::strtod(s.c_str(), &e);
    if (e && *e == '\0' && e != s.c_str()) return true;
    return false;
}

/* 是否需要给字符串加引号 —— 与 Rust `sml-value::text_rules::needs_quote` 同判据：
   空串 / true·false·null·inf·nan / 纯数字 / 注释前缀 / 破坏语法的字符 /
   首字符非字母下划线 / 含 [A-Za-z0-9_.-] 之外的字符。
   改前只判空白与 `:#{}"\\` ⇒ `"42"`、`"true"`、`"007"` 全写成裸词，
   自己 dump 的文本自己读回来就换了类型（台账 S-05）。 */
static bool dump_needs_quote(const std::string& s) {
    if (s.empty()) return true;
    if (s == "true" || s == "false" || s == "null" || s == "inf" || s == "nan") return true;
    if (dump_looks_number(s)) return true;
    if (s.rfind("--", 0) == 0 || s.rfind("//", 0) == 0 || s.rfind("/*", 0) == 0
        || s.rfind("*/", 0) == 0 || s[0] == '*' || s.rfind("_*", 0) == 0)
        return true;
    for (unsigned char c : s) {
        if (c == ' ' || c == '\t' || c == '\n' || c == '\r' ||
            c == ':' || c == '#' || c == '{' || c == '}' ||
            c == ',' || c == '[' || c == ']' ||
            c == '"' || c == '\\' || c == '/' || c == '*')
            return true;
    }
    const unsigned char f = (unsigned char)s[0];
    if (!((f >= 'A' && f <= 'Z') || (f >= 'a' && f <= 'z') || f == '_' || f >= 0x80))
        return true;
    for (unsigned char c : s) {
        const bool ok = (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z')
                     || (c >= '0' && c <= '9') || c == '_' || c == '-' || c == '.'
                     || c >= 0x80;
        if (!ok) return true;
    }
    return false;
}

/* 加引号**同时**要转义：`\\ \" \n \t \r \0`，其余控制字符走 `\u00XX`
   —— 与 Rust `quote_if_needed` 的转义集逐条一致。改前只转义 `\` 与 `"`，
   真换行 / 真 NUL 会以裸字节落进文本（台账 S-06）。 */
static std::string dump_escape_str(const std::string& s) {
    std::string out;
    out.reserve(s.size() + 2);
    char u[8];
    for (unsigned char c : s) {
        switch (c) {
            case '\\': out += "\\\\"; break;
            case '"':  out += "\\\""; break;
            case '\n': out += "\\n"; break;
            case '\t': out += "\\t"; break;
            case '\r': out += "\\r"; break;
            case 0:    out += "\\0"; break;
            default:
                if (c < 0x20 || c == 0x7f) {
                    std::snprintf(u, sizeof(u), "\\u%04X", (unsigned)c);
                    out += u;
                } else {
                    out += (char)c;
                }
        }
    }
    return out;
}

/* 浮点输出：整数值**必须保留小数点** —— `1.0` 若写成 `1`，回读就变成 Int，
   类型被静默改掉（与 Rust `format_float`、C 的 add_float 同口径，见 TODO 雷 5 B）。 */
static std::string fmt_float(double f) {
    std::ostringstream os;
    os << std::setprecision(17) << f;
    std::string s = os.str();
    if (std::isfinite(f) && std::floor(f) == f &&
        s.find_first_of("eE") == std::string::npos) {
        s += ".0";
    }
    return s;
}

/* 把值压成**一行**写。契约：只对「扁平」值调用（见 is_flat）——
   因此它内部的递归永远不会撞上容器，压出来的行里不会再有换行。
   ⚠️ **不筛键**（含 __type / __name），与 Rust `dump_inline` 的 `m.iter()` 一致。 */
static void dump_inline(const ValuePtr& v, std::string& out) {
    if (!v) { out += "null"; return; }
    switch (v->tag) {
        case Value::Tag::Null: out += "null"; break;
        case Value::Tag::Bool: out += v->b ? "true" : "false"; break;
        case Value::Tag::Int:  out += std::to_string(v->i); break;
        case Value::Tag::Float: {
            out += fmt_float(v->f);
            break;
        }
        case Value::Tag::Str:
            if (dump_needs_quote(v->s))
                out += "\"" + dump_escape_str(v->s) + "\"";
            else out += v->s;
            break;
        case Value::Tag::Arr: {
            out += "[ ";
            for (size_t k = 0; k < v->arr.size(); k++) {
                if (k) out += ", ";
                dump_inline(v->arr[k], out);
            }
            out += " ]";
            break;
        }
        case Value::Tag::Obj: {
            out += "{ ";
            bool first = true;
            for (auto& kv : v->obj) {
                if (!first) out += ", ";
                first = false;
                out += kv.first + ": ";
                dump_inline(kv.second, out);
            }
            out += " }";
            break;
        }
    }
}

static void dump_value(const ValuePtr& v, int indent, std::string& out) {
    if (!v) { out += "null"; return; }
    switch (v->tag) {
        case Value::Tag::Null: out += "null"; break;
        case Value::Tag::Bool: out += v->b ? "true" : "false"; break;
        case Value::Tag::Int:  out += std::to_string(v->i); break;
        case Value::Tag::Float: {
            out += fmt_float(v->f);
            break;
        }
        case Value::Tag::Str:
            if (dump_needs_quote(v->s))
                out += "\"" + dump_escape_str(v->s) + "\"";
            else out += v->s;
            break;
        case Value::Tag::Arr: {
            if (v->arr.empty()) { out += "[]"; break; }
            dump_array_body(v, indent, out);
            break;
        }
        case Value::Tag::Obj: {
            /* 空对象写 `{}`、非空写 `\n{ … }` —— 与 Rust `dump_block` 同判据
               （A 类：这里原先排除 __type/__name 判「空体」，只有元数据的块会被误判）。 */
            if (v->obj.empty()) { out += "{}"; break; }
            out += "\n";
            out += std::string((size_t)indent * 2, ' ');
            out += "{";
            dump_object_body(v, indent, out);
            break;
        }
    }
}

/* 写一个「元素」：扁平的走单行（dump_inline），含结构的**就地展开**成多行。
   调用方负责**已**写好本元素开头的换行与缩进（`indent` 即该缩进级别），
   因此这里不在开头补缩进；展开出来的续行由各自递归负责对齐。
   对应 Rust `dump.rs::dump_element`（W4 ② 的对齐点）。 */
static void dump_element(const ValuePtr& v, int indent, std::string& out) {
    if (!v) { out += "null"; return; }
    if (is_flat(v)) { dump_inline(v, out); return; }
    switch (v->tag) {
        case Value::Tag::Obj:
            /* 与 dump_value 的 Obj 分支只差这一行：这里是数组元素位置，
               `{` 要跟在**当前行**（缩进已由调用方写好），不能再另起一行。 */
            out += "{";
            dump_object_body(v, indent, out);
            break;
        case Value::Tag::Arr:
            dump_array_body(v, indent, out);
            break;
        default:
            dump_inline(v, out);
            break;
    }
}

std::string Parser::to_sml(const ValuePtr& v) {
    std::string out;
    if (!v) return out;
    if (v->tag == Value::Tag::Obj) {
        /* 顶层分叉与 Rust `to_sml` 逐字对应：带 `__type` 的对象是**裸块的树形**
           （`type [name] { … }` 解析出来的），按 `dump_block(0,0)` 渲染 —— 先换行再
           `{`、逐键、收尾 `}`；空对象写 `{}`。不带 `__type` 的才是「顶层逐键成行」。
           （A 类：原先这里无条件逐键、并把 __type/__name 跳过。） */
        if (v->has("__type")) {
            if (v->obj.empty()) { out += "{}"; return out; }
            out += "\n{";
            dump_object_body(v, 0, out);
            return out;
        }
        for (auto& kv : v->obj) {
            out += kv.first;
            out += starts_inline(kv.second) ? ": " : ":";
            dump_value(kv.second, 0, out);
            out += "\n";
        }
        return out;
    }
    /* 顶层非对象：与数组元素同一套规则（扁平单行 / 含结构展开），
       Rust 侧 to_sml 走的也是 dump_element（顶层数组曾被整坨压成一行并非此处问题，
       见 dump_element/is_flat）。 */
    dump_element(v, 0, out);
    return out;
}

} // namespace sml
