# 钉住：**块内独立一行的 `&base` ⇒ 字段展开（splice）**。
# 参照实现 Rust：`w` 得到 {a, b, c} 三个字段合并；C / C++ 未实现，会把裸词 `&base` 当键名
# ⇒ 结果里多一个 `&base` 键（严格契约下还会报 E-CONTRACT-004）。
# 这是 README 中英曾写反、由 W4-片段行级展开那条待办钉住的已知差异 ⇒ 预期 DIVERGE。
@base { a: 1  b: 2 }
w {
  c: 3
  &base
}
