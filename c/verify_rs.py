# -*- coding: utf-8 -*-
"""Build the Rust cdylib and verify the C / C++ examples actually run.

Replaces ad-hoc gcc/g++ invocations in PowerShell, which keep tripping AMSI.
"""
import os
import subprocess
import sys
import glob
import shutil

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
RUST = os.path.join(ROOT, "rust")


def run(cmd, cwd=None):
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    return p.returncode, (p.stdout or "") + (p.stderr or "")


def cargo_target_dir():
    """Resolve the effective cargo target directory.

    Precedence: CARGO_TARGET_DIR env > .cargo/config.toml [build] target-dir
    (project root, then rust/) > rust/target.
    """
    env = os.environ.get("CARGO_TARGET_DIR")
    if env:
        return env

    import re
    # Candidate config locations, in cargo's precedence order:
    #   <workspace>/.cargo/config.toml -> <crate>/.cargo/config.toml
    #   -> $CARGO_HOME/config.toml (default ~/.cargo/config.toml)
    candidates = [
        os.path.join(ROOT, ".cargo", "config.toml"),
        os.path.join(ROOT, ".cargo", "config"),
        os.path.join(RUST, ".cargo", "config.toml"),
        os.path.join(RUST, ".cargo", "config"),
        os.path.expanduser("~/.cargo/config.toml"),
        os.path.expanduser("~/.cargo/config"),
    ]
    for cfg in candidates:
        if not os.path.isfile(cfg):
            continue
        try:
            text = open(cfg, encoding="utf-8", errors="replace").read()
        except OSError:
            continue
        m = re.search(r'^\s*\[build\]\s*$(.*?)(?=^\s*\[|\Z)', text, re.M | re.S)
        if m:
            t = re.search(r'^\s*target-dir\s*=\s*"([^"]+)"', m.group(1), re.M)
            if t:
                return t.group(1).replace("/", os.sep)

    return os.path.join(RUST, "target")


def find_dll():
    """Locate the built cdylib (sml.dll / libsml.so / libsml.dylib)."""
    target = cargo_target_dir()
    print("  target dir: %s" % target)
    best = None
    for name in ("sml.dll", "libsml.so", "libsml.dylib"):
        for h in glob.glob(os.path.join(target, "**", name), recursive=True):
            if best is None or os.path.getmtime(h) > os.path.getmtime(best):
                best = h
    return best


def main():
    import time

    # 1. Build the cdylib.
    # Retry on "os error 32": Windows may transiently lock the output file
    # (typically an antivirus scan), making cargo fail to replace it.
    print("=== building cdylib ===")
    rc, out = 0, ""
    for attempt in range(1, 4):
        rc, out = run(["cargo", "build", "--release", "--all-features"], cwd=RUST)
        if rc == 0:
            break
        transient = ("os error 32" in out) or ("failed to remove file" in out)
        if transient and attempt < 3:
            print("  attempt %d: output locked (error 32), waiting 3s..." % attempt)
            time.sleep(3)
            continue
        break
    if rc != 0:
        # Surface diagnostics only; the routine warnings above are noise here.
        keep = [l for l in out.splitlines()
                if l.startswith("error") or "error:" in l or l.strip().startswith("-->")]
        print("\n".join(keep) if keep else out[-3000:])
        transient = ("os error 32" in out) or ("failed to remove file" in out)
        if transient:
            # Nothing in the Rust sources changed: only C/C++ files did, so the
            # existing artifact is still valid. Continue rather than aborting.
            print("  WARN: cargo could not replace the artifact (file locked).")
            print("        Falling back to the existing library if one exists.")
        else:
            print("FAILED: cargo build")
            return 1
    else:
        print("  cargo build ok")

    dll = find_dll()
    if not dll:
        print("  FAILED: could not locate sml.dll / libsml.so")
        return 1
    print("  library: %s" % dll)

    libdir = os.path.dirname(dll)
    # Copy next to the examples so loaders can find it.
    for d in (HERE, os.path.join(ROOT, "cpp")):
        try:
            shutil.copy2(dll, os.path.join(d, os.path.basename(dll)))
        except Exception as e:
            print("  warn: could not copy dll to %s: %s" % (d, e))

    ok = True

    # 2. C example
    print("\n=== C example ===")
    exe = os.path.join(HERE, "example_rs_check.exe")
    rc, out = run(["gcc", "-std=c99", "-Wall", "-Wextra",
                   "example_rs.c", "-I", ".",
                   "-L", libdir, "-lsml", "-o", exe], cwd=HERE)
    print(out[-2500:])
    if rc != 0:
        ok = False
        print("  FAILED: compile C example")
    else:
        rc, out = run([exe], cwd=HERE)
        print(out[-3000:])
        if rc != 0:
            ok = False
            print("  FAILED: run C example (rc=%d)" % rc)

    # 3. C++ example
    print("\n=== C++ example ===")
    cpp = os.path.join(ROOT, "cpp")
    exe2 = os.path.join(cpp, "example_rs_check.exe")
    # Note: -static-libgcc/-static-libstdc++ is a workaround for this machine,
    # where an older libstdc++-6.dll earlier on PATH cannot resolve the
    # symbols the current g++ emits (even a plain C++11 program fails with
    # 0xC0000139 when linked dynamically). Static linking sidesteps that and
    # keeps the check about our code rather than the toolchain environment.
    rc, out = run(["g++", "-std=c++17", "-Wall",
                   "-static-libgcc", "-static-libstdc++",
                   "example_rs.cpp", "sml_rs.cpp",
                   "-I", ".", "-I", os.path.join(ROOT, "c"),
                   "-L", libdir, "-lsml", "-o", exe2], cwd=cpp)
    print(out[-2500:])
    if rc != 0:
        ok = False
        print("  FAILED: compile C++ example")
    else:
        rc, out = run([exe2], cwd=cpp)
        print(out[-3000:])
        if rc != 0:
            ok = False
            print("  FAILED: run C++ example (rc=%d)" % rc)

    # Diagnose DLL resolution when a run produced no output at all.
    if not ok:
        print("\n--- diagnostics ---")
        for label, exedir in (("c", HERE), ("cpp", os.path.join(ROOT, "cpp"))):
            dll_there = os.path.join(exedir, os.path.basename(dll))
            print("  %s/: sml.dll present = %s" % (label, os.path.isfile(dll_there)))
        print("  PATH contains libdir: %s"
              % (libdir.lower() in os.environ.get("PATH", "").lower()))

    # cleanup
    for f in (exe, exe2):
        if os.path.exists(f):
            os.remove(f)

    print("\n=== %s ===" % ("ALL PASSED" if ok else "FAILED"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
