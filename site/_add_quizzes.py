import os, re

BOOK = os.path.join(os.path.dirname(__file__), "content", "book")
mapping = {
    "_index.md": "intro",
    "intro.md": "intro",
    "ch01-basics.md": "ch01",
    "ch02-blocks.md": "ch02",
    "ch03-fragments.md": "ch03",
    "ch04-include.md": "ch04",
    "ch05-contract.md": "ch05",
    "ch06-env-escape.md": "ch06",
    "ch07-languages.md": "ch07",
    "ch08-project.md": "ch08",
    "ch09-advanced.md": "ch09",
    "ch10-features.md": "ch10",
    "appendix.md": "appendix",
}

for fn, key in mapping.items():
    path = os.path.join(BOOK, fn)
    if not os.path.exists(path):
        print("skip", fn); continue
    with open(path, "r", encoding="utf-8") as f:
        txt = f.read()
    pg = '{{< sml-playground "%s" >}}' % key
    qz = '{{< sml-quiz "%s" >}}' % key
    if pg not in txt:
        print("!! no playground in", fn); continue
    if qz in txt:
        print("already has quiz", fn); continue
    # 在 playground 行之后插入一个空行 + quiz 短代码
    txt = txt.replace(pg, pg + "\n\n" + qz + "\n", 1)
    with open(path, "w", encoding="utf-8") as f:
        f.write(txt)
    print("added quiz ->", fn)
