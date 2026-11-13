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

        // 10. 诊断出口（P7）：带码带行号、不阻断解析、值树与 loads 相同
        Sml.Parsed dup = Sml.loadsWithDiags("a: 1\na: 2\n");
        check("10 重复键 -> 1 条 W-PARSE-001 @ line 2",
                dup.diagnostics.size() == 1
                        && "W-PARSE-001".equals(dup.diagnostics.get(0).codeStr)
                        && dup.diagnostics.get(0).line == 2
                        && dup.diagnostics.get(0).severity == 0,
                dup.diagnostics.toString());
        Map<String, Object> dupMap = (Map<String, Object>) dup.value;
        check("10 值树不变（仍是数组提升）",
                dupMap.get("a") instanceof List && ((List<?>) dupMap.get("a")).size() == 2,
                String.valueOf(dupMap.get("a")));
        Sml.Parsed clean = Sml.loadsWithDiags("k: 1\n");
        check("10 干净文档 -> 零诊断",
                clean.diagnostics.isEmpty() && Long.valueOf(1).equals(((Map<?, ?>) clean.value).get("k")),
                clean.diagnostics.toString());
        for (int i = 0; i < 200; i++) Sml.loadsWithDiags("a: 1\na: 2\n");
        check("10 200x loadsWithDiags 不崩溃", true, "");

        // 11. 完整选项（P7 第二片）：内置契约表 + env 注入
        try (Sml.Builtin bt = Sml.Builtin.create()) {
            check("11 空表 size==0", bt.size() == 0, "" + bt.size());
            boolean built = bt.addField("tls", "cert", "str", true)
                    && bt.addField("tls", "port", "int", false)
                    && bt.setRange("tls", "port", 1, 65535)
                    && bt.addField("tls", "mode", "enum", false)
                    && bt.addEnum("tls", "mode", "strict");
            check("11 建表 -> 1 份契约", built && bt.size() == 1, "" + bt.size());
            check("11 未知类型被拒（表不动）", !bt.addField("tls", "bad", "image", false), "");

            String src = "@feature enable type.weak\nsrv {\n  @weakbind tls\n"
                    + "  web { cert: 1 port: 99999 mode: nope }\n}\n"
                    + "@weakbind nosuch\nother { x: 1 }\n";
            Sml.Opts opts = new Sml.Opts();
            opts.flags = Sml.SML_F_BASIC | Sml.SML_F_WEAK;
            opts.builtins = bt;
            Sml.Parsed r = Sml.loadsFull(src, opts);
            long weak = r.diagnostics.stream()
                    .filter(d -> "W-CONTRACT-001".equals(d.codeStr)).count();
            check("11 内置表 -> 4 条 W-CONTRACT-001", weak == 4, r.diagnostics.toString());
            Map<?, ?> srv = (Map<?, ?>) ((Map<?, ?>) r.value).get("srv");
            Map<?, ?> web = (Map<?, ?>) srv.get("web");
            check("11 软校验只读：cert 仍是 int 1", Long.valueOf(1).equals(web.get("cert")),
                    String.valueOf(web.get("cert")));
        }

        Sml.Opts eopts = new Sml.Opts();
        eopts.env.add("SML_JAVA_PROBE=prod");
        Sml.Parsed r2 = Sml.loadsFull("k: $env.SML_JAVA_PROBE\n", eopts);
        check("11 env 注入 -> k=prod", "prod".equals(((Map<?, ?>) r2.value).get("k")),
                String.valueOf(r2.value));
        check("11 env 不改进程环境", System.getenv("SML_JAVA_PROBE") == null, "");

        try {
            Sml.loadsFull("@feature enable type.weak\n@weakbind tls\ns { x: 1 }\n", new Sml.Opts());
            check("11 基线不含 type.weak -> E-FEATURE-001", false, "未抛异常");
        } catch (Sml.SmlError e) {
            check("11 基线不含 type.weak -> E-FEATURE-001",
                    "E-FEATURE-001".equals(e.codeStr), e.codeStr);
        }

        // 12. 弱声明三表（P7 第四片）：都不进主树，从 loadsFullLists 取出
        String wsrc = "@feature enable type.weak\nsrv {\n  @weakbind tls\n"
                + "  web { cert: \"x\" }\n  @weakdata limits { max: 10 }\n}\n"
                + "@weakdoc base\n";
        Sml.Opts wopts = new Sml.Opts();
        wopts.flags = Sml.SML_F_BASIC | Sml.SML_F_WEAK;
        Sml.Parsed wp = Sml.loadsFullLists(wsrc, wopts);
        check("12 三表计数 1/1/1",
                wp.lists.binds.size() == 1 && wp.lists.data.size() == 1
                        && wp.lists.docs.size() == 1,
                wp.lists.binds.size() + "/" + wp.lists.data.size() + "/" + wp.lists.docs.size());
        Sml.WeakBind wb = wp.lists.binds.get(0);
        check("12 bind 的 path/name/line",
                "srv.web".equals(wb.path) && "tls".equals(wb.name) && wb.line > 0,
                wb.path + " " + wb.name);
        Sml.WeakData wd = wp.lists.data.get(0);
        check("12 weakdata 归属 srv/limits 且载荷可读",
                "srv".equals(wd.path) && "limits".equals(wd.name)
                        && wd.value instanceof Map
                        && Long.valueOf(10).equals(((Map<?, ?>) wd.value).get("max")),
                String.valueOf(wd.value));
        check("12 profile 在顶层（path 为空串）",
                "".equals(wp.lists.docs.get(0).path) && "base".equals(wp.lists.docs.get(0).name),
                wp.lists.docs.get(0).path);
        check("12 主树无痕迹",
                !((Map<?, ?>) ((Map<?, ?>) wp.value).get("srv")).containsKey("limits"), "");
        for (int i = 0; i < 100; i++) Sml.loadsFullLists(wsrc, wopts);
        check("12 100x loadsFullLists 不崩溃", true, "");

        // 13. 聚合结果（parseAll）：契约映射 + 契约表捕获 + 多错误
        String csrc = "@contract tls { cert: str }\nsrv { @is tls\n  cert: x\n}\n";
        Sml.Opts copts = new Sml.Opts();
        copts.flags = Sml.SML_F_BASIC | Sml.SML_F_CONTRACT;
        Sml.ParseAll pa = Sml.parseAll(csrc, copts);
        check("13 契约映射 srv -> tls",
                pa.contractMap.size() == 1 && "srv".equals(pa.contractMap.get(0).path)
                        && "tls".equals(pa.contractMap.get(0).name),
                pa.contractMap.toString());
        check("13 契约表已捕获（1 份）",
                pa.contracts != null && pa.contracts.size() == 1,
                pa.contracts == null ? "null" : "" + pa.contracts.size());

        Sml.Opts wopts2 = new Sml.Opts();
        wopts2.flags = Sml.SML_F_BASIC | Sml.SML_F_WEAK;
        wopts2.builtins = pa.contracts;
        Sml.Parsed lap = Sml.loadsFullLists(
                "@feature enable type.weak\n@weakbind nosuch\ns { x: 1 }\n", wopts2);
        check("13 闭环：注入捕获的表后报 decl.unknown_contract",
                lap.diagnostics.size() == 1
                        && "W-CONTRACT-001".equals(lap.diagnostics.get(0).codeStr),
                lap.diagnostics.toString());
        pa.contracts.close();

        String bad = "a { @is No1\n  x: 1 }\nb { @is No2\n  y: 2 }\n";
        Sml.ParseAll ba = Sml.parseAll(bad, copts);
        check("13 多错误：2 条且 severity=1、带行号",
                ba.errors.size() == 2
                        && "E-CONTRACT-001".equals(ba.errors.get(0).codeStr)
                        && ba.errors.get(0).line == 1 && ba.errors.get(0).severity == 1
                        && ba.errors.get(1).line == 3,
                ba.errors.toString());
        Map<String, Object> bv = (Map<String, Object>) ba.value;
        check("13 多错误：值树是尽力而为的部分结果",
                bv.size() == 2 && bv.containsKey("a") && bv.containsKey("b"),
                String.valueOf(bv.keySet()));

        try {
            Sml.parseAll(bad, copts, Sml.WANT_DIAGS);
            check("13 未请求 ERRORS -> 仍抛 SmlError", false, "未抛异常");
        } catch (Sml.SmlError e) {
            check("13 未请求 ERRORS -> 仍抛 SmlError",
                    "E-CONTRACT-001".equals(e.codeStr), e.codeStr);
        }

        for (int i = 0; i < 100; i++) Sml.parseAll(wsrc, wopts);
        check("13 100x parseAll 不崩溃", true, "");

        // 14. 文件入口（loadFileFull）：展开 include + 诊断 + 弱表
        Path tdir = Files.createTempDirectory("smljava_");
        try {
            Files.write(tdir.resolve("sub.sml"), "k: 1\n".getBytes("UTF-8"));
            Path mainPath = tdir.resolve("main.sml");
            Files.write(mainPath, ("@feature enable type.weak\ninclude \"sub.sml\"\n@weakdoc base\n")
                    .getBytes("UTF-8"));
            Sml.Opts fopts = new Sml.Opts();
            fopts.flags = Sml.SML_F_BASIC | Sml.SML_F_WEAK;
            Sml.Parsed fp = Sml.loadFileFull(mainPath.toString(), fopts);
            check("14 文件入口：include 已展开",
                    fp.value instanceof Map && Long.valueOf(1).equals(((Map<?, ?>) fp.value).get("k")),
                    String.valueOf(fp.value));
            check("14 文件入口：弱表带 @weakdoc",
                    fp.lists != null && fp.lists.docs.size() == 1
                            && "base".equals(fp.lists.docs.get(0).name),
                    fp.lists == null ? "null" : fp.lists.docs.toString());
            check("14 文件入口：干净文件 -> 零诊断", fp.diagnostics.isEmpty(),
                    fp.diagnostics.toString());
            try {
                Sml.loadFileFull(tdir.resolve("nope.sml").toString(), fopts);
                check("14 文件不存在 -> E-IO-001", false, "未抛异常");
            } catch (Sml.SmlError e) {
                check("14 文件不存在 -> E-IO-001", "E-IO-001".equals(e.codeStr), e.codeStr);
            }
        } finally {
            Files.deleteIfExists(tdir.resolve("sub.sml"));
            Files.deleteIfExists(tdir.resolve("main.sml"));
            Files.deleteIfExists(tdir);
        }

        System.out.println();
        if (fails == 0) {
            System.out.println("ALL OK");
        } else {
            System.out.println("SOME FAILED (" + fails + ")");
            System.exit(1);
        }
    }
}
