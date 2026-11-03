# 钉住：**include "inc/xxx.sml"** 的文本内联（带目录分量）。
# 已知（雷 6 #2 / capability-matrix §1.1）：**C++ 不支持裸 `include`**。
# 2026-11-03 起它**明确拒绝**（`E-FEATURE-001`，不再静默当成 `include: inc/lib.sml` 这条错树）
# ⇒ C++ 这一格是 **PARSE_FAIL** 而非 AGREE。按台账口径 PARSE_FAIL 与 DIVERGE 都算不一致，
# 所以本用例仍是 DIVERGE —— 但失败模式从「静默错树」变成「响亮报错」。
# 其余四端（Rust / C / JS / Lua）都必须把子文件的字段并进来。
include "inc/lib.sml"
root: 1
