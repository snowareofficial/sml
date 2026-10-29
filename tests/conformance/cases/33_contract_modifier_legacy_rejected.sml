# conformance: expect-error E-PARSE-006
# 遗留契约符号修饰符 `?` / `=` 已废弃；只保留单词式 `optional` / `default`。
# 两个方向一并钉住，防止任一实现重新静默接受、吞掉符号或造出垃圾字段。
@contract Addr {
    city: str
    country: str = CN
    zip: str?
}

a { @is Addr  city: beijing }
