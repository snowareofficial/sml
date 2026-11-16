# conformance: expect-error E-PARSE-030
# 钉住：同一文档重复引用同一弱 profile（跨块也算）⇒ E-PARSE-030（五端同码）。
@feature enable type.weak
@weakdoc api-docs
section { @weakdoc api-docs }
