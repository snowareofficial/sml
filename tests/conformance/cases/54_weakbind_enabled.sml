# conformance: expect-ok
# 开了 `type.weak` 之后，`@weakbind` 是**登记但不解释**：主树与去掉该行逐字节相同。
# 五端同码的依据是 `docs/swsml/spec/weakbind.md` §0 / §5（弱绑定零改树）。
@feature enable type.weak
@weakbind tls
server { port: 80 }
