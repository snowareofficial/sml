// SPDX-License-Identifier: MulanPSL-2.0
// `.ambr` reader for C# (P/Invoke → amberice-ffi cdylib).
// `.ambr` 读取（C# / P/Invoke → amberice-ffi cdylib）。
// Repository / 仓库：https://gitee.com/snoware/sml

using System;
using System.IO;
using System.Runtime.InteropServices;

namespace Amberice
{
    /// <summary>
    /// Reader for `.ambr`, the general-purpose binary envelope.
    /// <para>`.ambr`（通用二进制信封）读取器。</para>
    ///
    /// <para>
    /// A separate namespace on purpose: `.ambr` never depended on SML's parser, and
    /// `amberice` is its own product line. Referencing this type must not drag in `Sml`.
    /// </para>
    /// <para>
    /// 刻意独立命名空间：`.ambr` 从不依赖 SML 解析器，`amberice` 是独立产品线；
    /// 引用本类型不应拖进 `Sml`。
    /// </para>
    ///
    /// <para>
    /// Exposes only "bytes in, canonical text out" — the two symbols of `amberice-ffi`.
    /// Materialise a value tree with <c>Sml.Loads(Amberice.Container.LoadText(bytes))</c>.
    /// </para>
    /// <para>
    /// 只做「字节进、canonical 文本出」，即 `amberice-ffi` 的两个符号；
    /// 要值树用 <c>Sml.Loads(Amberice.Container.LoadText(bytes))</c>。
    /// </para>
    ///
    /// <para>
    /// Library: <c>amberice_ffi</c>, <b>not</b> <c>amberice</c> — the Rust crate is a pure
    /// rlib with no C ABI. Build with <c>cargo build -p amberice-ffi</c>; at run time put
    /// the artifact on <c>PATH</c>.
    /// </para>
    /// <para>
    /// 库名是 <c>amberice_ffi</c>，不是 <c>amberice</c>（Rust crate 是纯 rlib，没有 C ABI）。
    /// 构建 <c>cargo build -p amberice-ffi</c>；运行期把产物放进 <c>PATH</c>。
    /// </para>
    ///
    /// <para>
    /// Messages are not translated here: <c>swi18n</c> is not built yet, and a per-binding
    /// copy becomes a second, drifting wording. Error text is passed through verbatim from
    /// Rust (prefixed <c>AMBR:</c>).
    /// </para>
    /// <para>
    /// 文案不在绑定层翻译：<c>swi18n</c> 尚未建立，各端自造会变成第二份、会漂移的文案；
    /// 错误串原样透传 Rust（以 <c>AMBR:</c> 开头）。
    /// </para>
    /// </summary>
    public static class Container
    {
        private const string LIB = "amberice_ffi";
        private const int ErrSize = 256;

        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr ambr_text(byte[] buf, UIntPtr len, byte[] err, UIntPtr errsz);

        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern void ambr_free_str(IntPtr p);

        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr ambr_pack_text(byte[] text, UIntPtr len, out UIntPtr outLen, byte[] err, UIntPtr errsz);

        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern void ambr_free_bytes(IntPtr p, UIntPtr len);

        [DllImport(LIB, CallingConvention = CallingConvention.Cdecl)]
        private static extern uint ambr_caps();

        /// <summary>
        /// Capability bits of the loaded cdylib.
        /// <para>已加载 cdylib 的能力位。</para>
        /// </summary>
        [Flags]
        public enum Capability
        {
            None = 0,
            /// <summary>Decoding is available. ｜ 可解码。</summary>
            Read = 1 << 0,
            /// <summary>Encoding is available (built with <c>--features sml</c>). ｜ 可编码。</summary>
            Write = 1 << 1,
        }

        /// <summary>
        /// Query this build instead of assuming a symbol exists. A read-only cdylib simply
        /// does not export the write symbols.
        /// <para>查询本次构建，而不是假定符号存在；只读 cdylib 不导出写符号。</para>
        /// </summary>
        public static Capability Capabilities
        {
            get
            {
                try { return (Capability)ambr_caps(); }
                catch (EntryPointNotFoundException) { return Capability.Read; }
            }
        }

        /// <summary>
        /// Decoding failed; the message starts with <c>AMBR:</c> and comes from Rust.
        /// <para>解码失败；消息以 <c>AMBR:</c> 开头，来自 Rust 侧。</para>
        /// </summary>
        public sealed class AmbericeException : System.Exception
        {
            public AmbericeException(string message) : base(message) { }
        }

        /// <summary>
        /// Decode `.ambr` bytes into canonical text (UTF-8).
        /// <para>把 <c>.ambr</c> 字节解成 canonical 文本（UTF-8）。</para>
        /// </summary>
        public static string LoadText(byte[] data)
        {
            if (data == null) throw new ArgumentNullException(nameof(data));
            if (data.Length == 0) throw new AmbericeException("AMBR: [E-AMBR-001] empty input");

            var err = new byte[ErrSize];
            IntPtr ptr = ambr_text(data, (UIntPtr)data.Length, err, (UIntPtr)err.Length);
            if (ptr == IntPtr.Zero)
                throw new AmbericeException(ReadErr(err));

            try
            {
                return Marshal.PtrToStringUTF8(ptr) ?? string.Empty;
            }
            finally
            {
                ambr_free_str(ptr);
            }
        }

        /// <summary>
        /// Decode a `.ambr` file.
        /// <para>解一个 <c>.ambr</c> 文件。</para>
        /// </summary>
        public static string LoadTextFile(string path) => LoadText(File.ReadAllBytes(path));

        /// <summary>
        /// Decode from any stream (network, embedded resource, ZIP entry).
        /// The stream is read to the end and is not disposed.
        /// <para>从任意流解码（网络、内嵌资源、ZIP 条目）；读到结尾，不负责释放。</para>
        /// </summary>
        public static string Load(Stream input)
        {
            if (input == null) throw new ArgumentNullException(nameof(input));
            using (var buffer = new MemoryStream())
            {
                input.CopyTo(buffer);
                return LoadText(buffer.ToArray());
            }
        }

        /// <summary>
        /// Pack SML source text into `.ambr` bytes. Requires the cdylib built with
        /// <c>--features sml</c> (the write path needs a parser; the read path does not).
        /// <para>把 SML 源文本打成 <c>.ambr</c> 字节。需要带 <c>--features sml</c> 构建的 cdylib
        /// —— 写路径需要解析器，读路径不需要。</para>
        ///
        /// <para>
        /// Objects keep source order: canonical bytes require it. Round-trip is
        /// <c>LoadText(PackText(text)) == canonical form of text</c>.
        /// </para>
        /// <para>
        /// 对象保留源序（canonical 字节的前提）。往返判据是
        /// <c>LoadText(PackText(text)) == text 的 canonical 形式</c>。
        /// </para>
        /// </summary>
        public static byte[] PackText(string text)
        {
            if (text == null) throw new ArgumentNullException(nameof(text));
            if ((Capabilities & Capability.Write) == 0)
                throw new AmbericeException(
                    "AMBR: [E-AMBR-003] this build is read-only — rebuild amberice-ffi with --features sml / " +
                    "本次构建只读：请用 --features sml 重新构建 amberice-ffi");

            byte[] utf8 = System.Text.Encoding.UTF8.GetBytes(text);
            if (utf8.Length == 0) throw new AmbericeException("AMBR: [E-AMBR-001] empty input");

            var err = new byte[ErrSize];
            UIntPtr outLen;
            IntPtr ptr = ambr_pack_text(utf8, (UIntPtr)utf8.Length, out outLen, err, (UIntPtr)err.Length);
            if (ptr == IntPtr.Zero)
                throw new AmbericeException(ReadErr(err));

            try
            {
                int n = checked((int)outLen.ToUInt64());
                var bytes = new byte[n];
                Marshal.Copy(ptr, bytes, 0, n);
                return bytes;
            }
            finally
            {
                ambr_free_bytes(ptr, outLen);
            }
        }

        private static string ReadErr(byte[] err)
        {
            int n = Array.IndexOf(err, (byte)0);
            if (n < 0) n = err.Length;
            string msg = System.Text.Encoding.UTF8.GetString(err, 0, n);
            return string.IsNullOrEmpty(msg) ? "AMBR: [E-AMBR-008] decode failed (no message)" : msg;
        }
    }
}
