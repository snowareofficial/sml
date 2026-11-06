// SPDX-License-Identifier: MulanPSL-2.0
// AmbrTest.java — `.ambr` 解码器对拍容器向量（T7 / N03，纯 Java）。
//
// 纯 Java，不依赖 Rust cdylib：本测试只编译 `sml.Ambr`，不需要 SML_LIB。
// 运行：javac -d out bindings/java/src/main/java/sml/Ambr.java \
//                  bindings/java/src/test/java/sml/AmbrTest.java
//       java -cp out sml.AmbrTest [fixture.json]
package sml;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;

public final class AmbrTest {
    private static int fails = 0;
    private static int cases = 0;

    private static void check(String name, boolean cond, String detail) {
        if (cond) {
            System.out.println("  ok: " + name);
        } else {
            System.out.println("  FAIL: " + name + " — " + detail);
            fails++;
        }
    }

    private static byte[] hexToBytes(String hex) {
        final byte[] out = new byte[hex.length() / 2];
        for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(hex.substring(i * 2, i * 2 + 2), 16);
        }
        return out;
    }

    private static String quoted(String text, int from, String needle, int[] next) {
        final int at = text.indexOf(needle, from);
        if (at < 0) return null;
        final int s = at + needle.length();
        final int e = text.indexOf('"', s);
        if (e < 0) return null;
        next[0] = e + 1;
        return text.substring(s, e);
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

    @SuppressWarnings("unchecked")
    private static void checkCase(String id, byte[] bytes) {
        final Object v;
        try {
            v = Ambr.load(bytes);
        } catch (RuntimeException e) {
            check(id, false, e.getMessage());
            return;
        }
        switch (id) {
            case "scalar-int":
                check(id, v instanceof Long && ((Long) v) == 1L, "期望整数 1，实得 " + v);
                break;
            case "flat-array": {
                boolean ok = v instanceof List && ((List<Object>) v).size() == 3
                        && ((List<Object>) v).get(0).equals(1L)
                        && ((List<Object>) v).get(2).equals(3L);
                check(id, ok, "期望 [1 2 3]");
                break;
            }
            case "strtab-dedup": {
                Map<String, Object> m = (Map<String, Object>) v;
                check(id, "k".equals(m.get("k")) && "k".equals(m.get("other")), "期望 k/other 都是 k");
                break;
            }
            case "nested-containers": {
                Map<String, Object> m = (Map<String, Object>) v;
                Map<String, Object> cfg = (Map<String, Object>) m.get("config");
                List<Object> tags = (List<Object>) m.get("tags");
                check(id, cfg != null && Long.valueOf(8080).equals(cfg.get("port"))
                        && tags != null && tags.size() == 1 && "x".equals(tags.get(0)),
                        "期望 config.port=8080 / tags=[x]");
                break;
            }
            case "typed-block": {
                Map<String, Object> m = (Map<String, Object>) v;
                check(id, "ssh".equals(m.get("__name")) && "allow".equals(m.get("action")),
                        "期望 __name=ssh / action=allow");
                break;
            }
            default:
                break;
        }
    }

    public static void main(String[] args) throws IOException {
        final String path = resolveFixture(args);
        final String json = Files.readString(Path.of(path), StandardCharsets.UTF_8);
        System.out.println("[ambr] 解码器对拍（Java，纯实现）");

        int pos = 0;
        while (true) {
            final int[] next = new int[1];
            final String id = quoted(json, pos, "\"id\": \"", next);
            if (id == null) break;
            final String hex = quoted(json, next[0], "\"ambr_hex\": \"", next);
            if (hex == null) break;
            checkCase(id, hexToBytes(hex));
            cases++;
            pos = next[0];
        }

        System.out.println("[ambr] " + cases + " 例，" + fails + " 个失败");
        if (cases != 21) {
            System.out.println("FAIL: 期望 21 例，实得 " + cases);
            fails++;
        }
        System.out.println(fails == 0 ? "[ambr] ALL OK" : "[ambr] FAILED");
        if (fails != 0) System.exit(1);
    }
}
