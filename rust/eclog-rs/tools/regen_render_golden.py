#!/usr/bin/env python3
"""Rebuild the render conformance expectations from the C++ oracle.

Usage (from anywhere):
    python rust/eclog-rs/tools/regen_render_golden.py --check
    python rust/eclog-rs/tools/regen_render_golden.py --write

`--check` recompiles Eclog-CPP, re-renders every golden input with both schemes
(plus the two constructed-value edge cases) and compares the bytes with the
committed `tests/golden/render/*.render`. Any drift exits non-zero.

Requirements: a C++11 compiler (`g++` by default) and a local clone of
https://github.com/Vallest/Eclog-CPP (default `<repo>/_scratch/eclog-cpp`,
which is gitignored). This is a developer tool: CI has neither.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(CRATE))

GOLDEN = os.path.join(CRATE, "tests", "golden")
RENDER = os.path.join(GOLDEN, "render")

SCHEMES = ("typical", "json")
EDGE_CASES = ("typical", "compact")

# `edge.cpp` 与 `tests/render_merge.rs` 的 `edge_tree()` 必须同步修改。
EDGE_OUTPUTS = ("edge.typical.render", "edge.compact.render")


def inputs() -> list[str]:
    names = [
        name
        for name in sorted(os.listdir(GOLDEN))
        if name.endswith(".ecl") or name.endswith(".json")
    ]
    return names


def build(cc: str, eclog_cpp: str, workdir: str) -> tuple[str, str]:
    compact = os.path.join(eclog_cpp, "Compact")
    impl = os.path.join(compact, "Eclog.cpp")
    if not os.path.exists(impl):
        raise SystemExit(
            f"找不到 Eclog-CPP 的 amalgamation：{impl}\n"
            f"用 --eclog-cpp 指向浅克隆（git -c http.sslVerify=false clone --depth 1 "
            f"https://github.com/Vallest/Eclog-CPP.git _scratch/eclog-cpp）"
        )

    suffix = ".exe" if os.name == "nt" else ""
    built = []
    for name in ("oracle", "edge"):
        out = os.path.join(workdir, name + suffix)
        cmd = [
            cc,
            "-std=c++11",
            "-O1",
            "-I",
            compact,
            os.path.join(HERE, name + ".cpp"),
            impl,
            "-o",
            out,
        ]
        proc = subprocess.run(cmd, capture_output=True, text=True)
        if proc.returncode != 0:
            raise SystemExit(
                "编译失败：" + " ".join(cmd) + "\n" + proc.stdout + proc.stderr
            )
        built.append(out)

    return built[0], built[1]


def render_all(oracle: str, edge: str) -> dict[str, bytes]:
    """Return {output file name: bytes}."""
    produced: dict[str, bytes] = {}

    for name in inputs():
        source = os.path.join(GOLDEN, name)
        for scheme in SCHEMES:
            proc = subprocess.run([oracle, source, scheme], capture_output=True)
            if proc.returncode != 0:
                raise SystemExit(
                    f"{name} [{scheme}] 渲染失败：{proc.stderr.decode('utf-8', 'replace')}"
                )
            produced[f"{name}.{scheme}.render"] = proc.stdout

    for scheme in EDGE_CASES:
        proc = subprocess.run([edge, scheme], capture_output=True)
        if proc.returncode != 0:
            raise SystemExit(
                f"edge [{scheme}] 渲染失败：{proc.stderr.decode('utf-8', 'replace')}"
            )
        produced[f"edge.{scheme}.render"] = proc.stdout

    return produced


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--eclog-cpp",
        default=os.path.join(REPO, "_scratch", "eclog-cpp"),
        help="Eclog-CPP 浅克隆路径（默认 <repo>/_scratch/eclog-cpp）",
    )
    parser.add_argument("--cc", default="g++", help="C++11 编译器（默认 g++）")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--write", action="store_true", help="重新生成期望文件")
    mode.add_argument("--check", action="store_true", help="只比对，不改文件")
    args = parser.parse_args()

    if not args.write and not args.check:
        args.check = True

    workdir = tempfile.mkdtemp(prefix="eclog-oracle-")
    try:
        oracle, edge = build(args.cc, args.eclog_cpp, workdir)
        produced = render_all(oracle, edge)
    finally:
        shutil.rmtree(workdir, ignore_errors=True)

    missing = sorted(set(EDGE_OUTPUTS) - set(produced))
    if missing:
        raise SystemExit(f"未产出这些期望文件：{missing}")

    if args.write:
        os.makedirs(RENDER, exist_ok=True)
        expected_names = set(produced)
        for name, data in produced.items():
            with open(os.path.join(RENDER, name), "wb") as fh:
                fh.write(data)
        stale = sorted(set(os.listdir(RENDER)) - expected_names)
        print(f"已写入 {len(produced)} 份期望（{len(inputs())} 输入 × {len(SCHEMES)} scheme + 2 边界）")
        if stale:
            print("目录里多出这些未被本次生成的文件（未删除，请人工确认）：" + ", ".join(stale))
        return 0

    mismatches = []
    for name, data in sorted(produced.items()):
        path = os.path.join(RENDER, name)
        if not os.path.exists(path):
            mismatches.append(f"{name}: 缺文件")
            continue
        with open(path, "rb") as fh:
            actual = fh.read()
        if actual != data:
            mismatches.append(
                f"{name}: 期望 {len(actual)} 字节 / oracle {len(data)} 字节"
            )

    extras = sorted(set(os.listdir(RENDER)) - set(produced)) if os.path.isdir(RENDER) else []

    if mismatches or extras:
        print("渲染期望与 oracle 不一致：")
        for item in mismatches:
            print("  " + item)
        for item in extras:
            print(f"  {item}: 目录里多出、oracle 不再产出")
        return 1

    print(
        f"OK：{len(produced)} 份期望与 oracle 逐字节一致"
        f"（{len(inputs())} 输入 × {len(SCHEMES)} scheme + 2 边界）"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
