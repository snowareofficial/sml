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
    """把最新 js/sml.mjs 同步到 static/sml.mjs，保证 playground 与 shortcode 用同一解析器。

    同时维护**对外跨站入口** static/lib/：
        import { parse } from "https://sml.swebase.cn/lib/sml.mjs"
    放行头见 static/_headers。wasm 与桥接脚本必须一起放进去 —— 只给 sml.mjs
    的话，调用方用不了 wasm 对照（sml-rs.js 会去 fetch sml.wasm）。
    """
    if not os.path.exists(JS_SRC):
        print("!! 跳过 sml.mjs 同步：找不到", JS_SRC)
        return
    shutil.copyfile(JS_SRC, STATIC_SML)
    print("sml.mjs 同步 ->", STATIC_SML)
    lib = os.path.join(SITE, "static", "lib")
    os.makedirs(lib, exist_ok=True)
    shutil.copyfile(JS_SRC, os.path.join(lib, "sml.mjs"))
    for name in ("sml.wasm", "sml-rs.js", "sml-verify.js"):
        s = os.path.join(SITE, "static", name)
        if os.path.exists(s):
            shutil.copyfile(s, os.path.join(lib, name))
    print("跨站入口同步 ->", lib)


def gen_site_data():
    """生成两个**数据驱动页面**依赖的 JSON：
      - errors.json        错误码表（源：errors/codes.sml）
      - search-index.json  教科书搜索索引（源：content/**/*.md）

    必须在 hugo 之前生成 —— 它们在 static/ 下，Hugo 构建时会把 static 复制进 public。
    失败时只告警不阻断（与 EPUB 一致）：页面会显示「加载失败」而不是整站构建挂掉，
    这样定位问题比「构建直接失败但不说哪里错」容易。
    """
    tools = [
        (os.path.join(os.path.dirname(SITE), "errors", "gen_json.py"), "错误码表"),
        (os.path.join(SITE, "tools", "gen_search_index.py"), "搜索索引"),
    ]
    for script, label in tools:
        if not os.path.exists(script):
            print("!! %s 生成脚本缺失，跳过：%s" % (label, script))
            continue
        r = subprocess.call([sys.executable, script])
        if r != 0:
            print("!! %s 生成失败（退出码 %d），页面将显示加载失败" % (label, r))


def main():
    if "--serve" in sys.argv:
        subprocess.call(["hugo", "server"], cwd=SITE)
        return 0
    sync_sml_js()   # 先同步最新解析器到 static/
    gen_site_data()  # 再生成数据驱动页面用的 JSON
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
