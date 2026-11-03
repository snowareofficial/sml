# 钉住：**include "inc/xxx.sml"** 的文本内联（带目录分量）。
# 历史（雷 6 #2 / capability-matrix §1.1）：C++ 一度把裸 `include` 当普通键（静默错树），
# 2026-11-03 先改为明确拒绝（`E-FEATURE-001`），**同日补齐**行首裸形式的真正展开 ⇒
# 现在五端（Rust / C / C++ / JS / Lua）都必须把子文件的字段并进来，本用例应 AGREE。
include "inc/lib.sml"
root: 1
