
static std::vector<Token> tokenize(const std::string& text, std::string* err) {
    std::vector<Token> toks;
    size_t i = 0, n = text.size();
    size_t line = 1;
    while (i < n) {
        char c = text[i];
        char c2 = (i + 1 < n) ? text[i+1] : '\0';

        if (c == '\n') { line++; i++; continue; }
        if (is_space(c)) { i++; continue; }

        // comments
        if (c == '#') { while (i < n && text[i] != '\n') i++; continue; }
        if (c == '-' && c2 == '-') { while (i < n && text[i] != '\n') i++; continue; }
        if (c == '/' && c2 == '/') { while (i < n && text[i] != '\n') i++; continue; }
        if (c == '/' && c2 == '*') {
            i += 2;
            bool closed = false;
            while (i + 1 < n && !(text[i] == '*' && text[i+1] == '/')) {
                if (text[i] == '\n') line++;
                i++;
            }
            if (i + 1 < n) { i += 2; closed = true; }
            if (!closed) { if (err) *err = code_prefix(SML_E_LEX_002,
                             "sml: 未闭合的块注释 /* ... */（遇到文件结尾）"); return toks; }
            continue;
        }
        if (c == '_' && c2 == '*') {
            i += 2;
            bool closed = false;
            while (i + 1 < n && !(text[i] == '*' && text[i+1] == '_')) {
                if (text[i] == '\n') line++;
                i++;
            }
            if (i + 1 < n) { i += 2; closed = true; }
            if (!closed) { if (err) *err = code_prefix(SML_E_LEX_003,
                             "sml: 未闭合的块注释 _* ... *_（遇到文件结尾）"); return toks; }
            continue;
        }

        if (c == '$' && i + 1 < n && text[i + 1] == '{') {
            // `${var}` 整体作为一个 Word（`@for` 插值）：否则 `{` / `}` 会被当结构记号切开，
            // 键名位置也认不出 `${`（`${h}_port` 会散成 `${h`、`}`、`_port`）。
            // Keep `${var}` as one Word so it survives structural splitting.
            size_t close = text.find('}', i + 2);
            if (close != std::string::npos) {
                toks.emplace_back(Token::T::Word, text.substr(i, close - i + 1), line);
                i = close + 1;
                continue;
            }
        }
        switch (c) {
            case '{': toks.emplace_back(Token::T::LBrace, "{", line); i++; continue;
            case '}': toks.emplace_back(Token::T::RBrace, "}", line); i++; continue;
            case '[': toks.emplace_back(Token::T::LBracket, "[", line); i++; continue;
            case ']': toks.emplace_back(Token::T::RBracket, "]", line); i++; continue;
            case ',': toks.emplace_back(Token::T::Comma, ",", line); i++; continue;
            case ':': toks.emplace_back(Token::T::Colon, ":", line); i++; continue;
            case '@': {
                // 孤立 `@`：其后紧跟空白 / 行尾 ⇒ 没有指令名或片段名。
                // 与 Rust 的 `Tok::BareAt` 同判据（sml-lex/src/lib.rs 的 `'@'` 分支）。
                const char n2 = (i + 1 < n) ? text[i + 1] : '\0';
                const bool bare = (n2 == '\0' || n2 == ' ' || n2 == '\t' || n2 == '\n' || n2 == '\r');
                toks.emplace_back(bare ? Token::T::BareAt : Token::T::At, "@", line);
                i++; continue;
            }
            /* ⚠️ `$` **不再**切出独立 token：Rust 的词法器不把 `$` 当特殊字符，
               `$env.NAME` 整个是一个**裸词**（`coerce_word` 里早有 `$env.` 分支，
               见 sml-lex/src/lib.rs 的 coerce_word 与 "$env 内联" 的既有注释）。
               改前这里发 Token::T::Dollar，于是值位置的 `k: $env.X` 被切成
               `$` + `env.X` 两个 token：值退化成 null，而 `env.X` 掉到**键位置变出一个新键**
               —— 实测 examples/secrets.sml 得到 `resendApiKey: null` 与凭空多出的
               `env.RESEND_API_KEY` 键（Rust 是 `resendApiKey: ""`）。
               删掉这一支后，下面的裸词循环会把 `$env.X` 整体读成一个词，与 Rust 同构。 */

            case '"': {
                i++;
                std::string buf;
                bool closed = false;
                while (i < n) {
                    char cc = text[i];
                    if (cc == '"') { closed = true; i++; break; }
                    // 转义符后**直接**遇文件结尾 ⇒ E-LEX-006（与 Rust 词法层同码）。
                    // 语义上不是「少了结束引号」，而是「少了一个被转义的字符」。
                    // T21 裁决（2026-11-06）：五端统一到 006。
                    if (cc == '\\' && i + 1 >= n) {
                        if (err) *err = code_prefix(SML_E_LEX_006,
                            "sml: 字符串中的转义符后遇到文件结束");
                        return toks;
                    }
                    if (cc == '\\' && i + 1 < n) {
                        char e = text[i+1];
                        if (e == 'n') buf.push_back('\n');
                        else if (e == 't') buf.push_back('\t');
                        else if (e == 'r') buf.push_back('\r');
                        else if (e == '0') buf.push_back('\0');
                        /* 接受集必须与 Rust 的严格集一致（见 sml-lex/src/lib.rs:210 的
                           自述文案）：\n \t \r \0 \" \\ \uXXXX。
                           此前这里还接受 C 风格的 \a \b \f \v \'，而 Rust 对它们报
                           E-LEX-004（「未知转义」）—— 同一输入两端行为不同。按
                           E-LEX-004 的 impls([rust cpp]) 与「严格策略」note 收窄。 */
                        else if (e == '\\') buf.push_back('\\');
                        else if (e == '"') buf.push_back('"');
                        else if (e == 'u') {
                            // \u{XXXX} or \uXXXX handled later in unescape_unicode
                            buf.push_back('\\'); buf.push_back('u');
                            i += 2;
                            // copy the rest of the escape verbatim until non-hex (or closing brace)
                            while (i < n) {
                                char h = text[i];
                                if (h == '}') { buf.push_back('}'); i++; break; }
                                if (is_xdigit_( (unsigned char)h ) ||
                                    (h>='A'&&h<='F')||(h>='a'&&h<='f')) {
                                    buf.push_back(h); i++;
                                } else break;
                            }
                            continue;
                        }
                        else {
                            /* 未知转义：**报错，绝不静默保留**。
                               codes.sml 的 E-LEX-004 明确把 cpp 列进 impls，note 是
                               「严格策略：未知转义即失败，避免路径与正则被静默损坏」——
                               但此处此前把 `\` 与字符双双塞回 buf 静默放行
                               （探针实测 `k: "\z"` 返回成功、无错误，见 _w10_probe_log.txt
                               「LEX-004 unknown escape | ok=1 | err=」）。
                               报错后立即收手，与未闭合字符串走同一出口约定。 */
                            if (err) *err = code_prefix(SML_E_LEX_004,
                                std::string("sml: 字符串含未知转义符 \\") + e +
                                "（仅支持 \\n \\t \\r \\0 \\\" \\\\ \\uXXXX）");
                            return toks;
                        }
                        i += 2;
                        continue;
                    }
                    buf.push_back(cc);
                    if (cc == '\n') line++;
                    i++;
                }
                if (!closed) { if(err)*err=code_prefix(SML_E_LEX_001, "sml: unterminated quoted string"); return toks; }
                // store quoted literal with surrounding quotes to mark as string
                toks.emplace_back(Token::T::Word, "\"" + buf + "\"", line);
                continue;
            }
            default: break;
        }

        // bare word: read until whitespace or structural char
        // Note: '@','$' handled above as separate tokens ONLY at word start;
        // inside a word they are ordinary characters.
        std::string buf;
        while (i < n) {
            char cc = text[i];
            if (is_space(cc)) break;
            if (cc == '{' || cc == '}' || cc == '[' || cc == ']' ||
                cc == ',' || cc == ':' || cc == '#') break;
            if (cc == '/' && i+1 < n && (text[i+1]=='/' || text[i+1]=='*')) break;
            if (cc == '-' && i+1 < n && text[i+1]=='-') break;
            if (cc == '_' && i+1 < n && text[i+1]=='*') break;
            buf.push_back(cc);
            i++;
        }
        if (!buf.empty()) toks.emplace_back(Token::T::Word, buf, line);
    }
    return toks;
}

// ===========================================================================
// coerce  (mirrors Rust coerce / coerce_word)
// ===========================================================================
static bool looks_like_int(const std::string& t) {
    if (t.empty()) return false;
    size_t s = 0;
    if (t[0]=='+'||t[0]=='-') s=1;
    if (s>=t.size()) return false;
    bool any=false;
    for (size_t k=s;k<t.size();k++){ if(!is_digit(t[k])) return false; any=true; }
    return any;
}
static bool looks_like_float(const std::string& t) {
    // matches Rust float regex
    // ^[+-]?(\d+\.\d*|\.\d+|\d+)([eE][+-]?\d+)?$
    std::string s = t;
    size_t i=0;
    if (!s.empty() && (s[0]=='+'||s[0]=='-')) i=1;
    if (i>=s.size()) return false;
    bool has_dot=false, has_digit=false, has_e=false;
    for (; i<s.size(); i++){
        char c=s[i];
        if (is_digit(c)) has_digit=true;
        else if (c=='.') { if(has_dot||has_e) return false; has_dot=true; }
        else if (c=='e'||c=='E') { if(!has_digit||has_e) return false; has_e=true; has_digit=false; }
        else if ((c=='+'||c=='-') && has_e && (i+1<s.size()) && is_digit(s[i+1])) { /* ok */ }
        else return false;
    }
    return has_digit && (has_dot || has_e);
}
