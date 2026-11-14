/* =====================================================================
** 1. 值构造 / 释放 / 容器
** ===================================================================== */

sml_value *sml_new_null(void) {
    sml_value *v = (sml_value *)calloc(1, sizeof(sml_value));
    if (v) v->type = SML_NULL;
    return v;
}

sml_value *sml_new_bool(int b) {
    sml_value *v = sml_new_null();
    if (v) { v->type = SML_BOOL; v->u.boolean = b ? 1 : 0; }
    return v;
}

sml_value *sml_new_int(long long i) {
    sml_value *v = sml_new_null();
    if (v) { v->type = SML_INT; v->u.i = i; }
    return v;
}

sml_value *sml_new_float(double f) {
    sml_value *v = sml_new_null();
    if (v) { v->type = SML_FLOAT; v->u.f = f; }
    return v;
}

sml_value *sml_new_strn(const char *s, size_t n) {
    sml_value *v = sml_new_null();
    if (!v) return NULL;
    v->type = SML_STR;
    v->u.s = (char *)malloc(n + 1);
    if (!v->u.s) { free(v); return NULL; }
    memcpy(v->u.s, s, n);
    v->u.s[n] = '\0';
    v->slen = n;   /* 保留长度：串内可以有 `\0`（C 字符串本身到 NUL 即止）。 */
    return v;
}

sml_value *sml_new_str(const char *s) {
    return sml_new_strn(s ? s : "", s ? strlen(s) : 0);
}

sml_value *sml_new_array(void) {
    sml_value *v = sml_new_null();
    if (v) {
        v->type = SML_ARRAY;
        v->u.arr.items = NULL;
        v->u.arr.len = v->u.arr.cap = 0;
    }
    return v;
}

sml_value *sml_new_object(void) {
    sml_value *v = sml_new_null();
    if (v) {
        v->type = SML_OBJECT;
        v->u.obj.head = v->u.obj.tail = NULL;
        v->u.obj.len = 0;
    }
    return v;
}

void sml_free(sml_value *v) {
    if (!v) return;
    switch (v->type) {
        case SML_STR:
            free(v->u.s);
            break;
        case SML_ARRAY: {
            size_t i;
            for (i = 0; i < v->u.arr.len; i++) sml_free(v->u.arr.items[i]);
            free(v->u.arr.items);
            break;
        }
        case SML_OBJECT: {
            sml_field *f = v->u.obj.head;
            while (f) {
                sml_field *nx = f->next;
                free(f->key);
                sml_free(f->value);
                free(f);
                f = nx;
            }
            break;
        }
        default:
            break;
    }
    free(v);
}

/* 深拷贝 (用于契约默认值回填, 避免共享引用被修改) */
static sml_value *sml_clone(const sml_value *v) {
    if (!v) return NULL;
    switch (v->type) {
        case SML_NULL:  return sml_new_null();
        case SML_BOOL:  return sml_new_bool(v->u.boolean);
        case SML_INT:   return sml_new_int(v->u.i);
        case SML_FLOAT: return sml_new_float(v->u.f);
        case SML_STR:   return sml_new_strn(v->u.s, v->slen);
        case SML_ARRAY: {
            sml_value *a = sml_new_array();
            size_t i;
            for (i = 0; i < v->u.arr.len; i++) sml_arr_push(a, sml_clone(v->u.arr.items[i]));
            return a;
        }
        case SML_OBJECT: {
            sml_value *o = sml_new_object();
            sml_field *f;
            for (f = v->u.obj.head; f; f = f->next) sml_obj_set(o, f->key, sml_clone(f->value));
            return o;
        }
    }
    return sml_new_null();
}

void sml_obj_set(sml_value *obj, const char *key, sml_value *val) {
    if (!obj || obj->type != SML_OBJECT || !key || !val) return;
    /* 替换已存在的键 */
    sml_field *f;
    for (f = obj->u.obj.head; f; f = f->next) {
        if (strcmp(f->key, key) == 0) {
            sml_free(f->value);
            f->value = val;
            return;
        }
    }
    /* 追加 */
    f = (sml_field *)calloc(1, sizeof(sml_field));
    if (!f) return;
    f->key = strdup(key);
    f->value = val;
    if (obj->u.obj.tail) obj->u.obj.tail->next = f;
    else obj->u.obj.head = f;
    obj->u.obj.tail = f;
    obj->u.obj.len++;
}

sml_value *sml_obj_get(const sml_value *obj, const char *key) {
    if (!obj || obj->type != SML_OBJECT || !key) return NULL;
    sml_field *f;
    for (f = obj->u.obj.head; f; f = f->next) {
        if (strcmp(f->key, key) == 0) return f->value;
    }
    return NULL;
}

sml_value *sml_get_path(const sml_value *v, const char *path) {
    if (!v || !path) return NULL;
    const sml_value *cur = v;
    char buf[256];
    size_t plen = strlen(path);
    if (plen >= sizeof(buf)) return NULL;
    memcpy(buf, path, plen + 1);
    char *save = NULL;
    char *tok = strtok_r(buf, ".", &save);
    while (tok) {
        if (!cur || cur->type != SML_OBJECT) return NULL;
        cur = sml_obj_get(cur, tok);
        if (!cur) return NULL;
        tok = strtok_r(NULL, ".", &save);
    }
    return (sml_value *)cur;
}

void sml_arr_push(sml_value *arr, sml_value *val) {
    if (!arr || arr->type != SML_ARRAY || !val) return;
    if (arr->u.arr.len >= arr->u.arr.cap) {
        size_t ncap = arr->u.arr.cap ? arr->u.arr.cap * 2 : 8;
        sml_value **ni = (sml_value **)realloc(arr->u.arr.items, ncap * sizeof(sml_value *));
        if (!ni) return;
        arr->u.arr.items = ni;
        arr->u.arr.cap = ncap;
    }
    arr->u.arr.items[arr->u.arr.len++] = val;
}

sml_value *sml_arr_get(const sml_value *arr, size_t i) {
    if (!arr || arr->type != SML_ARRAY || i >= arr->u.arr.len) return NULL;
    return arr->u.arr.items[i];
}

size_t sml_arr_len(const sml_value *arr) {
    return (arr && arr->type == SML_ARRAY) ? arr->u.arr.len : 0;
}

void sml_free_str(char *s) { free(s); }
void sml_free_cstr(char *p) { free(p); }

const char *sml_version(void) {
    return "sml 0.2.0 (SNOWARE Markup Language, pure C)";
}

/* 版本字面量 → 主版本号（`docs/swsml/spec/version.md` §1）。
   | Version literal -> major number.
   返回 1..4 = 本实现支持；-1 = 5.0 族（Dev，未启用）；0 = 无法识别。
   接受四类写法（与 Rust `Edition::parse` 同口径）：
     `vN` 前缀 / `N` / `N.0` 点号 / `N0` 连写；另接受代号 `camelia`（等价 50）。
   注意 `N0` 与 `N.0` 是同一件事：`10` 读作 `1.0`，`40` 读作 `4.0`。 */
static int version_lit_major(const char *s) {
    char buf[32];
    if (!s) return 0;
    if (s[0] == '"') { /* 只写代号时是引号串 */
        size_t len = strlen(s);
        if (len < 3 || s[len - 1] != '"' || len - 2 >= sizeof buf) return 0;
        memcpy(buf, s + 1, len - 2);
        buf[len - 2] = '\0';
    } else {
        size_t n = strlen(s);
        if (n >= sizeof buf) return 0;
        memcpy(buf, s, n + 1);
    }
    if (strcmp(buf, "camelia") == 0) return -1;
    const char *p = buf;
    if (*p == 'v' || *p == 'V') p++;
    if (p[0] < '1' || p[0] > '9') return 0;
    const int major = p[0] - '0';
    const int ok = (p[1] == '\0')
                   || (p[1] == '.' && p[2] == '0' && p[3] == '\0')
                   || (p[1] == '0' && p[2] == '\0');
    if (!ok) return 0;
    return major <= 4 ? major : -1;
}

/* =====================================================================
** 1b. 字符 / 编码辅助
** ===================================================================== */

static int hexdigit(int c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

/* 把 Unicode 码点编码为 UTF-8, 写入 sb (slen 更新) */
static void put_utf8(char *sb, size_t *slen, unsigned long cp) {
    if (cp <= 0x7F) {
        sb[(*slen)++] = (char)cp;
    } else if (cp <= 0x7FF) {
        sb[(*slen)++] = (char)(0xC0 | (cp >> 6));
        sb[(*slen)++] = (char)(0x80 | (cp & 0x3F));
    } else if (cp <= 0xFFFF) {
        sb[(*slen)++] = (char)(0xE0 | (cp >> 12));
        sb[(*slen)++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        sb[(*slen)++] = (char)(0x80 | (cp & 0x3F));
    } else {
        sb[(*slen)++] = (char)(0xF0 | (cp >> 18));
        sb[(*slen)++] = (char)(0x80 | ((cp >> 12) & 0x3F));
        sb[(*slen)++] = (char)(0x80 | ((cp >> 6) & 0x3F));
        sb[(*slen)++] = (char)(0x80 | (cp & 0x3F));
    }
}

