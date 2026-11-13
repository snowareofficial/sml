// SPDX-License-Identifier: MulanPSL-2.0
// SML 多语言宿主绑定（Java / JDK Panama FFM）。
// 仓库：https://gitee.com/snoware/sml
// @version 0.1.0 (swsml 0.6.3)

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

    /** sml_diag 镜像：address code_str / address text / int line / int severity（8+8+4+4 = 24，无尾部填充）。 */
    private static final MemoryLayout SML_DIAG = MemoryLayout.structLayout(
            ValueLayout.ADDRESS.withName("code_str"),
            ValueLayout.ADDRESS.withName("text"),
            ValueLayout.JAVA_INT.withName("line"),
            ValueLayout.JAVA_INT.withName("severity"));

    /** sml_diag_list 镜像：address items / long len（16）。 */
    private static final MemoryLayout SML_DIAG_LIST = MemoryLayout.structLayout(
            ValueLayout.ADDRESS.withName("items"),
            ValueLayout.JAVA_LONG.withName("len"));

    /** sml_opts 镜像：long struct_size / int flags / 4 pad / addr env / long env_len / addr builtins（40）。 */
    private static final MemoryLayout SML_OPTS = MemoryLayout.structLayout(
            ValueLayout.JAVA_LONG.withName("struct_size"),
            ValueLayout.JAVA_INT.withName("flags"),
            MemoryLayout.paddingLayout(4),
            ValueLayout.ADDRESS.withName("env"),
            ValueLayout.JAVA_LONG.withName("env_len"),
            ValueLayout.ADDRESS.withName("builtins"));

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
            SML_PARSE_WITH_DIAGS = h("sml_parse_with_diags", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_DIAG_LIST_FREE = h("sml_diag_list_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS)),
            SML_BUILTIN_NEW = h("sml_builtin_new", FunctionDescriptor.of(ValueLayout.ADDRESS)),
            SML_BUILTIN_FREE = h("sml_builtin_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS)),
            SML_BUILTIN_COUNT = h("sml_builtin_count", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS)),
            SML_BUILTIN_ADD_FIELD = h("sml_builtin_add_field", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_INT)),
            SML_BUILTIN_SET_RANGE = h("sml_builtin_set_range", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_DOUBLE, ValueLayout.JAVA_DOUBLE)),
            SML_BUILTIN_ADD_ENUM = h("sml_builtin_add_enum", FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_PARSE_FULL = h("sml_parse_full", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_PARSE_FULL_LISTS = h("sml_parse_full_lists", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_WEAKLISTS_FREE = h("sml_weaklists_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS)),
            SML_PARSE_ALL = h("sml_parse_all", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.JAVA_INT, ValueLayout.ADDRESS)),
            SML_LOAD_FILE_FULL = h("sml_load_file_full", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_RESULT_VALUE = h("sml_result_value", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_RESULT_DIAGS = h("sml_result_diags", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_RESULT_ERRORS = h("sml_result_errors", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_RESULT_WEAK = h("sml_result_weak", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_RESULT_CONTRACT_MAP = h("sml_result_contract_map", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_RESULT_CONTRACTS = h("sml_result_contracts", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
            SML_RESULT_FREE = h("sml_result_free", FunctionDescriptor.ofVoid(ValueLayout.ADDRESS)),
            SML_BUILTIN_CLONE = h("sml_builtin_clone", FunctionDescriptor.of(ValueLayout.ADDRESS, ValueLayout.ADDRESS)),
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

    /** 一条非致命诊断（键冲突 / 片段覆盖 / 弱声明不匹配）。 */
    public static final class Diagnostic {
        public final String codeStr; // null = 该档暂无稳定码
        public final String text;
        public final int line;       // 1 起；0 = 未知
        public final int severity;   // 0 = warning，1 = error（预留）

        public Diagnostic(String codeStr, String text, int line, int severity) {
            this.codeStr = codeStr;
            this.text = text;
            this.line = line;
            this.severity = severity;
        }

        @Override
        public String toString() {
            String loc = line > 0 ? " @" + line : "";
            return (codeStr == null ? "<no code>" : codeStr) + loc + ": " + text;
        }
    }

    /** loadsWithDiags / loadsFull 的结果：值树 + 非致命诊断（+ 可选弱表）。 */
    public static final class Parsed {
        public final Object value;
        public final List<Diagnostic> diagnostics;
        /** 仅 `loadsFullLists` 填充；其余入口为 null。 */
        public final WeakLists lists;

        public Parsed(Object value, List<Diagnostic> diagnostics) {
            this(value, diagnostics, null);
        }

        public Parsed(Object value, List<Diagnostic> diagnostics, WeakLists lists) {
            this.value = value;
            this.diagnostics = diagnostics;
            this.lists = lists;
        }
    }

    /** 把选项写进一块本地 `sml_opts`；arena 负责放开临时内存。 */
    private static MemorySegment encodeOpts(Opts opts, Arena arena) {
        MemorySegment optsSeg = arena.allocate(SML_OPTS);
        optsSeg.fill((byte) 0);
        optsSeg.set(ValueLayout.JAVA_LONG, 0, SML_OPTS.byteSize());
        optsSeg.set(ValueLayout.JAVA_INT, 8, opts.flags);
        if (!opts.env.isEmpty()) {
            // char*[n]：按字节数分配再逐项写指针（不依赖 allocateArray 的预览期签名）。
            MemorySegment arr = arena.allocate(ValueLayout.ADDRESS.byteSize() * opts.env.size());
            for (int i = 0; i < opts.env.size(); i++) {
                arr.setAtIndex(ValueLayout.ADDRESS, i, arena.allocateFrom(opts.env.get(i)));
            }
            optsSeg.set(ValueLayout.ADDRESS, 16, arr);
        }
        optsSeg.set(ValueLayout.JAVA_LONG, 24, opts.env.size());
        if (opts.builtins != null && opts.builtins.valid()) {
            optsSeg.set(ValueLayout.ADDRESS, 32, opts.builtins.raw());
        }
        return optsSeg;
    }

    /** 把 C 侧三表拷成 Java 对象（拷完即可整体释放）。 */
    private static WeakLists readWeakLists(MemorySegment wl, Arena arena) throws Throwable {
        List<WeakBind> binds = new ArrayList<>();
        List<WeakData> data = new ArrayList<>();
        List<WeakDoc> docs = new ArrayList<>();

        MemorySegment bp = wl.get(ValueLayout.ADDRESS, 0);
        long nb = wl.get(ValueLayout.JAVA_LONG, 8);
        if (nb > 0 && !bp.equals(MemorySegment.NULL)) {
            MemorySegment base = bp.reinterpret(nb * SML_WEAK_BIND.byteSize());
            for (long i = 0; i < nb; i++) {
                long off = i * SML_WEAK_BIND.byteSize();
                binds.add(new WeakBind(readCString(base.get(ValueLayout.ADDRESS, off)),
                        readCString(base.get(ValueLayout.ADDRESS, off + 8)),
                        base.get(ValueLayout.JAVA_INT, off + 16)));
            }
        }

        MemorySegment dp = wl.get(ValueLayout.ADDRESS, 16);
        long nd = wl.get(ValueLayout.JAVA_LONG, 24);
        if (nd > 0 && !dp.equals(MemorySegment.NULL)) {
            MemorySegment base = dp.reinterpret(nd * SML_WEAK_DATA.byteSize());
            for (long i = 0; i < nd; i++) {
                long off = i * SML_WEAK_DATA.byteSize();
                MemorySegment v = base.get(ValueLayout.ADDRESS, off + 24);
                data.add(new WeakData(readCString(base.get(ValueLayout.ADDRESS, off)),
                        readCString(base.get(ValueLayout.ADDRESS, off + 8)),
                        base.get(ValueLayout.JAVA_INT, off + 16),
                        v.equals(MemorySegment.NULL) ? null : materialize(v, arena)));
            }
        }

        MemorySegment pp = wl.get(ValueLayout.ADDRESS, 32);
        long np = wl.get(ValueLayout.JAVA_LONG, 40);
        if (np > 0 && !pp.equals(MemorySegment.NULL)) {
            MemorySegment base = pp.reinterpret(np * SML_WEAK_BIND.byteSize());
            for (long i = 0; i < np; i++) {
                long off = i * SML_WEAK_BIND.byteSize();
                docs.add(new WeakDoc(readCString(base.get(ValueLayout.ADDRESS, off)),
                        readCString(base.get(ValueLayout.ADDRESS, off + 8)),
                        base.get(ValueLayout.JAVA_INT, off + 16)));
            }
        }

        return new WeakLists(binds, data, docs);
    }

    private static List<Diagnostic> readDiags(MemorySegment dl) {
        MemorySegment items = dl.get(ValueLayout.ADDRESS, 0);
        long len = dl.get(ValueLayout.JAVA_LONG, 8);
        List<Diagnostic> out = new ArrayList<>();
        if (len <= 0 || items.equals(MemorySegment.NULL)) return out;
        MemorySegment base = items.reinterpret(len * SML_DIAG.byteSize());
        long step = SML_DIAG.byteSize();
        for (long i = 0; i < len; i++) {
            long off = i * step;
            MemorySegment codeP = base.get(ValueLayout.ADDRESS, off);
            MemorySegment textP = base.get(ValueLayout.ADDRESS, off + 8);
            int line = base.get(ValueLayout.JAVA_INT, off + 16);
            int sev = base.get(ValueLayout.JAVA_INT, off + 20);
            String code = readCString(codeP);
            String msg = readCString(textP);
            out.add(new Diagnostic(code, msg == null ? "" : msg, line, sev));
        }
        return out;
    }

    public static Parsed loadsWithDiags(String text) throws SmlError {
        return loadsWithDiags(text, 0);
    }

    /** 解析文本并把**非致命诊断**一并交出；值树与 loads 逐字节相同。 */
    public static Parsed loadsWithDiags(String text, int flags) throws SmlError {
        if (text == null) throw new SmlError("E-INTERNAL-001", 0, 0, 0, 0, "", "loadsWithDiags: text is null");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment textSeg = arena.allocateFrom(text);
            MemorySegment err = arena.allocate(SML_ERROR);
            err.fill((byte) 0);
            MemorySegment dl = arena.allocate(SML_DIAG_LIST);
            dl.fill((byte) 0);
            MemorySegment root = (MemorySegment) SML_PARSE_WITH_DIAGS.invokeExact(textSeg, flags, dl, err);
            try {
                if (root == null || root.equals(MemorySegment.NULL)) {
                    throw errorFrom(err);
                }
                return new Parsed(materialize(root, arena), readDiags(dl));
            } finally {
                SML_DIAG_LIST_FREE.invokeExact(dl);
                if (root != null && !root.equals(MemorySegment.NULL)) {
                    SML_FREE.invokeExact(root);
                }
            }
        } catch (SmlError e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /** 弱声明族（opt-in，不在基线里）：flags 是**特性上限**，要校验弱绑定得显式带上。 */
    public static final int SML_F_WEAK = 1 << 19;

    /** 内置契约表（`@weakbind` 软校验用）；空表 ⇒ 软校验整段跳过。 */
    public static final class Builtin implements AutoCloseable {
        private MemorySegment p;

        private Builtin(MemorySegment p) {
            this.p = p;
        }

        public static Builtin create() {
            try {
                MemorySegment raw = (MemorySegment) SML_BUILTIN_NEW.invokeExact();
                return new Builtin(MemorySegment.ofAddress(raw.address()));
            } catch (Throwable t) {
                throw new RuntimeException(t);
            }
        }

        /** 接管一个已归本进程所有的 `sml_builtin*`（如 `sml_builtin_clone` 的产物）。 */
        static Builtin adopt(MemorySegment raw) {
            return new Builtin(MemorySegment.ofAddress(raw.address()));
        }

        public boolean valid() {
            return p != null && !p.equals(MemorySegment.NULL);
        }

        public int size() {
            if (!valid()) return 0;
            try {
                return (int) SML_BUILTIN_COUNT.invokeExact(p);
            } catch (Throwable t) {
                throw new RuntimeException(t);
            }
        }

        /** ty 取 any / str / int / num / bool / enum；false = 参数非法（表不动）。 */
        public boolean addField(String contract, String field, String ty, boolean required) {
            if (!valid()) return false;
            try (Arena arena = Arena.ofConfined()) {
                int rc = (int) SML_BUILTIN_ADD_FIELD.invokeExact(p, arena.allocateFrom(contract),
                        arena.allocateFrom(field), arena.allocateFrom(ty), required ? 1 : 0);
                return rc == 0;
            } catch (Throwable t) {
                throw new RuntimeException(t);
            }
        }

        public boolean setRange(String contract, String field, double min, double max) {
            if (!valid()) return false;
            try (Arena arena = Arena.ofConfined()) {
                int rc = (int) SML_BUILTIN_SET_RANGE.invokeExact(p, arena.allocateFrom(contract),
                        arena.allocateFrom(field), min, max);
                return rc == 0;
            } catch (Throwable t) {
                throw new RuntimeException(t);
            }
        }

        public boolean addEnum(String contract, String field, String value) {
            if (!valid()) return false;
            try (Arena arena = Arena.ofConfined()) {
                int rc = (int) SML_BUILTIN_ADD_ENUM.invokeExact(p, arena.allocateFrom(contract),
                        arena.allocateFrom(field), arena.allocateFrom(value));
                return rc == 0;
            } catch (Throwable t) {
                throw new RuntimeException(t);
            }
        }

        MemorySegment raw() {
            return p;
        }

        @Override
        public void close() {
            if (!valid()) return;
            try {
                SML_BUILTIN_FREE.invokeExact(p);
            } catch (Throwable t) {
                throw new RuntimeException(t);
            }
            p = null;
        }
    }

    /** sml_weakbind 镜像：address path / address name / int line / 4 pad（24）。 */
    private static final MemoryLayout SML_WEAK_BIND = MemoryLayout.structLayout(
            ValueLayout.ADDRESS.withName("path"),
            ValueLayout.ADDRESS.withName("name"),
            ValueLayout.JAVA_INT.withName("line"),
            MemoryLayout.paddingLayout(4));

    /** sml_weakdata 镜像：path / name / int line / 4 pad / address value（32）。 */
    private static final MemoryLayout SML_WEAK_DATA = MemoryLayout.structLayout(
            ValueLayout.ADDRESS.withName("path"),
            ValueLayout.ADDRESS.withName("name"),
            ValueLayout.JAVA_INT.withName("line"),
            MemoryLayout.paddingLayout(4),
            ValueLayout.ADDRESS.withName("value"));

    /** sml_weaklists 镜像：3 对 (指针, long)（48）。 */
    private static final MemoryLayout SML_WEAK_LISTS = MemoryLayout.structLayout(
            ValueLayout.ADDRESS.withName("binds"),
            ValueLayout.JAVA_LONG.withName("n_binds"),
            ValueLayout.ADDRESS.withName("data"),
            ValueLayout.JAVA_LONG.withName("n_data"),
            ValueLayout.ADDRESS.withName("docs"),
            ValueLayout.JAVA_LONG.withName("n_docs"));

    /** sml_contract_ref 镜像：address path / address name（16）。 */
    private static final MemoryLayout SML_CONTRACT_REF = MemoryLayout.structLayout(
            ValueLayout.ADDRESS.withName("path"),
            ValueLayout.ADDRESS.withName("name"));

    /** sml_contractmap 镜像：address items / long len（16）。 */
    private static final MemoryLayout SML_CONTRACT_MAP = MemoryLayout.structLayout(
            ValueLayout.ADDRESS.withName("items"),
            ValueLayout.JAVA_LONG.withName("len"));

    /** 一条弱绑定（`@weakbind`）：目标块路径 + 契约名。 */
    public static final class WeakBind {
        public final String path;
        public final String name;
        public final int line;

        public WeakBind(String path, String name, int line) {
            this.path = path;
            this.name = name;
            this.line = line;
        }
    }

    /** 一条弱数据（`@weakdata`）：载荷**不在主树里**，已物化成原生对象。 */
    public static final class WeakData {
        public final String path;
        public final String name;
        public final int line;
        public final Object value;

        public WeakData(String path, String name, int line, Object value) {
            this.path = path;
            this.name = name;
            this.line = line;
            this.value = value;
        }
    }

    /** 一条弱 profile 引用（`@weakdoc`）：只有名字。 */
    public static final class WeakDoc {
        public final String path;
        public final String name;
        public final int line;

        public WeakDoc(String path, String name, int line) {
            this.path = path;
            this.name = name;
            this.line = line;
        }
    }

    /** 弱声明三表；三者都不进主树。 */
    public static final class WeakLists {
        public final List<WeakBind> binds;
        public final List<WeakData> data;
        public final List<WeakDoc> docs;

        public WeakLists(List<WeakBind> binds, List<WeakData> data, List<WeakDoc> docs) {
            this.binds = binds;
            this.data = data;
            this.docs = docs;
        }
    }

    /** `loadsFull` 的选项：特性上限 + `$env` 覆盖表 + 内置契约表。 */
    public static final class Opts {
        /** 特性上限位（`SML_F_*`）；0 = 基线，不含 `type.weak`。 */
        public int flags = 0;
        /** `"KEY=VALUE"` 数组，供 `$env.X` 内联；只作用于本次解析。 */
        public List<String> env = new ArrayList<>();
        /** 内置契约表；null = 不做弱绑定软校验。 */
        public Builtin builtins = null;
    }

    public static Parsed loadsFull(String text) throws SmlError {
        return loadsFull(text, new Opts());
    }

    /** 解析文本，带完整选项（特性上限 / `$env` 注入 / 内置契约表）；值树与 loads 逐字节相同。 */
    public static Parsed loadsFull(String text, Opts opts) throws SmlError {
        if (text == null) throw new SmlError("E-INTERNAL-001", 0, 0, 0, 0, "", "loadsFull: text is null");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment textSeg = arena.allocateFrom(text);
            MemorySegment optsSeg = encodeOpts(opts, arena);
            MemorySegment err = arena.allocate(SML_ERROR);
            err.fill((byte) 0);
            MemorySegment dl = arena.allocate(SML_DIAG_LIST);
            dl.fill((byte) 0);
            MemorySegment root = (MemorySegment) SML_PARSE_FULL.invokeExact(textSeg, optsSeg, dl, err);
            try {
                if (root == null || root.equals(MemorySegment.NULL)) {
                    throw errorFrom(err);
                }
                return new Parsed(materialize(root, arena), readDiags(dl));
            } finally {
                SML_DIAG_LIST_FREE.invokeExact(dl);
                if (root != null && !root.equals(MemorySegment.NULL)) {
                    SML_FREE.invokeExact(root);
                }
            }
        } catch (SmlError e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /** `parseAll` 的 want 位（与 `c/sml_rs.h` 的 `SML_WANT_*` 逐位一致）。 */
    public static final int WANT_DIAGS = 1 << 0;
    public static final int WANT_ERRORS = 1 << 1;
    public static final int WANT_WEAK = 1 << 2;
    public static final int WANT_CONTRACT_MAP = 1 << 3;
    public static final int WANT_CONTRACTS = 1 << 4;
    public static final int WANT_ALL = 0x1F;

    /** 一条「某实体应用了某个契约」。 */
    public static final class ContractRef {
        public final String path;
        public final String name;

        public ContractRef(String path, String name) {
            this.path = path;
            this.name = name;
        }

        @Override
        public String toString() {
            return path + " -> " + name;
        }
    }

    /** `parseAll` 的全部产物；未请求的部分是空表 / null。 */
    public static final class ParseAll {
        public final Object value;
        public final List<Diagnostic> diagnostics;
        public final List<Diagnostic> errors;
        public final WeakLists weak;
        public final List<ContractRef> contractMap;
        /** 独立克隆的契约表（调用方负责 close）；未请求时为 null。 */
        public final Builtin contracts;

        public ParseAll(Object value, List<Diagnostic> diagnostics, List<Diagnostic> errors,
                        WeakLists weak, List<ContractRef> contractMap, Builtin contracts) {
            this.value = value;
            this.diagnostics = diagnostics;
            this.errors = errors;
            this.weak = weak;
            this.contractMap = contractMap;
            this.contracts = contracts;
        }
    }

    public static ParseAll parseAll(String text, Opts opts) throws SmlError {
        return parseAll(text, opts, WANT_ALL);
    }

    /**
     * 一次解析拿全部产物：值树 + 诊断 + 多错误 + 弱表 + 契约映射 + 契约表。
     *
     * <p>开了 {@link #WANT_ERRORS} 后，语法/契约错<b>不再抛异常</b>：值树是尽力而为的
     * 部分结果，全部错误在 {@code errors} 里（severity == 1）。{@code contracts} 是
     * <b>独立克隆</b>的 {@link Builtin}，可直接传给 {@link #loadsFull} 的
     * {@code opts.builtins}（「用 SML 写契约表、再注入回去」的闭环）。
     */
    public static ParseAll parseAll(String text, Opts opts, int want) throws SmlError {
        if (text == null) throw new SmlError("E-INTERNAL-001", 0, 0, 0, 0, "", "parseAll: text is null");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment textSeg = arena.allocateFrom(text);
            MemorySegment optsSeg = encodeOpts(opts, arena);
            MemorySegment err = arena.allocate(SML_ERROR);
            err.fill((byte) 0);
            MemorySegment r = (MemorySegment) SML_PARSE_ALL.invokeExact(textSeg, optsSeg, want, err);
            if (r == null || r.equals(MemorySegment.NULL)) throw errorFrom(err);
            try {
                Object value = materialize((MemorySegment) SML_RESULT_VALUE.invokeExact(r), arena);
                List<Diagnostic> diags = (want & WANT_DIAGS) == 0 ? new ArrayList<>()
                        : readDiags(((MemorySegment) SML_RESULT_DIAGS.invokeExact(r))
                                .reinterpret(SML_DIAG_LIST.byteSize()));
                List<Diagnostic> errs = (want & WANT_ERRORS) == 0 ? new ArrayList<>()
                        : readDiags(((MemorySegment) SML_RESULT_ERRORS.invokeExact(r))
                                .reinterpret(SML_DIAG_LIST.byteSize()));
                WeakLists weak = (want & WANT_WEAK) == 0
                        ? new WeakLists(new ArrayList<>(), new ArrayList<>(), new ArrayList<>())
                        : readWeakLists(((MemorySegment) SML_RESULT_WEAK.invokeExact(r))
                                .reinterpret(SML_WEAK_LISTS.byteSize()), arena);
                List<ContractRef> cmap = (want & WANT_CONTRACT_MAP) == 0
                        ? new ArrayList<>()
                        : readContractMap(((MemorySegment) SML_RESULT_CONTRACT_MAP.invokeExact(r))
                                .reinterpret(SML_CONTRACT_MAP.byteSize()));
                Builtin ctr = null;
                if ((want & WANT_CONTRACTS) != 0) {
                    // 借用的句柄不能活过结果 ⇒ 立刻克隆成宿主独占的一份。
                    MemorySegment cloned = (MemorySegment) SML_BUILTIN_CLONE
                            .invokeExact((MemorySegment) SML_RESULT_CONTRACTS.invokeExact(r));
                    ctr = Builtin.adopt(cloned);
                }
                return new ParseAll(value, diags, errs, weak, cmap, ctr);
            } finally {
                SML_RESULT_FREE.invokeExact(r);
            }
        } catch (SmlError e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    private static List<ContractRef> readContractMap(MemorySegment cm) throws Throwable {
        List<ContractRef> out = new ArrayList<>();
        MemorySegment items = cm.get(ValueLayout.ADDRESS, 0);
        long len = cm.get(ValueLayout.JAVA_LONG, 8);
        if (len <= 0 || items.equals(MemorySegment.NULL)) return out;
        MemorySegment base = items.reinterpret(len * SML_CONTRACT_REF.byteSize());
        long step = SML_CONTRACT_REF.byteSize();
        for (long i = 0; i < len; i++) {
            long off = i * step;
            out.add(new ContractRef(readCString(base.get(ValueLayout.ADDRESS, off)),
                    readCString(base.get(ValueLayout.ADDRESS, off + 8))));
        }
        return out;
    }

    /**
     * 同 {@link #loadFile}，另交出诊断与弱声明三表（文件入口版的 {@link #loadsFullLists}）。
     *
     * <p>展开 {@code include}，相对路径以文件所在目录为基准；跨 include 时行号可能为 0。
     */
    public static Parsed loadFileFull(String path, Opts opts) throws SmlError {
        if (path == null) throw new SmlError("E-INTERNAL-001", 0, 0, 0, 0, "", "loadFileFull: path is null");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment pathSeg = arena.allocateFrom(path);
            MemorySegment optsSeg = encodeOpts(opts, arena);
            MemorySegment err = arena.allocate(SML_ERROR);
            err.fill((byte) 0);
            MemorySegment dl = arena.allocate(SML_DIAG_LIST);
            dl.fill((byte) 0);
            MemorySegment wl = arena.allocate(SML_WEAK_LISTS);
            wl.fill((byte) 0);
            MemorySegment root = (MemorySegment) SML_LOAD_FILE_FULL
                    .invokeExact(pathSeg, optsSeg, wl, dl, err);
            try {
                if (root == null || root.equals(MemorySegment.NULL)) {
                    throw errorFrom(err);
                }
                return new Parsed(materialize(root, arena), readDiags(dl),
                        readWeakLists(wl, arena));
            } finally {
                SML_WEAKLISTS_FREE.invokeExact(wl);
                SML_DIAG_LIST_FREE.invokeExact(dl);
                if (root != null && !root.equals(MemorySegment.NULL)) {
                    SML_FREE.invokeExact(root);
                }
            }
        } catch (SmlError e) {
            throw e;
        } catch (Throwable t) {
            throw new RuntimeException(t);
        }
    }

    /** 同 `loadsFull`，另交出弱声明三表（`@weakbind` / `@weakdata` / `@weakdoc`）。 */
    public static Parsed loadsFullLists(String text, Opts opts) throws SmlError {
        if (text == null) throw new SmlError("E-INTERNAL-001", 0, 0, 0, 0, "", "loadsFullLists: text is null");
        try (Arena arena = Arena.ofConfined()) {
            MemorySegment textSeg = arena.allocateFrom(text);
            MemorySegment optsSeg = encodeOpts(opts, arena);
            MemorySegment err = arena.allocate(SML_ERROR);
            err.fill((byte) 0);
            MemorySegment dl = arena.allocate(SML_DIAG_LIST);
            dl.fill((byte) 0);
            MemorySegment wl = arena.allocate(SML_WEAK_LISTS);
            wl.fill((byte) 0);
            MemorySegment root = (MemorySegment) SML_PARSE_FULL_LISTS
                    .invokeExact(textSeg, optsSeg, wl, dl, err);
            try {
                if (root == null || root.equals(MemorySegment.NULL)) {
                    throw errorFrom(err);
                }
                return new Parsed(materialize(root, arena), readDiags(dl), readWeakLists(wl, arena));
            } finally {
                SML_WEAKLISTS_FREE.invokeExact(wl);
                SML_DIAG_LIST_FREE.invokeExact(dl);
                if (root != null && !root.equals(MemorySegment.NULL)) {
                    SML_FREE.invokeExact(root);
                }
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
