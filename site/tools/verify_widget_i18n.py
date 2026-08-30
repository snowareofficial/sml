# -*- coding: utf-8 -*-
"""验证交互部件 UI 文案是否已按语言切换（i18n 生效检查）。"""
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")

PUB = os.path.join(
    os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
    "site", "public",
)

# 中文 UI 标记 -> 英文 UI 标记
MARKS = [
    ("自测考题", "Self-check quiz"),
    ("交卷判分", "Submit"),
    ("动手练习", "Hands-on exercise"),
    ("虚拟文件", "Virtual files"),
    ("原始材料", "Source material"),
    ("你的 SML 答案", "Your SML answer"),
    ("得分", "Score"),
]

PAGES = [
    ("en", "book/ch01-basics/index.html", "en"),
    ("en", "book/ch11-challenges/index.html", "en"),
    ("", "book/ch01-basics/index.html", "zh"),
    ("", "book/ch11-challenges/index.html", "zh"),
]

allok = True
for lang_dir, rel, want in PAGES:
    p = os.path.join(PUB, lang_dir, rel) if lang_dir else os.path.join(PUB, rel)
    if not os.path.exists(p):
        print(f"{rel:42} [{want}] 缺失")
        allok = False
        continue
    t = open(p, encoding="utf-8", errors="replace").read()
    # 只在「可见文本 + 属性」中查找，排除 <script> 里的 JS 兜底默认值
    visible = re.sub(r"<script.*?</script>", "", t, flags=re.S)
    visible = re.sub(r"<style.*?</style>", "", visible, flags=re.S)
    zh_hits = [z for z, _ in MARKS if z in visible]
    en_hits = [e for _, e in MARKS if e in visible]
    # 统计残留中文（排除语言切换按钮里的"中文"）
    body = re.sub(r"<header>.*?</header>", "", t, flags=re.S)
    cjk = len(re.findall(r"[\u4e00-\u9fff]", body))
    status = "OK"
    if want == "en" and zh_hits:
        status = f"仍有中文UI: {zh_hits}"
        allok = False
    if want == "zh" and en_hits:
        status = f"混入英文UI: {en_hits}"
        allok = False
    print(f"{rel:42} [{want}] 中文残留={cjk:5} 英文标记={len(en_hits)} {status}")

print("\n结论:", "i18n 生效" if allok else "仍需处理")
