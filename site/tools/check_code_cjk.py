# -*- coding: utf-8 -*-
"""统计英文章节代码块内残留的中文注释规模。"""
import os
import re
import glob

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SITE = os.path.join(ROOT, "site")
DST = os.path.join(SITE, "content", "en", "book")

CODE_BLOCK = re.compile(r"```[a-z]*\n(.*?)```", re.DOTALL)
CJK = re.compile(r"[\u4e00-\u9fff]")

total = 0
for f in sorted(glob.glob(os.path.join(DST, "*.md"))):
    txt = open(f, encoding="utf-8").read()
    blocks = CODE_BLOCK.findall(txt)
    n = sum(1 for b in blocks if CJK.search(b))
    lines = sum(len([l for l in b.splitlines() if CJK.search(l)]) for b in blocks)
    if n:
        print(f"{os.path.basename(f):22} 含中文的代码块={n:2} 中文行={lines:3}")
        total += lines
print(f"\n代码块内中文注释总行数: {total}")
