# SML 自定义高亮配置（由「SML: 应用特殊颜色」生成；本文件自身也是 SML）
#
# 每个顶层块 = 一组：words 必填；color / background / bold / italic / underline /
# matchCase 可选；unit 可选，限定只对某种**语法单元**生效
# （contract / fragment / type / key / directive；缺省或 text = 按普通词着色）。

text_gateway: 
{
  words: [
    gateway
  ]
  color: "#e06c75"
  unit: text
}

