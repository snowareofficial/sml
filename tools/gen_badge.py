#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""gen_badge.py —— 生成 README / 官网用的徽章 SVG（手写，无第三方依赖）。

为什么手写而不是套 shields.io：
  1. shields.io 在国内访问不稳定，且徽章挂了会拖慢整页；
  2. 本项目要「**一个徽章承载多组数据**」（版本 + 下载量 + 版本数 | stars + forks），
     而 shields.io 一条徽章只表达一组，拼多个既割裂又对不齐；
  3. 数据源直接用官方 API（crates.io / Gitee），生成物是仓库内的 SVG 快照，
     离线可看、可 diff、可被 Gitee/GitHub 直接渲染。

产物：
  badge/swsml.svg   —— crates.io 多数据合一（含雪花图标）
  badge/gitee.svg   —— Gitee stars + forks 合一
并同步一份到 site/static/badge/，供官网首页直接引用。

数据是**快照**：需要刷新时重新运行本脚本。
    python tools/gen_badge.py
"""

import json
import os
import re
import sys
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_DIRS = [os.path.join(REPO, "badge"), os.path.join(REPO, "site", "static", "badge")]

CRATE = "swsml"
GITEE_REPO = "snoware/sml"
UA = "sml-badge-generator (https://sml.swebase.cn)"

SNOWFLAKE = "\u2744"  # ❄


def fetch_json(url, timeout=20):
    req = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def human(n):
    """1000 -> 1.0k，保持徽章宽度稳定。"""
    n = int(n)
    if n < 1000:
        return str(n)
    if n < 1_000_000:
        return "%.1fk" % (n / 1000.0)
    return "%.1fM" % (n / 1_000_000.0)


def esc(s):
    return (str(s).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
            .replace('"', "&quot;"))


# 字宽估算：ASCII 约 6.6px/字符（11px 字号 + 600 字重），CJK 约 11px
def text_width(s, size=11):
    w = 0.0
    for ch in s:
        w += 11.0 if ord(ch) > 0x2E80 else size * 0.6
    return w


def badge_svg(segments, icon=SNOWFLAKE, icon_bg="#1f6feb"):
    """segments: [(label, value, bg, fg)]，第一段紧贴图标。

    布局：[ 图标 ][ 文本段 1 ][ 文本段 2 ] ...，每段自带底色，段间无空隙。
    """
    H = 26
    PAD = 8.0
    icon_w = 22.0
    widths = []
    for label, value, _bg, _fg in segments:
        t = (label + " " if label else "") + value
        widths.append(text_width(t) + PAD * 2)
    W = icon_w + sum(widths)

    parts = []
    parts.append(
        '<svg xmlns="http://www.w3.org/2000/svg" width="%d" height="%d" '
        'viewBox="0 0 %d %d" role="img" aria-label="%s">'
        % (W, H, W, H, esc(" ".join((l + " " + v).strip() for l, v, _b, _f in segments)))
    )
    # 圆角裁剪：整体裁一次，避免逐段算圆角
    parts.append('<clipPath id="r"><rect width="%d" height="%d" rx="5" ry="5"/></clipPath>' % (W, H))
    parts.append('<g clip-path="url(#r)">')
    # 图标区（雪花）
    parts.append('<rect width="%d" height="%d" fill="%s"/>' % (icon_w, H, icon_bg))
    parts.append(
        '<text x="%.1f" y="18" font-family="Segoe UI Symbol,Verdana,sans-serif" font-size="14" '
        'fill="#ffffff" text-anchor="middle">%s</text>' % (icon_w / 2.0, SNOWFLAKE)
    )
    x = icon_w
    for (label, value, bg, fg), w in zip(segments, widths):
        parts.append('<rect x="%.1f" width="%.1f" height="%d" fill="%s"/>' % (x, w, H, bg))
        tx = x + PAD
        if label:
            parts.append(
                '<text x="%.1f" y="17" font-family="Verdana,DejaVu Sans,sans-serif" font-size="11" '
                'fill="%s" opacity="0.75">%s</text>' % (tx, fg, esc(label))
            )
            tx += text_width(label + " ")
        parts.append(
            '<text x="%.1f" y="17" font-family="Verdana,DejaVu Sans,sans-serif" font-size="11" '
            'font-weight="600" fill="%s">%s</text>' % (tx, fg, esc(value))
        )
        x += w
    parts.append("</g></svg>")
    return "".join(parts)


DARK = "#0d1117"
DARK2 = "#161b22"
BLUE = "#1f6feb"
BLUE2 = "#1158c7"
GREEN = "#238636"
GREEN2 = "#1a7f37"
GREY = "#21262d"
FGLIGHT = "#e6edf3"


def build_swsml_svg(crate, gitee_ok=True):
    v = crate.get("max_version") or crate.get("newest_version") or "?"
    dl = crate.get("downloads") or 0
    nver = crate.get("num_versions") or 0
    segs = [
        ("crates.io", "v" + str(v), DARK2, FGLIGHT),
        ("downloads", human(dl), BLUE, "#ffffff"),
        ("versions", str(nver), DARK2, FGLIGHT),
    ]
    return badge_svg(segs, icon=SNOWFLAKE, icon_bg=BLUE)


def build_gitee_svg(info):
    """Gitee：把 stars 与 forks 合成一条徽章（用户要求「stars+forks 合一」）。"""
    stars = info.get("stargazers_count", 0)
    forks = info.get("forks_count", 0)
    segs = [
        ("Gitee", "snoware/sml", DARK2, FGLIGHT),
        ("stars", str(stars), GREEN, "#ffffff"),
        ("forks", str(forks), GREEN2, "#ffffff"),
    ]
    return badge_svg(segs, icon=SNOWFLAKE, icon_bg=GREEN)


def main():
    try:
        crate = fetch_json("https://crates.io/api/v1/crates/%s" % CRATE).get("crate", {})
    except Exception as e:  # 网络不可用时保留旧产物，不让构建失败
        print("!! crates.io 拉取失败：%s" % e)
        crate = {}
    try:
        gitee = fetch_json("https://gitee.com/api/v5/repos/%s" % GITEE_REPO)
    except Exception as e:
        print("!! Gitee 拉取失败：%s" % e)
        gitee = {}

    if not crate and not gitee:
        print("!! 两个数据源都失败，未改动任何文件")
        return 1

    arts = {}
    if crate:
        arts["swsml.svg"] = build_swsml_svg(crate)
    if gitee:
        arts["gitee.svg"] = build_gitee_svg(gitee)

    for d in OUT_DIRS:
        os.makedirs(d, exist_ok=True)
        for name, svg in arts.items():
            with open(os.path.join(d, name), "w", encoding="utf-8", newline="\n") as f:
                f.write(svg)
            print("->", os.path.join(d, name))
    return 0


if __name__ == "__main__":
    sys.exit(main())
