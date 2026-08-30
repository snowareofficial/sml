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
JS_SRC = os.path.join(os.path.dirname(SITE), "js", "sml.mjs")   # 仓库根 /js/sml.mjs
STATIC_SML = os.path.join(SITE, "static", "sml.mjs")


def sync_sml_js():
    """把最新 js/sml.mjs 同步到 static/sml.mjs，保证 playground 与 shortcode 用同一解析器。"""
    if not os.path.exists(JS_SRC):
        print("!! 跳过 sml.mjs 同步：找不到", JS_SRC)
        return
    shutil.copyfile(JS_SRC, STATIC_SML)
    print("sml.mjs 同步 ->", STATIC_SML)


def main():
    if "--serve" in sys.argv:
        subprocess.call(["hugo", "server"], cwd=SITE)
        return 0
    sync_sml_js()   # 先同步最新解析器到 static/
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
        # Windows 下 wrangler 是 .cmd，且需保证 npm 全局路径在 PATH
        deploy_cmd = ["wrangler", "pages", "deploy", OUT, "--project-name", "sml-site"]
        if os.name == "nt":
            npm_global = os.path.join(os.environ.get("APPDATA", ""), "npm")
            if npm_global and npm_global not in os.environ.get("PATH", ""):
                os.environ["PATH"] = npm_global + os.pathsep + os.environ.get("PATH", "")
            deploy_cmd[0] = "wrangler.cmd"
        r = subprocess.call(deploy_cmd, cwd=SITE)
        return r
    return 0


if __name__ == "__main__":
    sys.exit(main())
