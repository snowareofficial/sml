// SPDX-License-Identifier: MulanPSL-2.0
// AmbrWriteTest.java — optional `.ambr` writer against the container vectors.
// AmbrWriteTest.java —— Java 端可选写入器对拍容器向量。
//
// Judgement / 判据:
//   1. `AmbrWrite.packText(canonical_text)` succeeds for all 21 vectors;
//   2. `Ambr.load` of the packed bytes succeeds for all 21;
//   3. the five shapes already asserted by `AmbrTest` keep their values.
//   Byte-for-byte canonical parity is asserted by the ends that own a canonical emitter
//   (Rust / JS). ｜ 逐字节对拍由有 canonical 发射器的端负责；这里验语义。
//
// Run / 运行:
//   javac -d out src/main/java/sml/*.java src/test/java/sml/*.java
//   AMBR_LIB=... java --enable-native-access=ALL-UNNAMED -cp out sml.AmbrWriteTest
package sml;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

public final class AmbrWriteTest {
    private static int failures = 0;

    public static void main(String[] args) throws Exception {
        final int caps = AmbrWrite.capabilities();
        System.out.println("[ambr-write] capabilities=" + caps
                + " (write=" + ((caps & AmbrWrite.CAP_WRITE) != 0) + ")");
        if ((caps & AmbrWrite.CAP_WRITE) == 0) {
            System.out.println("[ambr-write] SKIP: read-only cdylib / 只读构建");
            return;
        }

        final String path = resolveFixture(args);
        final String json = Files.readString(Path.of(path), StandardCharsets.UTF_8);

        int cases = 0;
        int packed = 0;
        int pos = 0;
        final int[] next = new int[1];

        while (true) {
            final String id = takeJsonString(json, pos, "\"id\": \"", next);
            if (id == null) break;
            pos = next[0];

            final String raw = takeJsonString(json, pos, "\"canonical_text\": \"", next);
            if (raw == null) break;
            pos = next[0];

            final String text = unescape(raw);
            cases++;
            try {
                final byte[] bytes = AmbrWrite.packText(text);
                packed++;
                checkShape(id, Ambr.load(bytes));
            } catch (RuntimeException e) {
                fail(id, e.getMessage());
            }
        }

        System.out.printf("[ambr-write] %d 例，%d 个 pack 成功，%d 个失败%n", cases, packed, failures);
        if (cases != 21) {
            System.out.printf("FAIL: 期望 21 例，实得 %d%n", cases);
            failures++;
        }
        System.out.println(failures == 0 ? "[ambr-write] ALL OK" : "[ambr-write] FAILED");
        if (failures != 0) System.exit(1);
    }

    @SuppressWarnings("unchecked")
    private static void checkShape(String id, Object v) {
        switch (id) {
            case "scalar-int":
                check(id, v instanceof Long && ((Long) v) == 1L, "期望整数 1，实得 " + v);
                break;
            case "flat-array": {
                final boolean ok = v instanceof List && ((List<Object>) v).size() == 3
                        && ((List<Object>) v).get(0).equals(1L)
                        && ((List<Object>) v).get(2).equals(3L);
                check(id, ok, "期望 [1 2 3]");
                break;
            }
            case "strtab-dedup": {
                final boolean ok = v instanceof java.util.Map
                        && "k".equals(((java.util.Map<String, Object>) v).get("k"))
                        && "k".equals(((java.util.Map<String, Object>) v).get("other"));
                check(id, ok, "期望 k/other 都是 k");
                break;
            }
            case "nested-containers": {
                final java.util.Map<String, Object> m = (java.util.Map<String, Object>) v;
                final Object cfg = m.get("config");
                final Object tags = m.get("tags");
                final boolean ok = cfg instanceof java.util.Map
                        && Long.valueOf(8080).equals(((java.util.Map<String, Object>) cfg).get("port"))
                        && tags instanceof List && "x".equals(((List<Object>) tags).get(0));
                check(id, ok, "期望 config.port=8080 / tags=[x]");
                break;
            }
            case "typed-block": {
                final java.util.Map<String, Object> m = (java.util.Map<String, Object>) v;
                final boolean ok = "ssh".equals(m.get("__name")) && "allow".equals(m.get("action"));
                check(id, ok, "期望 __name=ssh / action=allow");
                break;
            }
            default:
                break;
        }
    }

    private static void check(String id, boolean ok, String detail) {
        if (ok) {
            System.out.println("  ok: " + id);
        } else {
            fail(id, detail);
        }
    }

    private static void fail(String id, String detail) {
        System.out.println("FAIL: " + id + " — " + detail);
        failures++;
    }

    /**
     * Read a JSON string body from `from`, skipping escaped quotes. Leaves the body
     * **escaped**; `next[0]` is the position right after the closing quote.
     * ｜ 取 JSON 字符串体（跳过 \"），返回仍是转义态；next[0] 指向收尾引号之后。
     *
     * <p>Caller must advance its own cursor with {@code next[0]} — forgetting that makes
     * the scan loop forever on the first case (it did, once).</p>
     */
    private static String takeJsonString(String text, int from, String needle, int[] next) {
        final int at = text.indexOf(needle, from);
        if (at < 0) return null;
        int i = at + needle.length();
        final StringBuilder sb = new StringBuilder();
        while (i < text.length()) {
            final char c = text.charAt(i);
            if (c == '\\') {
                sb.append(c).append(text.charAt(i + 1));
                i += 2;
            } else if (c == '"') {
                next[0] = i + 1;
                return sb.toString();
            } else {
                sb.append(c);
                i++;
            }
        }
        return null;
    }

    private static String unescape(String s) {
        final StringBuilder sb = new StringBuilder(s.length());
        for (int i = 0; i < s.length(); i++) {
            final char c = s.charAt(i);
            if (c != '\\' || i + 1 >= s.length()) {
                sb.append(c);
                continue;
            }
            final char n = s.charAt(++i);
            switch (n) {
                case 'n': sb.append('\n'); break;
                case 't': sb.append('\t'); break;
                case 'r': sb.append('\r'); break;
                case 'b': sb.append('\b'); break;
                case 'f': sb.append('\f'); break;
                default: sb.append(n); break; // \" \\ \/ and unknowns
            }
        }
        return sb.toString();
    }

    private static String resolveFixture(String[] args) {
        if (args.length > 0) return args[0];
        final String[] candidates = {
            "tests/vectors/amberice-container.v1.json",
            "../tests/vectors/amberice-container.v1.json",
            "../../tests/vectors/amberice-container.v1.json",
            "../../../tests/vectors/amberice-container.v1.json",
        };
        for (String c : candidates) {
            if (Files.isRegularFile(Path.of(c))) return c;
        }
        return candidates[0];
    }
}
