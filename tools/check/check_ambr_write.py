#!/usr/bin/env python3
"""Cross-platform check for the optional `.ambr` writer (C / C++ / Python).

跨平台的 `.ambr` 可选写入器检查（C / C++ / Python）。

Why a separate script / 为什么单独一个脚本:
    Writing needs the `amberice-ffi` cdylib built with `--features sml`. Read-only jobs
    (and contributors without a built cdylib) must not fail because of that. So this
    script **skips loudly** when no library is found — it never fakes a pass.
    ｜ 写入需要带 `sml` feature 的 `amberice-ffi` cdylib。没有库时必须**显式跳过**，
    不能用「0 个失败」冒充通过。

Exit codes / 退出码:
    0  passed, or skipped (prints SKIP with the reason)
    1  a check failed
    2  a compiler / interpreter is missing (also a skip, not a failure)

Usage / 用法:
    python3 tools/check/check_ambr_write.py
    AMBR_LIB=/path/to/libamberice_ffi.so python3 tools/check/check_ambr_write.py
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BUILD = ROOT / "build" / "ambr_write"

LIB_NAMES = ("libamberice_ffi.so", "libamberice_ffi.dylib", "amberice_ffi.dll")


def log(msg: str) -> None:
    print(msg, flush=True)


def skip(reason: str) -> int:
    log(f"SKIP  {reason}")
    log("      (build amberice-ffi with --features sml to enable / 用 --features sml 构建后启用)")
    return 0


def find_lib() -> Path | None:
    """AMBR_LIB wins; otherwise look in rust/target/{release,debug}."""
    env = os.environ.get("AMBR_LIB")
    if env:
        p = Path(env)
        return p if p.is_file() else None
    for profile in ("release", "debug"):
        for name in LIB_NAMES:
            p = ROOT / "rust" / "target" / profile / name
            if p.is_file():
                return p
    # Windows: the import library lives next to the sources after gendef/dlltool.
    return None


def find_implib(lib: Path) -> Path | None:
    """Windows + MinGW needs a lib*.dll.a next to (or near) the dll."""
    env = os.environ.get("AMBR_IMPLIB")
    if env and Path(env).is_file():
        return Path(env)
    for cand in (lib.parent / "libamberice_ffi.dll.a", ROOT / "c" / "libamberice_ffi.dll.a"):
        if cand.is_file():
            return cand
    return None


def env_with_libdir(lib: Path) -> dict[str, str]:
    """Make the cdylib discoverable at run time, without touching the parent env."""
    env = dict(os.environ)
    d = str(lib.parent)
    if os.name == "nt":
        env["PATH"] = d + os.pathsep + env.get("PATH", "")
    elif sys.platform == "darwin":
        env["DYLD_LIBRARY_PATH"] = d + os.pathsep + env.get("DYLD_LIBRARY_PATH", "")
    else:
        env["LD_LIBRARY_PATH"] = d + os.pathsep + env.get("LD_LIBRARY_PATH", "")
    env["AMBR_LIB"] = str(lib)
    return env


def run(cmd: list[str], cwd: Path, env: dict[str, str] | None = None) -> int:
    shown = " ".join(cmd)
    log(f"  $ {shown}")
    try:
        return subprocess.call(cmd, cwd=str(cwd), env=env)
    except FileNotFoundError:
        log(f"  ! not found: {cmd[0]}")
        return 127


def toolchain_env(env: dict[str, str], compiler: str) -> dict[str, str]:
    """Put the compiler's own bin dir first: MinGW-built binaries need libstdc++ etc.
    ｜ 把编译器所在目录前置：MinGW 产物运行时要 libstdc++ / libgcc。"""
    out = dict(env)
    d = str(Path(compiler).resolve().parent)
    out["PATH"] = d + os.pathsep + out.get("PATH", "")
    return out


def check_c(lib: Path, env: dict[str, str]) -> int:
    cc = os.environ.get("CC") or shutil.which("gcc") or shutil.which("cc")
    if not cc:
        return 2

    BUILD.mkdir(parents=True, exist_ok=True)
    exe = BUILD / ("test_ambr_write" + (".exe" if os.name == "nt" else ""))

    link: list[str] = ["-L", str(lib.parent), "-lamberice_ffi"]
    if os.name == "nt":
        implib = find_implib(lib)
        if implib is None:
            log("  ! Windows: no libamberice_ffi.dll.a (run gendef + dlltool); skipping C")
            return 2
        link = [str(implib)]

    rc = run(
        [cc, "-std=c99", "-I", str(ROOT / "c"), "-o", str(exe),
         str(ROOT / "c" / "tests" / "test_ambr_write.c"),
         str(ROOT / "c" / "ambr.c"),
         str(ROOT / "c" / "ambr_write.c"),
         str(ROOT / "c" / "sml.c"),
         *link],
        cwd=ROOT / "c",
    )
    if rc != 0:
        return rc
    return run([str(exe)], cwd=ROOT / "c", env=toolchain_env(env, cc))


def check_cpp(lib: Path, env: dict[str, str]) -> int:
    cxx = os.environ.get("CXX") or shutil.which("g++") or shutil.which("c++")
    if not cxx:
        return 2

    BUILD.mkdir(parents=True, exist_ok=True)
    exe = BUILD / ("t_ambr_write_cpp" + (".exe" if os.name == "nt" else ""))

    link: list[str] = ["-L", str(lib.parent), "-lamberice_ffi"]
    if os.name == "nt":
        implib = find_implib(lib)
        if implib is None:
            log("  ! Windows: no libamberice_ffi.dll.a; skipping C++")
            return 2
        link = [str(implib)]

    rc = run(
        [cxx, "-std=c++17", "-I", str(ROOT / "cpp"), "-o", str(exe),
         str(ROOT / "cpp" / "tests" / "test_ambr_write.cpp"),
         str(ROOT / "cpp" / "ambr.cpp"),
         str(ROOT / "cpp" / "ambr_write.cpp"),
         str(ROOT / "cpp" / "sml.cpp"),
         *link],
        cwd=ROOT / "cpp",
    )
    if rc != 0:
        return rc
    return run([str(exe)], cwd=ROOT / "cpp", env=toolchain_env(env, cxx))


def check_python(lib: Path, env: dict[str, str]) -> int:
    py = sys.executable or shutil.which("python3")
    if not py:
        return 2
    return run([py, str(ROOT / "amberice" / "bindings" / "python" / "test_ambr.py")],
               cwd=ROOT, env=env)


def main() -> int:
    lib = find_lib()
    if lib is None:
        return skip("no amberice cdylib (AMBR_LIB / rust/target/{release,debug})")

    log(f"lib  {lib}")
    env = env_with_libdir(lib)

    results: list[tuple[str, int]] = []
    for name, fn in (("C", check_c), ("C++", check_cpp), ("Python", check_python)):
        log(f"--- {name} ---")
        results.append((name, fn(lib, env)))

    # Any other non-zero code is a real failure — a crashed test must never look like a skip.
    # 其它非零码一律算失败：崩溃的测试不许伪装成跳过。
    missing = [n for n, rc in results if rc in (2, 127)]
    failed = [n for n, rc in results if rc != 0 and rc not in (2, 127)]

    if failed:
        log(f"FAIL  {', '.join(failed)}")
        return 1
    if missing:
        log(f"SKIP  {', '.join(missing)} (toolchain or import library missing)")
    log("ALL OK" if not missing else "PASS (with skips)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
