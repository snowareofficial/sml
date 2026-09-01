"""校验「Rust 为参考实现、其余实现暂不保证」声明在各入口页面中确实存在。

覆盖：中文首页、英文首页、中英文第 7 章（多语言使用）。
"""
import os, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
PUB = os.path.join(ROOT, "public")

CASES = [
    (os.path.join(PUB, "index.html"), ["参考实现", "暂不保证"], "ZH home"),
    (os.path.join(PUB, "en", "index.html"), ["Reference implementation", "not guaranteed"], "EN home"),
    (os.path.join(PUB, "book", "ch07-languages", "index.html"), ["暂不保证"], "ZH ch07"),
    (os.path.join(PUB, "en", "book", "ch07-languages", "index.html"), ["not guaranteed"], "EN ch07"),
]

def main() -> int:
    bad = 0
    for path, needles, label in CASES:
        if not os.path.isfile(path):
            print(f"  [!] {label}: 缺少文件 {path}（先运行 build_site.py）")
            bad += 1
            continue
        html = open(path, encoding="utf-8").read()
        missing = [n for n in needles if n not in html]
        if missing:
            print(f"  [!] {label}: 缺少声明 {missing}")
            bad += 1
        else:
            print(f"  [ok] {label}")
    print("OK: 实现状态声明齐全" if bad == 0 else f"WARN: {bad} 处缺失")
    return 1 if bad else 0

if __name__ == "__main__":
    sys.exit(main())
