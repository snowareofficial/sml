@version v4

# 用 custom 后端把 SML 描述转换成 Dockerfile
# 约定（顶层字段）：base / maintainer / workdir / ports / deps / cmd
# 注意：custom 仅支持固定占位符 {value}/{key}/{nested}/{items:TPL}，
#       且 select_rule 会用「字段名」匹配 match 规则，
#       所以这里直接用 match: <字段名> 命中，并按此顺序定序输出。
rules: [
  { match: "base"      template: "FROM {value}\n" }
  { match: "maintainer" template: "MAINTAINER {value}\n" }
  { match: "workdir"   template: "WORKDIR {value}\n" }
  { match: "ports"     template: "{items:EXPOSE {value}\n}" }
  { match: "deps"      template: "{items:RUN apt-get update && apt-get install -y {value}\n}" }
  { match: "cmd"       template: "CMD {value}\n" }
]
