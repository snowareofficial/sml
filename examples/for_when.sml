# SPDX-License-Identifier: MulanPSL-2.0
# 解析期条件 `@when` 与解析期循环 `@for` 组合示例。
#
# 运行（需 Rust 端 soupc，且文档已 `@feature enable when`/`for`）：
#   soupc examples/for_when.sml
#
# 本文件演示：
#   1. `@when` 按环境变量在解析期裁剪字段；
#   2. `@for` 把有限列表展开为数组，循环体用 `${h}` 只读插值；
#   3. 组合陷阱：外层 `@when` 写在 `@for` 字段前，条件为假则整个字段消失。

@version v1
@feature enable when
@feature enable for

# 仅 prod 环境出现的日志级别
@when $env.ENV == "prod"
log_level: warn

# 调试模式开关（真值测试：非空且非 "0"/"false"）
@when $env.DEBUG
verbose: true

# 有界循环：三台主机展开为数组
hosts: @for h in web api db {
  name: "${h}"
  port: 8080
}

# 嵌套循环：矩阵单元
matrix: @for r in 1 2 {
  row: "${r}"
  cols: @for c in x y {
    cell: "${r}-${c}"
  }
}
