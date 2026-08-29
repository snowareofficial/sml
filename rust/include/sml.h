/*
** SPDX-License-Identifier: MulanPSL-2.0
** sml.h — SML (SNOWARE Markup Language) C 绑定
**
** SML 是声明式数据/配置格式 (JSON/YAML 的替代品)。本头文件声明
** Rust cdylib (sml-rs, 产物 libsml.dll / libsml.so / libsml.dylib)
** 导出的 C-ABI。链接方式:
**   - 直接链接: -lsml (Windows) / -lsml (Unix, 需 -L)
**   - 动态加载: LoadLibrary/dlopen 按需解析下列符号
**
** 值经 JSON 文本桥接 (sml_parse 输出 JSON; sml_dump 接受 JSON)。
** 字符串返回值由 sml_free 释放 (Rust 分配器, 勿用 libc free)。
*/

#ifndef SML_H
#define SML_H

#ifdef __cplusplus
extern "C" {
#endif

#if defined(_WIN32) && defined(SML_SHARED)
#  ifdef SML_BUILD
#    define SML_API __declspec(dllexport)
#  else
#    define SML_API __declspec(dllimport)
#  endif
#else
#  define SML_API
#endif

/* 解析 SML 文本 -> JSON 字符串 (调用方 sml_free 释放); 失败返回 NULL */
SML_API char *sml_parse(const char *text);

/* 接受 JSON 字符串, 序列化为 SML 文本 (调用方 sml_free 释放); 失败返回 NULL */
SML_API char *sml_dump(const char *json);

/* 释放由 sml_parse / sml_dump 返回的字符串 */
SML_API void  sml_free(char *p);

/* 版本字符串 (调用方 sml_free) */
SML_API char *sml_version(void);

#ifdef __cplusplus
}
#endif

#endif /* SML_H */
