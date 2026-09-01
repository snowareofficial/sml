@version v1
# =============================================================================
#  SML -> LVGL (v9 UI XML) 多层示例：设置面板
#  ---------------------------------------------------------------------------
#  转换：python tools/sml2lvgl.py --sml examples/lvgl/settings.sml --out build/settings
#        （若 smlconv 不在 rust/target/{debug,release} 下，追加 --smlconv 指向二进制）
#  产物：build/settings/{project.xml,globals.xml,screens/settings.xml,ui_gen.c,ui_gen.h}
#
#  应用侧只需：#include "ui_gen.h"，调用 ui_settings_create() 得到屏幕，
#  并实现 5 个事件回调（on_dark_changed / on_bright_changed / on_user_changed /
#  on_sign_changed / on_about_ok），签名均为 void xxx(lv_event_t * e)。
#
#  本示例演示「多层嵌套」结构（与 rust/src/emit/xml.rs 的 to_lvgl 约定一致）：
#    screen
#      └─ tabview                        (选项卡容器)
#           ├─ tab "外观"                (第 1 个页签)
#           │     └─ container           (纵向布局容器)
#           │           ├─ label         (标题)
#           │           ├─ switch         (开关：深色模式)
#           │           └─ slider         (滑杆：亮度)
#           ├─ tab "账户"                (第 2 个页签)
#           │     └─ container
#           │           ├─ label          (用户名)
#           │           └─ textarea       (签名输入)
#           └─ tab "关于"
#                 └─ container
#                       ├─ label          (版本信息)
#                       └─ btn_ok         (按钮)
#      └─ keyboard                     (底部软键盘，挂在 screen 下)
#
#  SML 侧约定（LVGL 后端）：
#   - 裸块 `screen/login { }`          -> <screen name="login">
#   - 裸块 `tabview tv { }`            -> <tabview name="tv">
#   - 裸块 `tab 外观 { }`              -> <tab name="外观">（第二词作 name）
#   - 裸块 `container c1 { }`          -> <container name="c1">
#   - 块内对象字段（如 `tab1: { ... }`）即子部件；也可用 children: [ ... ] 显式列序
#   - 标量字段 -> LVGL 属性（x/y/width/height/text/align/value ...）
#        `100%` / `LV_SIZE_CONTENT` / `content` 在生成 C 时由 sml2lvgl.py 处理
#   - `on_click: handler` / `on_value_changed: handler`
#        -> <event name="click" handler="handler"/> / <event name="value_changed" .../>
#   - 回调名需在应用侧实现：void handler(lv_event_t * e);
# =============================================================================

screen settings {
    width: 320
    height: 480

    # ===================== 选项卡容器 =====================
    tabview tv {
        x: 0
        y: 0
        width: 320
        height: 400

        # ---- 第 1 页：外观 ----
        tab 外观 {
            # 纵向布局容器：放标题 + 开关 + 滑杆
            container c_look {
                flex_flow: column
                x: 8
                y: 8
                width: 304
                height: 360

                label lb_theme {
                    text: "外观设置"
                    width: 288
                    height: 28
                }

                label lb_dark {
                    text: "深色模式"
                    x: 0
                    y: 40
                    width: 200
                    height: 24
                }
                switch sw_dark {
                    x: 240
                    y: 40
                    width: 48
                    height: 24
                    on_value_changed: on_dark_changed
                }

                label lb_bright {
                    text: "亮度"
                    x: 0
                    y: 80
                    width: 200
                    height: 24
                }
                slider sl_bright {
                    x: 0
                    y: 110
                    width: 288
                    height: 16
                    min_value: 0
                    max_value: 100
                    value: 80
                    on_value_changed: on_bright_changed
                }
            }
        }

        # ---- 第 2 页：账户 ----
        tab 账户 {
            container c_acct {
                flex_flow: column
                x: 8
                y: 8
                width: 304
                height: 360

                label lb_user {
                    text: "用户名"
                    width: 288
                    height: 24
                }
                textarea ta_user {
                    placeholder-text: "请输入用户名"
                    x: 0
                    y: 32
                    width: 288
                    height: 40
                    on_value_changed: on_user_changed
                }

                label lb_sign {
                    text: "个性签名"
                    x: 0
                    y: 90
                    width: 288
                    height: 24
                }
                textarea ta_sign {
                    placeholder-text: "说点什么…"
                    x: 0
                    y: 122
                    width: 288
                    height: 80
                    on_value_changed: on_sign_changed
                }
            }
        }

        # ---- 第 3 页：关于 ----
        tab 关于 {
            container c_about {
                flex_flow: column
                x: 8
                y: 8
                width: 304
                height: 360

                label lb_ver {
                    text: "SML × LVGL  Demo  v1.0"
                    width: 288
                    height: 28
                }
                label lb_tip {
                    text: "这是一个由 SML 描述的多层界面。"
                    width: 288
                    height: 48
                }
                button btn_ok {
                    text: "知道了"
                    x: 94
                    y: 200
                    width: 132
                    height: 40
                    on_click: on_about_ok
                }
            }
        }
    }

    # ===================== 底部软键盘 =====================
    # 挂在第 1 层 screen 下，与 tabview 平级（演示跨层并列）
    keyboard kb {
        x: 0
        y: 400
        width: 320
        height: 80
    }
}
