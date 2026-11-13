// SPDX-License-Identifier: MulanPSL-2.0
// SML 多语言宿主绑定（C# / P/Invoke）。
// 仓库：https://gitee.com/snoware/sml
// @version 0.1.0 (swsml 0.6.3)

using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

namespace Sml
{
    /// <summary>
    /// SML 宿主绑定（C# / P/Invoke）—— 经 C-ABI 调 Rust cdylib（sml / libsml）。
    ///
    /// 读路径（Loads / LoadFile / 取值）直接遍历 v3 值树，不经 JSON：类型保真、富错误直接到手。
    /// 仅 Dumps 按文档走官方桥 obj -> JSON -> sml_dump（ABI 无值树构建器）。
    /// ABI 宪法 = c/sml_rs.h（Rust cdylib v3 全集）。不重写 SML 解析器。
    ///
    /// 内存纪律（§0.3）：
    ///   * sml_loads / sml_load_file 返回的根：物化后 sml_free。
    ///   * sml_str_dup / sml_key_at / sml_dumps / sml_dump / sml_version_str 返回的 char*：读后 sml_free_str
    ///     （sml_key_at 的键也是堆副本，必须 free）。
    ///   * sml_get / sml_at / sml_get_path 返回借用指针：不释放、不跨根释放使用。
    ///   * sml_version 返回静态串：只读，不释放。
    ///
    /// 注意（UTF-8）：Rust 的 char* 一律为 UTF-8 字节，故入参用托管 UTF-8 字节缓冲（CoTaskMem），
    /// 出参用 Marshal.PtrToStringUTF8 读取；切勿用 ANSI/Unicode 变体，否则中文乱码。
    /// </summary>
    public static class Sml
    {
        // 特性位常量（与 c/sml_rs.h 逐位一致）
        public const int F_BAREWORD_STR = 1 << 0;
        public const int F_INCLUDE = 1 << 1;
        public const int F_ENV = 1 << 2;
        public const int F_CONTRACT = 1 << 3;
        public const int F_FRAGMENT = 1 << 4;
        public const int F_TOP_ARRAY = 1 << 5;
        public const int F_NAMESPACE = 1 << 6;
        public const int F_IMPLICIT_NS = 1 << 7;
        public const int F_MULTI_INCLUDE = 1 << 8;
        public const int F_GLOB_INCLUDE = 1 << 9;
        public const int F_REGEX_INCLUDE = 1 << 10;
        public const int F_EXT_REWRITE = 1 << 11;
        public const int F_BASIC = F_BAREWORD_STR | F_INCLUDE | F_FRAGMENT;

        // 值类型
        private const int T_NULL = 0, T_BOOL = 1, T_INT = 2, T_FLOAT = 3,
                          T_STR = 4, T_ARRAY = 5, T_OBJECT = 6;

        /// <summary>sml_error 镜像：int×3 + 4 字节显式补齐 + size_t(8) + byte[128/256/16]，共 424。</summary>
        [StructLayout(LayoutKind.Sequential, Pack = 8)]
        public struct SmlErrorStruct
        {
            public int code;
            public int line;
            public int column;
            public int _pad;                 // 显式补齐，使 position(8 字节) 落在偏移 16
            public ulong position;           // size_t（64 位 8 字节）
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 128)]
            public byte[] source;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 256)]
            public byte[] text;
            [MarshalAs(UnmanagedType.ByValArray, SizeConst = 16)]
            public byte[] code_str;
        }

        /// <summary>sml_diag 镜像：char* code_str / char* text / int line / int severity（24 字节）。</summary>
        [StructLayout(LayoutKind.Sequential, Pack = 8)]
        public struct SmlDiagStruct
        {
            public IntPtr code_str;
            public IntPtr text;
            public int line;
            public int severity;
        }

        /// <summary>sml_diag_list 镜像：items 指针 + size_t len（16 字节）。</summary>
        [StructLayout(LayoutKind.Sequential, Pack = 8)]
        public struct SmlDiagListStruct
        {
            public IntPtr items;
            public UIntPtr len;
        }

        /// <summary>一条非致命诊断（键冲突 / 片段覆盖 / 弱声明不匹配）。</summary>
        public sealed class Diagnostic
        {
            public string Code { get; }     // null = 该档暂无稳定码
            public string Text { get; }
            public int Line { get; }        // 1 起；0 = 未知
            public int Severity { get; }    // 0 = warning，1 = error（预留）

            public Diagnostic(string code, string text, int line, int severity)
            {
                Code = code;
                Text = text;
                Line = line;
                Severity = severity;
            }

            public override string ToString()
            {
                string loc = Line > 0 ? $" @{Line}" : "";
                return $"{Code ?? "<no code>"}{loc}: {Text}";
            }
        }

        /// <summary>sml_opts 镜像：size_t struct_size / uint flags / 4 pad / char** env / size_t env_len / 指针（40）。</summary>
        [StructLayout(LayoutKind.Sequential, Pack = 8)]
        public struct SmlOptsStruct
        {
            public UIntPtr struct_size;
            public uint flags;
            public uint _pad;        // 显式补齐，使 env 落在偏移 16
            public IntPtr env;
            public UIntPtr env_len;
            public IntPtr builtins;
        }

        /// <summary>内置契约表（`@weakbind` 软校验用）；空表 ⇒ 软校验整段跳过。</summary>
        public sealed class Builtin : IDisposable
        {
            private IntPtr _p;

            private Builtin(IntPtr p) { _p = p; }

            public static Builtin Create() => new Builtin(sml_builtin_new());

            /// <summary>接管一个已归本进程所有的 sml_builtin*（如 sml_builtin_clone 的产物）。</summary>
            internal static Builtin Adopt(IntPtr p) => p == IntPtr.Zero ? null : new Builtin(p);

            public bool Valid => _p != IntPtr.Zero;

            public int Count => _p == IntPtr.Zero ? 0 : (int)sml_builtin_count(_p);

            /// <summary>声明字段；ty 取 any / str / int / num / bool / enum。false = 参数非法（表不动）。</summary>
            public bool AddField(string contract, string field, string ty, bool required)
            {
                if (_p == IntPtr.Zero) return false;
                IntPtr cc = ToUtf8(contract), cf = ToUtf8(field), ct = ToUtf8(ty);
                try
                {
                    return sml_builtin_add_field(_p, cc, cf, ct, required ? 1 : 0) == 0;
                }
                finally { FreeUtf8(cc); FreeUtf8(cf); FreeUtf8(ct); }
            }

            public bool SetRange(string contract, string field, double min, double max)
            {
                if (_p == IntPtr.Zero) return false;
                IntPtr cc = ToUtf8(contract), cf = ToUtf8(field);
                try { return sml_builtin_set_range(_p, cc, cf, min, max) == 0; }
                finally { FreeUtf8(cc); FreeUtf8(cf); }
            }

            public bool AddEnum(string contract, string field, string value)
            {
                if (_p == IntPtr.Zero) return false;
                IntPtr cc = ToUtf8(contract), cf = ToUtf8(field), cv = ToUtf8(value);
                try { return sml_builtin_add_enum(_p, cc, cf, cv) == 0; }
                finally { FreeUtf8(cc); FreeUtf8(cf); FreeUtf8(cv); }
            }

            internal IntPtr Raw => _p;

            public void Dispose()
            {
                if (_p != IntPtr.Zero)
                {
                    sml_builtin_free(_p);
                    _p = IntPtr.Zero;
                }
            }
        }

        /// <summary>sml_weakbind 镜像：path / name / int line / 4 pad（24）。</summary>
        [StructLayout(LayoutKind.Sequential, Pack = 8)]
        public struct SmlWeakBindStruct
        {
            public IntPtr path;
            public IntPtr name;
            public int line;
            public int _pad;
        }

        /// <summary>sml_weakdata 镜像：path / name / int line / 4 pad / value（32）。</summary>
        [StructLayout(LayoutKind.Sequential, Pack = 8)]
        public struct SmlWeakDataStruct
        {
            public IntPtr path;
            public IntPtr name;
            public int line;
            public int _pad;
            public IntPtr value;
        }

        /// <summary>sml_contract_ref 镜像：path / name（16）。</summary>
        [StructLayout(LayoutKind.Sequential, Pack = 8)]
        public struct SmlContractRefStruct
        {
            public IntPtr path;
            public IntPtr name;
        }

        /// <summary>sml_contractmap 镜像：items 指针 + size_t len（16）。</summary>
        [StructLayout(LayoutKind.Sequential, Pack = 8)]
        public struct SmlContractMapStruct
        {
            public IntPtr items;
            public UIntPtr len;
        }

        /// <summary>一条「某实体应用了某个契约」。</summary>
        public sealed class ContractRef
        {
            public string Path { get; }
            public string Name { get; }
            public ContractRef(string path, string name) { Path = path; Name = name; }
            public override string ToString() => Path + " -> " + Name;
        }

        /// <summary>parseAll 的 want 位（与 c/sml_rs.h 的 SML_WANT_* 逐位一致）。</summary>
        public const int WantDiags = 1 << 0;
        public const int WantErrors = 1 << 1;
        public const int WantWeak = 1 << 2;
        public const int WantContractMap = 1 << 3;
        public const int WantContracts = 1 << 4;
        public const int WantAll = 0x1F;

        /// <summary>一次解析的全部产物；未请求的部分是空表 / null。</summary>
        public sealed class ParseAll
        {
            public object Value { get; internal set; }
            public List<Diagnostic> Diagnostics { get; internal set; } = new List<Diagnostic>();
            public List<Diagnostic> Errors { get; internal set; } = new List<Diagnostic>();
            public WeakLists Weak { get; internal set; } = new WeakLists();
            public List<ContractRef> ContractMap { get; internal set; } = new List<ContractRef>();
            /// <summary>独立克隆的契约表（调用方负责 Dispose）；未请求时为 null。</summary>
            public Builtin Contracts { get; internal set; }
        }

        /// <summary>sml_weaklists 镜像：3 对（指针, size_t）（48）。</summary>
        [StructLayout(LayoutKind.Sequential, Pack = 8)]
        public struct SmlWeakListsStruct
        {
            public IntPtr binds;
            public UIntPtr n_binds;
            public IntPtr data;
            public UIntPtr n_data;
            public IntPtr docs;
            public UIntPtr n_docs;
        }

        /// <summary>一条弱绑定（`@weakbind`）：目标块路径 + 契约名。</summary>
        public sealed class WeakBind
        {
            public string Path { get; }
            public string Name { get; }
            public int Line { get; }
            public WeakBind(string path, string name, int line) { Path = path; Name = name; Line = line; }
        }

        /// <summary>一条弱数据（`@weakdata`）：载荷不在主树里，已物化。</summary>
        public sealed class WeakData
        {
            public string Path { get; }
            public string Name { get; }
            public int Line { get; }
            public object Value { get; }
            public WeakData(string path, string name, int line, object value)
            {
                Path = path; Name = name; Line = line; Value = value;
            }
        }

        /// <summary>一条弱 profile 引用（`@weakdoc`）：只有名字。</summary>
        public sealed class WeakDoc
        {
            public string Path { get; }
            public string Name { get; }
            public int Line { get; }
            public WeakDoc(string path, string name, int line) { Path = path; Name = name; Line = line; }
        }

        /// <summary>弱声明三表；三者都不进主树。</summary>
        public sealed class WeakLists
        {
            public List<WeakBind> Binds { get; } = new List<WeakBind>();
            public List<WeakData> Data { get; } = new List<WeakData>();
            public List<WeakDoc> Docs { get; } = new List<WeakDoc>();
        }

        /// <summary>LoadsFull 的选项：特性上限 + `$env` 覆盖表 + 内置契约表。</summary>
        public sealed class Opts
        {
            /// <summary>特性上限位（F_*）；0 = 基线，不含 type.weak。</summary>
            public uint Flags { get; set; }
            /// <summary>"KEY=VALUE" 数组，供 $env.X 内联；只作用于本次解析。</summary>
            public List<string> Env { get; } = new List<string>();
            /// <summary>内置契约表；null = 不做弱绑定软校验。</summary>
            public Builtin Builtins { get; set; }
        }

        private const string LIB = "sml";

        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_loads(IntPtr text, uint flags, ref SmlErrorStruct err);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_parse_with_diags(IntPtr text, uint flags,
            ref SmlDiagListStruct diags, ref SmlErrorStruct err);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern void sml_diag_list_free(ref SmlDiagListStruct list);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_parse_full(IntPtr text, ref SmlOptsStruct opts,
            ref SmlDiagListStruct diags, ref SmlErrorStruct err);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_parse_full_lists(IntPtr text, ref SmlOptsStruct opts,
            ref SmlWeakListsStruct lists, ref SmlDiagListStruct diags, ref SmlErrorStruct err);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern void sml_weaklists_free(ref SmlWeakListsStruct lists);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_parse_all(IntPtr text, ref SmlOptsStruct opts,
            int want, ref SmlErrorStruct err);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_load_file_full(IntPtr path, ref SmlOptsStruct opts,
            ref SmlWeakListsStruct lists, ref SmlDiagListStruct diags, ref SmlErrorStruct err);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_result_value(IntPtr r);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_result_diags(IntPtr r);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_result_errors(IntPtr r);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_result_weak(IntPtr r);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_result_contract_map(IntPtr r);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_result_contracts(IntPtr r);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern void sml_result_free(IntPtr r);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_builtin_clone(IntPtr b);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_builtin_new();
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern void sml_builtin_free(IntPtr b);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern uint sml_builtin_count(IntPtr b);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern int sml_builtin_add_field(IntPtr b, IntPtr contract, IntPtr field,
            IntPtr ty, int required);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern int sml_builtin_set_range(IntPtr b, IntPtr contract, IntPtr field,
            double min, double max);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern int sml_builtin_add_enum(IntPtr b, IntPtr contract, IntPtr field,
            IntPtr value);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_load_file(IntPtr path, uint flags, ref SmlErrorStruct err);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern void sml_free(IntPtr v);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern void sml_free_str(IntPtr p);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern int sml_typeof(IntPtr v);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_get(IntPtr v, IntPtr key);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_at(IntPtr v, UIntPtr idx);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern UIntPtr sml_size(IntPtr v);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_key_at(IntPtr v, UIntPtr idx);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_str_dup(IntPtr v);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern long sml_int_value(IntPtr v);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern double sml_real_value(IntPtr v);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern int sml_bool_value(IntPtr v);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_dumps(IntPtr v, uint flags);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_dump(IntPtr json);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_dump_err(IntPtr json, ref SmlErrorStruct err);
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_version();
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern uint sml_features_mask();
        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_feature_name(uint bit);

        /// <summary>富错误异常（字段与 c/sml_rs.h 的 sml_error 对齐）。</summary>
        public class SmlException : Exception
        {
            public string CodeStr { get; }
            public int Code { get; }
            public int Line { get; }
            public int Column { get; }
            public long Position { get; }
            public string Source { get; }
            public string Text { get; }

            public SmlException(string codeStr, int code, int line, int column,
                                long position, string source, string text)
                : base(codeStr + ": " + text +
                       ((line != 0 || column != 0) ? $" (line {line}, column {column})" : ""))
            {
                CodeStr = codeStr;
                Code = code;
                Line = line;
                Column = column;
                Position = position;
                Source = source;
                Text = text;
            }
        }

        // ---- UTF-8 入参缓冲 ----
        private static IntPtr ToUtf8(string s)
        {
            if (s == null) return IntPtr.Zero;
            return Marshal.StringToCoTaskMemUTF8(s);
        }
        private static void FreeUtf8(IntPtr p)
        {
            if (p != IntPtr.Zero) Marshal.ZeroFreeCoTaskMemUTF8(p);
        }

        private static string ReadUtf8(IntPtr p)
        {
            if (p == IntPtr.Zero) return null;
            return Marshal.PtrToStringUTF8(p);
        }

        // 读走并释放堆副本字符串
        private static string TakeStr(IntPtr p)
        {
            if (p == IntPtr.Zero) return null;
            string s = Marshal.PtrToStringUTF8(p);
            sml_free_str(p);
            return s;
        }

        private static SmlException ErrorFrom(SmlErrorStruct e)
        {
            return new SmlException(
                Field(e.code_str), e.code, e.line, e.column, (long)e.position,
                Field(e.source), Field(e.text));
        }

        private static string Field(byte[] b)
        {
            if (b == null) return "";
            int n = Array.IndexOf(b, (byte)0);
            if (n < 0) n = b.Length;
            return Encoding.UTF8.GetString(b, 0, n);
        }

        private static SmlErrorStruct NewError()
        {
            return new SmlErrorStruct
            {
                source = new byte[128],
                text = new byte[256],
                code_str = new byte[16]
            };
        }

        // ---- 值树物化 ----
        private static object Materialize(IntPtr v)
        {
            if (v == IntPtr.Zero) return null;
            int t = sml_typeof(v);
            switch (t)
            {
                case T_NULL: return null;
                case T_BOOL: return sml_bool_value(v) != 0;
                case T_INT: return sml_int_value(v);
                case T_FLOAT: return sml_real_value(v);
                case T_STR: return TakeStr(sml_str_dup(v));
                case T_ARRAY:
                {
                    ulong n = (ulong)sml_size(v);
                    var arr = new List<object>();
                    for (ulong i = 0; i < n; i++)
                        arr.Add(Materialize(sml_at(v, (UIntPtr)i)));
                    return arr;
                }
                case T_OBJECT:
                {
                    ulong n = (ulong)sml_size(v);
                    var m = new Dictionary<string, object>();
                    for (ulong i = 0; i < n; i++)
                    {
                        IntPtr kp = sml_key_at(v, (UIntPtr)i);
                        string key = TakeStr(kp);
                        if (key == null) continue;
                        IntPtr ck = ToUtf8(key);
                        IntPtr cv = sml_get(v, ck);
                        FreeUtf8(ck);
                        m[key] = Materialize(cv);
                    }
                    return m;
                }
                default: return null;
            }
        }

        public static object Loads(string text, uint flags = 0)
        {
            if (text == null) throw new SmlException("E-INTERNAL-001", 0, 0, 0, 0, "", "Loads: text is null");
            IntPtr ctext = ToUtf8(text);
            var err = NewError();
            try
            {
                IntPtr root = sml_loads(ctext, flags, ref err);
                if (root == IntPtr.Zero) throw ErrorFrom(err);
                try { return Materialize(root); }
                finally { sml_free(root); }
            }
            finally { FreeUtf8(ctext); }
        }

        /// <summary>
        /// 解析文本，并把**非致命诊断**一并交出（P7）。
        /// 与 <see cref="Loads"/> 的差别只有返回值：值树逐字节相同，诊断不阻断解析。
        /// </summary>
        public static object LoadsWithDiags(string text, uint flags, out List<Diagnostic> diagnostics)
        {
            if (text == null) throw new SmlException("E-INTERNAL-001", 0, 0, 0, 0, "", "LoadsWithDiags: text is null");
            IntPtr ctext = ToUtf8(text);
            var err = NewError();
            var dlist = new SmlDiagListStruct();
            try
            {
                IntPtr root = sml_parse_with_diags(ctext, flags, ref dlist, ref err);
                try
                {
                    if (root == IntPtr.Zero) throw ErrorFrom(err);
                    diagnostics = ReadDiags(dlist);
                    return Materialize(root);
                }
                finally
                {
                    sml_diag_list_free(ref dlist);
                    if (root != IntPtr.Zero) sml_free(root);
                }
            }
            finally { FreeUtf8(ctext); }
        }

        private static List<Diagnostic> ReadDiags(SmlDiagListStruct dlist)
        {
            var outList = new List<Diagnostic>();
            ulong n = (ulong)dlist.len;
            int size = Marshal.SizeOf(typeof(SmlDiagStruct));
            for (ulong i = 0; i < n; i++)
            {
                IntPtr at = dlist.items + (int)i * size;
                var d = Marshal.PtrToStructure<SmlDiagStruct>(at);
                string code = d.code_str == IntPtr.Zero ? null : ReadUtf8(d.code_str);
                string msg = d.text == IntPtr.Zero ? "" : ReadUtf8(d.text);
                outList.Add(new Diagnostic(code, msg, d.line, d.severity));
            }
            return outList;
        }

        /// <summary>弱声明族（opt-in，不在基线里）：Flags 是特性上限，要校验弱绑定得显式带上。</summary>
        public const uint F_WEAK = 1u << 19;

        /// <summary>
        /// 解析文本，带完整选项（特性上限 / $env 注入 / 内置契约表）；值树与 Loads 逐字节相同。
        /// </summary>
        public static object LoadsFull(string text, Opts opts, out List<Diagnostic> diagnostics)
        {
            return ParseFullCore(text, opts, false, out diagnostics, out _);
        }

        /// <summary>同 LoadsFull，另交出弱声明三表（`@weakbind` / `@weakdata` / `@weakdoc`）。</summary>
        public static object LoadsFullLists(string text, Opts opts,
            out List<Diagnostic> diagnostics, out WeakLists lists)
        {
            return ParseFullCore(text, opts, true, out diagnostics, out lists);
        }

        private static object ParseFullCore(string text, Opts opts, bool wantLists,
            out List<Diagnostic> diagnostics, out WeakLists lists)
        {
            if (text == null) throw new SmlException("E-INTERNAL-001", 0, 0, 0, 0, "", "LoadsFull: text is null");
            opts ??= new Opts();
            lists = wantLists ? new WeakLists() : null;
            IntPtr ctext = ToUtf8(text);
            var envHandles = new List<IntPtr>();
            GCHandle pin = default(GCHandle);
            try
            {
                var o = new SmlOptsStruct();
                o.struct_size = (UIntPtr)Marshal.SizeOf<SmlOptsStruct>();
                o.flags = opts.Flags;
                foreach (string kv in opts.Env) envHandles.Add(ToUtf8(kv));
                if (envHandles.Count > 0)
                {
                    // 指针数组要 pin 住：交给 C 的是它的地址。
                    IntPtr[] arr = envHandles.ToArray();
                    pin = GCHandle.Alloc(arr, GCHandleType.Pinned);
                    o.env = pin.AddrOfPinnedObject();
                    o.env_len = (UIntPtr)envHandles.Count;
                }
                if (opts.Builtins != null && opts.Builtins.Valid) o.builtins = opts.Builtins.Raw;

                var err = NewError();
                var dlist = new SmlDiagListStruct();
                var wlist = new SmlWeakListsStruct();
                IntPtr root = sml_parse_full_lists(ctext, ref o, ref wlist, ref dlist, ref err);
                try
                {
                    if (root == IntPtr.Zero) throw ErrorFrom(err);
                    diagnostics = ReadDiags(dlist);
                    if (wantLists) lists = ReadWeakLists(wlist);
                    return Materialize(root);
                }
                finally
                {
                    sml_weaklists_free(ref wlist);
                    sml_diag_list_free(ref dlist);
                    if (root != IntPtr.Zero) sml_free(root);
                }
            }
            finally
            {
                if (pin.IsAllocated) pin.Free();
                foreach (IntPtr h in envHandles) FreeUtf8(h);
                FreeUtf8(ctext);
            }
        }

        /// <summary>
        /// 同 <see cref="LoadFile"/>，另交出诊断与弱声明三表（文件入口版的
        /// <see cref="LoadsFullLists"/>）。展开 <c>include</c>，相对路径以文件所在目录为基准。
        /// </summary>
        public static object LoadFileFull(string path, Opts opts,
            out List<Diagnostic> diagnostics, out WeakLists lists)
        {
            if (path == null) throw new SmlException("E-INTERNAL-001", 0, 0, 0, 0, "", "LoadFileFull: path is null");
            opts ??= new Opts();
            IntPtr cpath = ToUtf8(path);
            var envHandles = new List<IntPtr>();
            GCHandle pin = default(GCHandle);
            try
            {
                var o = new SmlOptsStruct();
                o.struct_size = (UIntPtr)Marshal.SizeOf<SmlOptsStruct>();
                o.flags = opts.Flags;
                foreach (string kv in opts.Env) envHandles.Add(ToUtf8(kv));
                if (envHandles.Count > 0)
                {
                    IntPtr[] arr = envHandles.ToArray();
                    pin = GCHandle.Alloc(arr, GCHandleType.Pinned);
                    o.env = pin.AddrOfPinnedObject();
                    o.env_len = (UIntPtr)envHandles.Count;
                }
                if (opts.Builtins != null && opts.Builtins.Valid) o.builtins = opts.Builtins.Raw;

                var err = NewError();
                var dlist = new SmlDiagListStruct();
                var wlist = new SmlWeakListsStruct();
                IntPtr root = sml_load_file_full(cpath, ref o, ref wlist, ref dlist, ref err);
                try
                {
                    if (root == IntPtr.Zero) throw ErrorFrom(err);
                    diagnostics = ReadDiags(dlist);
                    lists = ReadWeakLists(wlist);
                    return Materialize(root);
                }
                finally
                {
                    sml_weaklists_free(ref wlist);
                    sml_diag_list_free(ref dlist);
                    if (root != IntPtr.Zero) sml_free(root);
                }
            }
            finally
            {
                if (pin.IsAllocated) pin.Free();
                foreach (IntPtr h in envHandles) FreeUtf8(h);
                FreeUtf8(cpath);
            }
        }

        /// <summary>
        /// 一次解析拿全部产物：值树 + 诊断 + 多错误 + 弱表 + 契约映射 + 契约表。
        ///
        /// <para>开了 <see cref="WantErrors"/> 后，语法/契约错<b>不再抛异常</b>：值树是
        /// 尽力而为的部分结果，全部错误在 <see cref="ParseAll.Errors"/> 里（severity = 1）。
        /// <see cref="ParseAll.Contracts"/> 是独立克隆的表，可直接传给
        /// <see cref="LoadsFull"/> 的 <see cref="Opts.Builtins"/>。</para>
        /// </summary>
        public static ParseAll ParseAllOf(string text, Opts opts, int want = WantAll)
        {
            if (text == null) throw new SmlException("E-INTERNAL-001", 0, 0, 0, 0, "", "ParseAllOf: text is null");
            opts ??= new Opts();
            IntPtr ctext = ToUtf8(text);
            var envHandles = new List<IntPtr>();
            GCHandle pin = default(GCHandle);
            try
            {
                var o = new SmlOptsStruct();
                o.struct_size = (UIntPtr)Marshal.SizeOf<SmlOptsStruct>();
                o.flags = opts.Flags;
                foreach (string kv in opts.Env) envHandles.Add(ToUtf8(kv));
                if (envHandles.Count > 0)
                {
                    IntPtr[] arr = envHandles.ToArray();
                    pin = GCHandle.Alloc(arr, GCHandleType.Pinned);
                    o.env = pin.AddrOfPinnedObject();
                    o.env_len = (UIntPtr)envHandles.Count;
                }
                if (opts.Builtins != null && opts.Builtins.Valid) o.builtins = opts.Builtins.Raw;

                var err = NewError();
                IntPtr r = sml_parse_all(ctext, ref o, want, ref err);
                if (r == IntPtr.Zero) throw ErrorFrom(err);
                try
                {
                    var res = new ParseAll { Value = Materialize(sml_result_value(r)) };
                    if ((want & WantDiags) != 0)
                        res.Diagnostics = ReadDiags(Marshal.PtrToStructure<SmlDiagListStruct>(sml_result_diags(r)));
                    if ((want & WantErrors) != 0)
                        res.Errors = ReadDiags(Marshal.PtrToStructure<SmlDiagListStruct>(sml_result_errors(r)));
                    if ((want & WantWeak) != 0)
                        res.Weak = ReadWeakLists(Marshal.PtrToStructure<SmlWeakListsStruct>(sml_result_weak(r)));
                    if ((want & WantContractMap) != 0)
                    {
                        var cm = Marshal.PtrToStructure<SmlContractMapStruct>(sml_result_contract_map(r));
                        int size = Marshal.SizeOf(typeof(SmlContractRefStruct));
                        for (ulong i = 0; i < (ulong)cm.len; i++)
                        {
                            var it = Marshal.PtrToStructure<SmlContractRefStruct>(cm.items + (int)i * size);
                            res.ContractMap.Add(new ContractRef(ReadUtf8(it.path), ReadUtf8(it.name)));
                        }
                    }
                    if ((want & WantContracts) != 0)
                        res.Contracts = Builtin.Adopt(sml_builtin_clone(sml_result_contracts(r)));
                    return res;
                }
                finally
                {
                    sml_result_free(r);
                }
            }
            finally
            {
                if (pin.IsAllocated) pin.Free();
                foreach (IntPtr h in envHandles) FreeUtf8(h);
                FreeUtf8(ctext);
            }
        }

        private static WeakLists ReadWeakLists(SmlWeakListsStruct wl)
        {
            var res = new WeakLists();
            int bindSize = Marshal.SizeOf(typeof(SmlWeakBindStruct));
            int dataSize = Marshal.SizeOf(typeof(SmlWeakDataStruct));
            for (ulong i = 0; i < (ulong)wl.n_binds; i++)
            {
                var s = Marshal.PtrToStructure<SmlWeakBindStruct>(wl.binds + (int)i * bindSize);
                res.Binds.Add(new WeakBind(ReadUtf8(s.path), ReadUtf8(s.name), s.line));
            }
            for (ulong i = 0; i < (ulong)wl.n_data; i++)
            {
                var s = Marshal.PtrToStructure<SmlWeakDataStruct>(wl.data + (int)i * dataSize);
                res.Data.Add(new WeakData(ReadUtf8(s.path), ReadUtf8(s.name), s.line,
                    s.value == IntPtr.Zero ? null : Materialize(s.value)));
            }
            for (ulong i = 0; i < (ulong)wl.n_docs; i++)
            {
                // sml_weakdoc 与 sml_weakbind 同布局。
                var s = Marshal.PtrToStructure<SmlWeakBindStruct>(wl.docs + (int)i * bindSize);
                res.Docs.Add(new WeakDoc(ReadUtf8(s.path), ReadUtf8(s.name), s.line));
            }
            return res;
        }

        public static object LoadFile(string path, uint flags = 0)
        {
            if (path == null) throw new SmlException("E-INTERNAL-001", 0, 0, 0, 0, "", "LoadFile: path is null");
            IntPtr cpath = ToUtf8(path);
            var err = NewError();
            try
            {
                IntPtr root = sml_load_file(cpath, flags, ref err);
                if (root == IntPtr.Zero) throw ErrorFrom(err);
                try { return Materialize(root); }
                finally { sml_free(root); }
            }
            finally { FreeUtf8(cpath); }
        }

        public static string Dumps(object obj, uint flags = 0)
        {
            string js = System.Text.Json.JsonSerializer.Serialize(obj);
            IntPtr cjs = ToUtf8(js);
            try
            {
                IntPtr outp = sml_dump(cjs);
                if (outp == IntPtr.Zero)
                {
                    var err = NewError();
                    sml_dump_err(cjs, ref err);
                    throw ErrorFrom(err);
                }
                return TakeStr(outp);
            }
            finally { FreeUtf8(cjs); }
        }

        public static string Version()
        {
            return ReadUtf8(sml_version()); // 静态，不释放
        }

        public static List<string> Features()
        {
            uint mask = sml_features_mask();
            var names = new List<string>();
            for (int b = 0; b < 64 && (1u << b) <= mask; b++)
            {
                IntPtr p = sml_feature_name((uint)b);
                string n = ReadUtf8(p); // 静态，不释放
                if (!string.IsNullOrEmpty(n)) names.Add(n);
            }
            return names;
        }

        /// <summary>在已物化对象上按 "a.b.c" / "a.0" 路径便利取值（宿主侧，不走 ABI）。</summary>
        public static object GetIn(object obj, string path)
        {
            object cur = obj;
            foreach (string seg in path.Split('.'))
            {
                if (cur == null) return null;
                if (cur is Dictionary<string, object> m)
                {
                    m.TryGetValue(seg, out cur);
                }
                else if (cur is List<object> a)
                {
                    if (int.TryParse(seg, out int idx) && idx >= 0 && idx < a.Count)
                        cur = a[idx];
                    else return null;
                }
                else return null;
            }
            return cur;
        }
    }
}
