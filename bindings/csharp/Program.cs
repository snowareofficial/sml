// SPDX-License-Identifier: MulanPSL-2.0
// SML C# 绑定自测。
// 仓库：https://gitee.com/snoware/sml
// @version 0.1.0 (swsml 0.6.3)

using System;
using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using Sml;
// SmlException is nested in the Sml class; inside `namespace Sml.Test` the name `Sml`
// resolves to the namespace first, so alias it explicitly.
using SmlException = Sml.Sml.SmlException;

namespace Sml.Test
{
    /// <summary>SML C# 绑定测试（§4 黄金向量 + 内存稳定）。运行见 README。需 dotnet SDK 与 libsml。</summary>
    public static class Program
    {
        private static int _failures;

        private static void Check(string name, bool cond, string detail)
        {
            if (cond) Console.WriteLine("  PASS  " + name);
            else { Console.WriteLine("  FAIL  " + name + "  " + detail); _failures++; }
        }

        private const string Golden = @"server {
  host: ""example.com""
  port: 8080
  ratio: 0.75
  tls: false
  tags: [ a b c ]
}
";

        private static Dictionary<string, object> Expected()
        {
            return new Dictionary<string, object>
            {
                ["server"] = new Dictionary<string, object>
                {
                    ["host"] = "example.com",
                    ["port"] = 8080L,
                    ["ratio"] = 0.75,
                    ["tls"] = false,
                    ["tags"] = new List<object> { "a", "b", "c" }
                }
            };
        }

        private static bool DictEq(Dictionary<string, object> a, Dictionary<string, object> b)
        {
            if (a.Count != b.Count) return false;
            foreach (var kv in a)
            {
                if (!b.TryGetValue(kv.Key, out var bv)) return false;
                if (!ValEq(kv.Value, bv)) return false;
            }
            return true;
        }

        private static bool ValEq(object a, object b)
        {
            if (a is Dictionary<string, object> da && b is Dictionary<string, object> db) return DictEq(da, db);
            if (a is List<object> la && b is List<object> lb)
            {
                if (la.Count != lb.Count) return false;
                for (int i = 0; i < la.Count; i++) if (!ValEq(la[i], lb[i])) return false;
                return true;
            }
            if (a is long la2 && b is long lb2) return la2 == lb2;
            if (a is double da2 && b is double db2) return da2 == db2;
            return Equals(a, b);
        }

        public static int Main()
        {
            var exp = Expected();

            // 1. Loads 黄金向量
            var obj = (Dictionary<string, object>)Sml.Loads(Golden);
            Check("1 Loads(golden)==expected (port long, ratio double, tls bool, tags 3)",
                  DictEq(obj, exp), "got=" + JsonSerializer.Serialize(obj));

            // 2. 往返
            string dumped = Sml.Dumps(exp);
            var back = (Dictionary<string, object>)Sml.Loads(dumped);
            Check("2 round-trip Dumps/Loads", DictEq(back, exp), "got=" + JsonSerializer.Serialize(back));

            // 3. 富错误：未闭合块 -> E-PARSE-001 + text 非空
            try
            {
                Sml.Loads("bad { a: 1");
                Check("3 rich error (unclosed)", false, "未抛异常");
            }
            catch (SmlException e)
            {
                Check("3 rich error code_str==E-PARSE-001", e.CodeStr == "E-PARSE-001", "got=" + e.CodeStr);
                Check("3 rich error text 非空", !string.IsNullOrEmpty(e.Text), "text=" + e.Text);
                Check("3 不是通用 parse failed", e.Text.IndexOf("parse failed", StringComparison.OrdinalIgnoreCase) < 0, e.Text);
            }

            // 4. 错误定位：第 2 行契约错误 -> line==2, E-CONTRACT-001
            try
            {
                Sml.Loads("a: 1\n@is NoSuchContract\n");
                Check("4 line location", false, "未抛异常");
            }
            catch (SmlException e)
            {
                Check("4 line==2", e.Line == 2, "line=" + e.Line);
                Check("4 code_str==E-CONTRACT-001", e.CodeStr == "E-CONTRACT-001", "got=" + e.CodeStr);
            }

            // 5. LoadFile + include
            string dir = Path.Combine(Path.GetTempPath(), "sml_w28_" + Guid.NewGuid().ToString("N"));
            Directory.CreateDirectory(dir);
            string sub = Path.Combine(dir, "_inc_sub.sml");
            string main = Path.Combine(dir, "_inc_main.sml");
            File.WriteAllText(sub, "k: 1\n");
            File.WriteAllText(main, "include \"_inc_sub.sml\"\nm: 2\n");
            var inc = (Dictionary<string, object>)Sml.LoadFile(main);
            Check("5 include: 子文件字段 k=1", inc.TryGetValue("k", out var kv) && kv is long && (long)kv == 1, "got=" + kv);
            Check("5 include: 主文件字段 m=2", inc.TryGetValue("m", out var mv) && mv is long && (long)mv == 2, "got=" + mv);
            Directory.Delete(dir, true);

            // 6. flags 沙箱
            string envDoc = "x: $env.NOPE\n";
            bool baseOk;
            string baseCode = "";
            try { Sml.Loads(envDoc, 0); baseOk = true; }
            catch (SmlException e) { baseOk = e.CodeStr != "E-FEATURE-002"; baseCode = e.CodeStr; }
            Check("6 baseline(flags=0) 不因特性报错", baseOk, "code=" + baseCode);

            int noEnv = Sml.F_BASIC & ~Sml.F_ENV;
            try
            {
                Sml.Loads(envDoc, (uint)noEnv);
                Check("6 sandbox: 无 ENV 应报错", false, "未抛异常");
            }
            catch (SmlException e)
            {
                Check("6 sandbox code_str==E-FEATURE-002", e.CodeStr == "E-FEATURE-002", "got=" + e.CodeStr);
            }

            // 7. version / features
            string v = Sml.Version();
            Check("7 version() 非空", !string.IsNullOrEmpty(v), v);
            var feats = Sml.Features();
            Check("7 features() 含 'include'", feats.Contains("include"), string.Join(",", feats));
            Check("7 features() 含 'env'/'contract'", feats.Contains("env") && feats.Contains("contract"), string.Join(",", feats));

            // 8. 内存稳定
            for (int i = 0; i < 1000; i++) Sml.Loads(Golden);
            Check("8 1000x Loads 不崩溃", true, "");

            // 9. GetIn
            Check("9 GetIn 路径取值", Sml.GetIn(obj, "server.host") is string s && s == "example.com", "");

            // 9b. 诊断出口（P7）：带码带行号、不阻断解析、值树与 Loads 相同
            {
                object dup = Sml.LoadsWithDiags("a: 1\na: 2\n", 0, out var diags);
                Check("9b 重复键 -> 1 条 W-PARSE-001 @ line 2",
                      diags.Count == 1 && diags[0].Code == "W-PARSE-001"
                      && diags[0].Line == 2 && diags[0].Severity == 0,
                      diags.Count + " diag(s): " + string.Join(" | ", diags));
                bool promoted = dup is Dictionary<string, object> dd
                                && dd.TryGetValue("a", out var av)
                                && av is List<object> list && list.Count == 2;
                Check("9b 值树不变（仍是数组提升）", promoted, JsonSerializer.Serialize(dup));
                Sml.LoadsWithDiags("k: 1\n", 0, out var none);
                Check("9b 干净文档 -> 零诊断", none.Count == 0, none.Count + " diag(s)");
                for (int i = 0; i < 200; i++) Sml.LoadsWithDiags("a: 1\na: 2\n", 0, out _);
                Check("9b 200x LoadsWithDiags 不崩溃", true, "");
            }

            // 9c. 完整选项（P7 第二片）：内置契约表 + env 注入
            using (var bt = Sml.Builtin.Create())
            {
                Check("9c 空表 Count==0", bt.Valid && bt.Count == 0, "" + bt.Count);
                bool built = bt.AddField("tls", "cert", "str", true)
                    && bt.AddField("tls", "port", "int", false)
                    && bt.SetRange("tls", "port", 1, 65535)
                    && bt.AddField("tls", "mode", "enum", false)
                    && bt.AddEnum("tls", "mode", "strict");
                Check("9c 建表 -> 1 份契约", built && bt.Count == 1, "" + bt.Count);
                Check("9c 未知类型被拒（表不动）", !bt.AddField("tls", "bad", "image", false), "");

                var wopts = new Sml.Opts();
                wopts.Flags = Sml.F_BASIC | Sml.F_WEAK;
                wopts.Builtins = bt;
                string wsrc = "@feature enable type.weak\nsrv {\n  @weakbind tls\n"
                    + "  web { cert: 1 port: 99999 mode: nope }\n}\n"
                    + "@weakbind nosuch\nother { x: 1 }\n";
                object wobj = Sml.LoadsFull(wsrc, wopts, out var wdiags);
                int weak = 0;
                foreach (var d in wdiags) if (d.Code == "W-CONTRACT-001") weak++;
                Check("9c 内置表 -> 4 条 W-CONTRACT-001", weak == 4, wdiags.Count + " diag(s)");
                bool readOnly = wobj is Dictionary<string, object> wm
                    && wm.TryGetValue("srv", out var wsv) && wsv is Dictionary<string, object> wsrv
                    && wsrv.TryGetValue("web", out var wwv) && wwv is Dictionary<string, object> wweb
                    && wweb.TryGetValue("cert", out var wcv) && wcv is long wcl && wcl == 1;
                Check("9c 软校验只读：cert 仍是 1", readOnly, JsonSerializer.Serialize(wobj));
            }

            {
                var eopts = new Sml.Opts();
                eopts.Env.Add("SML_CS_PROBE=prod");
                object eobj = Sml.LoadsFull("k: $env.SML_CS_PROBE\n", eopts, out _);
                bool envOk = eobj is Dictionary<string, object> em
                    && em.TryGetValue("k", out var ekv) && ekv is string eks && eks == "prod";
                Check("9c env 注入 -> k=prod", envOk, JsonSerializer.Serialize(eobj));
                Check("9c env 不改进程环境",
                      Environment.GetEnvironmentVariable("SML_CS_PROBE") == null, "");

                try
                {
                    Sml.LoadsFull("@feature enable type.weak\n@weakbind tls\ns { x: 1 }\n",
                                  new Sml.Opts(), out _);
                    Check("9c 基线不含 type.weak -> E-FEATURE-001", false, "未抛异常");
                }
                catch (SmlException e)
                {
                    Check("9c 基线不含 type.weak -> E-FEATURE-001",
                          e.CodeStr == "E-FEATURE-001", e.CodeStr);
                }

                // 9d. 弱声明三表（P7 第四片）：都不进主树，从 LoadsFullLists 取出
                var wl2opts = new Sml.Opts();
                wl2opts.Flags = Sml.F_BASIC | Sml.F_WEAK;
                string wsrc2 = "@feature enable type.weak\nsrv {\n  @weakbind tls\n"
                    + "  web { cert: \"x\" }\n  @weakdata limits { max: 10 }\n}\n"
                    + "@weakdoc base\n";
                object wl2obj = Sml.LoadsFullLists(wsrc2, wl2opts, out _, out var wl2);
                Check("9d 三表计数 1/1/1",
                      wl2.Binds.Count == 1 && wl2.Data.Count == 1 && wl2.Docs.Count == 1,
                      wl2.Binds.Count + "/" + wl2.Data.Count + "/" + wl2.Docs.Count);
                Check("9d bind 的 path/name/line",
                      wl2.Binds[0].Path == "srv.web" && wl2.Binds[0].Name == "tls"
                          && wl2.Binds[0].Line > 0, wl2.Binds[0].Path);
                bool payloadOk = wl2.Data[0].Value is Dictionary<string, object> pl
                    && pl.TryGetValue("max", out var pmax) && pmax is long pl10 && pl10 == 10;
                Check("9d weakdata 归属 srv/limits 且载荷可读",
                      wl2.Data[0].Path == "srv" && wl2.Data[0].Name == "limits" && payloadOk,
                      JsonSerializer.Serialize(wl2.Data[0].Value));
                Check("9d profile 在顶层（path 为空串）",
                      wl2.Docs[0].Path == "" && wl2.Docs[0].Name == "base", wl2.Docs[0].Path);
                bool noTrace = wl2obj is Dictionary<string, object> wm2
                    && wm2.TryGetValue("srv", out var sv2) && sv2 is Dictionary<string, object> srv2
                    && !srv2.ContainsKey("limits");
                Check("9d 主树无痕迹", noTrace, "");
                for (int i = 0; i < 100; i++) Sml.LoadsFullLists(wsrc2, wl2opts, out _, out _);
                Check("9d 100x LoadsFullLists 不崩溃", true, "");
            }

            // 9e. 聚合结果（ParseAllOf）：契约映射 + 契约表捕获 + 多错误
            {
                var copts = new Sml.Opts();
                copts.Flags = Sml.F_BASIC | Sml.F_CONTRACT;
                var pa = Sml.ParseAllOf(
                    "@contract tls { cert: str }\nsrv { @is tls\n  cert: x\n}\n", copts);
                Check("9e 契约映射 srv -> tls",
                      pa.ContractMap.Count == 1 && pa.ContractMap[0].Path == "srv"
                          && pa.ContractMap[0].Name == "tls",
                      string.Join(" | ", pa.ContractMap));
                Check("9e 契约表已捕获（1 份）",
                      pa.Contracts != null && pa.Contracts.Count == 1,
                      pa.Contracts == null ? "null" : "" + pa.Contracts.Count);

                var w2opts = new Sml.Opts();
                w2opts.Flags = Sml.F_BASIC | Sml.F_WEAK;
                w2opts.Builtins = pa.Contracts;
                var lap = Sml.LoadsFullLists(
                    "@feature enable type.weak\n@weakbind nosuch\ns { x: 1 }\n", w2opts,
                    out var ldiags, out _);
                Check("9e 闭环：注入捕获的表后报 decl.unknown_contract",
                      ldiags.Count == 1 && ldiags[0].Code == "W-CONTRACT-001",
                      string.Join(" | ", ldiags));
                pa.Contracts.Dispose();

                string bad2 = "a { @is No1\n  x: 1 }\nb { @is No2\n  y: 2 }\n";
                var ba = Sml.ParseAllOf(bad2, copts);
                Check("9e 多错误：2 条且 severity=1、带行号",
                      ba.Errors.Count == 2 && ba.Errors[0].Code == "E-CONTRACT-001"
                          && ba.Errors[0].Line == 1 && ba.Errors[0].Severity == 1
                          && ba.Errors[1].Line == 3,
                      string.Join(" | ", ba.Errors));
                bool partial = ba.Value is Dictionary<string, object> bv
                               && bv.Count == 2 && bv.ContainsKey("a") && bv.ContainsKey("b");
                Check("9e 多错误：值树是尽力而为的部分结果", partial,
                      JsonSerializer.Serialize(ba.Value));

                try
                {
                    Sml.ParseAllOf(bad2, copts, Sml.WantDiags);
                    Check("9e 未请求 ERRORS -> 仍抛异常", false, "未抛异常");
                }
                catch (SmlException e)
                {
                    Check("9e 未请求 ERRORS -> 仍抛异常",
                          e.CodeStr == "E-CONTRACT-001", e.CodeStr);
                }

                string wsrc3 = "@feature enable type.weak\n@weakbind tls\nsrv { x: 1 }\n";
                var w3opts = new Sml.Opts();
                w3opts.Flags = Sml.F_BASIC | Sml.F_WEAK;
                for (int i = 0; i < 100; i++)
                    Sml.ParseAllOf(wsrc3, w3opts);
                Check("9e 100x ParseAllOf 不崩溃", true, "");
            }

            // 9f. 文件入口（LoadFileFull）：展开 include + 诊断 + 弱表
            {
                string tdir = Path.Combine(Path.GetTempPath(), "smlcs_" + Guid.NewGuid().ToString("N"));
                Directory.CreateDirectory(tdir);
                try
                {
                    File.WriteAllText(Path.Combine(tdir, "sub.sml"), "k: 1\n");
                    string mainPath = Path.Combine(tdir, "main.sml");
                    File.WriteAllText(mainPath,
                        "@feature enable type.weak\ninclude \"sub.sml\"\n@weakdoc base\n");
                    var fopts = new Sml.Opts();
                    fopts.Flags = Sml.F_BASIC | Sml.F_WEAK;
                    object fobj = Sml.LoadFileFull(mainPath, fopts, out var fdiags, out var fwl);
                    bool incOk = fobj is Dictionary<string, object> fm
                                 && fm.TryGetValue("k", out var fk) && fk is long fkl && fkl == 1;
                    Check("9f 文件入口：include 已展开", incOk, JsonSerializer.Serialize(fobj));
                    Check("9f 文件入口：弱表带 @weakdoc",
                          fwl.Docs.Count == 1 && fwl.Docs[0].Name == "base",
                          string.Join(" | ", fwl.Docs.ConvertAll(d => d.Name)));
                    Check("9f 文件入口：干净文件 -> 零诊断", fdiags.Count == 0,
                          fdiags.Count + " diag(s)");
                    try
                    {
                        Sml.LoadFileFull(Path.Combine(tdir, "nope.sml"), fopts, out _, out _);
                        Check("9f 文件不存在 -> E-IO-001", false, "未抛异常");
                    }
                    catch (SmlException e)
                    {
                        Check("9f 文件不存在 -> E-IO-001", e.CodeStr == "E-IO-001", e.CodeStr);
                    }
                }
                finally
                {
                    Directory.Delete(tdir, true);
                }
            }

            // 10. `.ambr` 跨端容器向量（22 例，逐字符）
            RunAmbrVectors();

            Console.WriteLine();
            if (_failures == 0) { Console.WriteLine("ALL OK"); return 0; }
            Console.WriteLine("SOME FAILED (" + _failures + ")");
            return 1;
        }

        /// <summary>`.ambr` 22 例：hex → LoadText → 与 canonical_text 逐字符比较。</summary>
        private static void RunAmbrVectors()
        {
            string? path = FindRepoFile(Path.Combine("tests", "vectors", "amberice-container.v1.json"));
            if (path == null)
            {
                Check("10 .ambr 向量文件存在", false, "未找到 tests/vectors/amberice-container.v1.json");
                return;
            }

            using var doc = JsonDocument.Parse(File.ReadAllText(path));
            int total = 0, ok = 0;
            foreach (var c in doc.RootElement.GetProperty("cases").EnumerateArray())
            {
                total++;
                string id = c.GetProperty("id").GetString() ?? "?";
                byte[] bytes = Convert.FromHexString(c.GetProperty("ambr_hex").GetString() ?? "");
                string want = c.GetProperty("canonical_text").GetString() ?? "";
                try
                {
                    if (Amberice.Container.LoadText(bytes) == want) ok++;
                    else Console.WriteLine("  FAIL  ambr " + id + "  文本不一致");
                }
                catch (Exception e)
                {
                    Console.WriteLine("  FAIL  ambr " + id + "  " + e.Message);
                }
            }
            Check("10 .ambr 22 例逐字符", total == 22 && ok == 22, ok + "/" + total);

            // 11. 写入往返：顶层容器必须原样回来；顶层标量被 sml-parse 以
            //     E-PARSE-008 拒绝（unfinished §4.2 的已知欠账），单独计数。
            int rt = 0, scalars = 0;
            foreach (var c in doc.RootElement.GetProperty("cases").EnumerateArray())
            {
                string id = c.GetProperty("id").GetString() ?? "?";
                string want = c.GetProperty("canonical_text").GetString() ?? "";
                try
                {
                    if (Amberice.Container.LoadText(Amberice.Container.PackText(want)) == want) rt++;
                    else Console.WriteLine("  FAIL  ambr pack " + id + "  往返不一致");
                }
                catch (Amberice.Container.AmbericeException e) when (e.Message.Contains("E-PARSE-008"))
                {
                    scalars++;
                }
                catch (Exception e)
                {
                    Console.WriteLine("  FAIL  ambr pack " + id + "  " + e.Message);
                }
            }
            Check("11 .ambr 写入往返 22 例", rt == 22 && scalars == 0, rt + " round-trip, " + scalars + " scalar");
        }

        /// <summary>从 bin 目录或当前目录向上找仓库根下的相对路径。</summary>
        private static string? FindRepoFile(string relative)
        {
            foreach (var start in new[] { AppContext.BaseDirectory, Directory.GetCurrentDirectory() })
            {
                var dir = new DirectoryInfo(start);
                while (dir != null)
                {
                    string p = Path.Combine(dir.FullName, relative);
                    if (File.Exists(p)) return p;
                    dir = dir.Parent;
                }
            }
            return null;
        }
    }
}
