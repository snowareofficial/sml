# 钉住：**顶层对象数组**（`[ { } { } ]`）。
# 历史：Rust 曾只认键值块 ⇒「能 dump 出来、读不回去」；C 的 parse_array 曾没有 `[` 分支
# ⇒ 内层 `]` 被外层当结束符、剩余 token 变成假键。
# 已知（capability-matrix §1）：`top-level-array` 只有 Rust + C 支持，
# JS 报 E-PARSE-003、C++/Lua 报 E-PARSE-006 ⇒ 本用例预期 DIVERGE，已登记 known_divergences.md。
# ⚠️ 值里刻意**不用**裸写的 `2026-08-30T10:00:00Z`：含 `:` 的裸词五端都按 E-PARSE-006 拒
# （README 的顶层数组示例恰好写了它 ⇒ 那个示例本身跑不过，已记进 known_divergences.md）。
[
  { to: a@b.c  status: ok }
  { to: x@y.z  status: fail }
]
