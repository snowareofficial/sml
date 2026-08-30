#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""检查英文章节行内代码还原质量：对比中英文行内代码集合差异。"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SITE = os.path.join(ROOT, "site")
SRC = os.path.join(SITE, "content", "book")
DST = os.path.join(SITE, "content", "en", "book")

CODE_BLOCK = re.compile(r"```.*?```", re.DOTALL)
INLINE = re.compile(r"`[^`\n]*`")


def codes(text):
    out = []
    pos = 0
    for m in CODE_BLOCK.finditer(text):
        out.extend(x.group(0) for x in INLINE.finditer(text[pos:m.start()]))
        pos = m.end()
    out.extend(x.group(0) for x in INLINE.finditer(text[pos:]))
    return out


for fn in sorted(os.listdir(DST)):
    if not fn.endswith(".md"):
        continue
    zh = open(os.path.join(SRC, fn), encoding="utf-8").read()
    en = open(os.path.join(DST, fn), encoding="utf-8").read()
    zc, ec = codes(zh), codes(en)
    from collections import Counter
    c_en, c_zh = Counter(ec), Counter(zc)
    # 英文中重复次数明显多于中文的（错位特征）
    dup = [c for c, n in c_en.items() if n > c_zh.get(c, 0) + 1]
    missing = [c for c in c_zh if c_en.get(c, 0) < c_zh[c] and c not in ec]
    print(f"{fn:22} zh_inline={len(zc):3} en_inline={len(ec):3} 重复异常={len(dup)} 缺失={len(missing)}")
    if dup:
        print("    重复:", dup[:5])
    if missing:
        print("    缺失:", missing[:5])
