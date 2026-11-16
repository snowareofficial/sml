# conformance: expect-error E-LEX-007
# 标识符白名单（门控 `lex.ident`）：键名里的 `.` 是路径语法保留字符 ⇒ 拒绝。
@feature enable lex.ident
a.b: 1
