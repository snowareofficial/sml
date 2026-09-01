#!/usr/bin/env python3
"""把本目录的 SML 源转译为 Slint DSL。

    python build.py                 # calculator.sml -> calculator.slint
    python build.py foo.sml         # 指定源

依赖 rust/ 下的 smlconv（先 `python rust/build_smlconv.py` 构建）。
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SMLCONV = os.path.join(HERE, "..", "..", "rust", "..", "rust")
CANDIDATES = [
    r"E:/smv-target/debug/smlconv.exe",
    os.path.join(HERE, "..", "..", "rust", "target", "debug", "smlconv.exe"),
]


def find_smlconv() -> str:
    env = os.environ.get("SMLCONV")
    if env and os.path.isfile(env):
        return env
    for p in CANDIDATES:
        if os.path.isfile(p):
            return os.path.normpath(p)
    raise SystemExit("找不到 smlconv，请先运行：python rust/build_smlconv.py")


def main() -> int:
    src = sys.argv[1] if len(sys.argv) > 1 else "calculator.sml"
    src_path = os.path.join(HERE, src)
    out_path = os.path.splitext(src_path)[0] + ".slint"

    proc = subprocess.run(
        [find_smlconv(), "-i", src_path, "--to", "slint", "-o", out_path],
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
