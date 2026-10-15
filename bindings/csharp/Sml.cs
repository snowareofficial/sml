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

        private const string LIB = "sml";

        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr sml_loads(IntPtr text, uint flags, ref SmlErrorStruct err);
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
