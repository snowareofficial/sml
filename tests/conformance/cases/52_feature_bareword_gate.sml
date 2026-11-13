# conformance: expect-error E-FEATURE-005
# `@feature disable bareword-string` 后纯字符串裸词必须加引号（五端同码）。
# 判据与 Rust `sml-lex` 的 `!features.has(BarewordStr)` 同源；v2/v3 的严格模式走同一个码。
@feature disable bareword-string
k: hello
