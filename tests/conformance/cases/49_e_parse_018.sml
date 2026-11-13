# conformance: expect-error E-PARSE-018
# P1 先行用例（E 组）：`@for` 的 `in` 之后至少须有一个枚举项。
# 期望 Rust 报 E-PARSE-018；四端差异登记见 tests/conformance/known_divergences.md。
@version v1
@feature enable for
hosts: @for h in { name: "${h}" }
