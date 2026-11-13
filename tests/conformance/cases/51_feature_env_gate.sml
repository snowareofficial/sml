# conformance: expect-error E-FEATURE-002
# `@feature disable env` 后引号串含 `$env.` 必须被拒（五端同码）。
# 三端为此把预处理特性集接进了词法 / 值处理层，见 docs/swsml/plan/crossend-sync.md §0.7。
@feature disable env
k: "$env.NOPE"
