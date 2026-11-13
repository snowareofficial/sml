# conformance: expect-error E-PARSE-015
# P1 先行用例（E 组）：`@when` 的比较运算符后缺少比较值。
# 期望 Rust 报 E-PARSE-015；四端差异登记见 tests/conformance/known_divergences.md。
@version v1
@feature enable when
@when $env.D ==
a: 1
