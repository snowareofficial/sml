/*
 * SPDX-License-Identifier: MulanPSL-2.0
 * SML v1..v4 桥接头文件（Rust cdylib 后端）。
 * 仓库：https://gitee.com/snoware/sml
 * @file sml_rs.h
 * @brief SML v1..v4 C-ABI 桥接：经 Rust cdylib 获得 v1..v4 全集能力（$env / glob-include / @feature / @contract）。
 * @version 0.2.0 (swsml 0.6.4)
 */

/*
 * sml_rs.h — SML v1..v4 桥接头文件 (Rust cdylib 后端)
 *
 * 本文件不实现解析器，而是桥接 Rust crate `swsml` 编译出的 cdylib
 * (sml.dll / libsml.so / libsml.dylib)，从而获得 v1..v4 完整能力：
 * $env 内联 / glob-include / @feature / @contract。
 *
 * 与 sml.h 的关系 —— 两个后端，二选一链接，不可同时 include：
 *   sml.h    纯 C99 自包含实现，零依赖，功能为基础集。
 *   sml_rs.h 本文件，桥接 Rust cdylib，功能为 v1..v4 全集。
 * 两者刻意保持命名与语义对齐（sml_free 释放值树、sml_free_str 释放字符串），
 * 便于切换后端时少改代码 —— 但值模型不同，不可混用。
 *
 * 链接方式 (Windows / mingw)：
 *   gcc example_rs.c -L<target>/release -lsml -o example_rs
 * 运行时需 sml.dll 在 PATH 或同目录。
 *
 * 生命周期（务必遵守）：
 *   1. sml_loads / sml_load_file 返回的根指针 —— 调用方 sml_free 释放。
 *   2. sml_get / sml_get_path / sml_at 返回**借用**指针，不可释放，
 *      且随根节点一同失效（根节点 free 后不可再用）。
 *   3. 任何 char* 输出 —— 调用方 sml_free_str 释放。
 *
 * 设计取舍（对标 jansson / tomlc99）：
 *   - sml_error 结构体：jansson 的 line/column/position/source/text
 *     详细定位，省去"只返回 NULL 却不知哪错了"的调试困境。
 *   - flags 位标志：替代旧版的 opts_json 字符串参数。C 侧没有 JSON 库
 *     也能精确控制特性，不再强迫用户为了用 SML 先集成 cJSON。
 *   - xxx_in 单行取值：tomlc99 的便利风格，配置读取一行搞定。
 *   - 值树可直接遍历：不必先序列化成 JSON 再解析一遍。
 */
#ifndef SML_RS_H
#define SML_RS_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ==================================================================== *
 * 错误
 * ==================================================================== */

typedef enum {
    SML_OK = 0,
    SML_ERR_SYNTAX,           /* 语法错误 */
    SML_ERR_FEATURE_DISABLED, /* 使用了未启用的特性 */
    SML_ERR_VERSION_MISMATCH, /* @version 不在允许范围 */
    SML_ERR_CONTRACT,         /* @contract 契约校验失败 */
    SML_ERR_INCLUDE_LOOP,     /* include 循环引用 */
    SML_ERR_IO,               /* 文件读取失败 */
    SML_ERR_UTF8,             /* 非法 UTF-8 */
    SML_ERR_INTERNAL          /* 内部错误（如传入 NULL） */
} sml_errc;

typedef struct {
    int    code;            /* sml_errc；成功时为 SML_OK */
    int    line;            /* 1-based 行号；0 = 未知 */
    int    column;          /* 1-based 列号；0 = 未知 */
    size_t position;        /* 字节偏移 */
    char   source[128];     /* 来源（文件名或 <string>） */
    char   text[256];       /* 错误信息（人读；**不含**码） */
    char   code_str[16];    /* 错误码，如 "E-PARSE-008"；见 errors/codes.sml。
                             * code 是粗粒度枚举（9 档，供 switch），
                             * code_str 才是稳定契约（同因同码、不随文案变）。 */
} sml_error;

/* ==================================================================== *
 * 值与类型
 * ==================================================================== */

typedef enum {
    SML_TYPE_NULL = 0,
    SML_TYPE_BOOL,
    SML_TYPE_INT,
    SML_TYPE_FLOAT,
    SML_TYPE_STR,
    SML_TYPE_ARRAY,
    SML_TYPE_OBJECT
} sml_type;

/* 不透明句柄：内部结构由 Rust 侧维护，调用方只能通过下面的函数访问 */
typedef struct sml_value sml_value;

/* ==================================================================== *
 * 特性位标志
 *
 * flags == 0 表示「默认基线」（含 bareword-string / include / fragment
 * 等基础特性），与 jansson 的 flags=0 语义一致。
 * flags != 0 时按位精确构造，可用于收紧允许范围（安全沙箱场景）。
 * ==================================================================== */

#define SML_F_BAREWORD_STR  (1u << 0)  /* 裸词即字符串（v1 行为） */
#define SML_F_INCLUDE       (1u << 1)  /* include "x.sml" */
#define SML_F_ENV           (1u << 2)  /* $env.VAR 内联 */
#define SML_F_CONTRACT      (1u << 3)  /* @contract / @is */
#define SML_F_FRAGMENT      (1u << 4)  /* @frag 定义 / &frag 引用 */
#define SML_F_TOP_ARRAY     (1u << 5)  /* 顶层裸数组 */
#define SML_F_NAMESPACE     (1u << 6)  /* include ... as ns */
#define SML_F_IMPLICIT_NS   (1u << 7)  /* 无扩展名 include 默认产生 ns */
#define SML_F_MULTI_INCLUDE (1u << 8)  /* include "a", "b" as y */
/* Note for maintainers: a slash immediately followed by an asterisk inside a
 * block comment triggers -Wcomment. Keep glob examples free of that pair. */
#define SML_F_GLOB_INCLUDE  (1u << 9)  /* glob include: wildcard path matching */
#define SML_F_REGEX_INCLUDE (1u << 10) /* regex include: path matched by pattern */
#define SML_F_EXT_REWRITE   (1u << 11) /* include "x.conf" -> "x.sml" */
#define SML_F_WHEN          (1u << 12) /* @when 条件裁剪 */
#define SML_F_FOR           (1u << 13) /* @for 有界循环 */
#define SML_F_TYPED_BLOCK   (1u << 14) /* 块级类型标注：`<契约名> <块名> { .. }` */
#define SML_F_CASE_INSENSITIVE (1u << 15) /* 契约名查找首字母大小写不敏感 */
#define SML_F_BLOCK_NAME    (1u << 16) /* 块级标注里"非契约词"可作块名 */
/* 18th bit. 文档里写作 `type.featag`（第一个带域名，见 docs/swsml/spec/features.md）。
 * Only the Rust parser knows this name so far; C/C++/Lua reject it (E-FEATURE-003 / E-PARSE-005).
 * Rust: bit is registered, grammar arrives in S2 -> enabling it changes nothing yet. */
#define SML_F_FEATAG        (1u << 17)
/* 19th bit: identifier whitelist (`lex.ident`, S1b). Rust-only for now; the C
 * parser does not validate identifiers, so this bit is a placeholder there. */
#define SML_F_IDENT         (1u << 18)
/* 20th bit: weak-declaration family (`type.weak`, docs/swsml/spec/weakbind.md).
 * 文档里写作 `@feature enable type.weak`。目前只有 Rust 认识它；其余端在
 * 发布前那次统一同步里补上（`E-FEATURE-003` / `E-PARSE-005` 是现状，不是终态）。 */
#define SML_F_WEAK          (1u << 19)

/* 常用组合 */
#define SML_F_BASIC  (SML_F_BAREWORD_STR | SML_F_INCLUDE | SML_F_FRAGMENT)
#define SML_F_V3_ALL (0xFFFFFFFFu)

/* ==================================================================== *
 * 加载
 * ==================================================================== */

/* 解析文本。err 可为 NULL（不关心错误详情时）。失败返回 NULL。 */
sml_value *sml_loads(const char *text, unsigned flags, sml_error *err);

/* 解析文件：展开 include，相对路径以文件所在目录为基准。
 * 注意：include 语法要求路径带引号 —— include "conf.d/x.sml"。 */
sml_value *sml_load_file(const char *path, unsigned flags, sml_error *err);

/* ==================================================================== *
 * 非致命诊断（v3.1）
 *
 * 与 sml_loads 的差别只有一个出参：flags 语义、值树所有权、err 用法完全相同，
 * 成功时值树与 sml_loads 逐字节相同。交出的是「键冲突 W-PARSE-001 /
 * 片段覆盖 W-PARSE-002 / 弱声明不匹配 W-CONTRACT-001」这类**不阻断解析**的诊断。
 * 致命错误仍只走 err（sml_error 带 code_str 与行列）。
 * ==================================================================== */

typedef enum {
    SML_SEV_WARNING = 0,
    SML_SEV_ERROR   = 1   /* 预留：后续「多错误收集」出口用 */
} sml_severity;

typedef struct {
    char *code_str;   /* 稳定码，如 "W-PARSE-001"；NULL = 该档暂无码 */
    char *text;       /* 人读文案（**不含**码，与 sml_error.text 同口径） */
    int   line;       /* 1 起；0 = 未知（跨 include 时没有可映射的单一原文） */
    int   severity;   /* sml_severity */
} sml_diag;

typedef struct {
    sml_diag *items;  /* 空表时为 NULL */
    size_t    len;    /* 恒被写入：0 表示没有诊断 */
} sml_diag_list;

/* 解析文本并交出非致命诊断。out_diags / err 均可为 NULL。
 * 失败返回 NULL，*out_diags 被置为空表。 */
sml_value *sml_parse_with_diags(const char *text, unsigned flags,
                                sml_diag_list *out_diags, sml_error *err);

/* 释放 sml_parse_with_diags 交出的诊断表（NULL 安全、可重复调用）。
 * **不释放值树**；释放后 items/len 被清零。 */
void sml_diag_list_free(sml_diag_list *list);

/* ==================================================================== *
 * 内置契约表 + 扩展选项（v3.1）
 *
 * 弱绑定（`@weakbind`）的软校验要比对**宿主内置**的那份契约表；表里含
 * 不可序列化的校验器，故只能加法式构造（与纯 C 端 sml.h 同形状）。
 * 表是只读输入：注入后解析器不改它，也不往值树里写任何东西。
 * ==================================================================== */

typedef struct sml_builtin sml_builtin;

/* 新建空表（失败返回 NULL）。**空表 ⇒ 软校验整段跳过** —— 否则所有弱声明
 * 都会报 decl.unknown_contract。 */
sml_builtin *sml_builtin_new(void);

/* 释放（NULL 安全）。注入用的表在 sml_parse_full 返回后即可释放。 */
void sml_builtin_free(sml_builtin *b);

/* 表里有几份契约（NULL 返回 0）。 */
unsigned sml_builtin_count(const sml_builtin *b);

/* 复制一份内置契约表（NULL 返回 NULL）。
 * 用途：把 sml_result_contracts 交出的**借用**表变成宿主**独占**的句柄，于是
 * 「读完结果 → 释放结果 → 继续拿表当内置表注入」不用吊着结果句柄的命。
 * 克隆出来的表用 sml_builtin_free 释放（归调用方所有）。 */
sml_builtin *sml_builtin_clone(const sml_builtin *b);

/* 声明一个字段：契约不存在时自动建，同名字段**后写覆盖**。
 * ty 取 "any" / "str" / "int" / "num" / "bool" / "enum"（枚举值另用
 * sml_builtin_add_enum 续填）；required 非 0 表示必填。
 * 返回 0 = 成功；非 0 = 参数非法（NULL / 未知类型），此时表**不动**。 */
int sml_builtin_add_field(sml_builtin *b, const char *contract,
                          const char *field, const char *ty, int required);

/* 给已有字段设数值区间（含端点）。返回 0 = 成功；非 0 = 契约/字段不存在。 */
int sml_builtin_set_range(sml_builtin *b, const char *contract,
                          const char *field, double min, double max);

/* 给 enum 字段追加一个合法取值。返回 0 = 成功；非 0 = 不存在或字段不是 enum。 */
int sml_builtin_add_enum(sml_builtin *b, const char *contract,
                         const char *field, const char *value);

typedef struct {
    size_t struct_size;          /* 必填 sizeof(sml_opts)：将来加字段靠它判长度 */
    unsigned flags;              /* SML_F_* 的**特性上限**；0 = 基线。
                                  * 注意 `type.weak` 是 opt-in、不在基线里 ⇒
                                  * 要校验弱绑定请传 `SML_F_BASIC | SML_F_WEAK`
                                  * （或 sml_features_mask()），否则文档的
                                  * `@feature enable type.weak` 被交集剔除、
                                  * `@weakbind` 随即报 E-FEATURE-001。 */
    const char *const *env;      /* "KEY=VALUE" 数组，供 $env.X 内联；可 NULL */
    size_t env_len;              /* env 的元素个数 */
    const sml_builtin *builtins; /* 内置契约表；NULL = 不做弱绑定软校验 */
} sml_opts;

/* 解析文本，带完整选项（特性上限 / env 注入 / 内置契约表），并交出非致命诊断。
 * 值树与 sml_loads(text, flags) 逐字节相同；`@weakbind` 与表对不上时产出
 * W-CONTRACT-001（**不阻断**）。opts / out_diags / err 均可 NULL。
 * 失败返回 NULL，*out_diags 被置为空表。 */
sml_value *sml_parse_full(const char *text, const sml_opts *opts,
                          sml_diag_list *out_diags, sml_error *err);

/* ==================================================================== *
 * 弱声明三表（v3.1）
 *
 * `@weakbind` / `@weakdata` / `@weakdoc` 的产物**都不进主树**；这里按纯 C 端
 * 同形交出（path 形如 "srv.web" / "list[0]"，顶层为空串；line 1 起，0 = 未知）。
 * 整表由 sml_weaklists_free 回收 —— 其中 sml_weakdata.value **不要**单独 sml_free。
 * ==================================================================== */

typedef struct {
    char *path;
    char *name;
    int   line;
} sml_weakbind;

typedef struct {
    char *path;
    char *name;
    int   line;
    sml_value *value;   /* 载荷值树；随表释放 */
} sml_weakdata;

typedef struct {
    char *path;
    char *name;
    int   line;
} sml_weakdoc;

typedef struct {
    sml_weakbind *binds; size_t n_binds;
    sml_weakdata *data;  size_t n_data;
    sml_weakdoc  *docs;  size_t n_docs;
} sml_weaklists;

/* 释放三张弱表（NULL 安全、可重复调用）；不释放值树。 */
void sml_weaklists_free(sml_weaklists *lists);

/* 同 sml_parse_full，另交出弱声明三表（out_lists 可为 NULL）。 */
sml_value *sml_parse_full_lists(const char *text, const sml_opts *opts,
                                sml_weaklists *out_lists, sml_diag_list *out_diags,
                                sml_error *err);

/* 解析**文件**（展开 include），带完整选项 / 诊断 / 弱声明三表。
 * 跨 include 时行号可能为 0（token 跨文件合并，没有可映射的单一原文）。 */
sml_value *sml_load_file_full(const char *path, const sml_opts *opts,
                              sml_weaklists *out_lists, sml_diag_list *out_diags,
                              sml_error *err);

/* ==================================================================== *
 * 聚合结果（v3.1）
 *
 * 一次解析 -> 一个结果句柄 -> 用 getter 按需取。这样新增输出只加一个
 * SML_WANT_* 位与一个 getter，既有符号的签名不再随能力增长而改。
 * 所有 getter 返回的都是**借用**指针：随 sml_result_free 一并失效，
 * 不要对它们调 sml_free / sml_builtin_free / sml_*_list_free。
 * ==================================================================== */

#define SML_WANT_DIAGS        (1u << 0)  /* 非致命诊断 */
#define SML_WANT_ERRORS       (1u << 1)  /* 收集多条错误（语法错也返回句柄） */
#define SML_WANT_WEAK         (1u << 2)  /* 弱声明三表 */
#define SML_WANT_CONTRACT_MAP (1u << 3)  /* 契约映射：哪个实体应用了哪些契约 */
#define SML_WANT_CONTRACTS    (1u << 4)  /* 本次解析收集到的契约表（可再当内置表注入） */

typedef struct sml_result sml_result;

/* 一条「某实体应用了某个契约」。 */
typedef struct {
    char *path;   /* 实体路径（"srv.web" / "list[0]"）；顶层为空串 */
    char *name;   /* 契约名（已 resolve，与 @contract 定义的键一致） */
} sml_contract_ref;

/* 契约映射的扁平表示：每个 (实体, 契约) 一行，按应用顺序。 */
typedef struct {
    sml_contract_ref *items;
    size_t len;
} sml_contractmap;

/* 解析文本，交出聚合结果；失败（且未请求 SML_WANT_ERRORS）返回 NULL 并填 err。 */
sml_result *sml_parse_all(const char *text, const sml_opts *opts,
                          unsigned want, sml_error *err);

sml_value          *sml_result_value        (const sml_result *r); /* 借用，勿 sml_free */
const sml_diag_list *sml_result_diags       (const sml_result *r);
const sml_diag_list *sml_result_errors      (const sml_result *r); /* severity = 1 */
const sml_weaklists *sml_result_weak        (const sml_result *r);
const sml_contractmap *sml_result_contract_map(const sml_result *r);
const sml_builtin  *sml_result_contracts    (const sml_result *r); /* 借用，勿 sml_builtin_free */

/* 释放聚合结果（NULL 安全）：值树、各表、字符串、契约表一并回收。 */
void sml_result_free(sml_result *r);

/* ==================================================================== *
 * 遍历（返回借用指针，不可释放）
 * ==================================================================== */

int          sml_typeof  (const sml_value *v);
const sml_value *sml_get     (const sml_value *v, const char *key);
const sml_value *sml_get_path(const sml_value *v, const char *dotted);
const sml_value *sml_at      (const sml_value *v, size_t idx);
size_t       sml_size    (const sml_value *v);

/* 对象第 idx 个键（NUL 结尾的堆副本，调用方 sml_free_str 释放；非对象 / 越界返回 NULL）。
 * 内部键为 Rust String（非 NUL 结尾），无法安全「借用」为 C 串，故返回堆副本——语义同 sml_str_dup。
 * 顺序与 sml_get 一致（对象为 BTreeMap，键按字典序稳定排列）。
 * 这是 v2 宿主绑定（TASK-agent-w28 §2.3-A）为「不经 JSON 完整物化对象」补的极小键迭代符号。 */
char *sml_key_at(const sml_value *v, size_t idx);

/* ==================================================================== *
 * 标量取值
 *
 * 类型不符时返回零值（0 / 0.0 / NULL），故取值前建议先用 sml_typeof 判别，
 * 或用下面的 xxx_in 系列（通过 ok 参数回传是否真的取到）。
 * ==================================================================== */

/* 拷进调用方缓冲区，返回不含 NUL 的长度。
 * buf 为 NULL 或 buflen 为 0 时只返回所需长度，便于两次调用模式：
 *     size_t n = sml_str_copy(v, NULL, 0);
 *     char  *b = malloc(n + 1);
 *     sml_str_copy(v, b, n + 1);          */
size_t sml_str_copy(const sml_value *v, char *buf, size_t buflen);

/* 字符串副本，调用方 sml_free_str 释放；非字符串返回 NULL */
char      *sml_str_dup  (const sml_value *v);
long long  sml_int_value(const sml_value *v);
double     sml_real_value(const sml_value *v);
int        sml_bool_value(const sml_value *v);

/* ==================================================================== *
 * 单行便利取值（tomlc99 风格）
 * ==================================================================== */

/* 按 "a.b.c" 路径取字符串，调用方 sml_free_str 释放；取不到返回 NULL */
char     *sml_str_in (const sml_value *v, const char *path);
long long sml_int_in (const sml_value *v, const char *path, int *ok);
int       sml_bool_in(const sml_value *v, const char *path, int *ok);

/* ==================================================================== *
 * 序列化与释放
 * ==================================================================== */

/* 序列化为 SML 文本，调用方 sml_free_str 释放 */
char *sml_dumps(const sml_value *v, unsigned flags);

void sml_free    (sml_value *v);  /* 释放值树根节点（NULL 安全） */
void sml_free_str(char *p);       /* 释放返回的字符串（NULL 安全） */

/* ==================================================================== *
 * 元数据
 * ==================================================================== */

const char *sml_version(void);        /* 静态字符串，无需释放 */
unsigned    sml_features_mask(void);  /* 受支持特性的位掩码 */
const char *sml_feature_name(unsigned bit); /* 位 -> 名字；越界返回 NULL */

/* ==================================================================== *
 * 旧版 JSON 字符串 API
 *
 * 保留给「宿主已有 JSON 处理流程」的场景：解析直接产出 JSON 文本。
 * 若要从 C 侧遍历结果，请改用上面的值树 API —— 旧 API 会迫使你再引入
 * 一个 JSON 库，那时直接用该库即可，SML 便失去替代意义。
 * 这些函数返回的字符串一律用 sml_free_str 释放。
 * ==================================================================== */

char *sml_parse      (const char *text);                    /* -> JSON */
char *sml_parse_file (const char *path);                    /* -> JSON */
char *sml_parse_ex   (const char *text, const char *opts_json); /* -> JSON */
char *sml_dump       (const char *json);                    /* JSON -> SML */
/* 同上，但**失败时写 err**（W16 新增符号）。
 *
 * 为什么另开符号而不改 `sml_dump`：它的签名是**已发布的 C-ABI 契约**，加参数会
 * 破坏既有调用方 —— 与 v3 扩展的 `sml_parse_ex` 同一做法（新符号并存）。
 *
 * 失败情形与码：
 *   输入不是合法 JSON（或嵌套过深）⇒ E-PARSE-012（兜底：底层只给「是/否」）
 *   序列化深度超过上限        ⇒ E-LIMIT-004（此时**不再**吐出带占位文本的 SML）
 * err 可为 NULL（行为退化成与 sml_dump 相同，只是仍不写诊断）。 */
char *sml_dump_err   (const char *json, sml_error *err);
char *sml_features   (void);                                /* -> JSON 数组 */
char *sml_version_str(void);                                /* 版本串（副本） */

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SML_RS_H */
