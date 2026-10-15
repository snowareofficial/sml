// SPDX-License-Identifier: MulanPSL-2.0
// SML Java 绑定自测。
// 仓库：https://gitee.com/snoware/sml
// @version 0.1.0 (swsml 0.6.3)

package sml;

import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** SML Java 绑定测试（§4 黄金向量 + 内存稳定）。运行：
 *  java --enable-native-access=ALL-UNNAMED -cp <out> sml.SmlTest
 *  （需 SML_LIB 指向 libsml） */
public class SmlTest {
    static int fails = 0;

    static void check(String name, boolean cond, String detail) {
        if (cond) {
            System.out.println("  PASS  " + name);
        } else {
            System.out.println("  FAIL  " + name + "  " + detail);
            fails++;
        }
    }

    static final String GOLDEN = "server {\n"
            + "  host: \"example.com\"\n"
            + "  port: 8080\n"
            + "  ratio: 0.75\n"
            + "  tls: false\n"
            + "  tags: [ a b c ]\n"
            + "}\n";

    @SuppressWarnings("unchecked")
    static Map<String, Object> expected() {
        Map<String, Object> server = new LinkedHashMap<>();
        server.put("host", "example.com");
        server.put("port", 8080L);
        server.put("ratio", 0.75);
        server.put("tls", false);
        server.put("tags", List.of("a", "b", "c"));
        Map<String, Object> root = new LinkedHashMap<>();
        root.put("server", server);
        return root;
    }

    public static void main(String[] args) throws Exception {
        Map<String, Object> exp = expected();

        // 1. loads 黄金向量
        Object obj = Sml.loads(GOLDEN);
        check("1 loads(golden)==expected (port Long, ratio Double, tls Boolean, tags 3)",
                exp.equals(obj),
                "got=" + obj);

        // 2. 往返
        Object back = Sml.loads(Sml.dumps(obj));
        check("2 round-trip dumps/loads", exp.equals(back), "got=" + back);

        // 3. 富错误：未闭合块 -> E-PARSE-001 + text 非空
        try {
            Sml.loads("bad { a: 1");
            check("3 rich error (unclosed)", false, "未抛异常");
        } catch (Sml.SmlError e) {
            check("3 rich error code_str==E-PARSE-001", "E-PARSE-001".equals(e.codeStr), "got=" + e.codeStr);
            check("3 rich error text 非空", !e.text.isEmpty(), "text=" + e.text);
            check("3 不是通用 parse failed", !e.text.toLowerCase().contains("parse failed"), e.text);
        }

        // 4. 错误定位：第 2 行契约错误 -> line==2, E-CONTRACT-001
        try {
            Sml.loads("a: 1\n@is NoSuchContract\n");
            check("4 line location", false, "未抛异常");
        } catch (Sml.SmlError e) {
            check("4 line==2", e.line == 2, "line=" + e.line);
            check("4 code_str==E-CONTRACT-001", "E-CONTRACT-001".equals(e.codeStr), "got=" + e.codeStr);
        }

        // 5. load_file + include
        Path dir = Files.createTempDirectory("sml_w28");
        Path sub = dir.resolve("_inc_sub.sml");
        Path main = dir.resolve("_inc_main.sml");
        Files.writeString(sub, "k: 1\n");
        Files.writeString(main, "include \"_inc_sub.sml\"\nm: 2\n");
        Map<String, Object> inc = (Map<String, Object>) Sml.loadFile(main.toString());
        check("5 include: 子文件字段 k=1", Long.valueOf(1).equals(inc.get("k")), "got=" + inc.get("k"));
        check("5 include: 主文件字段 m=2", Long.valueOf(2).equals(inc.get("m")), "got=" + inc.get("m"));
        Files.deleteIfExists(main);
        Files.deleteIfExists(sub);
        Files.deleteIfExists(dir);

        // 6. flags 沙箱
        String envDoc = "x: $env.NOPE\n";
        boolean baseOk;
        try {
            Sml.loads(envDoc, 0);
            baseOk = true;
        } catch (Sml.SmlError e) {
            baseOk = !"E-FEATURE-002".equals(e.codeStr);
        }
        check("6 baseline(flags=0) 不因特性报错", baseOk, "baseOk=" + baseOk);

        int noEnv = Sml.SML_F_BASIC & ~Sml.SML_F_ENV;
        try {
            Sml.loads(envDoc, noEnv);
            check("6 sandbox: 无 ENV 应报错", false, "未抛异常");
        } catch (Sml.SmlError e) {
            check("6 sandbox code_str==E-FEATURE-002", "E-FEATURE-002".equals(e.codeStr), "got=" + e.codeStr);
        }

        // 7. version / features
        String v = Sml.version();
        check("7 version() 非空", !v.isEmpty(), v);
        List<String> feats = Sml.features();
        check("7 features() 含 'include'", feats.contains("include"), feats.toString());
        check("7 features() 含 'env'/'contract'",
                feats.contains("env") && feats.contains("contract"), feats.toString());

        // 8. 内存稳定
        for (int i = 0; i < 1000; i++) Sml.loads(GOLDEN);
        check("8 1000x loads 不崩溃", true, "");

        // 9. get_in
        check("9 get_in 路径取值", "example.com".equals(Sml.getIn(obj, "server.host")), "");

        System.out.println();
        if (fails == 0) {
            System.out.println("ALL OK");
        } else {
            System.out.println("SOME FAILED (" + fails + ")");
            System.exit(1);
        }
    }
}
