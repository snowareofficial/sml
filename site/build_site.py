#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""build_site.py — 构建 SML / BamZap 独立官网（Hugo）。

用法:
    python tools/sml-site/build_site.py              # 构建到 out/build/sml-site
    python tools/sml-site/build_site.py --serve      # 本地预览 (hugo server)
"""
import os
import subprocess
import sys
import shutil

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SITE = os.path.join(ROOT, "tools", "sml-site")
OUT = os.path.join(ROOT, "out", "build", "sml-site")


def main():
    if "--serve" in sys.argv:
        subprocess.call(["hugo", "server"], cwd=SITE)
        return 0
    shutil.rmtree(OUT, ignore_errors=True)
    os.makedirs(OUT, exist_ok=True)
    r = subprocess.call(["hugo", "--destination", OUT, "--ignoreCache", "--logLevel", "warn"],
                        cwd=SITE)
    if r != 0:
        print("!! 构建失败")
        return r
    print(f"SML / BamZap 官网 -> {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
