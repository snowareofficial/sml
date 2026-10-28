# 钉住：**引号内整串恰为 `$env.X`** 的口径（三档中的第二档）。
# 已知（capability-matrix §1.4 #6）：Rust / C++ / JS / Lua 会替换；
# **C 不替换**（C 只在裸词做 `$env`）⇒ 预期 DIVERGE，已登记 known_divergences.md。
token: "$env.SML_CONF_TOKEN"
