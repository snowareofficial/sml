# conformance: expect-error E-PARSE-028
# 钉住 B3：块头首词（类型词）以 `!` 开头 ⇒ E-PARSE-028（featag 与块名歧义）。
@version v4
@feature enable type.featag
!S name { }
