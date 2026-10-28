# 钉住：**裸块 `Type name { }` 的元数据**（`__type` / `__name` 进值树）。
# 本用例**故意不写** `@feature enable typed-block`：C / C++ / Lua 没有 `@feature`，
# 加上就变成「三端直接失败」，测不到元数据这一层。
# 已知（capability-matrix §1 / 三·十）：C / C++ 保留 `__type`/`__name` 元数据，
# **Lua 连元数据都丢** ⇒ 预期 DIVERGE，已登记 known_divergences.md。
User u1 {
  name: alice
  age: 30
}
