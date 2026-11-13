# conformance: expect-error E-FEATURE-001
# `@feature disable top-level-array` 后顶层数组必须被拒（五端同码）。
# 三端为此补齐了词法前的 `@feature` 预处理扫描，见 docs/swsml/plan/crossend-sync.md §0.7。
@feature disable top-level-array
[1 2]
