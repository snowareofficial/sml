# conformance: expect-error E-PARSE-029
# 钉住 B4：`@weakbind` 后是数组（不是块）⇒ E-PARSE-029（五端同码）。
@feature enable type.weak
@weakbind tls
a: [1 2]
