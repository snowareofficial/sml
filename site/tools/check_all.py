# -*- coding: utf-8 -*-
"""统一质量检查入口：一次性跑完英文章节的所有检查。"""
import os
import re
import sys
import glob
import subprocess

sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SITE = os.path.join(ROOT, "site")
EN_DIR = os.path.join(SITE, "content", "en", "book")
ZH_DIR = os.path.join(SITE, "content", "zh", "book")

CODE_BLOCK = re.compile(r"```[a-z]*\n(.*?)```", re.DOTALL)
INLINE = re.compile(r"`[^`\n]*`")
CJK = re.compile(r"[\u4e00-\u9fff]")
LINK = re.compile(r"\]\((/en/book/[^)]*)\)")

print("=" * 72)
print("1. 中文残留")
print("=" * 72)
t_code = t_body = 0
for f in sorted(glob.glob(os.path.join(EN_DIR, "*.md"))):
    raw = open(f, encoding="utf-8").read()
    body = CODE_BLOCK.sub("", raw)
    codes = CODE_BLOCK.findall(raw)
    c_lines = [l for b in codes for l in b.splitlines() if CJK.search(l)]
    b_lines = [l for l in body.splitlines() if CJK.search(l)]
    if c_lines or b_lines:
        print(f"\n{os.path.basename(f)} 代码={len(c_lines)} 正文={len(b_lines)}")
        for l in (b_lines + c_lines)[:8]:
            print(f"   {l.strip()[:92]}")
    t_code += len(c_lines)
    t_body += len(b_lines)
print(f"\n合计: 代码={t_code} 正文={t_body}")

print()
print("=" * 72)
print("2. 链接完整性（/en/book/xxx 是否有对应英文页）")
print("=" * 72)
valid = {os.path.splitext(os.path.basename(p))[0] for p in glob.glob(os.path.join(EN_DIR, "*.md"))}
bad = []
for f in sorted(glob.glob(os.path.join(EN_DIR, "*.md"))):
    t = open(f, encoding="utf-8").read()
    for m in LINK.finditer(t):
        tgt = m.group(1)
        # 以斜杠结尾 => 目录首页，对应 _index.md，视为有效
        if tgt.endswith("/"):
            continue
        slug = tgt.rstrip("/").split("/")[-1]
        if " " in tgt:
            bad.append((os.path.basename(f), tgt, "含空格"))
        elif slug not in valid:
            bad.append((os.path.basename(f), tgt, "无对应页"))
if bad:
    for b in bad:
        print("  BAD:", b)
else:
    print("  全部链接有效")

print()
print("=" * 72)
print("3. 行内代码守恒（中英对照）")
print("=" * 72)
from collections import Counter  # noqa: E402

def codes(text):
    out, pos = [], 0
    for m in CODE_BLOCK.finditer(text):
        out.extend(x.group(0) for x in INLINE.finditer(text[pos:m.start()]))
        pos = m.end()
    out.extend(x.group(0) for x in INLINE.finditer(text[pos:]))
    return out

issues = 0
for f in sorted(glob.glob(os.path.join(EN_DIR, "*.md"))):
    zp = os.path.join(ZH_DIR, os.path.basename(f))
    if not os.path.exists(zp):
        continue
    zc = codes(open(zp, encoding="utf-8").read())
    ec = codes(open(f, encoding="utf-8").read())
    if len(zc) != len(ec):
        print(f"  {os.path.basename(f)}: 中文{len(zc)} != 英文{len(ec)}")
        issues += 1
print("  全部守恒" if not issues else f"  {issues} 处不一致")

print()
print("=" * 72)
print("4. 标题 / 表格结构")
print("=" * 72)
bad_h = bad_t = 0
for f in sorted(glob.glob(os.path.join(EN_DIR, "*.md"))):
    t = open(f, encoding="utf-8").read()
    if re.search(r"(?m)^#+\s+#", t):
        print(f"  标题异常: {os.path.basename(f)}")
        bad_h += 1
    # 表格：| 行之间不应有空行
    lines = t.split("\n")
    for i in range(len(lines) - 2):
        if (lines[i].strip().startswith("|") and lines[i].strip().endswith("|")
                and lines[i + 1].strip() == ""
                and lines[i + 2].strip().startswith("|")):
            print(f"  表格断行: {os.path.basename(f)}")
            bad_t += 1
            break
print(f"  标题异常={bad_h} 表格断行={bad_t}")
