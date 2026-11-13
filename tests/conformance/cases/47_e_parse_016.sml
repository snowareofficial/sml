# conformance: expect-error E-PARSE-016
# P1 先行用例（E 组）：`@when` 只作用于紧邻下一个字段/块，连续出现即为错。
# 期望 Rust 报 E-PARSE-016；四端差异登记见 tests/conformance/known_divergences.md。
@version v1
@feature enable when
@when $env.D == "prod"
@when $env.D == "dev"
a: 1
