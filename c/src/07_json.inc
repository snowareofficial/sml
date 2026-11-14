/* =====================================================================
** 5. JSON 互转 (自带极简 JSON 解析/序列化, 零外部依赖)
** ===================================================================== */

/* 最小 JSON -> sml_value (字符串/数字/bool/null/数组/对象) */
static sml_value *json_to_value(const char **pp);
static sml_value *json_to_value_depth(const char **pp, int depth);

/* 入口 wrapper：JSON 侧同样需要深度上限，
   否则 `[[[[ … ]]]]` 这种输入会把递归下降的栈打穿（段错误，接不住）。 */
static sml_value *json_to_value(const char **pp) {
    return json_to_value_depth(pp, 0);
}

/* 读一个 JSON 字符串（`*pp` 指向开引号），游标推到闭引号之后。
   **为什么单独成函数**：它需要 4 KB 解码缓冲。留在递归函数体内时，每一层递归都要背这
   4 KB —— 128 层就是 512 KB 栈；抽出来后递归帧只剩指针和几个局部量（实测 4.2 KB → 384 B）。 */
static sml_value *json_read_string(const char **pp) {
    const char *p = *pp;
    char buf[4096];
    size_t bl = 0;
    if (*p == '"') p++;
    while (*p && *p != '"') {
        if (*p == '\\' && p[1]) {
            p++;
            switch (*p) {
                case 'n': buf[bl++] = '\n'; break;
                case 't': buf[bl++] = '\t'; break;
                case 'r': buf[bl++] = '\r'; break;
                case '"': buf[bl++] = '"'; break;
                case '\\': buf[bl++] = '\\'; break;
                case 'u': {
                    int has_brace = 0;
                    if (*(p + 1) == '{') { has_brace = 1; p++; }
                    unsigned long cp = 0;
                    int cnt = 0;
                    while (1) {
                        if (has_brace) {
                            if (*(p + 1) == '}') { p++; break; }
                            if (!*(p + 1)) break;
                        } else {
                            if (cnt >= 4) break;
                        }
                        int h = hexdigit((unsigned char)*(p + 1));
                        if (h < 0) break;
                        cp = cp * 16 + (unsigned long)h;
                        cnt++;
                        p++;
                    }
                    put_utf8(buf, &bl, cp);
                    break;
                }
                default: buf[bl++] = *p; break;
            }
            p++;
        } else {
            buf[bl++] = *p++;
        }
        if (bl >= sizeof(buf) - 5) break;
    }
    if (*p == '"') p++;
    buf[bl] = '\0';
    *pp = p;
    /* 用带长度的构造：JSON 的 `\u0000` 解出真 NUL，`sml_new_str` 会在那儿截断。 */
    return sml_new_strn(buf, bl);
}

static sml_value *json_to_value_depth(const char **pp, int depth) {
    const char *p = *pp;
    /* 超限即返回 null：调用方只有 `if (v) ... push/set`，会自然跳过，
       且外层循环仍靠 *p 推进，不会死循环。 */
    if (depth > SML_MAX_VALUE_DEPTH) return sml_new_null();
    while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r') p++;
    if (*p == '{') {
        p++;
        sml_value *obj = sml_new_object();
        while (*p && *p != '}') {
            while (*p == ' ' || *p == '"' || *p == '\t' || *p == '\n' || *p == '\r') p++;
            char key[256];
            size_t kl = 0;
            while (*p && *p != '"') { if (kl < 255) key[kl++] = *p; p++; }
            key[kl] = '\0';
            if (*p == '"') p++;
            while (*p && *p != ':') p++;
            if (*p == ':') p++;
            sml_value *v = json_to_value_depth(&p, depth + 1);
            if (v) sml_obj_set(obj, key, v);
            while (*p && *p != ',' && *p != '}') p++;
            if (*p == ',') p++;
        }
        if (*p == '}') p++;
        *pp = p;
        return obj;
    } else if (*p == '[') {
        p++;
        sml_value *arr = sml_new_array();
        while (*p && *p != ']') {
            sml_value *v = json_to_value_depth(&p, depth + 1);
            if (v) sml_arr_push(arr, v);
            while (*p && *p != ',' && *p != ']') p++;
            if (*p == ',') p++;
        }
        if (*p == ']') p++;
        *pp = p;
        return arr;
    } else if (*p == '"') {
        *pp = p;
        return json_read_string(pp);
    } else if (strncmp(p, "true", 4) == 0) { *pp = p + 4; return sml_new_bool(1); }
    else if (strncmp(p, "false", 5) == 0) { *pp = p + 5; return sml_new_bool(0); }
    else if (strncmp(p, "null", 4) == 0) { *pp = p + 4; return sml_new_null(); }
    else {
        char num[64];
        size_t nl = 0;
        while (*p && (isdigit((unsigned char)*p) || *p == '-' || *p == '+' ||
                      *p == '.' || *p == 'e' || *p == 'E')) {
            if (nl < 63) num[nl++] = *p;
            p++;
        }
        num[nl] = '\0';
        if (nl) {
            char *end = NULL;
            long long iv = strtoll(num, &end, 10);
            if (end && *end == '\0') return sml_new_int(iv);
            double fv = strtod(num, &end);
            if (end && *end == '\0') return sml_new_float(fv);
        }
        return sml_new_null();
    }
}

static void value_to_json(sbuf *b, const sml_value *v) {
    char num[64];
    if (!v) { sb_add(b, "null"); return; }
    switch (v->type) {
        case SML_NULL: sb_add(b, "null"); break;
        case SML_BOOL: sb_add(b, v->u.boolean ? "true" : "false"); break;
        case SML_INT: snprintf(num, sizeof(num), "%lld", v->u.i); sb_add(b, num); break;
        case SML_FLOAT: add_float(b, v->u.f); break;
        case SML_STR:
            sb_addc(b, '"');
            for (size_t k = 0; k < v->slen; k++) {
                unsigned char ch = (const unsigned char)v->u.s[k];
                if (ch == '"') sb_add(b, "\\\"");
                else if (ch == '\\') sb_add(b, "\\\\");
                else if (ch == '\n') sb_add(b, "\\n");
                else if (ch == '\t') sb_add(b, "\\t");
                else if (ch == 0) sb_add(b, "\\u0000");
                else if (ch < 0x20) { char u[8]; snprintf(u, sizeof(u), "\\u%04X", ch); sb_add(b, u); }
                else sb_addc(b, (char)ch);
            }
            sb_addc(b, '"');
            break;
        case SML_ARRAY: {
            sb_addc(b, '[');
            size_t i;
            for (i = 0; i < v->u.arr.len; i++) {
                if (i) sb_addc(b, ',');
                value_to_json(b, v->u.arr.items[i]);
            }
            sb_addc(b, ']');
            break;
        }
        case SML_OBJECT: {
            sb_addc(b, '{');
            int first = 1;
            sml_field *f;
            for (f = v->u.obj.head; f; f = f->next) {
                if (!first) sb_addc(b, ',');
                first = 0;
                sb_addc(b, '"');
                sb_add(b, f->key);
                sb_addc(b, '"');
                sb_addc(b, ':');
                value_to_json(b, f->value);
            }
            sb_addc(b, '}');
            break;
        }
    }
}

char *sml_parse_json(const char *text) {
    if (!text) return NULL;
    sml_value *v = sml_parse(text, NULL, 0);
    if (!v) return NULL;
    sbuf b;
    memset(&b, 0, sizeof(b));
    value_to_json(&b, v);
    sml_free(v);
    return b.buf ? b.buf : strdup("null");
}

char *sml_dump_from_json(const char *json) {
    if (!json) return NULL;
    const char *p = json;
    sml_value *v = json_to_value(&p);
    if (!v) return NULL;
    char *out = sml_dump(v);
    sml_free(v);
    return out;
}

