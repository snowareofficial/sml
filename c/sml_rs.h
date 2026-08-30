/*
 * sml_rs.h — SML v3 桥接头文件 (Rust cdylib 后端)
 *
 * 与 sml.h (纯 C native 实现) 并存: 本文件不实现解析器, 而是桥接
 * rust crate `swsml` 编译出的 cdylib (sml.dll / libsml.so / libsml.dylib),
 * 从而获得完整的 v3 能力: $env 内联 / glob-include / @feature / @contract。
 *
 * 链接方式 (Windows / mingw):
 *   gcc example_rs.c -L<target>/release -lsml -o example_rs
 * 运行时需 sml.dll 在 PATH 或同目录。
 *
 * 所有返回的 char* 必须由调用方用 sml_free() 释放; 失败返回 NULL。
 */
#ifndef SML_RS_H
#define SML_RS_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>

/* 基础解析 (等价于 sml_parse, 但走 Rust 后端) */
char *sml_parse(const char *text);

/* 序列化: JSON -> SML */
char *sml_dump(const char *json);

/*
 * v3 扩展解析。
 * opts_json 可选, 支持字段:
 *   {"features":["glob-include","contract"],
 *    "env":{"APP_ENV":"prod"},
 *    "allow":["v1","v3"]}
 * - features: 调用方额外启用的特性 (与文档 @feature 取交集)
 * - env:      临时注入进程环境, 供 $env.X 内联 (调用期间设置并恢复)
 * - allow:    限定文档声明版本必须在此范围; 空数组/省略 = 不限制
 * opts 传 NULL 等价于 "{}"。
 */
char *sml_parse_ex(const char *text, const char *opts_json);

/* 从文件解析 (自动处理 include / glob / @contract 校验, 带文件上下文) */
char *sml_parse_file(const char *path);

/* 返回当前支持的特性名 JSON 数组, 如 ["include","env",...] */
char *sml_features(void);

/* 版本字符串, 如 "sml 0.4.0" */
char *sml_version(void);

/* 释放上述函数返回的字符串 (NULL 安全) */
void sml_free(char *p);

#ifdef __cplusplus
}
#endif

#endif /* SML_RS_H */
