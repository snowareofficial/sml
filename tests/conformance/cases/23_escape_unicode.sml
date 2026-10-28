# 钉住：**固定四位 `\uXXXX`** 转义（不是 `\u{...}`）。
# 已知两条静默缺陷（雷 6 #3 / capability-matrix §1.2）：
#   JS：`js/sml.mjs` 的 `i += 4` 少跳一位 ⇒ `a\u0041b` 解出 `aA1b`（正确应为 `aAb`）；
#   C++：只认 `\u{41}`，固定四位形式报 E-LEX-005（这里即「一端报错、其余成功」）。
# ⇒ 预期 DIVERGE，已登记 known_divergences.md。
u1: "a\u0041b"
u2: "x\u00e9y"
