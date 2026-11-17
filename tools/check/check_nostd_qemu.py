#!/usr/bin/env python3
# SPDX-License-Identifier: MulanPSL-2.0
"""amberice no_std 子集在 QEMU 的 Cortex-M 上跑一遍。

与 `check_nostd_wasm.py` 的分工：wasm 证「32 位 + 零 std 的语义正确」；这里多证一层 ——
**裸机链接（链接脚本 / 向量表 / .bss 清零）与真 ARM 核心上的执行**。

机理解释：`-machine mps2-an500`（Cortex-M7，4 MiB ZBT + 4 MiB SRAM）跑
`rust/nostd-probe-qemu`，它复用 `rust/nostd-probe` 的同一份 `workflow()`，用 semihosting
`debug::exit` 把结论交给 QEMU 的退出码。

本机没有原生 qemu 时自动借 WSL（`wsl -e qemu-system-arm`）；两者都没有则跳过（返回 0）。
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CRATE = ROOT / "rust" / "nostd-probe-qemu"
TARGET = "thumbv7em-none-eabi"
ELF = CRATE / "target" / TARGET / "release" / "nostd-probe-qemu"


def wsl_path(p: Path) -> str:
    """"X:\\a\\b" -> "/mnt/x/a/b"。"""
    s = str(p)
    if len(s) > 1 and s[1] == ":":
        return f"/mnt/{s[0].lower()}{s[2:].replace(chr(92), '/')}"
    return s.replace("\\", "/")


def main() -> int:
    env = dict(os.environ)
    env["CARGO_TARGET_DIR"] = str(CRATE / "target")
    env["CARGO_INCREMENTAL"] = "0"

    print("构建 thumbv7em-none-eabi ...")
    p = subprocess.run(
        ["cargo", "build", "--release", "--target", TARGET],
        cwd=str(CRATE),
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if p.returncode != 0:
        print((p.stdout or "") + (p.stderr or ""))
        print("失败：构建没通过")
        return p.returncode
    if not ELF.exists():
        print(f"失败：产物不存在 {ELF}")
        return 4
    print(f"产物 {ELF.name}: {ELF.stat().st_size / 1024:.1f} KiB")

    args = [
        "-cpu", "cortex-m7",
        "-machine", "mps2-an500",
        "-nographic",
        "-semihosting-config", "enable=on,target=native",
        "-kernel", str(ELF),
    ]

    if shutil.which("qemu-system-arm"):
        cmd = ["qemu-system-arm", *args]
    elif shutil.which("wsl"):
        # WSL 里跑：路径换成 /mnt/<drive>/...
        args[-1] = wsl_path(ELF)
        cmd = ["wsl", "-e", "qemu-system-arm", *args]
    else:
        print("跳过：既没有 qemu-system-arm，也没有 wsl")
        return 0

    r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120)
    out = (r.stdout or "") + (r.stderr or "")
    print(out.strip())

    if r.returncode != 0:
        print(f"失败：QEMU 退出码 {r.returncode}")
        return r.returncode or 1
    if "run() = 0" not in out:
        print("失败：输出里没有 `run() = 0`")
        return 5
    print("通过：Cortex-M 上 run() = 0")
    return 0


if __name__ == "__main__":
    sys.exit(main())
