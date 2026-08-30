# -*- coding: utf-8 -*-
"""扫描 en/book 英文文档中可疑的机器翻译损坏 token。"""
import os
import re
import sys

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

BASE = os.path.join("content", "en", "book")

# 已知合理的缩写/术语白名单
ALLOW = {
    "SML", "JSON", "YAML", "TOML", "API", "URL", "UTF", "ASCII", "UTF8",
    "HTML", "CSS", "CLI", "README", "TODO", "NOTE", "TIP", "WARNING",
    "Rust", "Lua", "Soup", "SNOWARE", "XQZ",  # XQZ 是待修目标，下面单独报
    "GET", "POST", "PUT", "HTTP", "HTTPS", "SSH", "ID", "OK",
    "INT", "FLOAT", "BOOL", "STR", "NUM", "NULL", "TRUE", "FALSE",
    "CN", "US", "UI", "DB", "CPU", "RAM", "SQL", "GUID", "UUID",
}

# 连续 3+ 大写字母
BIG = re.compile(r"\b[A-Z]{3,}\b")
# 中文标点残留在英文正文里
CJK_PUNC = re.compile(r"[，。、；：？！「」『』（）【】]")


def main():
    hits_big, hits_punc, hits_xqz = [], [], []
    for name in sorted(os.listdir(BASE)):
        if not name.endswith(".md"):
            continue
        path = os.path.join(BASE, name)
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
        in_code = False
        for i, line in enumerate(lines, 1):
            if line.strip().startswith("```"):
                in_code = not in_code
                continue
            if "XQZ" in line:
                hits_xqz.append((name, i, line.strip()))
            for m in BIG.findall(line):
                if m not in ALLOW:
                    hits_big.append((name, i, m, line.strip()[:90]))
            if not in_code and CJK_PUNC.search(line):
                hits_punc.append((name, i, line.strip()[:90]))

    print("=== XQZ / 明确损坏 ===")
    for n, i, l in hits_xqz:
        print("  %s:%d  %s" % (n, i, l[:110]))

    print("\n=== 可疑大写 token（不在白名单）===")
    for n, i, m, l in hits_big:
        print("  %s:%d  [%s]  %s" % (n, i, m, l))

    print("\n=== 正文残留中文标点 ===")
    for n, i, l in hits_punc:
        print("  %s:%d  %s" % (n, i, l))

    print("\n合计: XQZ=%d  可疑大写=%d  中文标点=%d"
          % (len(hits_xqz), len(hits_big), len(hits_punc)))


if __name__ == "__main__":
    main()
