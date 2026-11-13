# conformance: expect-error E-PARSE-017
# P1 先行用例（E 组）：悬空的 `@when`（后面没有字段或块）。
# 期望 Rust 报 E-PARSE-017；四端差异登记见 tests/conformance/known_divergences.md。
@version v1
@feature enable when
a: 1
@when $env.D == "prod"
