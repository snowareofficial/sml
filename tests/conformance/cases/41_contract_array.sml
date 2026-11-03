# 钉住：**契约数组** `peers: [Endpoint]` —— 数组元素逐个按被引用契约校验。
# 背景：Rust 侧直到 2026-11-03 才支持（此前解析器在数组元素位置只认内置/外置类型，
# 非内置名一律报 E-CONTRACT-012）；JS 因递归 parseFieldSpec 天然支持。
# 本用例只需「能解析且值树一致」——元素的**校验**由各端的契约用例负责，不在这里比。
@contract Endpoint {
  host: str
  port: int optional
}
@contract Cluster {
  name: str
  peers: [Endpoint]
}
c { @is Cluster
  name: edge
  peers: [
    { host: a }
    { host: b port: 8080 }
  ]
}
