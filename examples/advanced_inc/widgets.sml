# 用于演示「部分引用」的单文件：含多个顶层 widget 键
# 主文件可 import "advanced_inc/widgets.sml" { widget_login, widget_search }
# 只挑出指定键，extra 键不会被引入。
widget_login {
    kind: form
    endpoint: /auth/login
    timeout: 30
}
widget_search {
    kind: search
    endpoint: /api/search
    timeout: 15
}
extra_secret {
    token: "should-not-leak"
}
