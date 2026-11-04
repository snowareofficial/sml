# `.smlex` 的形状 —— 自举（用 SML 描述 SML 的 profile 格式）
#
# 用途：给编辑器/插件提供补全与悬停的形状来源。
#   smltools -i rust/smltools/profile.sml --dump-builtins > profile-builtins.json
#
# ⚠️ 这些契约**只描述形状**，不执行任何检查；`.smlex` 的解释权在工具侧。
#    规范见 docs/smlex-format-draft.md（v1.0）。
# ⚠️ 全部用 loose：格式明确要求「未知字段 ⇒ warning + 忽略」（前向兼容），
#    若用 strict，一个更新版本的 profile 在老工具里就会被判成错。

@contract profile loose {
    name: str required
    version: int required
    description: str optional
    applies_to: applies_to optional
    checks: checks optional
    hints: hints optional
}

@contract applies_to loose {
    glob: [str] optional
    requires: [str] optional
}

@contract checks loose {
    require_top_level: [str] optional
    forbid_contracts: [str] optional
    forbid_keys: [str] optional
}

@contract hints loose {
    keys: [str] optional
}

# `hints.values` 是「键 → 候选数组」，键名开放，无法用字段语法表达 ⇒ 不声明，
# loose 下未声明字段照常放行（这正是选 loose 的实际用处）。
# `emit` 是「后端名 → 参数块」，后端名由各工具自己定 ⇒ 同上，只留一个空壳。
@contract emit loose {
}
