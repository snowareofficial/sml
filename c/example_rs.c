/*
 * example_rs.c — SML v3 via the Rust cdylib backend (sml_rs.h value-tree API).
 *
 * Build (Windows / mingw, with sml.dll on PATH or next to the exe):
 *   gcc -std=c99 example_rs.c -L<E:/snoware-target>/release -lsml -o example_rs
 *   (import library: libsml.dll.a; with MSVC link against sml.lib instead)
 *
 * This example walks through the lifecycle rules that matter most:
 *   1. root pointer from sml_loads / sml_load_file  -> sml_free
 *   2. sml_get / sml_get_path / sml_at -> borrowed, do NOT free
 *   3. any char* output                            -> sml_free_str
 */
#include "sml_rs.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <direct.h>
#define MKDIR(p) _mkdir(p)
#else
#include <sys/stat.h>
#define MKDIR(p) mkdir((p), 0755)
#endif

static int failures = 0;

#define CHECK(cond, msg)                                                       \
    do {                                                                       \
        if (!(cond)) {                                                         \
            printf("  [FAIL] %s (line %d)\n", msg, __LINE__);                  \
            failures++;                                                        \
        } else {                                                               \
            printf("  [ ok ] %s\n", msg);                                      \
        }                                                                      \
    } while (0)

/* ------------------------------------------------------------------ *
 * 1. Load a document and walk the value tree directly (no JSON lib).
 * ------------------------------------------------------------------ */
static void demo_traverse(void) {
    printf("=== 1. traverse the value tree ===\n");

    const char *doc = "name: John\n"
                      "age: 27\n"
                      "active: true\n"
                      "server {\n"
                      "  host: web.example\n"
                      "  port: 8080\n"
                      "}\n"
                      "tags: [ a b c ]\n";

    sml_error err;
    sml_value *root = sml_loads(doc, 0, &err);
    if (!root) {
        printf("  load failed: [%d] %s\n", err.code, err.text);
        failures++;
        return;
    }

    CHECK(sml_typeof(root) == SML_TYPE_OBJECT, "root is an object");
    CHECK(sml_size(root) == 5, "root has 5 fields");

    /* Scalars */
    const sml_value *name = sml_get(root, "name");
    CHECK(name && sml_typeof(name) == SML_TYPE_STR, "name is a string");
    char *name_copy = sml_str_dup(name);
    CHECK(name_copy && strcmp(name_copy, "John") == 0, "name == \"John\"");
    sml_free_str(name_copy);

    CHECK(sml_int_value(sml_get(root, "age")) == 27, "age == 27");
    CHECK(sml_bool_value(sml_get(root, "active")) == 1, "active == true");

    /* Dotted path — one call instead of nesting sml_get */
    char *host = sml_str_in(root, "server.host");
    CHECK(host && strcmp(host, "web.example") == 0, "server.host read via path");
    sml_free_str(host);

    int ok = 0;
    CHECK(sml_int_in(root, "server.port", &ok) == 8080 && ok,
          "server.port read via path");

    /* Missing path: value is 0 AND ok == 0 */
    ok = 1;
    CHECK(sml_int_in(root, "server.nope", &ok) == 0 && ok == 0,
          "missing path reports ok == 0");

    /* Arrays */
    const sml_value *tags = sml_get(root, "tags");
    CHECK(tags && sml_typeof(tags) == SML_TYPE_ARRAY, "tags is an array");
    CHECK(sml_size(tags) == 3, "tags has 3 elements");
    CHECK(sml_at(tags, 3) == NULL, "out-of-range index returns NULL");

    /* Serialize back to SML */
    char *dumped = sml_dumps(root, 0);
    CHECK(dumped != NULL, "sml_dumps produced output");
    sml_free_str(dumped);

    sml_free(root);
    printf("\n");
}

/* ------------------------------------------------------------------ *
 * 2. Precise error reporting: line / column / source / message.
 * ------------------------------------------------------------------ */
static void demo_error(void) {
    printf("=== 2. error reporting ===\n");

    /* Referencing an undefined contract is a guaranteed failure.
     * (The parser is deliberately tolerant of unclosed quotes/blocks,
     *  so those are NOT good error test cases.) */
    const char *bad = "@is NoSuchContract\nx: 1\n";
    sml_error err;
    sml_value *root = sml_loads(bad, 0, &err);

    CHECK(root == NULL, "undefined contract -> NULL");
    CHECK(err.code == SML_ERR_CONTRACT, "err.code == SML_ERR_CONTRACT");
    CHECK(strlen(err.text) > 0, "err.text is filled");
    CHECK(strcmp(err.source, "<string>") == 0, "err.source == \"<string>\"");
    printf("       code=%d source=%s text=%s\n", err.code, err.source, err.text);

    /* NULL input must not crash */
    root = sml_loads(NULL, 0, &err);
    CHECK(root == NULL, "NULL text is handled safely");
    printf("\n");
}

/* ------------------------------------------------------------------ *
 * 3. String copy: the two-call pattern avoids guessing a buffer size.
 * ------------------------------------------------------------------ */
static void demo_str_copy(void) {
    printf("=== 3. str_copy without a fixed buffer ===\n");

    /* Quote the value: an unquoted bare word stops at the first space,
     * so `greeting: hello world` would parse as just "hello". */
    sml_error err;
    sml_value *root = sml_loads("greeting: \"hello world\"\n", 0, &err);
    if (!root) {
        printf("  load failed: %s\n", err.text);
        failures++;
        return;
    }

    const sml_value *g = sml_get(root, "greeting");

    /* First call with NULL just returns the required length. */
    size_t need = sml_str_copy(g, NULL, 0);
    char *buf = (char *)malloc(need + 1);
    size_t wrote = sml_str_copy(g, buf, need + 1);

    CHECK(wrote == need, "second call reports the same length");
    CHECK(strcmp(buf, "hello world") == 0, "copied string matches");
    printf("       need=%zu buf=\"%s\"\n", need, buf);

    free(buf);
    sml_free(root);
    printf("\n");
}

/* ------------------------------------------------------------------ *
 * 4. Feature flags: tighten what a document is allowed to use.
 * ------------------------------------------------------------------ */
static void demo_flags(void) {
    printf("=== 4. feature flags ===\n");

    printf("  supported feature mask: 0x%x\n", sml_features_mask());
    for (unsigned bit = 0; bit < 32; ++bit) {
        const char *n = sml_feature_name(bit);
        if (n) {
            printf("    bit %2u -> %s\n", bit, n);
        }
    }

    /* A document using $env should fail when SML_F_ENV is not granted. */
    const char *doc = "secret: $env.MY_TOKEN\n";

    sml_error err;
    sml_value *root = sml_loads(doc, SML_F_BASIC, &err); /* env NOT enabled */
    CHECK(root == NULL, "env denied when SML_F_ENV is absent");
    if (root) {
        sml_free(root);
    }

    /* ... and succeed once it is granted (assuming baseline allows it). */
    root = sml_loads(doc, SML_F_BASIC | SML_F_ENV, &err);
    printf("  with SML_F_ENV: %s\n", root ? "loaded" : "still failed");
    if (root) {
        sml_free(root);
    }
    printf("\n");
}

/* ------------------------------------------------------------------ *
 * 5. File loading expands `include` (paths must be quoted).
 * ------------------------------------------------------------------ */
static void demo_include(void) {
    printf("=== 5. include expansion ===\n");

    /* Create: <tmp>/sml_rs_example/{main.sml, conf.d/extra.sml} */
    const char *tmp = getenv("TMP");
    if (!tmp) {
        tmp = getenv("TEMP");
    }
    if (!tmp) {
        tmp = ".";
    }

    char dir[512];
    snprintf(dir, sizeof(dir), "%s/sml_rs_example", tmp);
    char subdir[600];
    snprintf(subdir, sizeof(subdir), "%s/conf.d", dir);

    MKDIR(dir);
    MKDIR(subdir);

    char extra[640];
    snprintf(extra, sizeof(extra), "%s/extra.sml", subdir);
    FILE *f = fopen(extra, "wb");
    if (f) {
        fputs("from_name: ops\nmonth_count: 12\n", f);
        fclose(f);
    }

    char main_path[600];
    snprintf(main_path, sizeof(main_path), "%s/main.sml", dir);
    f = fopen(main_path, "wb");
    if (f) {
        fputs("include \"conf.d/extra.sml\"\nport: 8080\n", f);
        fclose(f);
    }

    sml_error err;
    sml_value *root = sml_load_file(main_path, 0, &err);
    if (!root) {
        printf("  [FAIL] load_file: [%d] %s\n", err.code, err.text);
        failures++;
        return;
    }

    char *who = sml_str_in(root, "from_name");
    CHECK(who && strcmp(who, "ops") == 0, "included field merged");
    sml_free_str(who);

    int ok = 0;
    CHECK(sml_int_in(root, "month_count", &ok) == 12 && ok,
          "included field merged (month_count)");
    CHECK(sml_int_in(root, "port", &ok) == 8080, "own field still present");

    sml_free(root);
    printf("\n");
}

int main(void) {
    printf("sml version: %s\n\n", sml_version()); /* static, do NOT free */

    demo_traverse();
    demo_error();
    demo_str_copy();
    demo_flags();
    demo_include();

    if (failures == 0) {
        printf("=== ALL CHECKS PASSED ===\n");
        return 0;
    }
    printf("=== %d CHECK(S) FAILED ===\n", failures);
    return 1;
}
