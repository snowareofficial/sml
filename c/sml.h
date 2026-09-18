/*
** SPDX-License-Identifier: MulanPSL-2.0
** sml.h — SML (SNOWARE Markup Language) 纯 C 实现
**
** 声明式数据/配置格式 (JSON/YAML 替代品)。本文件为自包含 C99 实现，
** 零外部依赖，可静态链接或并入宿主工程。语法与 Lua/Rust/JS 实现对齐：
**   裸词字符串 / 引号串（转义 + $env 内联）/ true/false/null / 数字 /
**   块 key { } / 裸块 type name { } / 数组 [ ] / 逗号可选 / # 注释 /
**   @name { } 片段定义 & 引用。
**
** 编译:  gcc sml.c example.c -o demo    (或把 sml.c 直接并入工程)
**
** 值模型: sml_value (type 判别 + 联合)。
** 解析:   sml_parse(text) -> sml_value* (失败返回 NULL, err 填充错误信息)
** 序列化: sml_dump(v)    -> char* (调用方 sml_free_str 释放)
** 释放:   sml_free(v)    /  sml_free_str(s)
**
** 两个后端，二选一链接（不可同时 include）:
**   sml.h     — 本文件，纯 C99 自包含实现。零依赖，功能为基础集。
**   sml_rs.h  — 桥接 Rust crate `swsml` 的 cdylib，提供 v3 完整能力
**               ($env 内联 / glob-include / @feature / @contract)。
**   两者的 sml_free / sml_free_str 语义一致，但值模型不同，不要混用。
*/

#ifndef SML_H
#define SML_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    SML_NULL = 0,
    SML_BOOL,
    SML_INT,
    SML_FLOAT,
    SML_STR,
    SML_ARRAY,
    SML_OBJECT,   /* 块/对象; __type/__name 元数据以保留键存放 */
} sml_type;

typedef struct sml_value sml_value;

/* 字段 (对象条目) */
typedef struct sml_field {
    char *key;          /* 键名 (含 __type/__name 元数据键) */
    sml_value *value;
    struct sml_field *next;
} sml_field;

struct sml_value {
    sml_type type;
    union {
        int boolean;
        long long i;          /* SML_INT */
        double f;             /* SML_FLOAT */
        char *s;              /* SML_STR */
        struct {              /* SML_ARRAY */
            sml_value **items;
            size_t len, cap;
        } arr;
        struct {              /* SML_OBJECT */
            sml_field *head, *tail;
            size_t len;
        } obj;
    } u;
};

/* ---- 构造 / 释放 ---- */
sml_value *sml_new_null(void);
sml_value *sml_new_bool(int b);
sml_value *sml_new_int(long long i);
sml_value *sml_new_float(double f);
sml_value *sml_new_str(const char *s);
sml_value *sml_new_strn(const char *s, size_t n);
sml_value *sml_new_array(void);
sml_value *sml_new_object(void);
void sml_free(sml_value *v);

/* ---- 容器操作 ---- */
/* 对象: 设置/取字段 (取不到返回 NULL; 若字段已存在则替换) */
void sml_obj_set(sml_value *obj, const char *key, sml_value *val);
sml_value *sml_obj_get(const sml_value *obj, const char *key);
/* 对象: 支持 "a.b.c" 点路径 */
sml_value *sml_get_path(const sml_value *v, const char *path);
/* 数组: 追加 / 取元素 */
void sml_arr_push(sml_value *arr, sml_value *val);
sml_value *sml_arr_get(const sml_value *arr, size_t i);
size_t sml_arr_len(const sml_value *arr);

/* ---- 解析 ---- */
/* 错误码约定 (W10):
**   失败时写入 err 的消息**以错误码开头**, 码与文案之间用一个空格分隔, 形如
**       "E-LIMIT-001 嵌套过深（超过 128 层），疑似递归或恶意输入"
**   码内不含空格, 故调用方取「第一个空格之前」的部分即为码:
**       char code[16];
**       sscanf(err, "%15s", code);   // -> "E-LIMIT-001"
**   码是稳定契约、文案不是: 码相同即同一件事, 各端措辞可以不同。
**   码表 (唯一事实来源) 见 errors/codes.sml; C 侧宏见 sml_codes.h, 用宏而非手打字符串。
**   err 允许为 NULL 或 errsz 为 0, 此时一个字节都不写 (失败与否由返回值 NULL 表达);
**   成功时 err[0] 会置 '\0' (若非 NULL)。
**   没有例外: 连内存分配失败也带码 (E-LIMIT-010) —— C 要自己管内存, malloc 失败是可报条件。 */
/* 解析 SML 文本。成功返回新值; 失败返回 NULL 并把错误写入 err(若非 NULL, 至少 256 字节) */
sml_value *sml_parse(const char *text, char *err, size_t errsz);
/* 解析 SML 文件: 展开 include / @include 文本内联后解析。
** 相对路径以文件所在目录为基准。失败返回 NULL 并写 err (同上, 带码)。 */
sml_value *sml_parse_file(const char *path, char *err, size_t errsz);

/* ---- 序列化 ---- */
/* 序列化为 SML 文本 (round-trip)。返回调用方 sml_free_str 释放的字符串 */
char *sml_dump(const sml_value *v);
void sml_free_str(char *s);

/* ---- JSON 互转 (纯 C 实现, 自带极简 JSON 解析/序列化, 零外部依赖) ---- */
/* 解析 SML 文本并序列化为 JSON 字符串 (调用方 sml_free_cstr 释放); 失败返回 NULL。
** 适合宿主已有 JSON 处理流程的场景; 若要直接遍历值树,
** 用 sml_parse + sml_obj_get / sml_get_path 更省一次序列化开销。 */
char *sml_parse_json(const char *text);
/* 接受 JSON 字符串, 输出 SML 文本 (调用方 sml_free_cstr 释放) */
char *sml_dump_from_json(const char *json);
void sml_free_cstr(char *p);

/* 版本 */
const char *sml_version(void);

#ifdef __cplusplus
}
#endif

#endif /* SML_H */
