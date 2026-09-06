# =============================================================================
#  Crystalic 版卡片：相对坐标 + flow 自动排布，无需手算 x/y
#  ---------------------------------------------------------------------------
#  转换：crystalic -i cards_crystalic.sml --to svg -o cards.svg
#  对比：examples/svg/chart.sml（旧写法，所有 cx/cy/x1/y1 均为手算）
# =============================================================================

svg {
    width: 640
    height: 300

    # 背景铺满
    rect bg {
        size: [100% 100%]
        fill: "#0f1117"
    }

    # 标题：距左 24、距上 20，字号 20
    text title {
        at: [24 20]
        text: "月度概览"
        font-size: 20
        fill: "#eef2ff"
    }

    # 三张卡片横向排布：间距 20，上下留白 64
    view cards {
        at: [24 64]
        size: [592 200]
        flow: row
        gap: 20
        children: [

        view card1 {
            width: fill
            height: 100%
            radius: 12
            fill: "#1b2030"
            children: [
                text { at: [16 24] text: "销售额" font-size: 13 fill: "#9aa3b8" }
                text { at: [16 64] text: "¥42.5K" font-size: 28 fill: "#2ecc71" }
                rect { at: [16 140] size: [72% 8] fill: "#2ecc71" rx: 4 }
            ]
        }

        view card2 {
            width: fill
            height: 100%
            radius: 12
            fill: "#1b2030"
            children: [
                text { at: [16 24] text: "用户数" font-size: 13 fill: "#9aa3b8" }
                text { at: [16 64] text: "8,234" font-size: 28 fill: "#60a5fa" }
                rect { at: [16 140] size: [55% 8] fill: "#60a5fa" rx: 4 }
            ]
        }

        view card3 {
            width: fill
            height: 100%
            radius: 12
            fill: "#1b2030"
            children: [
                text { at: [16 24] text: "留存率" font-size: 13 fill: "#9aa3b8" }
                text { at: [16 64] text: "91%" font-size: 28 fill: "#a78bfa" }
                rect { at: [16 140] size: [91% 8] fill: "#a78bfa" rx: 4 }
            ]
        }

        ]
    }

    # 右下角时间戳：用边锚定，改画布尺寸也不会跑偏
    text stamp {
        x-: 24
        y-: 16
        text: "更新于 2026-09-02"
        font-size: 11
        fill: "#5b6172"
    }
}
