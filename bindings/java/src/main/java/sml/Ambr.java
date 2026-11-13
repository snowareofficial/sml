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

    /** 解码失败：消息统一带稳定码 `AMBR: [E-AMBR-xxx] <detail>`。 */
    public static final class AmbrException extends RuntimeException {
        private static final long serialVersionUID = 1L;
        /** 默认按「容器格式」归码。｜ Defaults to the container-format code. */
        public AmbrException(String msg) { this("E-AMBR-002", msg); }
        public AmbrException(String code, String msg) { super("AMBR: [" + code + "] " + msg); }
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

    /* --- 摘要算法（§20）：SHA-256 / SM3 / BLAKE3，供按 manifest 的 hash_id 校验段摘要。
     * 纯 Java 实现，不依赖 JCA（`MessageDigest` 未必有 SM3，且会造成端间结果漂移）。
     * ｜ Dependency-free SHA-256 / SM3 / BLAKE3 for segment-digest verification. --- */

    private static final int[] SHA_K = {
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    };

    private static int rotr(int x, int n) { return (x >>> n) | (x << (32 - n)); }
    private static int rotl(int x, int n) { return (x << n) | (x >>> (32 - n)); }

    /** 公共填充：0x80 + 0 + 8B 大端 bitlen ｜ shared padding. */
    private static byte[] pad(byte[] data) {
        final long bits = (long) data.length * 8L;
        int padLen = 64 - ((data.length + 9) % 64);
        if (padLen == 64) padLen = 0;
        final byte[] out = new byte[data.length + 1 + padLen + 8];
        System.arraycopy(data, 0, out, 0, data.length);
        out[data.length] = (byte) 0x80;
        for (int i = 0; i < 8; i++) out[out.length - 1 - i] = (byte) (bits >>> (8 * i));
        return out;
    }

    private static int be32(byte[] b, int at) {
        return ((b[at] & 0xff) << 24) | ((b[at + 1] & 0xff) << 16)
             | ((b[at + 2] & 0xff) << 8) | (b[at + 3] & 0xff);
    }

    private static int le32(byte[] b, int at) {
        return (b[at] & 0xff) | ((b[at + 1] & 0xff) << 8)
             | ((b[at + 2] & 0xff) << 16) | ((b[at + 3] & 0xff) << 24);
    }

    private static void putBe(byte[] out, int at, int v) {
        out[at] = (byte) (v >>> 24);
        out[at + 1] = (byte) (v >>> 16);
        out[at + 2] = (byte) (v >>> 8);
        out[at + 3] = (byte) v;
    }

    private static void putLe(byte[] out, int at, int v) {
        out[at] = (byte) v;
        out[at + 1] = (byte) (v >>> 8);
        out[at + 2] = (byte) (v >>> 16);
        out[at + 3] = (byte) (v >>> 24);
    }

    static byte[] sha256(byte[] data) {
        final int[] h = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                         0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
        final byte[] msg = pad(data);
        final int[] w = new int[64];
        for (int off = 0; off < msg.length; off += 64) {
            for (int j = 0; j < 16; j++) w[j] = be32(msg, off + j * 4);
            for (int j = 16; j < 64; j++) {
                final int s0 = rotr(w[j - 15], 7) ^ rotr(w[j - 15], 18) ^ (w[j - 15] >>> 3);
                final int s1 = rotr(w[j - 2], 17) ^ rotr(w[j - 2], 19) ^ (w[j - 2] >>> 10);
                w[j] = w[j - 16] + s0 + w[j - 7] + s1;
            }
            int a = h[0], b = h[1], c = h[2], d2 = h[3];
            int e = h[4], f = h[5], g = h[6], hh = h[7];
            for (int j = 0; j < 64; j++) {
                final int s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
                final int ch = (e & f) ^ (~e & g);
                final int t1 = hh + s1 + ch + SHA_K[j] + w[j];
                final int s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
                final int maj = (a & b) ^ (a & c) ^ (b & c);
                final int t2 = s0 + maj;
                hh = g; g = f; f = e; e = d2 + t1;
                d2 = c; c = b; b = a; a = t1 + t2;
            }
            h[0] += a; h[1] += b; h[2] += c; h[3] += d2;
            h[4] += e; h[5] += f; h[6] += g; h[7] += hh;
        }
        final byte[] out = new byte[32];
        for (int i = 0; i < 8; i++) putBe(out, i * 4, h[i]);
        return out;
    }

    private static int[] SM3_IV = {0x7380166f, 0x4914b2b9, 0x172442d7, 0xda8a0600,
                                   0xa96f30bc, 0x163138aa, 0xe38dee4d, 0xb0fb0e4e};

    static byte[] sm3(byte[] data) {
        final int[] h = SM3_IV.clone();
        final byte[] msg = pad(data);
        final int[] w = new int[68];
        final int[] w1 = new int[64];
        for (int off = 0; off < msg.length; off += 64) {
            for (int j = 0; j < 16; j++) w[j] = be32(msg, off + j * 4);
            for (int j = 16; j < 68; j++) {
                final int x = w[j - 16] ^ w[j - 9] ^ rotl(w[j - 3], 15);
                w[j] = (x ^ rotl(x, 15) ^ rotl(x, 23)) ^ rotl(w[j - 13], 7) ^ w[j - 6];
            }
            for (int j = 0; j < 64; j++) w1[j] = w[j] ^ w[j + 4];
            int a = h[0], b = h[1], c = h[2], d2 = h[3];
            int e = h[4], f = h[5], g = h[6], hh = h[7];
            for (int j = 0; j < 64; j++) {
                final int tj = j < 16 ? 0x79cc4519 : 0x7a879d8a;
                final int ss1 = rotl(rotl(a, 12) + e + rotl(tj, j), 7);
                final int ss2 = ss1 ^ rotl(a, 12);
                final int ff = j < 16 ? (a ^ b ^ c) : ((a & b) | (a & c) | (b & c));
                final int gg = j < 16 ? (e ^ f ^ g) : ((e & f) | (~e & g));
                final int tt1 = ff + d2 + ss2 + w1[j];
                final int tt2 = gg + hh + ss1 + w[j];
                d2 = c; c = rotl(b, 9); b = a; a = tt1;
                hh = g; g = rotl(f, 19); f = e; e = tt2 ^ rotl(tt2, 9) ^ rotl(tt2, 17);
            }
            h[0] ^= a; h[1] ^= b; h[2] ^= c; h[3] ^= d2;
            h[4] ^= e; h[5] ^= f; h[6] ^= g; h[7] ^= hh;
        }
        final byte[] out = new byte[32];
        for (int i = 0; i < 8; i++) putBe(out, i * 4, h[i]);
        return out;
    }

    private static final int[] B3_IV = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                                        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
    private static final int[] B3_PERM = {2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8};

    private static int[] b3Words(byte[] buf, int len) {
        final int[] m = new int[16];
        final int full = len / 4;
        for (int i = 0; i < full; i++) m[i] = le32(buf, i * 4);
        final int rem = len % 4;
        if (rem > 0) {
            int x = 0;
            for (int i = 0; i < rem; i++) x |= (buf[full * 4 + i] & 0xff) << (8 * i);
            m[full] = x;
        }
        return m;
    }

    private static void b3Compress(int[] cv, int[] blockWords, long counter, int blockLen,
                                   int flags, int[] out) {
        final int[] s = {cv[0], cv[1], cv[2], cv[3], cv[4], cv[5], cv[6], cv[7],
                         B3_IV[0], B3_IV[1], B3_IV[2], B3_IV[3],
                         (int) counter, (int) (counter >>> 32), blockLen, flags};
        final int[] m = blockWords.clone();
        for (int r = 0; r < 7; r++) {
            b3Round(s, m);
            final int[] old = m.clone();
            for (int i = 0; i < 16; i++) m[i] = old[B3_PERM[i]];
        }
        for (int i = 0; i < 8; i++) {
            out[i] = s[i] ^ s[i + 8];
            out[i + 8] = s[i + 8] ^ cv[i];
        }
    }

    private static void b3G(int[] s, int a, int b, int c, int d, int mx, int my) {
        s[a] = s[a] + s[b] + mx;
        s[d] = rotr(s[d] ^ s[a], 16);
        s[c] = s[c] + s[d];
        s[b] = rotr(s[b] ^ s[c], 12);
        s[a] = s[a] + s[b] + my;
        s[d] = rotr(s[d] ^ s[a], 8);
        s[c] = s[c] + s[d];
        s[b] = rotr(s[b] ^ s[c], 7);
    }

    private static void b3Round(int[] s, int[] m) {
        b3G(s, 0, 4, 8, 12, m[0], m[1]);
        b3G(s, 1, 5, 9, 13, m[2], m[3]);
        b3G(s, 2, 6, 10, 14, m[4], m[5]);
        b3G(s, 3, 7, 11, 15, m[6], m[7]);
        b3G(s, 0, 5, 10, 15, m[8], m[9]);
        b3G(s, 1, 6, 11, 12, m[10], m[11]);
        b3G(s, 2, 7, 8, 13, m[12], m[13]);
        b3G(s, 3, 4, 9, 14, m[14], m[15]);
    }

    static byte[] blake3(byte[] msg) {
        final int CHUNK_START = 1, CHUNK_END = 2, PARENT = 4, ROOT = 8;
        int[] cv = B3_IV.clone();
        long counter = 0;
        byte[] buf = new byte[64];
        int bufLen = 0, blocks = 0;
        final int[][] stack = new int[54][];
        int stackLen = 0;
        int pos = 0;

        while (pos < msg.length) {
            final int chunkLen = 64 * blocks + bufLen;
            if (chunkLen == 1024) {
                // 收一个 chunk 的 CV
                final int[] out = new int[16];
                b3Compress(cv, b3Words(buf, bufLen), counter, bufLen, (blocks == 0 ? CHUNK_START : 0) | CHUNK_END, out);
                final int[] newCv = java.util.Arrays.copyOfRange(out, 0, 8);
                long total = counter + 1;
                stack[stackLen++] = newCv;
                while (total % 2 == 0 && stackLen >= 2) {
                    final int[] right = stack[--stackLen];
                    final int[] left = stack[--stackLen];
                    final int[] w = new int[16];
                    System.arraycopy(left, 0, w, 0, 8);
                    System.arraycopy(right, 0, w, 8, 8);
                    final int[] po = new int[16];
                    b3Compress(B3_IV, w, 0, 64, PARENT, po);
                    stack[stackLen++] = java.util.Arrays.copyOfRange(po, 0, 8);
                    total /= 2;
                }
                counter++;
                cv = B3_IV.clone();
                buf = new byte[64];
                bufLen = 0;
                blocks = 0;
            }
            int take = 1024 - (64 * blocks + bufLen);
            if (take > msg.length - pos) take = msg.length - pos;
            for (int i = 0; i < take; i++) {
                if (bufLen == 64) {
                    final int[] out = new int[16];
                    b3Compress(cv, b3Words(buf, 64), counter, 64, blocks == 0 ? CHUNK_START : 0, out);
                    cv = java.util.Arrays.copyOfRange(out, 0, 8);
                    blocks++;
                    buf = new byte[64];
                    bufLen = 0;
                }
                buf[bufLen++] = msg[pos++];
            }
        }

        // 最后一个 chunk 的 output，再与栈上的 CV 逐层合并
        int[] inputCv = cv;
        int[] words = b3Words(buf, bufLen);
        int blockLen = bufLen;
        int flags = (blocks == 0 ? CHUNK_START : 0) | CHUNK_END;
        for (int i = stackLen - 1; i >= 0; i--) {
            final int[] cur = new int[16];
            b3Compress(inputCv, words, counter, blockLen, flags, cur);
            final int[] w = new int[16];
            System.arraycopy(stack[i], 0, w, 0, 8);
            System.arraycopy(cur, 0, w, 8, 8);
            inputCv = B3_IV;
            words = w;
            blockLen = 64;
            flags = PARENT;
        }
        final int[] out = new int[16];
        b3Compress(inputCv, words, 0, blockLen, flags | ROOT, out);
        final byte[] res = new byte[32];
        for (int i = 0; i < 8; i++) putLe(res, i * 4, out[i]);
        return res;
    }

    /** 按 §20 的 hash_id 取摘要；`null` = 本端算不了（预留号 / 多级元组）⇒ 跳过校验。 */
    private static byte[] digestFor(int hashId, byte[] body) {
        if (hashId == 0x30) return sha256(body);
        if (hashId == 0x31) return sm3(body);
        if (hashId == 0x32) return blake3(body);
        return null;
    }

    /** 从 MANIFEST(0x04) 段体读 `hash_id`（自包含 canonical 编码）。 */
    private static Integer manifestHashId(byte[] body) {
        final Reader r = new Reader(body);
        final long count = r.varint();
        if (count < 0 || count > body.length) return null;
        final List<String> strtab = new ArrayList<>();
        for (long i = 0; i < count; i++) {
            final long n = r.varint();
            if (n < 0 || r.pos + n > r.len) return null;
            strtab.add(new String(body, r.pos, (int) n, java.nio.charset.StandardCharsets.UTF_8));
            r.pos += (int) n;
        }
        final Object tree = decodeNode(r, strtab, 0);
        if (!(tree instanceof Map)) return null;
        final Object raw = ((Map<?, ?>) tree).get("hash_id");
        if (raw == null) return 0x30;                       // 缺省 = §21 默认 SHA-256
        if (raw instanceof Number) {
            final long v = ((Number) raw).longValue();
            return (v >= 0 && v <= 255) ? Integer.valueOf((int) v) : null;
        }
        if (raw instanceof List) return 0x3E;               // 多级元组：转义号，本端不实现
        return null;
    }

    private static final class Parts {
        byte[] value, strtab, typed, schema;
        boolean typedOpaque, schemaOpaque;
        int hashId = 0x30;
        boolean digestSkipped;
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

        // 引导顺序：先找 MANIFEST 取 hash_id，再用它验其余段（连 manifest 自己一起）。
        // ｜ Bootstrap: read hash_id from MANIFEST first, then verify with it.
        {
            long scan = bodyAt;
            for (long i = 0; i < count; i++) {
                final int o = (int) tableOff + (int) (i * SEG_HDR);
                if (o + SEG_HDR > bytes.length) break;
                final int sid = bytes[o] & 0xff;
                final int scomp = bytes[o + 1] & 0xff;
                final int senc = bytes[o + 2] & 0xff;
                final long slen = u32(bytes, o + 5);
                if (scan + slen > bytes.length) break;
                if (sid == 0x04 && scomp == 0 && senc == 0) {
                    final Integer found = manifestHashId(
                        java.util.Arrays.copyOfRange(bytes, (int) scan, (int) (scan + slen)));
                    if (found != null) p.hashId = found;
                    break;
                }
                scan = alignUp(scan + slen, granule);
            }
        }

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
                final byte[] vb = java.util.Arrays.copyOfRange(bytes, from, to);
                final byte[] vgot = digestFor(p.hashId, vb);
                if (vgot == null) p.digestSkipped = true;
                else if (!java.util.Arrays.equals(vgot, java.util.Arrays.copyOfRange(bytes, o + 9, o + 41))) {
                    throw new AmbrException("E-AMBR-004", "段摘要不符（段体被改动？）");
                }
                if (id == SEG_VALUE) p.value = java.util.Arrays.copyOfRange(bytes, from, to);
                else p.strtab = java.util.Arrays.copyOfRange(bytes, from, to);
            } else if (id == 0x04) {
                if (comp == 0 && enc == 0) {
                    final byte[] body = java.util.Arrays.copyOfRange(bytes, from, to);
                    final byte[] got = digestFor(p.hashId, body);
                    if (got == null) p.digestSkipped = true;
                    else if (!java.util.Arrays.equals(got, java.util.Arrays.copyOfRange(bytes, o + 9, o + 41))) {
                        throw new AmbrException("E-AMBR-004", "段摘要不符（段体被改动？）");
                    }
                }
            } else if (id == SEG_TYPED || id == SEG_SCHEMA) {
                if (comp != 0 || enc != 0) {
                    if (id == SEG_TYPED) p.typedOpaque = true;
                    else p.schemaOpaque = true;
                } else {
                    final byte[] body = java.util.Arrays.copyOfRange(bytes, from, to);
                    final byte[] got = digestFor(p.hashId, body);
                    if (got == null) p.digestSkipped = true;
                    else if (!java.util.Arrays.equals(got, java.util.Arrays.copyOfRange(bytes, o + 9, o + 41))) {
                        throw new AmbrException("E-AMBR-004", "段摘要不符（段体被改动？）");
                    }
                    if (id == SEG_TYPED) p.typed = body;
                    else p.schema = body;
                }
            }
            bodyAt = alignUp(bodyAt + len, granule);
        }
        return p;
    }

    /** 解码时的可观测信息：摘要算法与「是否真的验过」。
     * ｜ Observability: which digest algorithm was declared, and whether it was verified. */
    public static final class Info {
        public final Object value;
        public final int hashId;
        public final boolean digestSkipped;

        Info(Object value, int hashId, boolean digestSkipped) {
            this.value = value;
            this.hashId = hashId;
            this.digestSkipped = digestSkipped;
        }
    }

    /** Decode a whole `.ambr` image. ｜ 解一整份 `.ambr`。 */
    public static Object load(byte[] bytes) {
        return loadWithInfo(bytes).value;
    }

    /** Same, with the digest algorithm and whether verification was skipped.
     * ｜ 同上，并交出摘要算法与「是否跳过校验」。 */
    public static Info loadWithInfo(byte[] bytes) {
        final Parts p = walk(bytes);
        if (p.value == null) throw new AmbrException("找不到 VALUE 段");

        final List<String> strtab = new ArrayList<>();
        if (p.strtab != null) parseStrtab(p.strtab, strtab);

        final Reader r = new Reader(p.value);
        final Object v = decodeNode(r, strtab, 0);
        if (r.pos != r.len) throw new AmbrException("VALUE 段读完仍有剩余字节");
        return new Info(v, p.hashId, p.digestSkipped);
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
