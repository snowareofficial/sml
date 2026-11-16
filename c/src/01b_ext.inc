/* =====================================================================
** 1b. 外置扩展点（与 Rust `sml::ext` / `sml::contract_ext` 对齐）
** =====================================================================
**
** 为什么有这一层：SML 是通用数据格式，但下游常有「给文档挂带类型的元数据块，
** 且**不进主数据树**」与「`image` / `link` / `time` 这类领域类型」的需求。
** 把这些语义塞进本库会让数据格式沾上具体业务；外置扩展点让**类型名与校验规则
** 都留在下游仓库**，本库只负责「找到 → 取参数 → 读块 → 收集诊断」。
**
** 三条设计约束（与 Rust 同）：
**   1. **不注册任何扩展时，行为与既有解析完全一致**（零扩展 = 零影响）。
**   2. **内置名不可被占用**：与内置指令 / 内置类型同名 ⇒ `E-EXT-002`（注册期就拒，
**      不静默覆盖，避免方言改写核心语义）。
**   3. 回调只处理语义，**不碰解析游标**；参数 / 块由本库先解析好再交给回调。
**
** 用法（C，见 `sml.h` 的 `sml_parse_ext`）：
**
**     static sml_ext_outcome on_form(void *ud, const char *arg, sml_value *body,
**                                    sml_value **emit, char *err, size_t errsz) {
**         (void)ud; (void)arg; (void)body; (void)emit; (void)err; (void)errsz;
**         return SML_EXT_DISCARD;              // 元数据块：不进主树
**     }
**     static int check_image(void *ud, const sml_value *v, char *err, size_t errsz) {
**         (void)ud;
**         if (v && v->type == SML_STR) return 1;
**         snprintf(err, errsz, "image 须为附件 id 字符串");
**         return 0;
**     }
**     static const sml_ext_directive dirs[] = { { "form", 0, on_form, NULL } };
**     static const sml_ext_type      tys[]  = { { "image", check_image, NULL } };
**     static const sml_ext ext = { dirs, 1, tys, 1 };
**     sml_value *v = sml_parse_ext(text, &ext, err, sizeof err, NULL, NULL);
**
** ---------------------------------------------------------------------- */

/* 公开类型（`sml_ext_outcome` / `sml_ext_directive` / `sml_ext_type` / `sml_ext`）
   定义在 `sml.h` 里 —— 这里是本编译单元的实现部分，只放辅助函数，避免重复 typedef。 */

/* 内置指令名（禁止外置占用，与 Rust `BUILTIN_DIRECTIVES` 同表）。 */
static const char *const SML_BUILTIN_DIRECTIVES[] = {
    "contract", "is", "type", "version", "feature",
    "when", "for", "weakbind", "weakdata", "weakdoc"
};

/* 内置类型名（禁止外置占用，与 Rust `BUILTIN_TYPES` 同表）。 */
static const char *const SML_BUILTIN_TYPES[] = {
    "str", "int", "num", "bool", "any", "enum"
};

/* 内置修饰符名（禁止外置占用，与 Rust `BUILTIN_MODIFIERS` 同表）。 */
static const char *const SML_BUILTIN_MODIFIERS[] = {
    "optional", "required", "default", "min", "max"
};

static const sml_ext_directive *sml_ext_find_directive(const sml_ext *e, const char *name) {
    size_t i;
    if (!e || !e->directives || !name) return NULL;
    for (i = 0; i < e->n_directives; i++)
        if (e->directives[i].name && strcmp(e->directives[i].name, name) == 0)
            return &e->directives[i];
    return NULL;
}

static const sml_ext_type *sml_ext_find_type(const sml_ext *e, const char *name) {
    size_t i;
    if (!e || !e->types || !name) return NULL;
    for (i = 0; i < e->n_types; i++)
        if (e->types[i].name && strcmp(e->types[i].name, name) == 0)
            return &e->types[i];
    return NULL;
}

static const sml_ext_modifier *sml_ext_find_modifier(const sml_ext *e, const char *name) {
    size_t i;
    if (!e || !e->modifiers || !name) return NULL;
    for (i = 0; i < e->n_modifiers; i++)
        if (e->modifiers[i].name && strcmp(e->modifiers[i].name, name) == 0)
            return &e->modifiers[i];
    return NULL;
}

/* 注册期校验：与内置同名 / 重复注册 / 空名 ⇒ `E-EXT-002`（fail-closed）。
   在 `sml_parse_ext` 入口调用一次，用户不必自己记着「先注册再校验」。 */
static int sml_ext_validate(const sml_ext *e, char *err, size_t errsz) {
    size_t i, j;
    if (!e) return 0;
    for (i = 0; i < e->n_directives; i++) {
        const char *n = e->directives[i].name;
        if (!n || !*n) {
            set_err(err, errsz, SML_E_EXT_002 " 扩展指令名不可为空");
            return -1;
        }
        for (j = 0; j < sizeof SML_BUILTIN_DIRECTIVES / sizeof *SML_BUILTIN_DIRECTIVES; j++) {
            if (strcmp(n, SML_BUILTIN_DIRECTIVES[j]) == 0) {
                set_err(err, errsz, SML_E_EXT_002
                        " 不可注册与内置指令同名的扩展指令 `@%s`（内置："
                        "contract / is / type / version / feature / when / for / "
                        "weakbind / weakdata / weakdoc）", n);
                return -1;
            }
        }
        for (j = 0; j < i; j++) {
            if (e->directives[j].name && strcmp(n, e->directives[j].name) == 0) {
                set_err(err, errsz, SML_E_EXT_002 " 扩展指令 `@%s` 已注册", n);
                return -1;
            }
        }
    }
    for (i = 0; i < e->n_types; i++) {
        const char *n = e->types[i].name;
        if (!n || !*n) {
            set_err(err, errsz, SML_E_EXT_002 " 扩展类型名不可为空");
            return -1;
        }
        for (j = 0; j < sizeof SML_BUILTIN_TYPES / sizeof *SML_BUILTIN_TYPES; j++) {
            if (strcmp(n, SML_BUILTIN_TYPES[j]) == 0) {
                set_err(err, errsz, SML_E_EXT_002
                        " 不可注册与内置类型同名的扩展类型 `%s`（内置："
                        "str / int / num / bool / any / enum）", n);
                return -1;
            }
        }
        for (j = 0; j < i; j++) {
            if (e->types[j].name && strcmp(n, e->types[j].name) == 0) {
                set_err(err, errsz, SML_E_EXT_002 " 扩展类型 `%s` 已注册", n);
                return -1;
            }
        }
    }
    for (i = 0; i < e->n_modifiers; i++) {
        const char *n = e->modifiers[i].name;
        if (!n || !*n) {
            set_err(err, errsz, SML_E_EXT_002 " 扩展修饰符名不可为空");
            return -1;
        }
        for (j = 0; j < sizeof SML_BUILTIN_MODIFIERS / sizeof *SML_BUILTIN_MODIFIERS; j++) {
            if (strcmp(n, SML_BUILTIN_MODIFIERS[j]) == 0) {
                set_err(err, errsz, SML_E_EXT_002
                        " 不可注册与内置修饰符同名的扩展修饰符 `%s`", n);
                return -1;
            }
        }
        for (j = 0; j < i; j++) {
            if (e->modifiers[j].name && strcmp(n, e->modifiers[j].name) == 0) {
                set_err(err, errsz, SML_E_EXT_002 " 扩展修饰符 `%s` 已注册", n);
                return -1;
            }
        }
    }
    return 0;
}
