/* example.c — sml 纯 C 库用法示例
 *
 * 编译:  gcc sml.c example.c -o sml_demo
 * 运行:  ./sml_demo
 */
#include <stdio.h>
#include "sml.h"

int main(void) {
    printf("== %s ==\n", sml_version());

    const char *text =
        "name: John\n"
        "age: 27\n"
        "address: { city: NY, zip: 10001 }\n"
        "tags: [ dev tools ]\n"
        "active: true\n";

    char err[256] = {0};
    sml_value *v = sml_parse(text, err, sizeof(err));
    if (!v) {
        fprintf(stderr, "parse failed: %s\n", err);
        return 1;
    }

    /* 字段访问 */
    sml_value *name = sml_obj_get(v, "name");
    sml_value *age = sml_obj_get(v, "age");
    printf("name=%s age=%lld\n",
           name && name->type == SML_STR ? name->u.s : "?",
           age && age->type == SML_INT ? age->u.i : -1);

    /* 点路径 */
    sml_value *city = sml_get_path(v, "address.city");
    printf("address.city=%s\n", city && city->type == SML_STR ? city->u.s : "?");

    /* 序列化 round-trip */
    char *out = sml_dump(v);
    printf("--- dumped ---\n%s\n", out);
    sml_free_str(out);

    /* JSON 桥 */
    char *json = sml_parse_json(text);
    printf("--- as JSON ---\n%s\n", json ? json : "(null)");
    sml_free_cstr(json);

    sml_free(v);
    return 0;
}
