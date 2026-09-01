@version v1
# =============================================================================
#  SML -> LVGL (v9 UI XML) 登录界面
#  ---------------------------------------------------------------------------
#  转换：python tools/sml2lvgl.py --sml examples/lvgl/login.sml --out build/login
#        （SML -> 通用 LVGL XML -> LVGL Pro XML 工程 -> ui_gen.c / ui_gen.h）
#
#  无 LVGL Pro 许可时，工具内置 emitter 会直接输出可编译的 LVGL v9 C；
#  有 LVGL_CLI_TOKEN 时走 `lved generate` 生成官方代码。
#
#  SML 侧约定（LVGL 后端，见 rust/src/emit/xml.rs 的 to_lvgl）：
#   - 裸块 `screen login { }`      -> <screen name="login">（首词=__type=screen）
#   - 裸块 `label title { }`       -> <label name="title">（__type 去 lv_ 前缀）
#        也接受 `lv_label` 写法，效果相同
#   - 块内对象字段（如 `title: { ... }`）即为子部件；或用显式 children: [ ... ]
#   - 标量字段 -> LVGL 属性（x / y / width / height / text / align ...）
#   - `on_click: handler`          -> <event name="click" handler="handler"/>
#        事件名映射到 LVGL 事件（见 tools/sml2lvgl.py 的 TRIGGER_MAP）
#   - 回调名（handler）需在应用侧实现：void handler(lv_event_t * e);
# =============================================================================

screen login {
    width: 320
    height: 240

    # ---- 标题 ----
    label title {
        text: "欢迎回来"
        x: 16
        y: 14
        width: 288
        height: 28
    }

    # ---- 用户名输入框 ----
    text_input user {
        placeholder-text: "用户名"
        x: 16
        y: 52
        width: 288
        height: 36
    }

    # ---- 密码输入框（密码回显） ----
    text_input pass {
        password: true
        placeholder-text: "密码"
        x: 16
        y: 96
        width: 288
        height: 36
    }

    # ---- 登录按钮 ----
    button btn_login {
        text: "登录"
        x: 16
        y: 144
        width: 140
        height: 40
        on_click: on_login
    }

    # ---- 取消按钮 ----
    button btn_cancel {
        text: "取消"
        x: 164
        y: 144
        width: 140
        height: 40
        on_click: on_cancel
    }
}
