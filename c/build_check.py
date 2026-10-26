# -*- coding: utf-8 -*-
"""跨平台的 C 语法/构建检查脚本（替代在 PowerShell 里直接调 gcc）。

用法:
    python build_check.py             # 语法检查 sml.c + example.c
    python build_check.py --run       # 额外编译并运行 example 与回归用例
"""
import os
import subprocess
import sys

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

CC = "gcc"
FILES = ["sml.c", "example.c"]
# 回归用例：编译并运行（不是只做语法检查）。test_limits.c 盯的是「深嵌套打穿栈」
# 与「err 为空时的越界写 / 空指针写」—— 这两类问题语法检查一无所知。
# test_codes.c 盯的是「同一条件报的是哪个码」：码是跨端稳定契约，语法检查同样看不见。
TESTS = ["test_limits.c", "test_codes.c"]


def run(args):
    p = subprocess.run(args, capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    return p.returncode, p.stdout, p.stderr


def main():
    do_run = "--run" in sys.argv
    ok = True

    for f in FILES:
        rc, out, err = run([CC, "-std=c99", "-Wall", "-Wextra",
                            "-fsyntax-only", f])
        if rc == 0:
            print("[OK]   syntax  %s" % f)
        else:
            ok = False
            print("[FAIL] syntax  %s" % f)
            print((err or out)[:2000])

    if do_run and ok:
        rc, out, err = run([CC, "-std=c99", "-Wall", FILES[0], FILES[1],
                            "-o", "example_check"])
        if rc != 0:
            ok = False
            print("[FAIL] compile example")
            print((err or out)[:2000])
        else:
            print("[OK]   compile example_check")
            rc, out, err = run(["./example_check"])
            print("[%s] run example (rc=%d)" % ("OK" if rc == 0 else "FAIL", rc))
            if out:
                print(out[:1500])
            if rc != 0:
                ok = False

    if do_run:
        # 回归用例要链接后**真的跑**：这两类问题是「输入致死」，只做语法检查看不见。
        for f in TESTS:
            exe = f[:-2] + "_check"
            rc, out, err = run([CC, "-std=c99", "-Wall", "-Wextra",
                                FILES[0], f, "-o", exe])
            if rc != 0:
                ok = False
                print("[FAIL] compile %s" % f)
                print((err or out)[:2000])
                continue
            rc, out, err = run([os.path.join(".", exe)])
            print("[%s] run %s (rc=%d)" % ("OK" if rc == 0 else "FAIL", exe, rc))
            if out:
                print(out[:1500])
            if rc != 0:
                ok = False

    print("\n=== %s ===" % ("ALL PASSED" if ok else "FAILED"))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
