# 钉住：**include "inc/xxx.sml"** 的文本内联（带目录分量）。
# 已知（雷 6 #2 / capability-matrix §1.1）：**C++ 不支持裸 `include`** ——
# 它把整条指令静默当成键值 `include: inc/lib.sml`（不报错也不展开）⇒ 本用例预期 DIVERGE。
# 其余四端（Rust / C / JS / Lua）都必须把子文件的字段并进来。
include "inc/lib.sml"
root: 1
