# 钉住：契约**组合**（字段的类型是另一个契约，而不是继承）。
# 期望：o = { name: demo, inner: { x: 1, tag: t（由子契约默认值回填） } }
# 组合是多层嵌套 + 默认值回填两条路径的交叉点 ⇒ 任何一端只做一层都会露馅。
@contract Inner {
    x: int
    tag: str default t
}

@contract Outer {
    name: str
    inner: Inner
}

o { @is Outer  name: demo  inner { x: 1 } }
