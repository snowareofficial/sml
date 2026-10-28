# 钉住：**链式 include**（主文件 → inc/chain.sml → inc/lib.sml）。
# 判据是最终值树里有 lib.deep / sub / chain / root 四处 —— 漏展开任何一层都会少键。
# 已知：C++ 连裸 include 都不支持 ⇒ 预期 DIVERGE（见 08 的说明）。
include "inc/chain.sml"
root: 1
