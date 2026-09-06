# =============================================================================
#  Crystalic 版登录界面：不再手写 x/y，改用 flow 自动排布
#  ---------------------------------------------------------------------------
#  转换：crystalic -i login_crystalic.sml --to lvgl -o login.xml
#  对比：examples/lvgl/login.sml（旧写法，5 个部件全是硬编码坐标）
# =============================================================================

screen login {
    width: 320
    height: 240

    # 纵向排布：间距 12，四周留白 16，横向居中
    flow: column
    gap: 12
    padding: 16
    align: center

    children: [

    label title {
        text: "欢迎回来"
        height: 28
        width: 100%
    }

    text_input user {
        placeholder-text: "用户名"
        height: 36
        width: 100%
    }

    text_input pass {
        password: true
        placeholder-text: "密码"
        height: 36
        width: 100%
    }

    # 两个按钮并排，中间留 12；各占一半剩余宽度
    obj buttons {
        width: 100%
        height: 40
        flow: row
        gap: 12
        children: [
            button btn_login {
                text: "登录"
                height: 100%
                width: fill
                on_click: on_login
            }
            button btn_cancel {
                text: "取消"
                height: 100%
                width: fill
                on_click: on_cancel
            }
        ]
    }

    ]
}
