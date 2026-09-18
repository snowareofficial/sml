# -*- coding: utf-8 -*-
"""js/sml.mjs 的四份副本必须与源文件**逐字节一致** —— 这个脚本是防漂移的闸门。

为什么要有它：这条「改完 js/sml.mjs 要同步四处」的纪律此前只写在文档里，靠人记着。
结果 W16 的 JS 首批（提交 5bd0b64）只改了 `js/sml.mjs`，**四份副本全没同步** ——
官网 Playground 与 VSCode 扩展继续拿旧解析器跑：源码里已修好的 LEX/PARSE 报码、
`@feature` 吞整份文档的修复，在站点与编辑器里**一点都没生效**，而且没有任何提示。
（对照：早些时候的 d2910b1 同步过，所以问题不是"没人知道要同步"，是**没有闸门**。）

四份副本各有用途，缺一不可：
    site/static/sml.mjs               官网 Playground（shortcode 里 import "/sml.mjs"）
    site/static/lib/sml.mjs           跨站入口 https://sml.swebase.cn/lib/sml.mjs
    site/public/sml.mjs               Hugo 构建产物（由 site/static 拷来，**已跟踪**）
    editors/vscode/src/vendor/sml.mjs VSCode 扩展内置的解析器（诊断/补全都走它）

用法::

    python tools/check_js_copies.py          # 只校验（提交前 / 将来接 CI）
    python tools/check_js_copies.py --fix    # 用 js/sml.mjs 覆盖四份副本

退出码：校验模式下有不一致即 1（方便将来接进 CI 挡住合并）。
"""
import hashlib
import os
import shutil
import sys

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "js", "sml.mjs")
COPIES = [
    os.path.join(ROOT, "site", "static", "sml.mjs"),
    os.path.join(ROOT, "site", "static", "lib", "sml.mjs"),
    os.path.join(ROOT, "site", "public", "sml.mjs"),
    os.path.join(ROOT, "editors", "vscode", "src", "vendor", "sml.mjs"),
]


def sha(path):
    with open(path, "rb") as fh:
        return hashlib.sha256(fh.read()).hexdigest()


def main():
    fix = "--fix" in sys.argv
    if not os.path.isfile(SRC):
        print("MISSING 源文件: %s" % SRC)
        return 2
    src_hash = sha(SRC)
    print("源: js/sml.mjs  sha256=%s  %d bytes" % (src_hash[:16], os.path.getsize(SRC)))

    bad = []
    for dst in COPIES:
        rel = os.path.relpath(dst, ROOT).replace("\\", "/")
        if not os.path.isfile(dst):
            bad.append(rel)
            print("MISSING %-42s (不存在)" % rel)
            if fix:
                os.makedirs(os.path.dirname(dst), exist_ok=True)
                shutil.copyfile(SRC, dst)
                print("        -> 已创建")
            continue
        h = sha(dst)
        same = (h == src_hash)
        print("%-8s %-42s sha256=%s" % ("OK" if same else "DRIFT", rel, h[:16]))
        if not same:
            bad.append(rel)
            if fix:
                old_size = os.path.getsize(dst)
                shutil.copyfile(SRC, dst)
                print("        -> 已同步（%d -> %d bytes）" % (old_size, os.path.getsize(SRC)))

    if bad and not fix:
        print("\n%d 份副本与 js/sml.mjs 不一致：%s" % (len(bad), ", ".join(bad)))
        print("跑 `python tools/check_js_copies.py --fix` 修，然后重跑相关测试。")
        return 1
    if fix:
        # 同步后必须复核一遍：copy 之后仍不一致说明是别的问题（只读/路径写错）
        left = [os.path.relpath(d, ROOT).replace("\\", "/") for d in COPIES
                if not os.path.isfile(d) or sha(d) != src_hash]
        if left:
            print("\n同步后仍不一致：%s" % ", ".join(left))
            return 1
        print("\n四份副本已与 js/sml.mjs 逐字节一致。")
        return 0
    print("\n四份副本与 js/sml.mjs 逐字节一致。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
