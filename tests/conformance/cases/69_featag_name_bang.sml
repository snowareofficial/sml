# conformance: expect-error E-PARSE-028
# 钉住 B3：featag 区结束后，块名位置出现 `!` 开头的词 ⇒ E-PARSE-028。
@version v4
@feature enable type.featag
@contract S {
    name: str
    !x
}
S !x, !name { }
