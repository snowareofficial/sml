@version v1
# =============================================================================
#  SML -> Slint 登录界面
#  ---------------------------------------------------------------------------
#  转换：python examples/slint/build.py login.sml
#        （内部走 smltools -i login.sml --to slint -o login.slint）
#  预览：VS Code 装 Slint 扩展后打开 login.slint，点右上角 "Show Preview"
#
#  SML 侧约定（Slint 后端，见 rust/src/emit/slint.rs）：
#   - 裸块 `component Name { inherits: Base }` -> export component <name> inherits Base
#         （`inherits` 是字段，不是位置参数；`name: root` 设定组件 id，供 `root.xxx` 引用）
#   - 裸块 `ElementName { }`（如 VerticalLayout / Text / LineEdit / Button）
#         -> Slint 元素；块内 `name: xxx` 变为 `id := Element { }`
#   - 标量字段 -> Slint 属性 `key: value;`
#   - Slint 表达式 / 颜色 / 长度 / 枚举必须写成 "`expr`"（**双引号包反引号**）：
#         "`#0f1117`" / "`28px`" / "`root.user`" / "`center`" / "`a ? b : c`"
#         ⚠️ 不要写裸反引号 `#0f1117` —— `#` 是行注释符，会把后半行吞掉。
#         普通字符串 "text" 仍按 Slint 字符串字面量输出（自动加引号）。
#   - 事件 `on_edited:` / `on_clicked:` 的值写普通字符串（不加反引号）：
#         "root.user = self.text" -> edited => { root.user = self.text; }
#   - 声明块 `property` / `callback` / `function`
# =============================================================================

component Login {
    inherits: Window

    title: "SML × Slint 登录"
    width: "`360px`"
    height: "`640px`"
    background: "`#0f1117`"

    # ---- 状态 ----
    property user {
        type: string
        default: ""
    }
    property pass {
        type: string
        default: ""
    }
    property busy {
        type: bool
        default: false
    }
    property msg {
        type: string
        default: ""
    }
    callback login { }
    callback cancel { }

    name: root

    VerticalLayout {
        padding: "`28px`"
        spacing: "`16px`"
        horizontal-alignment: "`center`"

        Text {
            text: "欢迎回来"
            color: "`#f2f5ff`"
            font-size: "`24px`"
        }
        Text {
            text: "登录你的 SML 账户"
            color: "`#8b93a7`"
            font-size: "`13px`"
        }

        # ---- 用户名 ----
        Text {
            text: "用户名"
            color: "`#aeb6c8`"
            font-size: "`12px`"
        }
        LineEdit {
            placeholder-text: "请输入用户名"
            text: "`root.user`"
            on_edited: "root.user = self.text"
        }

        # ---- 密码 ----
        Text {
            text: "密码"
            color: "`#aeb6c8`"
            font-size: "`12px`"
        }
        LineEdit {
            placeholder-text: "请输入密码"
            text: "`root.pass`"
            echo-mode: "`password`"
            on_edited: "root.pass = self.text"
        }

        # ---- 提示信息 ----
        Text {
            text: "`root.msg`"
            color: "`#e05263`"
            font-size: "`12px`"
            visible: "`root.msg != \"\"`"
        }

        # ---- 登录按钮 ----
        Button {
            text: "`root.busy ? \"登录中…\" : \"登录\"`"
            enabled: "`!root.busy`"
            background: "`#4f7cff`"
            on_clicked: "root.login()"
        }

        # ---- 取消按钮 ----
        Button {
            text: "取消"
            background: "`#232838`"
            on_clicked: "root.cancel()"
        }
    }
}
