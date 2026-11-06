#!/usr/bin/env python3
"""Dry-run: report which packaging metadata exists and which is missing.

只报告，不发布。Dry-run: report, never publish.

Why / 为什么:
    Distribution strategy is an open decision (see `bindings/PACKAGING.md`). Until it is
    made, this script answers one question only: **what files would each package manager
    need that we do not have yet**. It never writes, never publishes, and exits 0 even
    when everything is missing — a report is not a gate.
    ｜ 分发策略还没裁决。本脚本只回答「各包管理器还缺哪些文件」，不写、不发；
    缺全是正常状态，退出码仍是 0。

Use `--strict` to make it a gate (exit 1 when anything is missing).
    ｜ 要当门禁用 `--strict`。
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# (label, path, target, needle)
# `needle` guards against "file exists but the metadata is not in it" — e.g. the C#
# project file exists yet carries no `PackageId`.
# ｜ needle 防「文件在、元数据不在」：如 C# 的 csproj 存在但没有 PackageId。
CHECKS: list[tuple[str, str, str, str | None]] = [
    ("Rust swsml", "rust/Cargo.toml", "crates.io", None),
    ("Rust amberice", "rust/amberice/Cargo.toml", "crates.io", None),
    ("Rust swml", "rust/swml/Cargo.toml", "crates.io (publish = false)", None),
    ("Python (SML)", "bindings/python/pyproject.toml", "PyPI", None),
    ("Python (.ambr)", "amberice/bindings/python/pyproject.toml", "PyPI", None),
    ("Ruby", "bindings/ruby/sml.gemspec", "RubyGems", None),
    ("Java", "bindings/java/pom.xml", "Maven", None),
    ("Go", "bindings/go/go.mod", "Go modules", None),
    ("C#", "bindings/csharp/Sml.csproj", "NuGet", "<PackageId>"),
    ("Lua", "lua/sml.rockspec", "LuaRocks", None),
    ("JS", "js/package.json", "npm", None),
    ("C/C++", "packaging/vcpkg/portfile.cmake", "vcpkg", None),
]


def main() -> int:
    strict = "--strict" in sys.argv[1:]

    width = max(len(label) for label, _, _, _ in CHECKS)
    missing = 0

    print(f"{'':6}{'end':<{width}}  {'target':<28}path")
    print("-" * (width + 46))
    for label, rel, target, needle in CHECKS:
        path = ROOT / rel
        present = path.is_file()
        if present and needle is not None:
            try:
                present = needle in path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                present = False
        if not present:
            missing += 1
        mark = "OK" if present else "MISS"
        print(f"{mark:<6}{label:<{width}}  {target:<28}{rel}")

    print()
    print(f"{len(CHECKS) - missing}/{len(CHECKS)} present, {missing} missing")
    print("this is a report, not a gate — see bindings/PACKAGING.md")
    print("（只报告，不是门禁；分发策略见 bindings/PACKAGING.md）")

    if strict and missing:
        print(f"STRICT: {missing} missing")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
