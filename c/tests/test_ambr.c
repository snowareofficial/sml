/*
** SPDX-License-Identifier: MulanPSL-2.0
** test_ambr.c — `.ambr` 解码器对拍容器向量（T7 / N03）。
**
** 判据：`tests/vectors/amberice-container.v1.json` 里的 22 个 `ambr_hex` 全部解码成功，
** 且若干形状（标量 / 数组 / 嵌套 / STRTAB 去重 / 元数据块）逐值正确。
** 这里只做最小 JSON 扫描（找 "id": " 与 "ambr_hex": " 两段），不引第三方。
**
** 运行：./test_ambr_check [fixture.json]
*/
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "ambr.h"

static int failures = 0;

static void ok(const char *what) {
    printf("  ok: %s\n", what);
}

static void fail(const char *what, const char *detail) {
    printf("FAIL: %s%s%s\n", what, detail ? " — " : "", detail ? detail : "");
    failures++;
}

static int hex_to_bytes(const char *hex, size_t n, unsigned char **out, size_t *out_len) {
    if (n % 2 != 0) return 0;
    unsigned char *b = (unsigned char *)malloc(n / 2 + 1);
    if (!b) return 0;
    for (size_t i = 0; i < n / 2; i++) {
        int hi = hex[i * 2], lo = hex[i * 2 + 1];
        int hv = (hi >= '0' && hi <= '9') ? hi - '0' : (hi >= 'a' && hi <= 'f') ? hi - 'a' + 10 : -1;
        int lv = (lo >= '0' && lo <= '9') ? lo - '0' : (lo >= 'a' && lo <= 'f') ? lo - 'a' + 10 : -1;
        if (hv < 0 || lv < 0) {
            free(b);
            return 0;
        }
        b[i] = (unsigned char)((hv << 4) | lv);
    }
    *out = b;
    *out_len = n / 2;
    return 1;
}

/* Copy the quoted string that starts right after `needle`. */
static int take_quoted(const char *p, const char *needle, char *dst, size_t dstsz) {
    const char *s = strstr(p, needle);
    if (!s) return 0;
    s += strlen(needle);
    const char *e = s;
    while (*e && *e != '"') e++;
    size_t n = (size_t)(e - s);
    if (n + 1 > dstsz) return 0;
    memcpy(dst, s, n);
    dst[n] = '\0';
    return 1;
}

static void check_case(const char *id, const unsigned char *bytes, size_t len) {
    char err[256];
    sml_value *v = sml_ambr_load(bytes, len, err, sizeof err);
    if (!v) {
        printf("FAIL: %s 解码失败 — %s\n", id, err);
        failures++;
        return;
    }

    if (!strcmp(id, "scalar-int")) {
        if (v->type == SML_INT && v->u.i == 1) ok(id);
        else fail(id, "期望整数 1");
    } else if (!strcmp(id, "flat-array")) {
        if (v->type == SML_ARRAY && sml_arr_len(v) == 3 && sml_arr_get(v, 0)->u.i == 1 &&
            sml_arr_get(v, 2)->u.i == 3) {
            ok(id);
        } else {
            fail(id, "期望 [1 2 3]");
        }
    } else if (!strcmp(id, "strtab-dedup")) {
        sml_value *k = sml_obj_get(v, "k");
        sml_value *other = sml_obj_get(v, "other");
        if (k && other && k->type == SML_STR && !strcmp(k->u.s, "k") &&
            !strcmp(other->u.s, "k")) {
            ok(id);
        } else {
            fail(id, "期望 k/other 都指向同一个串");
        }
    } else if (!strcmp(id, "nested-containers")) {
        sml_value *port = sml_get_path(v, "config.port");
        sml_value *tags = sml_obj_get(v, "tags");
        if (port && port->type == SML_INT && port->u.i == 8080 && tags &&
            tags->type == SML_ARRAY && sml_arr_len(tags) == 1 &&
            !strcmp(sml_arr_get(tags, 0)->u.s, "x")) {
            ok(id);
        } else {
            fail(id, "期望 config.port=8080 / tags=[x]");
        }
    } else if (!strcmp(id, "typed-block")) {
        /* 值树里没有 `User` 这一层：块名是渲染出来的，元数据是同级键。 */
        sml_value *name = sml_obj_get(v, "__name");
        sml_value *action = sml_obj_get(v, "action");
        if (name && name->type == SML_STR && !strcmp(name->u.s, "ssh") && action &&
            action->type == SML_STR && !strcmp(action->u.s, "allow")) {
            ok(id);
        } else {
            fail(id, "期望 User.__name=ssh / User.action=allow");
        }
    }
    /* Every case must at least decode; spot checks above are the extra belt. */
    sml_free(v);
}


/* hash_id 分派（§20）：读 `tests/vectors/amberice-hash-algos.v1.hex`
 * （SM3 / BLAKE3 各一份，由 Rust `dump_with` 生成），断言能解码且值正确；
 * 再改坏第一份的段摘要，断言必须拒绝 —— 否则「按 hash_id 换算法」是装饰。
 * ｜ Verify hash_id dispatch against Rust-generated SM3 / BLAKE3 fixtures, plus a
 * tampered-digest negative case. */
static void run_hash_fixtures(void) {
    const char *path = "../tests/vectors/amberice-hash-algos.v1.hex";
    FILE *f = fopen(path, "rb");
    if (!f) {
        fail("hash 夹具", path);
        return;
    }
    char line[4096];
    int cases = 0;
    while (fgets(line, sizeof line, f)) {
        if (line[0] == '#' || line[0] == '\n' || line[0] == '\r') continue;
        char id[32] = {0};
        char hex[4000] = {0};
        int hid = -1;
        if (sscanf(line, "%31s %d %3999s", id, &hid, hex) != 3) {
            fail("hash 夹具格式", line);
            continue;
        }
        unsigned char *bytes = NULL;
        size_t blen = 0;
        if (!hex_to_bytes(hex, strlen(hex), &bytes, &blen)) {
            fail(id, "hex 非法");
            continue;
        }
        char err[256];
        err[0] = '\0';
        sml_ambr_info info;
        info.hash_id = -1;
        info.digest_skipped = -1;
        sml_value *v = sml_ambr_load_ex(bytes, blen, &info, err, sizeof err);
        if (!v) {
            fail(id, err);
            free(bytes);
            continue;
        }
        sml_value *who = sml_obj_get(v, "who");
        sml_value *num = sml_obj_get(v, "n");
        if (who && who->type == SML_STR && strcmp(who->u.s, "u1") == 0 &&
            num && num->type == SML_INT && num->u.i == 7 && info.hash_id == hid &&
            info.digest_skipped == 0) {
            printf("  ok: hash_id 0x%02x 解码（%s），info=%d/%d\n", hid, id, info.hash_id, info.digest_skipped);
        } else {
            fail(id, "值不符");
        }
        sml_free(v);

        /* 反例：改坏段表首项摘要 ⇒ 必须拒绝。 */
        if (cases == 0 && blen > 49) {
            unsigned long long table_off = 0;
            for (int i = 7; i >= 0; i--) table_off = (table_off << 8) | bytes[8 + i];
            if ((size_t)table_off + 49 < blen) {
                bytes[(size_t)table_off + 9] ^= 0xff;
                char err2[256];
                err2[0] = '\0';
                sml_value *bad = sml_ambr_load(bytes, blen, err2, sizeof err2);
                if (bad) {
                    fail(id, "改坏摘要后仍解码成功");
                    sml_free(bad);
                } else {
                    printf("  ok: 改坏摘要 → 拒绝（%s）\n", id);
                }
            }
        }
        free(bytes);
        cases++;
    }
    fclose(f);
    if (cases != 2) {
        printf("FAIL: hash 夹具期望 2 例，实得 %d\n", cases);
        failures++;
    }
}

int main(int argc, char **argv) {
    const char *path = argc > 1 ? argv[1] : "../tests/vectors/amberice-container.v1.json";
    FILE *f = fopen(path, "rb");
    if (!f) {
        printf("[!] 打不开 %s（在 c/ 下运行，或把 fixture 路径作为第一个参数）\n", path);
        return 2;
    }
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    rewind(f);
    char *json = (char *)malloc((size_t)n + 1);
    if (!json) {
        fclose(f);
        return 2;
    }
    size_t got = fread(json, 1, (size_t)n, f);
    fclose(f);
    json[got] = '\0';

    printf("[ambr] 解码器对拍\n");
    int cases = 0;
    const char *p = json;
    for (;;) {
        char id[128];
        if (!take_quoted(p, "\"id\": \"", id, sizeof id)) break;
        const char *id_at = strstr(p, "\"id\": \"");
        char hex[8192];
        if (!take_quoted(id_at, "\"ambr_hex\": \"", hex, sizeof hex)) break;
        const char *hex_at = strstr(id_at, "\"ambr_hex\": \"");
        size_t hex_len = strlen(hex);
        (void)hex_len;

        unsigned char *bytes = NULL;
        size_t blen = 0;
        if (!hex_to_bytes(hex, strlen(hex), &bytes, &blen)) {
            printf("FAIL: %s 的 hex 非法\n", id);
            failures++;
        } else {
            check_case(id, bytes, blen);
            free(bytes);
        }
        cases++;
        p = hex_at + strlen("\"ambr_hex\": \"") + strlen(hex);
    }
    /* Negative control: corrupt one digest byte of the first segment (table at 40, digest
    ** starts at 49) and expect a refusal. Otherwise "verify the digest" is decorative.
    ** 反向用例：改坏一个摘要字节必须拒绝。 */
    {
        char hex[8192];
        if (take_quoted(json, "\"ambr_hex\": \"", hex, sizeof hex)) {
            unsigned char *b = NULL;
            size_t n = 0;
            if (hex_to_bytes(hex, strlen(hex), &b, &n) && n > 49) {
                char err[128];
                b[49] ^= 0xff;
                sml_value *v = sml_ambr_load(b, n, err, sizeof err);
                if (v) {
                    printf("FAIL: 改坏摘要后仍解码成功\n");
                    failures++;
                    sml_free(v);
                } else {
                    printf("  ok: 改坏摘要 → 拒绝\n");
                }
                free(b);
            }
        }
    }

    free(json);

    run_hash_fixtures();

    printf("[ambr] %d 例，%d 个失败\n", cases, failures);
    if (cases != 22) {
        printf("FAIL: 期望 22 例，实得 %d\n", cases);
        failures++;
    }
    printf("%s\n", failures ? "[ambr] FAILED" : "[ambr] ALL OK");
    return failures ? 1 : 0;
}
