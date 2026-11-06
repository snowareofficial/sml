// SPDX-License-Identifier: MulanPSL-2.0
// Ambr.java — pure-Java read-only `.ambr` decoder (T7 / N03).
// Ambr.java —— 纯 Java 的 `.ambr` 只读解码器（T7 / N03）。
//
// Why pure Java / 为什么不再走 FFI:
//   `Sml` 已经用 Panama FFM 调 Rust cdylib；但「读一份 .ambr」不该因此要求宿主带本地库
//   （Android、沙箱、只读容器都会卡住）。容器层是自描述的，纯 Java 解一遍更省事。
//
// Scope / 范围: 40 B header + 41 B table + VALUE/STRTAB; other segments skipped by len.
// Compressed/encrypted VALUE or STRTAB is refused, not guessed.
// ｜ 只解 VALUE/STRTAB；压缩/加密的段明确报错；其余段按 len 跳过。
//
// Value mapping / 值映射 (same as `Sml.loads`, so both paths compare directly):
//   Null→null, Bool→Boolean, Int→Long, Float→Double, Str→String,
//   Arr→ArrayList<Object>, Obj→LinkedHashMap<String,Object> (source order).
//
// Errors / 错误: no `E-AMBR-*` code exists yet (lands with T7) ⇒ message starts with
// `AMBR:` and carries no code. ｜ 错误暂不带码，前缀 `AMBR:`。
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
    private static final int SEG_HDR = 41;
    private static final int ALIGN_MIN = 8;
    private static final int ALIGN_BLOCK = 4096;
    private static final int MAX_DEPTH = 128;
    private static final long MAX_ELEMS = 1_000_000L;

    /** Decode a whole `.ambr` image. ｜ 解一整份 `.ambr`。 */
    public static Object load(byte[] bytes) {
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

        byte[] value = null;
        byte[] strtabBody = null;
        for (long i = 0; i < count; i++) {
            final int o = (int) tableOff + (int) (i * SEG_HDR);
            if (o + SEG_HDR > bytes.length) throw new AmbrException("段表越界");
            final int id = bytes[o] & 0xff;
            final int comp = bytes[o + 1] & 0xff;
            final int enc = bytes[o + 2] & 0xff;
            // len is at offset 5: id:u8 comp:u8 enc:u8 flags:u16 len:u32 sha256:32
            final long len = u32(bytes, o + 5);
            if (bodyAt + len > bytes.length) throw new AmbrException("段体长度越界");
            if (id == SEG_VALUE || id == SEG_STRTAB) {
                if (comp != 0 || enc != 0) {
                    throw new AmbrException("VALUE/STRTAB 压缩或加密过，本解码器不解");
                }
                final int from = (int) bodyAt;
                final int to = (int) (bodyAt + len);
                if (id == SEG_VALUE) value = java.util.Arrays.copyOfRange(bytes, from, to);
                else strtabBody = java.util.Arrays.copyOfRange(bytes, from, to);
            }
            bodyAt = alignUp(bodyAt + len, granule);
        }
        if (value == null) throw new AmbrException("找不到 VALUE 段");

        final List<String> strtab = new ArrayList<>();
        if (strtabBody != null) parseStrtab(strtabBody, strtab);

        final Reader r = new Reader(value);
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
