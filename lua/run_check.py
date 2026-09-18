# SPDX-License-Identifier: MulanPSL-2.0
"""lua/run_check.py — Lua 侧回归（角色等同 c/build_check.py、cpp/build_verify.py）。

三件事：
  1. `lua/main.lua` 无参自检 —— 入口能跑、能 require 到 lib.sml。
  2. `lua/main.lua <不存在的文件>` —— 宿主入口必须报 **E-IO-001**（不能只写一句不带码的话）。
  3. `lua/test_codes.lua` —— 「触发条件 → 期望码」套件（W10）。

解释器：按 luajit → lua5.4 → lua54 → lua 在 PATH 里找第一个可用的。
**找不到就明确失败**（退出码 1）而不是静默跳过 —— 静默跳过会让 CI 绿着却什么都没验，
这个仓库已经吃过一次「看着绿其实没跑」的亏（见 HANDOFF 的 W19）。

用法：
    python lua/run_check.py
"""

import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
INTERPRETERS = ["luajit", "lua5.4", "lua54", "lua"]


def find_interpreter():
    for name in INTERPRETERS:
        p = shutil.which(name)
        if p:
            return name, p
    return None, None


def run(exe, args, cwd=HERE):
    p = subprocess.run([exe] + args, cwd=cwd, capture_output=True)
    out = (p.stdout or b"").decode("utf-8", "replace")
    err = (p.stderr or b"").decode("utf-8", "replace")
    return p.returncode, out, err


def main():
    name, exe = find_interpreter()
    if not exe:
        sys.stderr.write(
            "找不到 Lua 解释器（试过 %s）。\n"
            "不静默跳过：没有解释器就等于这条回归没跑，退出码必须是非 0。\n"
            % " / ".join(INTERPRETERS)
        )
        return 1
    print(">>> 解释器: %s (%s)" % (name, exe))

    failed = 0

    # 1) 入口自检
    print(">>> %s main.lua" % name)
    rc, out, err = run(exe, ["main.lua"])
    sys.stdout.write(out)
    if err:
        sys.stderr.write(err)
    expect = "self-test: a=1 b.c='hi'"
    if rc != 0 or expect not in out:
        print("MAIN rc=%d 期望 rc=0 且输出含 %r" % (rc, expect))
        failed += 1
    else:
        print("MAIN rc=0")

    # 2) 宿主入口读不到文件 → E-IO-001
    print(">>> %s main.lua <不存在的文件>" % name)
    rc, out, err = run(exe, ["main.lua", "_no_such_file_.sml"])
    sys.stdout.write(out)
    if "E-IO-001" not in err:
        print("IO rc=%d stderr 里没看到 E-IO-001: %r" % (rc, err.strip()))
        failed += 1
    else:
        print("IO-CODE ok（stderr 带 E-IO-001）")

    # 3) 错误码套件
    print(">>> %s test_codes.lua" % name)
    rc, out, err = run(exe, ["test_codes.lua"])
    sys.stdout.write(out)
    if err:
        sys.stderr.write(err)
    print("CODES rc=%d" % rc)
    if rc != 0:
        failed += 1

    if failed:
        print("\n%d 组失败" % failed)
        return 1
    print("\nALL LUA CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
