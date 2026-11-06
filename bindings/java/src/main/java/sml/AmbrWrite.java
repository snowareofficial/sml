// SPDX-License-Identifier: MulanPSL-2.0
// AmbrWrite.java — optional `.ambr` writer for Java (Panama FFM → amberice_ffi).
// AmbrWrite.java —— Java 端可选启用的 `.ambr` 写入器（Panama FFM 调 amberice_ffi）。
//
// Why FFI here while `Ambr` stays pure Java / 为什么读纯 Java、写走 FFI:
//   `Ambr` deliberately has zero native dependencies: reading a `.ambr` must not force a
//   host (Android, sandbox, read-only container) to ship a shared library. Encoding is a
//   different job — a byte-exact canonical encoder is ~850 lines, and `Sml` already loads
//   a cdylib through Panama, so the channel exists.
//   ｜ `Ambr` 刻意零原生依赖：读不该强迫宿主带本地库。编码是另一件事 —— 逐字节的
//   canonical 编码器约 850 行，而 `Sml` 已经用 Panama 加载 cdylib，通道是现成的。
//
//   This is a **separate class**: an app that only reads never touches it.
//   ｜ 独立类：只读的应用完全不会碰到它。
//
// JDK / 版本: Panama FFM (`java.lang.foreign`) is final in JDK 22+. `Sml` already
//   requires it, so this adds no new floor.
//   ｜ 与 `Sml` 同一门槛，不额外提高。
//
// Library path / 库路径: env `AMBR_LIB` (full path) wins; otherwise `amberice_ffi` on
//   the loader path. Build with `cargo build -p amberice-ffi --features sml`.
package sml;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.charset.StandardCharsets;

public final class AmbrWrite {
    private AmbrWrite() {}

    /** Capability bit: decode. ｜ 能力位：解码。 */
    public static final int CAP_READ = 1 << 0;
    /** Capability bit: encode. ｜ 能力位：编码。 */
    public static final int CAP_WRITE = 1 << 1;

    private static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP = loadLookup();

    private static SymbolLookup loadLookup() {
        final String p = System.getenv("AMBR_LIB");
        if (p != null && !p.isEmpty()) {
            return SymbolLookup.libraryLookup(p, Arena.global());
        }
        for (String n : new String[]{"amberice_ffi", "libamberice_ffi"}) {
            try {
                System.loadLibrary(n);
                return SymbolLookup.loaderLookup();
            } catch (UnsatisfiedLinkError ignored) {
                // try the next name / 继续尝试下一个名字
            }
        }
        throw new UnsatisfiedLinkError(
                "cannot load the amberice cdylib (amberice_ffi); set AMBR_LIB to "
                + "amberice_ffi.{dll,so,dylib} / 无法加载 amberice cdylib，请用 AMBR_LIB 指定路径");
    }

    private static MethodHandle h(String name, FunctionDescriptor fd) {
        return LINKER.downcallHandle(LOOKUP.find(name)
                .orElseThrow(() -> new UnsatisfiedLinkError("缺失符号: " + name)), fd);
    }

    private static final MethodHandle
            AMBR_CAPS = h("ambr_caps", FunctionDescriptor.of(ValueLayout.JAVA_INT)),
            AMBR_PACK = h("ambr_pack_text", FunctionDescriptor.of(
                    ValueLayout.ADDRESS,
                    ValueLayout.ADDRESS, ValueLayout.JAVA_LONG,
                    ValueLayout.ADDRESS,
                    ValueLayout.ADDRESS, ValueLayout.JAVA_LONG)),
            AMBR_FREE_BYTES = h("ambr_free_bytes",
                    FunctionDescriptor.ofVoid(ValueLayout.ADDRESS, ValueLayout.JAVA_LONG));

    /** Capability bits of the loaded cdylib. ｜ 已加载 cdylib 的能力位。 */
    public static int capabilities() {
        try {
            return (int) AMBR_CAPS.invokeExact();
        } catch (Throwable t) {
            throw new Ambr.AmbrException("caps call failed / caps 调用失败: " + t);
        }
    }

    /**
     * Pack SML source text into `.ambr` bytes.
     * <p>Objects keep source order; a lone top-level scalar round-trips too.</p>
     * <p>把 SML 源文本打成 <code>.ambr</code> 字节；对象保源序，顶层标量也可往返。</p>
     *
     * @throws Ambr.AmbrException on a read-only build or an encoder error
     */
    public static byte[] packText(String text) {
        if (text == null) throw new Ambr.AmbrException("text is null / text 为 null");
        if ((capabilities() & CAP_WRITE) == 0) {
            throw new Ambr.AmbrException(
                    "this build is read-only — rebuild amberice-ffi with --features sml / "
                    + "本次构建只读：请用 --features sml 重新构建");
        }

        final byte[] utf8 = text.getBytes(StandardCharsets.UTF_8);
        try (Arena arena = Arena.ofConfined()) {
            final MemorySegment src = arena.allocate(Math.max(1, utf8.length));
            if (utf8.length > 0) {
                MemorySegment.copy(utf8, 0, src, ValueLayout.JAVA_BYTE, 0, utf8.length);
            }
            final MemorySegment outLen = arena.allocate(ValueLayout.JAVA_LONG);
            final MemorySegment err = arena.allocate(256);

            final MemorySegment p = (MemorySegment) AMBR_PACK.invokeExact(
                    src, (long) utf8.length, outLen, err, 256L);
            if (p.address() == 0) {
                throw new Ambr.AmbrException(err.getString(0));
            }

            final long n = outLen.get(ValueLayout.JAVA_LONG, 0);
            final byte[] out = p.reinterpret(n).toArray(ValueLayout.JAVA_BYTE);
            AMBR_FREE_BYTES.invokeExact(p, n);
            return out;
        } catch (Ambr.AmbrException e) {
            throw e;
        } catch (Throwable t) {
            throw new Ambr.AmbrException("pack failed / pack 失败: " + t);
        }
    }
}
