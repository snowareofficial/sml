#!/usr/bin/env python3
"""把本目录的 SML 源转译为 SVG。

    python build.py                 # chart.sml -> chart.svg
    python build.py foo.sml         # 指定源

依赖 rust/ 下的 smltools（先 `cargo build -p smltools`）。
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
# smltools 的定位统一走 tools/smltools_bin.py（**不写死本机盘符**：这里原先把
# <target-dir>、<target-dir> 这类本机路径写进了候选表，换机器就失效）
sys.path.insert(0, os.path.join(ROOT, "tools"))
from smltools_bin import find_smltools  # noqa: E402


def main() -> int:
    src = sys.argv[1] if len(sys.argv) > 1 else "chart.sml"
    src_path = os.path.join(HERE, src)
    out_path = os.path.splitext(src_path)[0] + ".svg"

    binary = find_smltools()
    if not binary:
        raise SystemExit("找不到 smltools，请先构建：cd rust && cargo build -p smltools")

    proc = subprocess.run(
        [binary, "-i", src_path, "--to", "svg", "-o", out_path],
        capture_output=True,
        text=True,
        errors="replace",
    )
    if proc.stdout.strip():
        print(proc.stdout.strip())
    if proc.stderr.strip():
        print(proc.stderr.strip(), file=sys.stderr)
    if proc.returncode != 0:
        return proc.returncode

    with open(out_path, encoding="utf-8") as f:
        text = f.read()
    print(f"OK -> {out_path}  ({len(text.splitlines())} 行)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
