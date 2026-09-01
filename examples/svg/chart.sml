@version v1
# =============================================================================
#  SML -> SVG 示例：迷你折线图（多层嵌套）
#  ---------------------------------------------------------------------------
#  转换：python examples/svg/build.py chart.sml
#        （内部走 smlconv -i chart.sml --to svg -o chart.svg）
#  预览：浏览器直接打开 chart.svg
#
#  SML 侧约定（SVG 后端，见 rust/src/emit/svg.rs）：
#   - 顶层 `svg { }`  -> <svg xmlns viewBox>（viewBox/width/height 可覆盖）
#   - 裸块块名即 SVG 元素名：`defs` / `linearGradient` / `stop` / `g` / `rect`
#     / `circle` / `path` / `line` / `text` ...，子块可任意深度嵌套
#   - 裸块第二词 -> 元素 `id`（如 `linearGradient sky { }` -> id="sky"）
#   - 标量字段 -> 元素属性：
#       * 数值属性（x/y/width/height/cx/cy/r/x1/y1/x2/y2/dx/dy/opacity/
#         stroke-width ...）必须是纯数字，否则该属性被丢弃
#       * 其余（fill/stroke/d/font-size/class ...）按字符串原样输出并转义
#   - `text` 字段 -> 元素文本内容（`<text>销售额</text>`）
#   - 事件属性（onload/onclick 等）出于安全一律丢弃
#
#  ⚠ 顺序很重要：SML 对象字段按名字排序存储，若把 `g`/`rect`/`text` 直接写成
#  同级字段，输出顺序会是「g 全部在前、rect 在后」，背景矩形反而盖住图表。
#  故本示例统一用 `children: [ ... ]` 数组显式声明绘制顺序（先画的在底层）。
#
#  本示例的层结构（按 children 顺序）：
#    svg
#      ├─ defs > linearGradient   (渐变定义，不参与绘制)
#      ├─ rect                    (渐变背景，最底层)
#      ├─ g axes                  (坐标轴)
#      ├─ g grid                  (网格线)
#      ├─ g series                (折线 path + 6 个数据点 circle)
#      ├─ g legend                (图例)
#      └─ text                    (标题，最上层)
# =============================================================================

svg {
    width: 320
    height: 200
    viewBox: "0 0 320 200"

    children: [

    # ---- 1. 渐变定义（不绘制，仅被引用）----
    defs {
        linearGradient sky {
            x1: 0
            y1: 0
            x2: 0
            y2: 1
            children: [
                stop {
                    offset: "0%"
                    stop-color: "#2b3350"
                }
                stop {
                    offset: "100%"
                    stop-color: "#0f1117"
                }
            ]
        }
    }

    # ---- 2. 背景（最底层）----
    rect {
        x: 0
        y: 0
        width: 320
        height: 200
        fill: "url(#sky)"
    }

    # ---- 3. 坐标轴 ----
    g axes {
        children: [
            line {
                x1: 40
                y1: 44
                x2: 40
                y2: 170
                stroke: "#5b6172"
                stroke-width: 1
            }
            line {
                x1: 40
                y1: 170
                x2: 300
                y2: 170
                stroke: "#5b6172"
                stroke-width: 1
            }
        ]
    }

    # ---- 4. 网格线 ----
    g grid {
        children: [
            line {
                x1: 40
                y1: 80
                x2: 300
                y2: 80
                stroke: "#232838"
                stroke-width: 1
            }
            line {
                x1: 40
                y1: 125
                x2: 300
                y2: 125
                stroke: "#232838"
                stroke-width: 1
            }
        ]
    }

    # ---- 5. 数据系列：先折线，再画圆点（点在折线之上）----
    g series {
        children: [
            path {
                d: "M 60 140 L 105 108 L 150 122 L 195 84 L 240 70 L 285 52"
                fill: "none"
                stroke: "#2ecc71"
                stroke-width: 3
            }
            circle {
                cx: 60
                cy: 140
                r: 4
                fill: "#2ecc71"
            }
            circle {
                cx: 105
                cy: 108
                r: 4
                fill: "#2ecc71"
            }
            circle {
                cx: 150
                cy: 122
                r: 4
                fill: "#2ecc71"
            }
            circle {
                cx: 195
                cy: 84
                r: 4
                fill: "#2ecc71"
            }
            circle {
                cx: 240
                cy: 70
                r: 4
                fill: "#2ecc71"
            }
            circle {
                cx: 285
                cy: 52
                r: 4
                fill: "#2ecc71"
            }
        ]
    }

    # ---- 6. 图例 ----
    g legend {
        children: [
            rect {
                x: 232
                y: 18
                width: 10
                height: 10
                fill: "#2ecc71"
            }
            text {
                x: 248
                y: 27
                fill: "#9aa3b8"
                font-size: 11
                text: "销售额"
            }
        ]
    }

    # ---- 7. 标题（最上层）----
    text {
        x: 16
        y: 26
        fill: "#eef2ff"
        font-size: 14
        text: "SML × SVG  月度销售额"
    }

    ]
}
