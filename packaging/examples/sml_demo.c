/*
 * sml_demo.c — 消费 libswsml 的 C-ABI（Rust cdylib 后端）解析 SML。
 *
 * 安装 deb 后构建：
 *   gcc sml_demo.c $(pkg-config --cflags --libs swsml) -o sml_demo
 * 运行：
 *   ./sml_demo
 *
 * 演示：值树遍历、单行取值（tomlc99 风格）、值树->SML 往返、错误诊断。
 * 生命周期：sml_loads 结果用 sml_free 释放；char* 输出用 sml_free_str 释放；
 *           sml_get/sml_at/sml_get_path 返回**借用**指针，不可释放、不跨越根节点。
 */
#include <sml/sml_rs.h>
#include <stdio.h>
#include <stdlib.h>

static void indent(int n){ for (int i = 0; i < n; ++i) fputs("  ", stdout); }

/* 递归打印值树（演示遍历 API）*/
static void dump(const sml_value *v, int depth){
    if (!v){ indent(depth); puts("(null)"); return; }
    switch (sml_typeof(v)){
    case SML_TYPE_OBJECT:
        indent(depth); printf("object (%zu keys)\n", sml_size(v));
        for (size_t i = 0; i < sml_size(v); ++i){
            char *key = sml_key_at(v, i);                 /* 堆副本，用完 sml_free_str */
            const sml_value *child = sml_get(v, key);      /* 借用，勿释放 */
            indent(depth + 1); printf("%s:\n", key);
            dump(child, depth + 2);
            sml_free_str(key);
        }
        break;
    case SML_TYPE_ARRAY:
        indent(depth); printf("array (%zu items)\n", sml_size(v));
        for (size_t i = 0; i < sml_size(v); ++i) dump(sml_at(v, i), depth + 1);
        break;
    case SML_TYPE_STR: {
        size_t n = sml_str_copy(v, NULL, 0);               /* 两次调用：先问长度 */
        char *buf = (char*)malloc(n + 1);
        sml_str_copy(v, buf, n + 1);
        indent(depth); printf("string \"%s\"\n", buf);
        free(buf);
        break;
    }
    case SML_TYPE_INT:   indent(depth); printf("int %lld\n",  sml_int_value(v));  break;
    case SML_TYPE_FLOAT: indent(depth); printf("float %g\n",  sml_real_value(v)); break;
    case SML_TYPE_BOOL:  indent(depth); printf("bool %s\n",  sml_bool_value(v) ? "true" : "false"); break;
    case SML_TYPE_NULL:  indent(depth); puts("null"); break;
    default:             indent(depth); puts("(unknown)");
    }
}

int main(void){
    printf("libswsml %s\n\n", sml_version());

    const char *doc =
        "name: John\n"
        "age: 30\n"
        "active: true\n"
        "score: 9.5\n"
        "tags: [ a b c ]\n"
        "server {\n"
        "  host: web.example\n"
        "  port: 8080\n"
        "}\n";

    sml_error err;
    sml_value *root = sml_loads(doc, 0, &err);
    if (!root){
        fprintf(stderr, "parse failed: [%s] %s\n", err.code_str, err.text);
        return 1;
    }

    puts("=== 值树 ===");
    dump(root, 0);

    puts("\n=== 单行取值（tomlc99 风格）===");
    char     *host = sml_str_in(root, "server.host");            /* 堆副本，需释放 */
    long long port = sml_int_in(root, "server.port", NULL);
    printf("server.host = %s\n", host ? host : "(null)");
    printf("server.port = %lld\n", port);
    sml_free_str(host);

    puts("\n=== 往返：值树 -> SML ===");
    char *back = sml_dumps(root, 0);
    if (back){ fputs(back, stdout); sml_free_str(back); }

    sml_free(root);

    puts("\n=== 错误诊断（引用未定义契约）===");
    sml_error e2;
    sml_value *bad = sml_loads("@is NoSuchContract\nx: 1\n", 0, &e2);
    if (!bad) printf("errc=%d code_str=%s\n  %s\n", e2.code, e2.code_str, e2.text);
    else sml_free(bad);

    return 0;
}
