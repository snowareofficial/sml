# 钉住：**loose 契约放宽未声明字段**（默认严格，放宽必须显式写 `loose`）。
# 期望：m = { latency: 12.5, customCounter: 7 } —— 未声明的 customCounter 保留下来。
# 对照：同样的输入在严格契约下应报 E-CONTRACT-003（本用例不测那一格）。
@contract Metrics loose {
    latency: num min 0
}

m { @is Metrics  latency: 12.5  customCounter: 7 }
