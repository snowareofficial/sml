# conformance: expect-error E-CONTRACT-019
# 钉住：同一契约里**重复声明**同名 featag ⇒ E-CONTRACT-019（五端同码）。
# 只查声明处；使用处允许同名多次（`!critical !critical` 会原样留在 __featags 里）。
@version v4
@feature enable type.featag
@contract S {
    name: str
    !critical
    !critical
}
s { @is S name: a }
