# conformance: expect-error E-PARSE-011
# `@` **紧跟**非名字记号（这里是 `{`，中间无空白）⇒ 不是「孤立 @」（那是 E-PARSE-004），
# 而是「at 符号后缺名字」⇒ Rust 报 E-PARSE-011。
@{
