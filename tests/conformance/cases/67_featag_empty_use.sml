# conformance: expect-error E-PARSE-026
# 钉住 B1：块头 featag 区里 `!` 后面没有名字 ⇒ E-PARSE-026（不是 021）。
@version v4
@feature enable type.featag
@contract S {
    name: str
    !x
}
S !, name { }
