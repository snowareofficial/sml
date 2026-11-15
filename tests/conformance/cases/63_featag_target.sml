# 钉住：featag 的 **S6 落点填充 / S7 from-featag / auto 占位 / 契约默认值** 五端同树。
# 判据是值树逐格相同，所以这里必须是**正例**：落点 `-> layout.density` 造出的键、
# `auto` 取声明里的 default、`from-featag` 字段由 featag 写入，三者都要真的进树。
# 单独一格的反证（未在本文）：同一份 `-> layout.density` 配**严格**契约时，五端同为
# E-CONTRACT-004 —— 说明 S6 都在契约应用**之前**跑（Rust `finish_bare_block` 同序）。
@feature enable typed-block, block-name, type.featag
@contract Menu loose {
    title: str
    !density: str default compact -> layout.density
}
@contract Seal loose {
    mark: str from-featag
    !mark: str -> mark
}
@contract Edge {
    region: str default cn-north-1
    !probe
}
Menu !density[auto], m1 { title: a }
Seal !mark[x], s1 { }
Edge !probe, e1 { }
