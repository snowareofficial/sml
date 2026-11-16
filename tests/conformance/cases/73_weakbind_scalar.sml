# conformance: expect-error E-PARSE-029
# 钉住 B3：`@weakbind` 后是标量（不是块）⇒ E-PARSE-029（五端同码）。
@feature enable type.weak
@weakbind tls
a: 1
