/* W4：把 .sml 解析后用 **C 侧** `sml_dump` 输出，供与 Rust `to_sml` 逐字节比对。
 *
 * 用法（仓库根）：
 *   gcc -std=c99 -Ic -o %TEMP%/dump_c.exe tools/dump_c.c c/sml.c
 *   dump_c.exe <file.sml> ...
 *
 * 输出协议（比对脚本按这个解析）：
 *   "CDUMP <路径>\n" + dump 原文      解析成功
 *   "CERR  <路径> <错误信息>\n"        解析失败
 *
 * 注：C 侧只有 `sml_parse_file`（没有公开 dump CLI），所以这个小程序是比对必需的那一半。
 */
#include <stdio.h>
#include "sml.h"

int main(int argc, char **argv) {
    for (int i = 1; i < argc; i++) {
        char err[512];
        err[0] = '\0';
        sml_value *v = sml_parse_file(argv[i], err, sizeof(err));
        if (!v) {
            printf("CERR %s %s\n", argv[i], err);
            continue;
        }
        char *s = sml_dump(v);
        printf("CDUMP %s\n", argv[i]);
        fputs(s, stdout);
        sml_free_str(s);
        sml_free(v);
    }
    return 0;
}
