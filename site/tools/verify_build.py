# -*- coding: utf-8 -*-
"""验证 Hugo 构建产物：lang 属性、表格渲染、标题、语言切换 active 状态。"""
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
PUB = os.path.join(ROOT, "site", "public")

CHECKS = [
    ("en", "index.html", "en"),
    ("en", "book/index.html", "en"),
    ("en", "book/ch01-basics/index.html", "en"),
    ("en", "book/ch10-features/index.html", "en"),
    ("en", "book/intro/index.html", "en"),
    ("", "index.html", "zh"),
    ("", "book/ch01-basics/index.html", "zh"),
]

print(f"{'页面':44} {'lang':6} {'表格':5} 标题")
print("-" * 100)
allok = True
for lang_dir, rel, want_lang in CHECKS:
    p = os.path.join(PUB, lang_dir, rel) if lang_dir else os.path.join(PUB, rel)
    if not os.path.exists(p):
        print(f"{rel:44} 缺失!")
        allok = False
        continue
    t = open(p, encoding="utf-8", errors="replace").read()
    m = re.search(r"<html[^>]*\blang=([\"']?)([a-zA-Z-]+)\1", t)
    lang = m.group(2) if m else "?"
    tbl = len(re.findall(r"<table", t))
    ti = re.search(r"<title>(.*?)</title>", t, re.S)
    title = (ti.group(1).strip()[:38] if ti else "?")
    ok = "" if lang == want_lang else "  <<< lang 错误"
    if ok:
        allok = False
    print(f"{rel:44} {lang:6} {tbl:<5} {title}{ok}")

# 检查语言切换按钮 active
print("\n=== 语言切换按钮 ===")
for lang_dir, rel in (("en", "book/ch01-basics/index.html"), ("", "book/ch01-basics/index.html")):
    p = os.path.join(PUB, lang_dir, rel) if lang_dir else os.path.join(PUB, rel)
    t = open(p, encoding="utf-8", errors="replace").read()
    m = re.search(r"<span class=[\"']?lang-switch[\"']?>(.*?)</span>", t, re.S)
    if m:
        seg = re.sub(r"\s+", " ", m.group(1)).strip()
        print(f"{rel:44} {seg[:100]}")
    else:
        print(f"{rel:44} 未找到切换按钮")

print("\n结论:", "全部通过" if allok else "存在问题")
