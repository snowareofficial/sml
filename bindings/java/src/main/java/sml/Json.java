package sml;

import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;

/** 极简 JSON 序列化（零依赖）。仅用于 dumps 的官方桥 obj -> JSON -> sml_dump。
 *  反序列化不需要（读路径走值树，不经 JSON）。 */
final class Json {
    private Json() {}

    static String stringify(Object o) {
        StringBuilder sb = new StringBuilder();
        write(o, sb);
        return sb.toString();
    }

    @SuppressWarnings("unchecked")
    private static void write(Object o, StringBuilder sb) {
        if (o == null) {
            sb.append("null");
        } else if (o instanceof Boolean) {
            sb.append(((Boolean) o) ? "true" : "false");
        } else if (o instanceof Long || o instanceof Integer) {
            sb.append(o.toString());
        } else if (o instanceof Double) {
            double d = (Double) o;
            if (d == Math.floor(d) && !Double.isInfinite(d)) {
                // 整数形态的 double 也按整数输出，便于 sml_dump 判为 Int
                sb.append(Long.toString((long) d));
            } else {
                sb.append(o.toString());
            }
        } else if (o instanceof String) {
            quote((String) o, sb);
        } else if (o instanceof List) {
            sb.append('[');
            boolean first = true;
            for (Object e : (List<Object>) o) {
                if (!first) sb.append(',');
                write(e, sb);
                first = false;
            }
            sb.append(']');
        } else if (o instanceof Map) {
            sb.append('{');
            boolean first = true;
            for (Map.Entry<String, Object> e : ((Map<String, Object>) o).entrySet()) {
                if (!first) sb.append(',');
                quote(e.getKey(), sb);
                sb.append(':');
                write(e.getValue(), sb);
                first = false;
            }
            sb.append('}');
        } else {
            throw new IllegalArgumentException("无法序列化为 JSON: " + o.getClass());
        }
    }

    private static void quote(String s, StringBuilder sb) {
        sb.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"': sb.append("\\\""); break;
                case '\\': sb.append("\\\\"); break;
                case '\n': sb.append("\\n"); break;
                case '\r': sb.append("\\r"); break;
                case '\t': sb.append("\\t"); break;
                case '\b': sb.append("\\b"); break;
                case '\f': sb.append("\\f"); break;
                default:
                    if (c < 0x20) {
                        sb.append(String.format("\\u%04x", (int) c));
                    } else {
                        sb.append(c);
                    }
            }
        }
        sb.append('"');
    }

    static byte[] utf8(String s) {
        return s.getBytes(StandardCharsets.UTF_8);
    }
}
