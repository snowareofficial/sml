#!/usr/bin/env python3
"""把本目录的 SML 源转译为 SVG。

    python build.py                 # chart.sml -> chart.svg
    python build.py foo.sml         # 指定源

依赖 rust/ 下的 smlconv（先 `cargo build -p smlconv`）。
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CANDIDATES = [
    os.environ.get("SMLCONV", ""),
    r"E:/snoware-target/debug/smlconv.exe",   # CARGO_TARGET_DIR 指向 E 盘时
    r"E:/snoware-target/release/smlconv.exe",
    r"E:/smv-target/debug/smlconv.exe",
    os.path.join(HERE, "..", "..", "rust", "target", "debug", "smlconv.exe"),
    os.path.join(HERE, "..", "..", "rust", "target", "release", "smlconv.exe"),
]


def find_smlconv() -> str:
    for p in CANDIDATES:
        if p and os.path.isfile(p):
            return os.path.normpath(p)
    raise SystemExit("找不到 smlconv，请先构建：cd rust && cargo build -p smlconv")


def main() -> int:
    src = sys.argv[1] if len(sys.argv) > 1 else "chart.sml"
    src_path = os.path.join(HERE, src)
    out_path = os.path.splitext(src_path)[0] + ".svg"

    proc = subprocess.run(
        [find_smlconv(), "-i", src_path, "--to", "svg", "-o", out_path],
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
