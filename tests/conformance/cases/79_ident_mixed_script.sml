# conformance: expect-error E-LEX-007
# 「每契约单一语种」（门控 `lex.ident`）：同一契约内的标识符必须同属一种文字系统。
# 这里是拉丁（name）与汉字（端口）混用 ⇒ Rust 报 E-LEX-007。
@feature enable lex.ident
@contract C { name: str 端口: int }
k: 1
