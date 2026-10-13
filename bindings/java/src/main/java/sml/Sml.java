package sml;

import java.lang.foreign.Arena;
import java.lang.foreign.FunctionDescriptor;
import java.lang.foreign.Linker;
import java.lang.foreign.MemoryLayout;
import java.lang.foreign.MemorySegment;
import java.lang.foreign.SequenceLayout;
import java.lang.foreign.SymbolLookup;
import java.lang.foreign.ValueLayout;
import java.lang.invoke.MethodHandle;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/** SML 宿主绑定（Java / JDK Panama FFM）—— 经 C-ABI 调 Rust cdylib（sml / libsml）。
 *
 * 读路径（loads / load_file / 取值）直接遍历 v3 值树，不经 JSON：类型保真、富错误直接到手。
 * 仅 dumps 按文档走官方桥 obj -> JSON -> sml_dump。ABI 宪法 = c/sml_rs.h。不重写 SML 解析器。
 *
 * 内存纪律（§0.3）：
 *   * sml_loads / sml_load_file 返回的根：物化后 sml_free。
 *   * sml_str_dup / sml_key_at / sml_dumps / sml_dump / sml_version_str 返回的 char*：读后 sml_free_str
 *     （sml_key_at 的键也是堆副本，必须 free）。
 *   * sml_get / sml_at / sml_get_path 返回借用指针：不释放、不跨根释放使用。
 *   * sml_version 返回静态串：只读，不释放。
 */
public final class Sml {

    // 特性位常量（与 c/sml_rs.h 逐位一致）
    public static final int SML_F_BAREWORD_STR = 1 << 0;
    public static final int SML_F_INCLUDE = 1 << 1;
    public static final int SML_F_ENV = 1 << 2;
    public static final int SML_F_CONTRACT = 1 << 3;
    public static final int SML_F_FRAGMENT = 1 << 4;
    public static final int SML_F_TOP_ARRAY = 1 << 5;
    public static final int SML_F_NAMESPACE = 1 << 6;
    public static final int SML_F_IMPLICIT_NS = 1 << 7;
    public static final int SML_F_MULTI_INCLUDE = 1 << 8;
    public static final int SML_F_GLOB_INCLUDE = 1 << 9;
    public static final int SML_F_REGEX_INCLUDE = 1 << 10;
    public static final int SML_F_EXT_REWRITE = 1 << 11;
    public static final int SML_F_BASIC = SML_F_BAREWORD_STR | SML_F_INCLUDE | SML_F_FRAGMENT;

    // 值类型
    private static final int T_NULL = 0, T_BOOL = 1, T_INT = 2, T_FLOAT = 3,
            T_STR = 4, T_ARRAY = 5, T_OBJECT = 6;

    /** sml_error 镜像（逐字节对齐 c/sml_rs.h：int×3 + size_t(8) + char[128/256/16]，共 424）。 */
    private static final MemoryLayout SML_ERROR = MemoryLayout.structLayout(
            ValueLayout.JAVA_INT.withName("code"),
            ValueLayout.JAVA_INT.withName("line"),
            ValueLayout.JAVA_INT.withName("column"),
            MemoryLayout.paddingLayout(4),
            ValueLayout.JAVA_LONG.withName("position"),
            MemoryLayout.sequenceLayout(128, ValueLayout.JAVA_BYTE).withName("source"),
            MemoryLayout.sequenceLayout(256, ValueLayout.JAVA_BYTE).withName("text"),
            MemoryLayout.sequenceLayout(16, ValueLayout.JAVA_BYTE).withName("code_str"));

    private static final Linker LINKER = Linker.nativeLinker();
    private static final SymbolLookup LOOKUP = loadLookup();

    private static SymbolLookup loadLookup() {
        String p = System.getenv("SML_LIB");
        if (p != null && !p.isEmpty()) {
            return SymbolLookup.libraryLookup(p, Arena.global());
        }
        for (String n : new String[]{"sml", "libsml"}) {
            try {
                System.loadLibrary(n);
                return SymbolLookup.loaderLookup();
            } catch (UnsatisfiedLinkError ignored) {
                // 继续尝试下一个名字
            }
        }
        throw new UnsatisfiedLinkError(
                "无法加载 SML cdylib（sml / libsml）。请设置环境变量 SML_LIB 指向 libsml.{dll,so,dylib}");
    }

    private static MethodHandle h(String name, FunctionDescriptor fd) {
        return LINKER.downcallHandle(LOOKUP.find(name)
                .orElseThrow(() -> new UnsatisfiedLinkError("缺失符号: " + name)), fd);
    }

    private static final MethodHandle
            SML_LOADS = h("sml_loads", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_INT, ValueLayout.ADDRESS)),
            SML_LOAD_FILE = h("sml_load_file", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_INT, ValueLayout.ADDRESS)),
            SML_FREE = h("sml_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS)),
            SML_FREE_STR = h("sml_free_str", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS)),
            SML_TYPEOF = h("sml_typeof", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS)),
            SML_GET = h("sml_get", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_AT = h("sml_at", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG)),
            SML_SIZE = h("sml_size", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS)),
            SML_KEY_AT = h("sml_key_at", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_LONG)),
            SML_STR_DUP = h("sml_str_dup", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_INT_VALUE = h("sml_int_value", FunctionDescriptor.of(ValueLayout.JAVA_LONG, ValueLayout.ADDRESS)),
            SML_REAL_VALUE = h("sml_real_value", FunctionDescriptor.of(ValueLayout.JAVA_DOUBLE, ValueLayout.ADDRESS)),
            SML_BOOL_VALUE = h("sml_bool_value", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS)),
            SML_DUMPS = h("sml_dumps", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_INT)),
            SML_DUMP = h("sml_dump", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_DUMP_ERR = h("sml_dump_err", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_VERSION = h("sml_version", FunctionDescriptor.of(ValueLayout.ADDRESS)),
            SML_FEATURES_MASK = h("sml_features_mask", FunctionDescriptor.of(ValueLayout.JAVA_INT)),
            SML_FEATURE_NAME = h("sml_feature_name", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.JAVA_INT));

    // ---- 字符串读 / 释放 ----
    private static String readCString(MemorySegment p) {
        if (p == null || p.equals(MemorySegment.NULL)) return null;
        long cap = 64;
        MemorySegment seg = p.reinterpret(cap);
        long n = 0;
        while (true) {
            if (n >= cap) {
                cap <<= 1;
                seg = p.reinterpret(cap);
            }
            if (seg.get(ValueLayout.JAVA_BYTE, n) == 0) break;
            n++;
        }
        byte[] b = new byte[(int) n];
        MemorySegment.copy(seg, ValueLayout.JAVA_BYTE, 0, b, 0, (int) n);
        return new String(b, StandardCharsets.UTF_8);
    }

    private static String takeStr(MemorySegment p) throws Throwable {
        if (p == null || p.equals(MemorySegment.NULL)) return null;
        String s = readCString(p);
        SML_FREE_STR.invokeExact(p);
        return s;
    }

    private static String cstrAt(MemorySegment base, long off, long len) {
        MemorySegment seg = base.asSlice(off, len);
        long n = 0;
        while (n < len && seg.get(ValueLayout.JAVA_BYTE, n) != 0) n++;
        byte[] b = new byte[(int) n];
        MemorySegment.copy(seg, ValueLayout.JAVA_BYTE, 0, b, 0, (int) n);
        return new String(b, StandardCharsets.UTF_8);
    }

    public static final class SmlError extends RuntimeException {
        public final String codeStr;
        public final int code, line, column;
        public final long position;
        public final String source, text;

        public SmlError(String codeStr, int code, int line, int column, long position,
                       String source, String text) {
            super(codeStr + ": " + text + ((line != 0 || column != 0) ? " (line " + line + ", column " + column + ")" : ""));
            this.codeStr = codeStr;
            this.code = code;
            this.line = line;
            this.column = column;
            this.position = position;
            this.source = source;
            this.text = text;
        }
    }

    private static SmlError errorFrom(MemorySegment err) {
        int code = err.get(ValueLayout.JAVA_INT, 0);
        int line = err.get(ValueLayout.JAVA_INT, 4);
        int column = err.get(ValueLayout.JAVA_INT, 8);
        long position = err.get(ValueLayout.JAVA_LONG, 16);
        String source = cstrAt(err, 24, 128);
        String text = cstrAt(err, 152, 256);
        String codeStr = cstrAt(err, 408, 16);
        return new SmlError(codeStr, code, line, column, position, source, text);
    }

    // ---- 值树物化 ----
    private static Object materialize(MemorySegment v, Arena arena) throws Throwable {
        int t = (int) SML_TYPEOF.invokeExact(v);
        switch (t) {
            case T_NULL:
                return null;
            case T_BOOL:
                return (int) SML_BOOL_VALUE.invokeExact(v) != 0;
            case T_INT:
                return (long) SML_INT_VALUE.invokeExact(v);
            case T_FLOAT:
                return (double) SML_REAL_VALUE.invokeExact(v);
            case T_STR: {
                MemorySegment p = (MemorySegment) SML_STR_DUP.invokeExact(v);
                return takeStr(p);
            }
            case T_ARRAY: {
                long n = (long) SML_SIZE.invokeExact(v);
                List<Object> a = new ArrayList<>();
                for (long i = 0; i < n; i++) {
                    MemorySegment e = (MemorySegment) SML_AT.invokeExact(v, i);
                    a.add(materialize(e, arena));
                }
                return a;
            }
            case T_OBJECT: {
                long n = (long) SML_SIZE.invokeExact(v);
                Map<String, Object> m = new LinkedHashMap<>();
                for (long i = 0; i < n; i++) {
                    MemorySegment kp = (MemorySegment) SML_KEY_AT.invokeExact(v, i);
                    String key = takeStr(kp);
                    if (key == null) continue;
                    MemorySegment kv = (MemorySegment) SML_GET.invokeExact(v, arena.allocateFrom(key));
                    m.put(key, materialize(kv, arena));
                }
                return m;
            }
            default:
                return null;
        }
    }

    public static Object loads(String text) throws SmlError {
        return loads(text, 0);
    }

    public static Object loads(String text, int flags) throws SmlError {
        if (text == null) throw new SmlError("E-INTERNAL-001", 0, 0, 0, 0, "", "loads: text is null");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment textSeg = arena.allocateFrom(text);
            MemorySegment err = arena.allocate(SML_ERROR);
            err.fill((byte) 0);
            MemorySegment root = (MemorySegment) SML_LOADS.invokeExact(textSeg, flags, err);
            if (root == null || root.equals(MemorySegment.NULL)) {
                throw errorFrom(err);
            }
            try {
                return materialize(root, arena);
            } finally {
                SML_FREE.invokeExact(root);
            }
        } catch (SmlError e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    public static Object loadFile(String path) throws SmlError {
        return loadFile(path, 0);
    }

    public static Object loadFile(String path, int flags) throws SmlError {
        if (path == null) throw new SmlError("E-INTERNAL-001", 0, 0, 0, 0, "", "loadFile: path is null");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment pathSeg = arena.allocateFrom(path);
            MemorySegment err = arena.allocate(SML_ERROR);
            err.fill((byte) 0);
            MemorySegment root = (MemorySegment) SML_LOAD_FILE.invokeExact(pathSeg, flags, err);
            if (root == null || root.equals(MemorySegment.NULL)) {
                throw errorFrom(err);
            }
            try {
                return materialize(root, arena);
            } finally {
                SML_FREE.invokeExact(root);
            }
        } catch (SmlError e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    public static String dumps(Object obj) throws SmlError {
        return dumps(obj, 0);
    }

    public static String dumps(Object obj, int flags) throws SmlError {
        try (Arena arena = Arena.ofConfined()) {
            String js = Json.stringify(obj);
            MemorySegment jsonSeg = arena.allocateFrom(js);
            MemorySegment out = (MemorySegment) SML_DUMP.invokeExact(jsonSeg);
            if (out == null || out.equals(MemorySegment.NULL)) {
                MemorySegment err = arena.allocate(SML_ERROR);
                err.fill((byte) 0);
                SML_DUMP_ERR.invokeExact(jsonSeg, err);
                throw errorFrom(err);
            }
            return takeStr(out);
        } catch (SmlError e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    public static String version() {
        try {
            MemorySegment p = (MemorySegment) SML_VERSION.invokeExact();
            return readCString(p); // 静态，不释放
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    public static List<String> features() {
        try {
            int mask = (int) SML_FEATURES_MASK.invokeExact();
            List<String> names = new ArrayList<>();
            for (int b = 0; b < 64 && (1 << b) <= mask; b++) {
                MemorySegment p = (MemorySegment) SML_FEATURE_NAME.invokeExact(b);
                String n = readCString(p); // 静态，不释放
                if (n != null && !n.isEmpty()) names.add(n);
            }
            return names;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /** 在已物化对象上按 'a.b.c' / 'a.0' 路径便利取值（宿主侧，不走 ABI）。 */
    @SuppressWarnings("unchecked")
    public static Object getIn(Object obj, String path) {
        Object cur = obj;
        for (String seg : path.split("\\.")) {
            if (cur == null) return null;
            if (cur instanceof Map) {
                cur = ((Map<String, Object>) cur).get(seg);
            } else if (cur instanceof List) {
                try {
                    int idx = Integer.parseInt(seg);
                    cur = ((List<Object>) cur).get(idx);
                } catch (NumberFormatException | IndexOutOfBoundsException e) {
                    return null;
                }
            } else {
                return null;
            }
        }
        return cur;
    }
}
