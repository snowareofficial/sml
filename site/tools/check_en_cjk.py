# -*- coding: utf-8 -*-
"""
检查英文章节里残留的中文分布（按 代码块内 / 正文 分类），
便于判断哪些该翻译、哪些应保留（教学示例值）。
"""
import os
import re
import sys
import glob

sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
EN_DIR = os.path.join(ROOT, "site", "content", "en", "book")

CODE_BLOCK = re.compile(r"```[a-z]*\n(.*?)```", re.DOTALL)
CJK = re.compile(r"[\u4e00-\u9fff]")

total_code = total_body = 0
for f in sorted(glob.glob(os.path.join(EN_DIR, "*.md"))):
    raw = open(f, encoding="utf-8").read()
    body = CODE_BLOCK.sub("", raw)
    codes = CODE_BLOCK.findall(raw)
    c_lines = [l for b in codes for l in b.splitlines() if CJK.search(l)]
    b_lines = [l for l in body.splitlines() if CJK.search(l)]
    if c_lines or b_lines:
        print(f"\n=== {os.path.basename(f)} 代码块中文行={len(c_lines)} 正文中文行={len(b_lines)} ===")
        for l in b_lines[:12]:
            print(f"  [正文] {l.strip()[:100]}")
        for l in c_lines[:12]:
            print(f"  [代码] {l.strip()[:100]}")
    total_code += len(c_lines)
    total_body += len(b_lines)

print(f"\n合计: 代码块中文行={total_code}  正文中文行={total_body}")
