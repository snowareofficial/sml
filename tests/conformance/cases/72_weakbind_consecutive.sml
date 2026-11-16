# conformance: expect-error E-PARSE-029
# 钉住 B2：连续两条 `@weakbind` ⇒ E-PARSE-029（五端同码）。
@feature enable type.weak
@weakbind a
@weakbind b
web { }
