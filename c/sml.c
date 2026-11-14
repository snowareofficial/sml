/*
** SPDX-License-Identifier: MulanPSL-2.0
** sml.c — SML (SNOWARE Markup Language) 纯 C 实现
**
** 零依赖 C99 单文件实现。API 见 sml.h。
**
** 实现结构:
**   1. 值构造/释放/容器操作
**   2. 词法 (token 流)  — 含 \u{XXXX}/\uXXXX 字符串转义
**   3. 递归下降解析 (块/数组/标量/片段) + 契约系统(@contract/@is)
**   4. 序列化 (round-trip)
**   5. JSON 桥 (C-ABI 兼容)
**   6. include / @include 文本内联 (sml_parse_file)
**
** Comments: line comments use '#' '--' or '//'; block comments use slash-star
**   and star-slash, or underscore-star and star-underscore (aligned with Rust/JS/Lua).
** Contracts: '@contract Name [loose] { ... }' defines a contract; '@is Name'
**   applies it (block-level or field-level). Aligned with Rust sml-rs: strict mode,
**   defaults, required, enum, min/max bounds, and nested ContractRef composition.
*/

/* `strdup` / `strtok_r` / `realpath` 是 POSIX / XSI，不是 ISO C99：严格 `-std=c99`
   下 glibc 不声明，编译器会把它们当成返回 int，指针直接被截断。
   - `_XOPEN_SOURCE 700` 覆盖 realpath（XSI），并隐含 POSIX.1-2008；
   - `_POSIX_C_SOURCE 200809L` 明确覆盖 strdup / strtok_r 与 realpath(path, NULL)。
   feature test macro 必须在任何系统头文件之前定义；MSVC 走自己的别名，不需要。*/
#if !defined(_WIN32)
#ifndef _XOPEN_SOURCE
#define _XOPEN_SOURCE 700
#endif
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#endif

#include "sml.h"
#include "sml_codes.h"   /* 错误码宏；唯一事实来源 errors/codes.sml（见 errors/README.md）*/

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include <math.h>
#include <stdarg.h>
#include <errno.h>   /* strtoll 溢出判定（ERANGE）：见 coerce_word 的 B10 分支 */

/* 以下 9 个模块按解析 / 序列化顺序 include —— unity 构建：仍是**同一个翻译单元**，
   预处理输出与拆分前逐行相同（仅模块边界的空行有增减，已用 `gcc -E` 对比确认）。
   拆分依据与「改成独立 TU」时的注意事项见 c/src/README.md。 */
#include "src/00_errors.inc"
#include "src/01_value.inc"
#include "src/02_lex.inc"
#include "src/03_contract.inc"
#include "src/04_coerce.inc"
#include "src/05_parse.inc"
#include "src/06_dump.inc"
#include "src/07_json.inc"
#include "src/08_include.inc"
