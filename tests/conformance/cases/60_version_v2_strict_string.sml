# conformance: expect-error E-FEATURE-005
# P5：`@version v2` 起裸词字符串必须加引号（版本决定特性**基集**，不只是字面量）。
# 五端判据一致：v1 开 bareword-string，v2/v3/v4 关。
@version v2
k: abc
