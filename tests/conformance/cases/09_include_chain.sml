# 钉住：**链式 include**（主文件 → inc/chain.sml → inc/lib.sml）。
# 判据是最终值树里有 lib.deep / sub / chain / root 四处 —— 漏展开任何一层都会少键。
# 历史：C++ 一度连裸 include 都不支持；2026-11-03 补齐行首裸形式的展开（与另四端同判据）
# ⇒ 该格现在应 AGREE（见 08 的说明）。
include "inc/chain.sml"
root: 1
