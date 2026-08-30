# 被主文件 glob-include 的 widget 定义（需 glob-include 特性）
# 注意：多个被 include 的文件若顶层出现同名键，会被聚合为数组。
# 这里用唯一键 widget_login / widget_search 各自独立可见。
widget_login {
    kind: form
    endpoint: /auth/login
    timeout: 30
}
