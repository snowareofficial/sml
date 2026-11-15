#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""把 Rust 的字符类语义导出成 C / Lua 表（模式匹配器的字符类）。

    python tools/gen/gen_unicode_tables.py          # 生成
    python tools/gen/gen_unicode_tables.py --check   # 只校验（给 CI）

**为什么必须由 Rust 生成**：`Class::Digit` 的语义是 Unicode（`char::is_numeric`，
不止 `0-9`，全角与罗马数字也算）。三端若各自手写区间，边界必然漂移，而字符类
漂移表现出来就是「同一份文档在一端通过、另一端报 E-CONTRACT-009」—— 最难查的
那类差异。导出同一张表，四端的判定就同源。

数据源 = `rust/sml-pattern/examples/gen_unicode_tables.rs`（它调用 Rust 标准库的
`char::is_numeric` / `is_alphabetic` / `is_whitespace`，逐码位扫 0..0x110000）。

产物：
    c/sml_pattern_unicode.h          C / C++（header-only，二分查表）
    lua/sml_pattern_unicode.lua      Lua（扁平 lo,hi 数组）
"""
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
RUST_DIR = os.path.join(ROOT, "rust")
C_OUT = os.path.join(ROOT, "c", "sml_pattern_unicode.h")
LUA_OUT = os.path.join(ROOT, "lua", "sml_pattern_unicode.lua")


def render(which):
    r = subprocess.run(
        ["cargo", "run", "-q", "-p", "sml-pattern", "--example", "gen_unicode_tables", "--", which],
        cwd=RUST_DIR,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if r.returncode != 0:
        raise SystemExit("生成失败：\n" + (r.stderr or r.stdout))
    # 统一成 LF + 末尾单个换行（与仓库其它生成物一致）
    return "\n".join(r.stdout.replace("\r\n", "\n").rstrip("\n").split("\n")) + "\n"


def write_or_check(path, text, check):
    old = None
    if os.path.exists(path):
        with open(path, encoding="utf-8", newline="") as fh:
            old = fh.read()
    rel = os.path.relpath(path, ROOT)
    if check:
        if old != text:
            print("  FAIL %s 与生成器不一致（请重跑 tools/gen/gen_unicode_tables.py）" % rel)
            return False
        print("  ok   %s" % rel)
        return True
    if old == text:
        print("  same %s（无变更）" % rel)
        return True
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)
    print("  write %s" % rel)
    return True


def main():
    check = "--check" in sys.argv
    ok = True
    ok &= write_or_check(C_OUT, render("c"), check)
    ok &= write_or_check(LUA_OUT, render("lua"), check)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
