# conformance: expect-error E-CONTRACT-004
# 严格契约的**数组元素**里多了一个契约未声明的键 ⇒ E-CONTRACT-004。
# 元素位置的契约**只校验、不回填**（元素不是字段，回填会改用户的树）；
# 本用例钉的是「元素也守严格性」这一格：契约级严格（默认）时，元素里的额外键必须被拒。
# ⚠️ 各端文案不同（C 端没有 path 概念，只报字段名；Rust 报 `c.peers[0].port`）：
#    **码是稳定契约、文案不是** ⇒ 同码即算一致。
@contract Endpoint { host: str }
@contract Cluster { peers: [Endpoint] }
c { @is Cluster
  peers: [ { host: a  port: 8080 } ]
}
