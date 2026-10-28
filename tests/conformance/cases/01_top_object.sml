# 钉住：顶层键值块 + 三种块写法（块冒号可省 / 显式冒号块 / 单行内联块）+ 单行数组。
# 这是最基础的形态，五端都必须一致；它同时是「键序不该影响比对」的对照组
# （C 保源序、Rust 用有序表 —— 归一化后键序差异不得出现在结果里）。
name: demo
ports: [ 80 443 ]
deploy {
  region: cn-north-1
  replicas: 2
}
meta: { owner: ops  level: 3 }
