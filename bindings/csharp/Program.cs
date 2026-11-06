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

            // 10. `.ambr` 跨端容器向量（21 例，逐字符）
            RunAmbrVectors();

            Console.WriteLine();
            if (_failures == 0) { Console.WriteLine("ALL OK"); return 0; }
            Console.WriteLine("SOME FAILED (" + _failures + ")");
            return 1;
        }

        /// <summary>`.ambr` 21 例：hex → LoadText → 与 canonical_text 逐字符比较。</summary>
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
            Check("10 .ambr 21 例逐字符", total == 21 && ok == 21, ok + "/" + total);

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
            Check("11 .ambr 写入往返 21 例", rt == 21 && scalars == 0, rt + " round-trip, " + scalars + " scalar");
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
