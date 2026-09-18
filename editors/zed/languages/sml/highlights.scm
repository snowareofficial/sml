; SML 高亮查询（Zed / Tree-sitter）。
;
; ⚠️ 节点名是本文件与 ../grammars/sml/grammar.js 之间的契约，见那份 grammar 的文件头表格。
;    重命名 grammar 里的节点而不改这里，高亮会**静默失效**（不报错，只是不上色）。
;
; ⚠️ 本文件位置就是 Zed 读取的位置（languages/sml/highlights.scm）。
;    `smltools --to highlight` 生成的是 `zed/highlights.scm` —— 那是**待复制**的暂存路径，
;    Zed 不会去读它。用方言定制高亮时，把生成结果复制到本文件位置即可。

(comment) @comment

(string) @string
(escape_sequence) @string.escape

(number) @number
["true" "false"] @boolean
(null) @constant

; `$env.PORT` 与 `&base`
(env_var) @variable
(fragment_ref) @variable

; `foo:` 里的 foo
(key) @property

; `@contract Foo` / `@is Foo` / `@type Foo` 里的 Foo
; 已知小瑕疵：`@version v4`、`@feature base` 的参数也会被当作类型名着色 —— 语法层
; 分不出「指令名是什么」（那要看文本），要精确限定得用谓词 + 辅助捕获，见 README。
(type_name) @type

; `@form` / `@contract` …（节点文本含 `@`）
(directive) @keyword

["{" "}" "[" "]" ":" ","] @punctuation
