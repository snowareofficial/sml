// SPDX-License-Identifier: MulanPSL-2.0
// Ambr.java — pure-Java read-only `.ambr` decoder (T7 / N03).
// Ambr.java —— 纯 Java 的 `.ambr` 只读解码器（T7 / N03）。
//
// Why pure Java / 为什么不再走 FFI:
//   `Sml` 已经用 Panama FFM 调 Rust cdylib；但「读一份 .ambr」不该因此要求宿主带本地库
//   （Android、沙箱、只读容器都会卡住）。容器层是自描述的，纯 Java 解一遍更省事。
//
// Scope / 范围: 40 B header + 41 B table + VALUE/STRTAB; other segments skipped by len.
// `loadTyped()` additionally reads TYPED (0x70) + SCHEMA (0x71) and renders the typed
// variables (D45); comp/enc forms of those two are ignored by `load` and refused by
// `loadTyped`. Compressed/encrypted VALUE or STRTAB is refused, not guessed.
// ｜ 主路径只解 VALUE/STRTAB；`loadTyped` 另解 TYPED/SCHEMA；压缩/加密的 VALUE/STRTAB 明确报错。
//
// Value mapping / 值映射 (same as `Sml.loads`, so both paths compare directly):
//   Null→null, Bool→Boolean, Int→Long, Float→Double, Str→String,
//   Arr→ArrayList<Object>, Obj→LinkedHashMap<String,Object> (source order).
//
// 错误：`errors/codes.sml` 的独立 `AMBR` 域（`E-AMBR-001`…`008`）；错误串形如
// `AMBR: [E-AMBR-002] <detail>`，宿主按码分支。码来自 `amberice::AmbrError`。
// ｜ Errors: standalone `AMBR` domain (`E-AMBR-001`..`008`); messages carry
//    `AMBR: [E-AMBR-002] <detail>` so hosts can branch on the code.
package sml;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class Ambr {
    private Ambr() {}

    /** Decoder failure. ｜ 解码失败。 */
    public static final class AmbrException extends RuntimeException {
        private static final long serialVersionUID = 1L;
        public AmbrException(String msg) { super("AMBR: " + msg); }
    }

    private static final int SEG_VALUE = 0x01;
    private static final int SEG_STRTAB = 0x02;
    private static final int SEG_TYPED = 0x70;
    private static final int SEG_SCHEMA = 0x71;
    private static final int SEG_HDR = 41;
    private static final int ALIGN_MIN = 8;
    private static final int ALIGN_BLOCK = 4096;
    private static final int MAX_DEPTH = 128;
    private static final long MAX_ELEMS = 1_000_000L;

    private static final class Parts {
        byte[] value, strtab, typed, schema;
        boolean typedOpaque, schemaOpaque;
    }

    private static Parts walk(byte[] bytes) {
        if (bytes == null || bytes.length < 40) throw new AmbrException("文件短于 40 B 头");
        if (bytes[0] != 'A' || bytes[1] != 'M' || bytes[2] != 'B' || bytes[3] != 'R') {
            throw new AmbrException("不是 .ambr 文件（magic 不是 AMBR）");
        }
        final int fmt = bytes[6] & 0xff;
        final int hdrLen = bytes[7] & 0xff;
        final long tableOff = u64(bytes, 8);
        final long count = u32(bytes, 16);
        final int flags = (int) u16(bytes, 20);
        if (fmt != 0) throw new AmbrException("不支持的字节布局版本 fmt=" + fmt);
        if (hdrLen < 40 || tableOff < hdrLen) throw new AmbrException("头部长度或段表偏移非法");

        final int granule = (flags & 1) != 0 ? ALIGN_BLOCK : ALIGN_MIN;
        long bodyAt = alignUp(tableOff + count * SEG_HDR, granule);
        final Parts p = new Parts();

        for (long i = 0; i < count; i++) {
            final int o = (int) tableOff + (int) (i * SEG_HDR);
            if (o + SEG_HDR > bytes.length) throw new AmbrException("段表越界");
            final int id = bytes[o] & 0xff;
            final int comp = bytes[o + 1] & 0xff;
            final int enc = bytes[o + 2] & 0xff;
            // len is at offset 5: id:u8 comp:u8 enc:u8 flags:u16 len:u32 sha256:32
            final long len = u32(bytes, o + 5);
            if (bodyAt + len > bytes.length) throw new AmbrException("段体长度越界");
            final int from = (int) bodyAt;
            final int to = (int) (bodyAt + len);
            if (id == SEG_VALUE || id == SEG_STRTAB) {
                if (comp != 0 || enc != 0) {
                    throw new AmbrException("VALUE/STRTAB 压缩或加密过，本解码器不解");
                }
                if (id == SEG_VALUE) p.value = java.util.Arrays.copyOfRange(bytes, from, to);
                else p.strtab = java.util.Arrays.copyOfRange(bytes, from, to);
            } else if (id == SEG_TYPED || id == SEG_SCHEMA) {
                if (comp != 0 || enc != 0) {
                    if (id == SEG_TYPED) p.typedOpaque = true;
                    else p.schemaOpaque = true;
                } else if (id == SEG_TYPED) {
                    p.typed = java.util.Arrays.copyOfRange(bytes, from, to);
                } else {
                    p.schema = java.util.Arrays.copyOfRange(bytes, from, to);
                }
            }
            bodyAt = alignUp(bodyAt + len, granule);
        }
        return p;
    }

    /** Decode a whole `.ambr` image. ｜ 解一整份 `.ambr`。 */
    public static Object load(byte[] bytes) {
        final Parts p = walk(bytes);
        if (p.value == null) throw new AmbrException("找不到 VALUE 段");

        final List<String> strtab = new ArrayList<>();
        if (p.strtab != null) parseStrtab(p.strtab, strtab);

        final Reader r = new Reader(p.value);
        final Object v = decodeNode(r, strtab, 0);
        if (r.pos != r.len) throw new AmbrException("VALUE 段读完仍有剩余字节");
        return v;
    }

    /** Decode a file. ｜ 解一个文件。 */
    public static Object loadFile(String path) {
        try {
            return load(Files.readAllBytes(Path.of(path)));
        } catch (IOException e) {
            throw new AmbrException("读文件失败：" + e.getMessage());
        }
    }

    private static final class Reader {
        final byte[] b;
        final int len;
        int pos = 0;

        Reader(byte[] b) { this.b = b; this.len = b.length; }

        int u8() {
            if (pos >= len) throw new AmbrException("读取越界");
            return b[pos++] & 0xff;
        }

        long u32() {
            if (pos + 4 > len) throw new AmbrException("读取越界");
            final long v = (b[pos] & 0xffL) | ((b[pos + 1] & 0xffL) << 8)
                    | ((b[pos + 2] & 0xffL) << 16) | ((b[pos + 3] & 0xffL) << 24);
            pos += 4;
            return v;
        }

        long varint() {
            long v = 0;
            int shift = 0;
            for (int i = 0; i < 10; i++) {
                final int byteV = u8();
                v |= (long) (byteV & 0x7f) << shift;
                if ((byteV & 0x80) == 0) return v;
                shift += 7;
            }
            throw new AmbrException("LEB128 超过 10 字节");
        }

        double f64() {
            if (pos + 8 > len) throw new AmbrException("浮点截断");
            long bits = 0;
            for (int i = 0; i < 8; i++) bits |= (b[pos + i] & 0xffL) << (8 * i);
            pos += 8;
            return Double.longBitsToDouble(bits);
        }

        float f32() {
            if (pos + 4 > len) throw new AmbrException("浮点截断");
            final int bits = (b[pos] & 0xff) | ((b[pos + 1] & 0xff) << 8)
                    | ((b[pos + 2] & 0xff) << 16) | ((b[pos + 3] & 0xff) << 24);
            pos += 4;
            return Float.intBitsToFloat(bits);
        }
    }

    private static Object decodeNode(Reader r, List<String> strtab, int depth) {
        if (depth > MAX_DEPTH) throw new AmbrException("嵌套超过上限");
        final int tag = r.u8();
        r.u8(); // flags: bit 0 = "has __type", a hint only
        final long subtreeLen = r.u32();
        final int payload = r.pos;
        Object value;

        switch (tag) {
            case 0: value = null; break;
            case 1: value = Boolean.FALSE; break;
            case 2: value = Boolean.TRUE; break;
            case 3: {
                final long z = r.varint();
                value = Long.valueOf((z >>> 1) ^ -(z & 1));
                break;
            }
            case 4: value = Double.valueOf(r.f64()); break;
            case 5: {
                final long id = r.varint();
                if (id < 0 || id >= strtab.size()) throw new AmbrException("字符串 id 越界");
                value = strtab.get((int) id);
                break;
            }
            case 6: {
                final long n = r.varint();
                if (n > MAX_ELEMS) throw new AmbrException("数组元素数超过上限");
                final List<Object> arr = new ArrayList<>((int) n);
                for (long i = 0; i < n; i++) arr.add(decodeNode(r, strtab, depth + 1));
                value = arr;
                break;
            }
            case 7: {
                final long n = r.varint();
                if (n > MAX_ELEMS) throw new AmbrException("对象字段数超过上限");
                final Map<String, Object> obj = new LinkedHashMap<>();
                for (long i = 0; i < n; i++) {
                    final long keyId = r.varint();
                    if (keyId < 0 || keyId >= strtab.size()) throw new AmbrException("键 id 越界");
                    final String key = strtab.get((int) keyId);
                    if (obj.containsKey(key)) throw new AmbrException("对象里出现重复键 " + key);
                    obj.put(key, decodeNode(r, strtab, depth + 1));
                }
                value = obj;
                break;
            }
            default:
                throw new AmbrException("未定义的节点 tag " + tag);
        }

        if (r.pos - payload != subtreeLen) throw new AmbrException("subtree_len 与实际消费不符");
        return value;
    }

    private static void parseStrtab(byte[] body, List<String> out) {
        final Reader r = new Reader(body);
        while (r.pos < r.len) {
            final long n = r.varint();
            if (n < 0 || r.pos + n > r.len) throw new AmbrException("STRTAB 项越界");
            out.add(new String(body, r.pos, (int) n, StandardCharsets.UTF_8));
            r.pos += (int) n;
        }
    }

    // ---------------------------------------------------------------------
    // SCHEMA（0x71）+ TYPED（0x70）动态渲染（D45 / §25.8）
    // ---------------------------------------------------------------------

    private enum TyKind {
        BOOL, U8, U16, U32, U64, I8, I16, I32, I64, F32, F64, STR, BYTES, VEC, OPT, NAMED
    }

    private static final class Ty {
        final TyKind kind;
        final String named;
        final Ty inner;
        Ty(TyKind kind, String named, Ty inner) {
            this.kind = kind;
            this.named = named;
            this.inner = inner;
        }
    }

    private static final class Field {
        final String name;
        final Ty ty;
        Field(String name, Ty ty) {
            this.name = name;
            this.ty = ty;
        }
    }

    private static final class TypeDef {
        final String id;
        final String name;
        final List<Field> fields;
        TypeDef(String id, String name, List<Field> fields) {
            this.id = id;
            this.name = name;
            this.fields = fields;
        }
    }

    private static final class Ent {
        long id, off, len;
        String typeId;
    }

    /** u64 的 type_id 按 16 位小写十六进制返回，避免 Java `long` 的符号问题。 */
    private static String u64Hex(Reader r) {
        final long lo = r.u32();
        final long hi = r.u32();
        return String.format("%08x%08x", Long.valueOf(hi), Long.valueOf(lo));
    }

    private static String hex(byte[] b, int off, int n) {
        final StringBuilder sb = new StringBuilder(n * 2);
        for (int i = 0; i < n; i++) {
            final int v = b[off + i] & 0xff;
            sb.append(Character.forDigit(v >>> 4, 16));
            sb.append(Character.forDigit(v & 15, 16));
        }
        return sb.toString();
    }

    @SuppressWarnings("unchecked")
    private static Ty parseTy(Object v) {
        if (v instanceof String) {
            final String s = (String) v;
            switch (s) {
                case "bool": return new Ty(TyKind.BOOL, null, null);
                case "u8": return new Ty(TyKind.U8, null, null);
                case "u16": return new Ty(TyKind.U16, null, null);
                case "u32": return new Ty(TyKind.U32, null, null);
                case "u64": return new Ty(TyKind.U64, null, null);
                case "i8": return new Ty(TyKind.I8, null, null);
                case "i16": return new Ty(TyKind.I16, null, null);
                case "i32": return new Ty(TyKind.I32, null, null);
                case "i64": return new Ty(TyKind.I64, null, null);
                case "f32": return new Ty(TyKind.F32, null, null);
                case "f64": return new Ty(TyKind.F64, null, null);
                case "str": return new Ty(TyKind.STR, null, null);
                case "bytes": return new Ty(TyKind.BYTES, null, null);
                default: throw new AmbrException("SCHEMA 里有未知 ty");
            }
        }
        if (v instanceof Map) {
            final Map<String, Object> m = (Map<String, Object>) v;
            if (m.containsKey("vec")) return new Ty(TyKind.VEC, null, parseTy(m.get("vec")));
            if (m.containsKey("opt")) return new Ty(TyKind.OPT, null, parseTy(m.get("opt")));
            if (m.containsKey("type")) {
                final Object idv = m.get("type");
                if (!(idv instanceof String) || !((String) idv).matches("[0-9a-f]{16}")) {
                    throw new AmbrException("SCHEMA 的 type 引用非法");
                }
                return new Ty(TyKind.NAMED, (String) idv, null);
            }
            throw new AmbrException("SCHEMA 的 ty 对象缺 vec/opt/type");
        }
        throw new AmbrException("SCHEMA 的 ty 必须是字符串或对象");
    }

    @SuppressWarnings("unchecked")
    private static Map<String, TypeDef> parseSchema(Object schema) {
        if (!(schema instanceof Map)) throw new AmbrException("SCHEMA 必须是对象");
        final Map<String, Object> top = (Map<String, Object>) schema;
        final Object ver = top.get("schema_version");
        if (!(ver instanceof Long) || ((Long) ver).longValue() != 1) {
            throw new AmbrException("schema_version 只支持 1");
        }
        final Object typesV = top.get("types");
        if (!(typesV instanceof List)) throw new AmbrException("SCHEMA 缺 types");
        final Map<String, TypeDef> out = new LinkedHashMap<>();
        for (Object tv : (List<?>) typesV) {
            if (!(tv instanceof Map)) throw new AmbrException("SCHEMA 的 types[] 必须是对象");
            final Map<String, Object> t = (Map<String, Object>) tv;
            final Object idv = t.get("id");
            if (!(idv instanceof String) || !((String) idv).matches("[0-9a-f]{16}")) {
                throw new AmbrException("SCHEMA 的 type id 非法");
            }
            final String id = (String) idv;
            if (out.containsKey(id)) throw new AmbrException("SCHEMA 的 type id 重复");
            final Object nv = t.get("name");
            if (!(nv instanceof String)) throw new AmbrException("SCHEMA 的 type 缺 name");
            final Object kv = t.get("kind");
            if (kv != null && !(kv instanceof String && "struct".equals(kv))) {
                throw new AmbrException("v1 只支持 kind: struct");
            }
            final Object fv = t.get("fields");
            if (!(fv instanceof List)) throw new AmbrException("SCHEMA 的 type 缺 fields");
            final List<Field> fields = new ArrayList<>();
            long last = -1;
            for (Object fo : (List<?>) fv) {
                if (!(fo instanceof Map)) throw new AmbrException("SCHEMA 的 fields[] 必须是对象");
                final Map<String, Object> f = (Map<String, Object>) fo;
                final Object fid = f.get("id");
                if (!(fid instanceof Long) || ((Long) fid).longValue() <= last) {
                    throw new AmbrException("SCHEMA 的 fields 必须按 id 严格升序");
                }
                last = ((Long) fid).longValue();
                final Object fn = f.get("name");
                if (!(fn instanceof String)) throw new AmbrException("SCHEMA 的 field 缺 name");
                fields.add(new Field((String) fn, parseTy(f.get("ty"))));
            }
            out.put(id, new TypeDef(id, (String) nv, fields));
        }
        return out;
    }

    private static Object decodeStruct(Reader r, TypeDef td, Map<String, TypeDef> types, int depth) {
        if (depth > MAX_DEPTH) throw new AmbrException("TYPED 嵌套超过上限");
        final Map<String, Object> o = new LinkedHashMap<>();
        for (Field f : td.fields) o.put(f.name, decodeTyped(r, f.ty, types, depth + 1));
        return o;
    }

    private static Object decodeTyped(Reader r, Ty t, Map<String, TypeDef> types, int depth) {
        if (depth > MAX_DEPTH) throw new AmbrException("TYPED 嵌套超过上限");
        switch (t.kind) {
            case BOOL: {
                final long z = r.varint();
                if (z != 0 && z != 1) throw new AmbrException("布尔的 tag 只能是 0/1");
                return Boolean.valueOf(z == 1);
            }
            case U8:
            case U16:
            case U32: {
                final long max = t.kind == TyKind.U8 ? 255L : (t.kind == TyKind.U16 ? 65535L : 4294967295L);
                final long z = r.varint();
                if (z < 0 || z > max) throw new AmbrException("整数超出目标类型范围");
                return Long.valueOf(z);
            }
            case U64: {
                final long z = r.varint();
                if (z < 0) throw new AmbrException("u64 超出 i64 范围，无法用 Int 表示");
                return Long.valueOf(z);
            }
            case I8:
            case I16:
            case I32:
            case I64: {
                final long z = r.varint();
                final long n = (z >>> 1) ^ -(z & 1);
                final long lim = t.kind == TyKind.I8 ? 127L
                        : t.kind == TyKind.I16 ? 32767L
                        : t.kind == TyKind.I32 ? 2147483647L
                        : 9223372036854775807L;
                if (n > lim || n < -lim - 1) throw new AmbrException("整数超出目标类型范围");
                return Long.valueOf(n);
            }
            case F32: return Double.valueOf(r.f32());
            case F64: return Double.valueOf(r.f64());
            case STR: {
                final long n = r.varint();
                if (n < 0 || r.pos + n > r.len) throw new AmbrException("字符串长度越界");
                final String s = new String(r.b, r.pos, (int) n, StandardCharsets.UTF_8);
                r.pos += (int) n;
                return s;
            }
            case BYTES: {
                final long n = r.varint();
                if (n < 0 || r.pos + n > r.len) throw new AmbrException("字节长度越界");
                final String s = hex(r.b, r.pos, (int) n);
                r.pos += (int) n;
                return s;
            }
            case VEC: {
                final long n = r.varint();
                if (n < 0 || n > MAX_ELEMS) throw new AmbrException("数组元素数越界");
                final List<Object> a = new ArrayList<>((int) n);
                for (long i = 0; i < n; i++) a.add(decodeTyped(r, t.inner, types, depth + 1));
                return a;
            }
            case OPT: {
                final long tag = r.varint();
                if (tag == 0) return null;
                if (tag == 1) return decodeTyped(r, t.inner, types, depth + 1);
                throw new AmbrException("Option 的 tag 只能是 0/1");
            }
            case NAMED: {
                final TypeDef td = types.get(t.named);
                if (td == null) throw new AmbrException("SCHEMA 里没有这个 type_id");
                return decodeStruct(r, td, types, depth + 1);
            }
            default:
                throw new AmbrException("未知 ty");
        }
    }

    /**
     * Render `TYPED` via the embedded `SCHEMA` (D45): `[{id, name, value}]`.
     * 用内嵌 SCHEMA 渲染 TYPED：`[{id, name, value}]`；无 TYPED 时返回空列表。
     */
    public static List<Object> loadTyped(byte[] bytes) {
        final List<Object> out = new ArrayList<>();
        final Parts p = walk(bytes);
        if (p.typedOpaque || p.schemaOpaque) {
            throw new AmbrException("TYPED/SCHEMA 压缩或加密过，本解码器不解");
        }
        if (p.typed == null) return out;
        if (p.schema == null) throw new AmbrException("没有 SCHEMA 段（0x71），无法按名渲染 TYPED");

        final Reader sr = new Reader(p.schema);
        final long sc = sr.varint();
        if (sc < 0 || sc > MAX_ELEMS) throw new AmbrException("L0 字符串个数非法");
        final List<String> sst = new ArrayList<>((int) sc);
        for (long i = 0; i < sc; i++) {
            final long n = sr.varint();
            if (n < 0 || sr.pos + n > sr.len) throw new AmbrException("L0 字符串长度越界");
            sst.add(new String(sr.b, sr.pos, (int) n, StandardCharsets.UTF_8));
            sr.pos += (int) n;
        }
        final Object schema = decodeNode(sr, sst, 0);
        if (sr.pos != sr.len) throw new AmbrException("SCHEMA 段读完仍有剩余字节");
        final Map<String, TypeDef> types = parseSchema(schema);

        final Reader tr = new Reader(p.typed);
        final long count = tr.varint();
        if (count < 0 || count > MAX_ELEMS) throw new AmbrException("TYPED 条目数非法");
        final List<Ent> ents = new ArrayList<>((int) count);
        for (long i = 0; i < count; i++) {
            final Ent e = new Ent();
            e.id = tr.varint();
            e.typeId = u64Hex(tr);
            e.off = tr.u32();
            e.len = tr.u32();
            if (e.id <= 0 || e.off < 0 || e.len < 0) throw new AmbrException("TYPED 目录被截断");
            ents.add(e);
        }
        final int base = tr.pos;

        for (Ent e : ents) {
            final TypeDef td = types.get(e.typeId);
            if (td == null) throw new AmbrException("SCHEMA 里没有这个 type_id");
            final long startL = (long) base + e.off;
            final long endL = startL + e.len;
            if (startL < 0 || endL < startL || endL > p.typed.length) {
                throw new AmbrException("TYPED 载荷越界");
            }
            final Reader er = new Reader(
                    java.util.Arrays.copyOfRange(p.typed, (int) startL, (int) endL));
            final Object val = decodeStruct(er, td, types, 0);
            if (er.pos != er.len) throw new AmbrException("TYPED 载荷解完仍有剩余字节");
            final Map<String, Object> item = new LinkedHashMap<>();
            item.put("id", Long.valueOf(e.id));
            item.put("name", td.name);
            item.put("value", val);
            out.add(item);
        }
        return out;
    }

    private static long alignUp(long x, long granule) {
        final long a = Math.max(granule, 1);
        return (x + a - 1) / a * a;
    }

    private static long u16(byte[] b, int o) {
        return (b[o] & 0xffL) | ((b[o + 1] & 0xffL) << 8);
    }

    private static long u32(byte[] b, int o) {
        return (b[o] & 0xffL) | ((b[o + 1] & 0xffL) << 8)
                | ((b[o + 2] & 0xffL) << 16) | ((b[o + 3] & 0xffL) << 24);
    }

    private static long u64(byte[] b, int o) {
        return u32(b, o) | (u32(b, o + 4) << 32);
    }
}
