/*
 * sml_rs.h — SML v3 桥接头文件 (Rust cdylib 后端)
 *
 * 本文件不实现解析器，而是桥接 Rust crate `swsml` 编译出的 cdylib
 * (sml.dll / libsml.so / libsml.dylib)，从而获得 v3 完整能力：
 * $env 内联 / glob-include / @feature / @contract。
 *
 * 与 sml.h 的关系 —— 两个后端，二选一链接，不可同时 include：
 *   sml.h    纯 C99 自包含实现，零依赖，功能为基础集。
 *   sml_rs.h 本文件，桥接 Rust cdylib，功能为 v3 全集。
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
    char   text[256];       /* 错误信息 */
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
 * 遍历（返回借用指针，不可释放）
 * ==================================================================== */

int          sml_typeof  (const sml_value *v);
const sml_value *sml_get     (const sml_value *v, const char *key);
const sml_value *sml_get_path(const sml_value *v, const char *dotted);
const sml_value *sml_at      (const sml_value *v, size_t idx);
size_t       sml_size    (const sml_value *v);

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
char *sml_features   (void);                                /* -> JSON 数组 */
char *sml_version_str(void);                                /* 版本串（副本） */

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SML_RS_H */
