# 钉住：**数组位置的裸块** —— `[ A B item { … } ]` 是**一个**元素（块），不是四个标量。
# Rust 的 `parse_array_inner` 与 `parse_block` 进的是同一个 `bare_child_frame`，
# 故多契约、块名、featag 区与 `__type` / `__name` / `__featags` 元数据逐字同构。
# `zs` 一格反过来钉判据：词序列后没有 `{` ⇒ 仍是两个标量元素（不是块）。
@feature enable typed-block, block-name, type.featag

@contract A { a: int }
@contract B { b: int }
@contract S { name: str !critical }

xs: [ A B item { a: 1, b: 2 } ]
ys: [ A { a: 3 } ]
fs: [ S !critical srv { name: a } ]
zs: [ plain word ]
