# -*- coding: utf-8 -*-
"""
修复被误改的 Markdown 标题：
  `# # 1.1 xxx`  -> `## 1.1 xxx`
  `## # xxx`     -> `### xxx`

起因：translate_code_comments.py 早期版本把 Markdown 标题误判为代码注释，
在 `#` 后插入了空格（已修复脚本）。本脚本做一次性还原。
"""
import os
import re
import glob
import sys

sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DST = os.path.join(ROOT, "site", "content", "en", "book")

# `^#{1,6}\s+#{1,6}\s` -> 合并 # 数量
PAT = re.compile(r"(?m)^(#{1,6})[ \t]+(#{1,6})(\s+)")

total = 0
for f in sorted(glob.glob(os.path.join(DST, "*.md"))):
    txt = open(f, encoding="utf-8").read()
    n = len(PAT.findall(txt))
    if not n:
        continue

    def _fix(m):
        return "#" * (len(m.group(1)) + len(m.group(2))) + m.group(3)

    new = PAT.sub(_fix, txt)
    if new != txt:
        open(f, "w", encoding="utf-8").write(new)
        print(f"[修复标题] {os.path.basename(f)}: {n} 处")
    total += n

print(f"完成，共修复 {total} 处标题")
