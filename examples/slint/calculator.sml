@version v1
# =============================================================================
#  SML -> Slint 科学计算器
#  ---------------------------------------------------------------------------
#  转换：smlconv -i calculator.sml --to slint -o calculator.slint
#  预览：VS Code 装 Slint 扩展后打开 calculator.slint，点右上角 "Show Preview"
#
#  SML 侧约定（本后端）：
#   - 裸块 `Type name { }`  -> Slint 元素/组件/声明，name 即 Slint 里的 id
#   - 形如 "`expr`" 的引号值 -> 原样输出为 Slint 表达式（颜色/长度/绑定），
#     不加引号；普通 "text" 仍输出为 Slint 字符串字面量
#   - property / callback / function 是声明块，不是子元素
#   - on_click / on_xxx 的值 -> Slint 回调体
# =============================================================================

# -----------------------------------------------------------------------------
# 按键组件：一个带按下/悬停反馈的圆角色块
# -----------------------------------------------------------------------------
component CalcButton {
    inherits: Rectangle

    property label { type: string default: "" }
    property bg { type: color default: "`#2b3145`" }

    callback clicked { }

    background: "`ta.pressed ? root.bg.darker(0.25) : (ta.has-hover ? root.bg.brighter(0.12) : root.bg)`"
    border-radius: "`8px`"
    min-height: "`48px`"
    min-width: "`40px`"

    TouchArea ta {
        on_click: "root.clicked()"
    }

    Text {
        width: "`100%`"
        height: "`100%`"
        text: "`root.label`"
        color: "`#eef2ff`"
        font-size: "`16px`"
        font-weight: 600
        horizontal-alignment: "`center`"
        vertical-alignment: "`center`"
    }
}

# -----------------------------------------------------------------------------
# 主窗口
# -----------------------------------------------------------------------------
component Calc {
    inherits: Window

    title: "SML × Slint 科学计算器"
    width: "`380px`"
    height: "`640px`"
    background: "`#0f1117`"

    # ---- 状态 ----
    property entry { type: string default: "0" }
    property hint { type: string default: "" }
    property acc { type: float default: 0 }
    property last { type: float default: 0 }
    property op { type: string default: "" }
    property fresh { type: bool default: true }
    property has-dot { type: bool default: false }

    # ---- 数字格式化：去掉浮点尾巴上多余的 0 ----
    function fmt {
        args: "v: float"
        returns: string
        code: "if (Math.abs(v) >= 1000000000000) { return v.to-precision(6); }
if (Math.abs(v - Math.round(v)) < 0.001) { return v.to-fixed(0); }
if (Math.abs(v * 10 - Math.round(v * 10)) < 0.001) { return v.to-fixed(1); }
if (Math.abs(v * 100 - Math.round(v * 100)) < 0.001) { return v.to-fixed(2); }
if (Math.abs(v * 1000 - Math.round(v * 1000)) < 0.001) { return v.to-fixed(3); }
if (Math.abs(v * 10000 - Math.round(v * 10000)) < 0.001) { return v.to-fixed(4); }
if (Math.abs(v * 100000 - Math.round(v * 100000)) < 0.001) { return v.to-fixed(5); }
return v.to-fixed(6);"
    }

    function is-digit {
        args: "k: string"
        returns: bool
        code: "return k == \"0\" || k == \"1\" || k == \"2\" || k == \"3\" || k == \"4\" || k == \"5\" || k == \"6\" || k == \"7\" || k == \"8\" || k == \"9\";"
    }

    # ---- 二元运算；o 为空表示「还没有挂起的操作符」 ----
    function apply-op {
        args: "a: float, b: float, o: string"
        returns: float
        code: "if (o == \"\") { return b; }
if (o == \"+\") { return a + b; }
if (o == \"-\") { return a - b; }
if (o == \"*\") { return a * b; }
if (o == \"/\") { if (b == 0) { return 0; } return a / b; }
if (o == \"^\") { return Math.pow(a, b); }
if (o == \"%\") { return Math.mod(a, b); }
return b;"
    }

    # ---- 所有按键的统一入口 ----
    function press {
        args: "key: string"
        code: "if (root.is-digit(key)) {
    if (root.fresh) { root.entry = key; root.fresh = false; }
    else if (root.entry == \"0\") { root.entry = key; }
    else { root.entry = root.entry + key; }
    return;
}
if (key == \".\") {
    if (root.fresh || root.entry == \"0\") { root.entry = \"0.\"; root.fresh = false; root.has-dot = true; return; }
    if (!root.has-dot) { root.entry = root.entry + \".\"; root.has-dot = true; }
    return;
}
if (key == \"+\" || key == \"-\" || key == \"*\" || key == \"/\" || key == \"^\" || key == \"%\") {
    root.acc = root.apply-op(root.acc, root.entry.to-float(), root.op);
    root.op = key;
    root.entry = root.fmt(root.acc);
    root.hint = root.fmt(root.acc) + \" \" + key;
    root.fresh = true;
    root.has-dot = false;
    return;
}
if (key == \"=\") {
    root.acc = root.apply-op(root.acc, root.entry.to-float(), root.op);
    root.last = root.acc;
    root.entry = root.fmt(root.acc);
    root.hint = \"\";
    root.op = \"\";
    root.fresh = true;
    root.has-dot = false;
    return;
}
if (key == \"C\") { root.entry = \"0\"; root.acc = 0; root.op = \"\"; root.hint = \"\"; root.fresh = true; root.has-dot = false; return; }
if (key == \"CE\") { root.entry = \"0\"; root.fresh = true; root.has-dot = false; return; }
if (key == \"sin\") { root.entry = root.fmt(Math.sin(root.entry.to-float() * 1deg)); root.fresh = true; return; }
if (key == \"cos\") { root.entry = root.fmt(Math.cos(root.entry.to-float() * 1deg)); root.fresh = true; return; }
if (key == \"tan\") { root.entry = root.fmt(Math.tan(root.entry.to-float() * 1deg)); root.fresh = true; return; }
if (key == \"ln\") { root.entry = root.fmt(Math.ln(root.entry.to-float())); root.fresh = true; return; }
if (key == \"log\") { root.entry = root.fmt(Math.log(root.entry.to-float(), 10)); root.fresh = true; return; }
if (key == \"sqr\") { root.entry = root.fmt(root.entry.to-float() * root.entry.to-float()); root.fresh = true; return; }
if (key == \"sqrt\") { root.entry = root.fmt(Math.sqrt(root.entry.to-float())); root.fresh = true; return; }
if (key == \"inv\") { root.entry = root.fmt(1 / root.entry.to-float()); root.fresh = true; return; }
if (key == \"neg\") { root.entry = root.fmt(-1 * root.entry.to-float()); root.fresh = true; return; }
if (key == \"pow10\") { root.entry = root.fmt(Math.pow(10, root.entry.to-float())); root.fresh = true; return; }
if (key == \"pi\") { root.entry = root.fmt(3.14159265358979); root.fresh = true; return; }
if (key == \"euler\") { root.entry = root.fmt(2.71828182845905); root.fresh = true; return; }
if (key == \"ans\") { root.entry = root.fmt(root.last); root.fresh = true; return; }"
    }

    VerticalLayout {
        padding: "`14px`"
        spacing: "`10px`"

        # 子元素的**书写顺序**由 children 数组承载（对象字段按名字排序，
        # 同名元素天然成组保序，不同类型混排时须显式列在 children 里）
        children: [

        # ---- 显示屏 ----
        Rectangle {
            background: "`#171a24`"
            border-radius: "`12px`"
            height: "`104px`"

            VerticalLayout {
                padding: "`14px`"
                spacing: "`6px`"

                Text {
                    text: "`root.hint`"
                    color: "`#6b7280`"
                    font-size: "`13px`"
                    horizontal-alignment: "`right`"
                }
                Text {
                    text: "`root.entry`"
                    color: "`#f2f5ff`"
                    font-size: "`36px`"
                    font-weight: 700
                    horizontal-alignment: "`right`"
                    vertical-alignment: "`center`"
                }
            }
        }

        # ---- 键盘 ----
        GridLayout {
            spacing: "`6px`"

            Row {
                CalcButton { label: "sin" bg: "`#232838`" on_click: "root.press(\"sin\")" }
                CalcButton { label: "cos" bg: "`#232838`" on_click: "root.press(\"cos\")" }
                CalcButton { label: "tan" bg: "`#232838`" on_click: "root.press(\"tan\")" }
                CalcButton { label: "ln" bg: "`#232838`" on_click: "root.press(\"ln\")" }
                CalcButton { label: "log" bg: "`#232838`" on_click: "root.press(\"log\")" }
            }
            Row {
                CalcButton { label: "x^2" bg: "`#232838`" on_click: "root.press(\"sqr\")" }
                CalcButton { label: "√" bg: "`#232838`" on_click: "root.press(\"sqrt\")" }
                CalcButton { label: "x^y" bg: "`#232838`" on_click: "root.press(\"^\")" }
                CalcButton { label: "1/x" bg: "`#232838`" on_click: "root.press(\"inv\")" }
                CalcButton { label: "π" bg: "`#232838`" on_click: "root.press(\"pi\")" }
            }
            Row {
                CalcButton { label: "C" bg: "`#e05263`" on_click: "root.press(\"C\")" }
                CalcButton { label: "CE" bg: "`#e05263`" on_click: "root.press(\"CE\")" }
                CalcButton { label: "±" bg: "`#232838`" on_click: "root.press(\"neg\")" }
                CalcButton { label: "÷" bg: "`#4f7cff`" on_click: "root.press(\"/\")" }
                CalcButton { label: "%" bg: "`#4f7cff`" on_click: "root.press(\"%\")" }
            }
            Row {
                CalcButton { label: "7" on_click: "root.press(\"7\")" }
                CalcButton { label: "8" on_click: "root.press(\"8\")" }
                CalcButton { label: "9" on_click: "root.press(\"9\")" }
                CalcButton { label: "×" bg: "`#4f7cff`" on_click: "root.press(\"*\")" }
                CalcButton { label: "e" bg: "`#232838`" on_click: "root.press(\"euler\")" }
            }
            Row {
                CalcButton { label: "4" on_click: "root.press(\"4\")" }
                CalcButton { label: "5" on_click: "root.press(\"5\")" }
                CalcButton { label: "6" on_click: "root.press(\"6\")" }
                CalcButton { label: "−" bg: "`#4f7cff`" on_click: "root.press(\"-\")" }
                CalcButton { label: "10^x" bg: "`#232838`" on_click: "root.press(\"pow10\")" }
            }
            Row {
                CalcButton { label: "1" on_click: "root.press(\"1\")" }
                CalcButton { label: "2" on_click: "root.press(\"2\")" }
                CalcButton { label: "3" on_click: "root.press(\"3\")" }
                CalcButton { label: "+" bg: "`#4f7cff`" on_click: "root.press(\"+\")" }
                CalcButton { label: "ANS" bg: "`#232838`" on_click: "root.press(\"ans\")" }
            }
            Row {
                CalcButton { label: "0" colspan: 2 on_click: "root.press(\"0\")" }
                CalcButton { label: "." on_click: "root.press(\".\")" }
                CalcButton { label: "=" bg: "`#2ecc71`" colspan: 2 on_click: "root.press(\"=\")" }
            }
        }
        ]
        }
    }