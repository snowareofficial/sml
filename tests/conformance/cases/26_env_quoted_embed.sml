# 钉住：**引号内嵌入 `pre-$env.X-post`** 的口径（三档中的第三档）。
# 已知：只有 Rust 做真正的字符串内插值；C / C++ / JS / Lua 的 `$env` 匹配是**整串锚定**的
# ⇒ 原样保留 `pre-$env.SML_CONF_TOKEN-post` ⇒ 预期 DIVERGE，已登记 known_divergences.md。
token: "pre-$env.SML_CONF_TOKEN-post"
