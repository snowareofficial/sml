# 钉住：**片段以值的形式引用**（`key: &base`）—— 五端都支持的形态。
# 与 11 的区别：这里 `&base` 出现在**值位置**（有键名），不是块内独立一行。
@base { region: cn-north-1  replicas: 2 }
name: web
deploy: &base
