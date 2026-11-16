# 契约数组的元素里缺一个**带 default** 的字段。
# 口径（P11 裁决）：元素位置**只校验、不回填**；`default` 自动视为可选
# （教科书 `ch05-contract` 的修饰符表：「`default <值>`：缺失时填入默认值（同时自动视为可选）」）
# ⇒ 缺它**不算缺失**（不报错），但也**不回填** ⇒ 元素里就没有这个键（五端值树逐格相同）。
@contract Endpoint { host: str  port: int default 8080 }
@contract Cluster { peers: [Endpoint] }
c { @is Cluster
  peers: [ { host: a } ]
}
