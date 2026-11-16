# conformance: expect-error E-CONTRACT-003
# 契约数组的元素逐个按被引用契约校验：第二个元素缺必填字段 `host` ⇒ 必填缺失。
@contract Endpoint { host: str }
@contract Cluster { peers: [Endpoint] }
c { @is Cluster
  peers: [ { host: a } { } ]
}
