// SPDX-License-Identifier: MIT OR Apache-2.0 OR MulanPSL-2.0
// 构造值边界用例的 oracle：用 ValueDesc 直接建树，覆盖记法降级、compact、inline、
// 注释与数值排版边界。输出 `tests/golden/render/edge.<scheme>.render`。
// Builder oracle for constructed-value edge cases: notation fallback, compact,
// inline, comments and number layout boundaries.
//
// 依赖上游 Eclog-CPP 的 amalgamation（MIT，Copyright (c) Vallest Systems LLC）。
// 与 `tests/render_merge.rs` 的 `edge_tree()` 逐项对应，改一处必须同步另一处。

#include <Eclog.h>

#include <cstdio>
#include <cstring>
#include <limits>
#include <string>

#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#endif

using namespace vallest;

int main(int argc, char** argv)
{
    if (argc < 2) {
        fprintf(stderr, "usage: edge <typical|compact>\n");
        return 64;
    }

#if defined(_WIN32)
    _setmode(_fileno(stdout), _O_BINARY);
#endif

    const bool compact = strcmp(argv[1], "compact") == 0;

    eclog::RendererConfig rc;
    if (compact) {
        rc.formatting = eclog::RendererConfig::formatting_compact;
    }

    eclog::MemoryOutputStream os;
    eclog::Renderer renderer(os, rc);

    renderer.beginRootObject();
    {
        eclog::ObjectRenderer root(renderer);

        // raw 记法：能否原样输出
        root.renderMember("raw_empty", eclog::StringDesc("", eclog::string_notation_raw));
        root.renderMember("raw_ctrl", eclog::StringDesc("\x1f", eclog::string_notation_raw));
        root.renderMember("raw_newline", eclog::StringDesc("Hello\nWorld!", eclog::string_notation_raw));
        root.renderMember("raw_path", eclog::StringDesc("C:\\Program Files\\Internet Explorer", eclog::string_notation_raw));
        root.renderMember("raw_quote", eclog::StringDesc("\"Hello World!\"", eclog::string_notation_raw));
        root.renderMember("raw_delim_quote", eclog::StringDesc("\"Hello World!\"", eclog::string_notation_raw, "dd"));
        root.renderMember("raw_delim_hit", eclog::StringDesc("\"dd", eclog::string_notation_raw, "dd"));

        // heredoc 记法：能否原样输出
        root.renderMember("hd_empty", eclog::StringDesc("", eclog::string_notation_heredoc, "T"));
        root.renderMember("hd_ctrl", eclog::StringDesc("\x1f", eclog::string_notation_heredoc, "T"));
        root.renderMember("hd_simple", eclog::StringDesc("Hello World!", eclog::string_notation_heredoc, "T"));
        root.renderMember("hd_tab", eclog::StringDesc("\tx\n    y\nz", eclog::string_notation_heredoc, "T"));
        root.renderMember("hd_tag_line", eclog::StringDesc("a\nT\nb", eclog::string_notation_heredoc, "T"));
        root.renderMember("hd_tag_indent", eclog::StringDesc("a\n  T\nb", eclog::string_notation_heredoc, "T"));
        root.renderMember("hd_tag_inside", eclog::StringDesc("a\nT99\nb", eclog::string_notation_heredoc, "T"));
        root.renderMember("hd_no_delim", eclog::StringDesc("x", eclog::string_notation_heredoc));

        // unquoted 记法：保留或降级
        root.renderMember("uq_true", eclog::StringDesc("true", eclog::string_notation_unquoted));
        root.renderMember("uq_inf", eclog::StringDesc("inf", eclog::string_notation_unquoted));
        root.renderMember("uq_bare", eclog::StringDesc("localhost", eclog::string_notation_unquoted));
        root.renderMember("uq_space", eclog::StringDesc("with space", eclog::string_notation_unquoted));
        root.renderMember("uq_empty", eclog::StringDesc("", eclog::string_notation_unquoted));
        root.renderMember("uq_lead_digit", eclog::StringDesc("9lead", eclog::string_notation_unquoted));
        root.renderMember("uq_tail", eclog::StringDesc("a-b.c", eclog::string_notation_unquoted));
        root.renderMember("uq_utf8", eclog::StringDesc("\xe4\xbd\xa0\xe5\xa5\xbd", eclog::string_notation_unquoted));

        // 数值：不看 raw，按数值重新格式化
        root.renderMember("num_zero", 0);
        root.renderMember("num_neg_zero", -0.0);
        root.renderMember("num_seventh", 1.0 / 7.0);
        root.renderMember("num_max", std::numeric_limits<double>::max());
        root.renderMember("num_i64max", std::numeric_limits<long long>::max());
        root.renderMember("num_nan", eclog::Number::nan());
        root.renderMember("num_neg_nan", eclog::Number::negNan());
        root.renderMember("num_inf", eclog::Number::infinity());
        root.renderMember("num_neg_inf", eclog::Number::negInfinity());

        // 数值排版边界：定点/指数的分界、补零、u64 边界、次正规数
        root.renderMember("n_1e-4", 1e-4);
        root.renderMember("n_1e-5", 1e-5);
        root.renderMember("n_neg_1e-4", -1e-4);
        root.renderMember("n_100", 100.0);
        root.renderMember("n_0_001", 0.001);
        root.renderMember("n_1e16", 1e16);
        root.renderMember("n_1e17", 1e17);
        root.renderMember("n_1234_5678", 1234.5678);
        root.renderMember("n_0_1", 0.1);
        root.renderMember("n_1_5", 1.5);
        root.renderMember("n_min_subnormal", 5e-324);
        root.renderMember("n_1e-308", 1e-308);
        root.renderMember("n_123456789012345678", 123456789012345678LL);
        root.renderMember("n_u64max", eclog::NumberDesc(18446744073709551615ULL));
        root.renderMember("n_i64_plus_one", eclog::NumberDesc(9223372036854775808ULL));
        root.renderMember("n_1e18_double", 1e18);

        // 键的记法
        root.renderMember(eclog::KeyDesc("", eclog::string_notation_unquoted), eclog::StringDesc("empty_key", eclog::string_notation_unquoted));
        root.renderMember(eclog::KeyDesc("q", eclog::string_notation_quoted), eclog::StringDesc("quoted_key", eclog::string_notation_unquoted));
        root.renderMember(eclog::KeyDesc("r", eclog::string_notation_raw), eclog::StringDesc("raw_key", eclog::string_notation_unquoted));
        root.renderMember(eclog::KeyDesc("h", eclog::string_notation_heredoc, "T"), eclog::StringDesc("heredoc_key", eclog::string_notation_unquoted));

        // 注释（compact 下会被忽略）
        root.renderComment("line one\nline two");

        // inline：同一行内用逗号
        root.beginArray("inline");
        {
            eclog::ArrayRenderer arr(root);

            arr.beginInline();
            for (int i = 0; i < 3; ++i) {
                arr.renderMember(i);
            }
            arr.endInline();
        }
        root.endArray();
    }
    renderer.endRootObject();

    renderer.close();

    fwrite(os.data(), 1, os.size(), stdout);
    return 0;
}
