# 钉住：**大于 2^53 的整数**（大整数分裂）。
# 现状（2026-11-14 实测）：
#   Rust / C / C++ 精确保留为整数；
#   JS / Lua 都**退化成字符串**（保真、类型变了）—— 两端的数值模型只有 double。
# ⇒ 三种口径 ⇒ 本用例如实 DIVERGE，按 run_conformance.py 的 WAIVERS / WAIVER_SIG 放行
#   （只抹平「十进制数字串 ↔ 整数」），理由见 known_divergences.md #11 / B-01。
#
# ⚠️ 历史上这条曾判 AGREE：JS `stringify` 对字符串不加引号 ⇒ 回读成 int ⇒ 假绿。
#   四端 dump 的引号判据已照 Rust `text_rules` 重写，数字样字符串一律带引号，盲区关闭。
id: 330106201503071234
imax: 9223372036854775807
