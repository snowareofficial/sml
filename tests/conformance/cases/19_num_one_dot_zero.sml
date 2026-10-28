# 钉住：**`1.0` 与 `1` 是不是同一件事**（浮点身份）。
# 已知（capability-matrix §1.3 #5）：只有 Rust 把 `1.0` 保留为浮点，
# C / C++ / JS / Lua 一律归一为整数 `1` ⇒ 预期 DIVERGE，已登记 known_divergences.md。
# 这一格正是「不能只用 JSON 当比对载体」的理由：`smltools --to json` 会把 `1.0` 写成 `1`，
# 只比 JSON 会把它静默吃掉 ⇒ 本套件另有一层「规范 SML 文本」比较，见 run_conformance.py。
ratio: 1.0
count: 1
