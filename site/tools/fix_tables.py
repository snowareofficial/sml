# -*- coding: utf-8 -*-
"""
修复机翻破坏的 Markdown 表格。

问题：中文源表格是连续行，机翻按段落逐行翻译后，行间插入了空行，
导致 Markdown 无法识别为表格（Hugo 会渲染成普通段落）。

修复：把「表格行之间」的多余空行删除，使表格块连续。
判据：连续的表格行（以 | 开头结尾），中间夹着纯空行 -> 删除空行。
"""
import os
import re
import glob
import sys

sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DST = os.path.join(ROOT, "site", "content", "en", "book")

TABLE_ROW = re.compile(r"^\s*\|.*\|\s*$")
SEP_ROW = re.compile(r"^\s*\|[\s:|-]+\|\s*$")


def is_row(line):
    return bool(TABLE_ROW.match(line))


total = 0
for f in sorted(glob.glob(os.path.join(DST, "*.md"))):
    lines = open(f, encoding="utf-8").read().split("\n")
    out = []
    i = 0
    fixed = 0
    while i < len(lines):
        line = lines[i]
        if not is_row(line):
            out.append(line)
            i += 1
            continue
        # 收集连续的「表格行」及其间空行
        block = [line]
        j = i + 1
        while j < len(lines):
            nxt = lines[j]
            if is_row(nxt):
                block.append(nxt)
                j += 1
            elif nxt.strip() == "":
                # 空行：只有当后面仍是表格行时才吞掉（表示是表格内部空行）
                k = j
                while k < len(lines) and lines[k].strip() == "":
                    k += 1
                if k < len(lines) and is_row(lines[k]):
                    # 吞掉这段空行
                    j = k
                    fixed += 1
                else:
                    break
            else:
                break
        out.extend(block)
        i = j
    if fixed:
        open(f, "w", encoding="utf-8").write("\n".join(out))
        print(f"[修复表格] {os.path.basename(f)}: 删除 {fixed} 处表格内空行")
    total += fixed

print(f"完成，共修复 {total} 处表格空行")
