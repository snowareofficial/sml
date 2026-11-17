#!/usr/bin/env python3
# SPDX-License-Identifier: MulanPSL-2.0
"""amberice no_std 子集的 wasm32 运行时门禁。

为什么验 wasm32 而不是 QEMU：本机没有 `qemu-system-arm`，而这里要证的两件事 wasm32 同样能证 ——
① 指针 32 位（`u64 as usize` / `usize as u32` 的截断假设真被走过）；
② 整份产物不链 std（探针自带 bump 分配器与 panic handler，import 表必须为空）。

退出码：0 通过；非 0 = 构建失败或探针返回的失败步骤号（见 `rust/nostd-probe/src/lib.rs`）。
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROBE = ROOT / "rust" / "nostd-probe"
RUNNER = Path(__file__).resolve().parent / "probe_nostd_wasm.mjs"
TARGET = "wasm32-unknown-unknown"


def run(cmd: list[str], cwd: Path, env: dict[str, str]) -> int:
    p = subprocess.run(cmd, cwd=str(cwd), env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")
    out = (p.stdout or "") + (p.stderr or "")
    if out.strip():
        print(out.rstrip())
    return p.returncode


def main() -> int:
    if shutil.which("node") is None:
        print("跳过：PATH 里没有 node")
        return 0

    env = dict(os.environ)
    # 固定的目标目录：不依赖调用方环境里的 CARGO_TARGET_DIR（本机那份可能在满盘的卷上）。
    env["CARGO_TARGET_DIR"] = str(PROBE / "target")
    env["CARGO_INCREMENTAL"] = "0"

    rc = run(["cargo", "build", "--release", "--target", TARGET], PROBE, env)
    if rc != 0:
        print("失败：构建没通过")
        return rc

    wasm = PROBE / "target" / TARGET / "release" / "nostd_probe.wasm"
    if not wasm.exists():
        print(f"失败：产物不存在 {wasm}")
        return 4

    print(f"产物 {wasm.name}: {wasm.stat().st_size / 1024:.1f} KiB")
    return run(["node", str(RUNNER), str(wasm)], ROOT, env)


if __name__ == "__main__":
    sys.exit(main())
