# conformance: expect-error E-PARSE-014
# P1 先行用例（E 组）：`@when` 左侧只接受 `$env.NAME`，文档内字段引用不支持。
# 期望 Rust 报 E-PARSE-014；四端差异登记见 tests/conformance/known_divergences.md。
@version v1
@feature enable when
@when foo == "bar"
a: 1
