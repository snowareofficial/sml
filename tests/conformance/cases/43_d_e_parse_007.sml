# conformance: expect-error E-PARSE-007
# P1 先行用例（D 组）：裸词中含逗号须显式报错，不许当普通分隔符切分。
# 期望五端同报 E-PARSE-007；差异登记见 tests/conformance/known_divergences.md。
# 收敛后把本文件移入 cases/ 参与默认套件。
a: 1, 2
