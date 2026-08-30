# -*- coding: utf-8 -*-
"""清理 en/book 英文正文里残留的中文全角标点（代码块内不处理）。"""
import os
import re
import sys

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

BASE = os.path.join("content", "en", "book")

# 全角 -> 半角映射（值末尾带空格的表示后面补一个空格）
MAP = [
    ("，", ", "),
    ("、", ", "),
    ("；", "; "),
    ("：", ": "),
    ("。", ". "),
    ("？", "? "),
    ("！", "! "),
    ("（", "("),
    ("）", ")"),
    ("【", "["),
    ("】", "]"),
    ("「", '"'),
    ("」", '"'),
    ("『", '"'),
    ("』", '"'),
]

FULL = "".join(m[0] for m in MAP)
TABLE = {ord(a): b for a, b in MAP}


def fix_line(s):
    if not any(ch in s for ch in FULL):
        return s
    out = s.translate(TABLE)
    # 行尾句号/逗号去掉多余空格
    out = re.sub(r"[.,;:!?] +$", lambda m: m.group(0).strip() + "", out)
    # 合并多余空格
    out = re.sub(r" {2,}", " ", out)
    # 修正 ") ," 类错位
    out = out.replace(") ,", "),")
    return out


def main():
    total = 0
    for name in sorted(os.listdir(BASE)):
        if not name.endswith(".md"):
            continue
        path = os.path.join(BASE, name)
        with open(path, encoding="utf-8", newline="") as f:
            text = f.read()
        lines = text.split("\n")
        in_code = False
        changed = 0
        for i, ln in enumerate(lines):
            if ln.lstrip().startswith("```"):
                in_code = not in_code
                continue
            if in_code:
                continue
            new = fix_line(ln)
            if new != ln:
                lines[i] = new
                changed += 1
        new_text = "\n".join(lines)
        # 单独修 XQZ 翻译损坏
        if "XQZ" in new_text:
            new_text = new_text.replace("`bool` XQZ `array[T]`", "`bool` `array[T]`")
            changed += 1
            print("  %s :: removed XQZ artifact" % name)
        if new_text != text:
            with open(path, "w", encoding="utf-8", newline="") as f:
                f.write(new_text)
            total += changed
            print("  %-22s %d 行标点已修正" % (name, changed))
    print("\n合计修正 %d 行" % total)


if __name__ == "__main__":
    main()
