# 内置契约表样例：只用于 `--dump-builtins` 的验收（不进产品路径）。
# 形状对齐 docs/weakbind-spec-draft.md §6.2。
@contract tls {
    host: str required
    port: int optional
    sni:  str default "a.example.com"
}

@contract cache loose {
    ttl: int min 1 max 86400
    mode: enum [lru fifo]
}
