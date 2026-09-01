"""检查 workflow 文件中是否残留会导致 GitHub 静默拒绝解析的箭头字符（U+2192）。"""
import glob, sys

BAD = "\u2192"
found = False
for p in glob.glob(".github/workflows/*.yml") + glob.glob(".github/workflows/*.yaml"):
    s = open(p, encoding="utf-8").read()
    if BAD in s:
        found = True
        for i, line in enumerate(s.splitlines(), 1):
            if BAD in line:
                print(f"  [!] {p}:{i}: {line.strip()}")
print("OK: 未发现 U+2192 箭头字符" if not found else "WARN: 发现箭头字符")
sys.exit(1 if found else 0)
