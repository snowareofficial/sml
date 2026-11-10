/* SPDX-License-Identifier: MulanPSL-2.0 */
/* C 端 SCHEMA/TYPED 动态渲染（D45 / §25.8）对拍。
** 编译：gcc -std=c99 -Wall -Wextra -I c -o test_typed c/sml.c c/ambr.c c/tests/test_ambr_typed.c
** 运行：./test_typed
*/
#include "ambr.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "typed_fixture.h"

static int hex2bin(const char *hex, unsigned char **out, size_t *n) {
    size_t len = strlen(hex);
    if (len % 2 != 0) return 0;
    unsigned char *b = (unsigned char *)malloc(len / 2);
    if (!b) return 0;
    for (size_t i = 0; i < len / 2; i++) {
        unsigned hi, lo;
        char a = hex[i * 2], c = hex[i * 2 + 1];
        if (a >= '0' && a <= '9') hi = (unsigned)(a - '0');
        else if (a >= 'a' && a <= 'f') hi = (unsigned)(a - 'a' + 10);
        else return 0;
        if (c >= '0' && c <= '9') lo = (unsigned)(c - '0');
        else if (c >= 'a' && c <= 'f') lo = (unsigned)(c - 'a' + 10);
        else return 0;
        b[i] = (unsigned char)((hi << 4) | lo);
    }
    *out = b;
    *n = len / 2;
    return 1;
}

static int check(int cond, const char *what) {
    if (!cond) printf("FAIL  %s\n", what);
    return cond ? 0 : 1;
}

int main(void) {
    unsigned char *bytes = NULL;
    size_t n = 0;
    if (!hex2bin(TYPED_FIXTURE_HEX, &bytes, &n)) {
        printf("FAIL  fixture hex\n");
        return 1;
    }
    char err[256] = {0};
    sml_value *vars = sml_ambr_load_typed(bytes, n, err, sizeof err);
    free(bytes);
    if (!vars) {
        printf("FAIL  load_typed: %s\n", err);
        return 1;
    }

    int bad = 0;
    bad += check(vars->type == SML_ARRAY && sml_arr_len(vars) == 1, "one variable");
    sml_value *item = sml_arr_get(vars, 0);
    sml_value *id = item ? sml_obj_get(item, "id") : NULL;
    sml_value *name = item ? sml_obj_get(item, "name") : NULL;
    sml_value *val = item ? sml_obj_get(item, "value") : NULL;
    bad += check(id && id->type == SML_INT && id->u.i == 1, "var id = 1");
    bad += check(name && name->type == SML_STR && strcmp(name->u.s, "User") == 0, "type name = User");
    bad += check(val && val->type == SML_OBJECT, "value is object");

    sml_value *uid = val ? sml_obj_get(val, "id") : NULL;
    sml_value *uname = val ? sml_obj_get(val, "name") : NULL;
    sml_value *tags = val ? sml_obj_get(val, "tags") : NULL;
    sml_value *note = val ? sml_obj_get(val, "note") : NULL;
    bad += check(uid && uid->type == SML_INT && uid->u.i == 42, "field id = 42");
    bad += check(uname && uname->type == SML_STR && strcmp(uname->u.s, "ada") == 0, "field name = ada");
    bad += check(tags && tags->type == SML_ARRAY && sml_arr_len(tags) == 2, "tags has 2 items");
    if (tags && tags->type == SML_ARRAY && sml_arr_len(tags) == 2) {
        sml_value *a = sml_arr_get(tags, 0), *b = sml_arr_get(tags, 1);
        bad += check(a && a->type == SML_STR && strcmp(a->u.s, "a") == 0, "tags[0] = a");
        bad += check(b && b->type == SML_STR && strcmp(b->u.s, "b") == 0, "tags[1] = b");
    }
    bad += check(note && note->type == SML_NULL, "note = null");

    sml_free(vars);
    if (bad) return 1;
    printf("ok  1/1 typed render\n");
    return 0;
}
