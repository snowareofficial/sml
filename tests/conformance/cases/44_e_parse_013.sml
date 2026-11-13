# conformance: expect-error E-PARSE-013
# P1 先行用例（E 组）：`@for` 循环变量只可用于值位置，键名位置不得插值。
# 期望 Rust 报 E-PARSE-013；四端差异登记见 tests/conformance/known_divergences.md。
@version v1
@feature enable for
hosts: @for h in a b { ${h}_port: 80 }
