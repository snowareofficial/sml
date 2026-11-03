# conformance: expect-error E-PARSE-005
# 钉住：**未注册指令不能静默变成片段**。
# `@form F { x: 1 }` 看起来像外置指令，但类型/名字是位置参数形式（自 v4 废弃）
# ⇒ 五端应统一报 E-PARSE-005，而不是把它收成名为 `form` 的片段（幽灵片段会让用户以为指令生效了）。
# 2026-11-03 实测：Rust 与 JS 均已同码报错（TODO 里「Rust 报错 vs JS 静默」那条已过期）。
@form F { x: 1 }
