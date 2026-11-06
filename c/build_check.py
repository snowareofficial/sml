# -*- coding: utf-8 -*-
"""跨平台的 C 语法/构建检查脚本（替代在 PowerShell 里直接调 gcc）。

目录约定（2026-11-06 起）：库源码在 c/；示例在 c/examples/；回归用例在 c/tests/；
**所有产物一律落到 c/build/**（被 .gitignore 忽略）—— 临时与永久分开。

用法:
    python build_check.py             # 语法检查 sml.c + examples/example.c
    python build_check.py --run       # 额外编译并运行 example 与回归用例
"""
import os
import subprocess
import sys

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = os.path.join(HERE, "build")
os.makedirs(BUILD, exist_ok=True)

CC = "gcc"
SRC = os.path.join(HERE, "sml.c")
FILES = [SRC, os.path.join(HERE, "examples", "example.c")]

# 回归用例：编译并运行（不是只做语法检查）。test_limits.c 盯的是「深嵌套打穿栈」
# 与「err 为空时的越界写 / 空指针写」—— 这类问题语法检查一无所知。
# test_codes.c 盯的是「同一条件报的是哪个码」：码是跨端稳定契约，语法检查同样看不见。
# 每条 = (源文件, 额外要链接的实现文件)。`.ambr` 解码器是独立单元，需要 abmr.c。
TESTS = [
    (os.path.join(HERE, "tests", "test_limits.c"), []),
    (os.path.join(HERE, "tests", "test_codes.c"), []),
    (os.path.join(HERE, "tests", "test_ambr.c"), [os.path.join(HERE, "ambr.c")]),
]


def run(args, cwd=HERE):
    # cwd 固定为 c/：用例里的相对路径（如 test_ambr.c 的 `../tests/vectors/...`）
    # 以 c/ 为基准，与本脚本从哪个目录被调用无关。
    p = subprocess.run(args, cwd=cwd, capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    return p.returncode, p.stdout, p.stderr


def main():
    do_run = "--run" in sys.argv
    ok = True

    for f in FILES:
        rc, out, err = run([CC, "-std=c99", "-Wall", "-Wextra",
                            "-I", HERE, "-fsyntax-only", f])
        if rc == 0:
            print("[OK]   syntax  %s" % os.path.relpath(f, HERE))
        else:
            ok = False
            print("[FAIL] syntax  %s" % os.path.relpath(f, HERE))
            print((err or out)[:2000])

    if do_run and ok:
        exe = os.path.join(BUILD, "example_check")
        rc, out, err = run([CC, "-std=c99", "-Wall", "-Wextra", "-I", HERE,
                            FILES[0], FILES[1], "-o", exe])
        if rc != 0:
            ok = False
            print("[FAIL] compile example")
            print((err or out)[:2000])
        else:
            print("[OK]   compile build/example_check")
            rc, out, err = run([exe])
            print("[%s] run example (rc=%d)" % ("OK" if rc == 0 else "FAIL", rc))
            if out:
                print(out[:1500])
            if rc != 0:
                ok = False

    if do_run:
        # 回归用例要链接后**真的跑**：这两类问题是「输入致死」，只做语法检查看不见。
        for f, extra in TESTS:
            stem = os.path.splitext(os.path.basename(f))[0]
            exe = os.path.join(BUILD, stem + "_check")
            rc, out, err = run([CC, "-std=c99", "-Wall", "-Wextra", "-I", HERE,
                                SRC] + extra + [f, "-o", exe])
            if rc != 0:
                ok = False
                print("[FAIL] compile %s" % os.path.relpath(f, HERE))
                print((err or out)[:2000])
                continue
            rc, out, err = run([exe])
            print("[%s] run %s (rc=%d)" % ("OK" if rc == 0 else "FAIL", os.path.basename(exe), rc))
            if out:
                print(out[:1500])
            if rc != 0:
                ok = False

    print("\n=== %s ===" % ("ALL PASSED" if ok else "FAILED"))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
