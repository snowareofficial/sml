#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""build_site.py — 构建 sml 官网（Hugo），输出到 public/（wrangler Pages 约定）。

用法:
    python build_site.py              # 构建到 site/public/
    python build_site.py --serve      # 本地预览 (hugo server)
    python build_site.py --deploy     # 构建 + wrangler pages deploy（sml.swebase.cn）
"""
import os
import subprocess
import sys
import shutil

SITE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(SITE, "public")


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
    print("sml 官网 -> %s" % OUT)
    # 教科书 EPUB（纯标准库，无第三方依赖）
    try:
        import gen_epub
        gen_epub.build_epub(os.path.join(OUT, "sml-book.epub"))
    except Exception as e:  # 即便 EPUB 失败也不阻断站点
        print("!! EPUB 生成失败（站点不受影响）:", e)
    if "--deploy" in sys.argv:
        r = subprocess.call(["wrangler", "pages", "deploy", OUT,
                             "--project-name", "sml-site"], cwd=SITE)
        return r
    return 0


if __name__ == "__main__":
    sys.exit(main())
