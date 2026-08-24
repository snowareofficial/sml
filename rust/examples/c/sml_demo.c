/*
** sml_demo.c — SML C 绑定用法示例
**
** 编译 (Windows, MSYS2/MinGW, 与 libsml.dll 同目录):
**   gcc sml_demo.c -I../include -L. -lsml -o sml_demo.exe
**   sml_demo.exe
**
** 编译 (Linux, 与 libsml.so 同目录):
**   gcc sml_demo.c -I../include -L. -lsml -o sml_demo
**   LD_LIBRARY_PATH=. ./sml_demo
*/
#include <stdio.h>
#include <stdlib.h>
#include "sml.h"

int main(void) {
    char *ver = sml_version();
    printf("sml version: %s\n", ver);
    sml_free(ver);

    const char *text =
        "name: John\n"
        "age: 27\n"
        "address: { city: NY, zip: 10001 }\n"
        "tags: [ dev tools ]\n";

    char *json = sml_parse(text);
    if (!json) {
        fprintf(stderr, "sml_parse failed\n");
        return 1;
    }
    printf("parsed -> JSON:\n%s\n", json);

    /* 再用 sml_dump 把 JSON 转回 SML (round-trip) */
    char *sml = sml_dump(json);
    if (sml) {
        printf("dumped -> SML:\n%s\n", sml);
        sml_free(sml);
    }
    sml_free(json);

    /* 失败路径: 非法 SML */
    char *bad = sml_parse("{ unclosed");
    if (!bad) {
        printf("(sml_parse 正确拒绝了非法输入)\n");
    } else {
        sml_free(bad);
    }
    return 0;
}
