# conformance: expect-error E-PARSE-026
# 钉住 B1：契约体里 `!` 后面没有名字（或又是 `!`）⇒ E-PARSE-026。
@version v4
@feature enable type.featag
@contract S {
    name: str
    !
}
s { @is S name: a }
