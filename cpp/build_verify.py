# SPDX-License-Identifier: MulanPSL-2.0
# build_verify.py - compile and run the C++ SML demo, capture output.
import subprocess, sys, os

HERE = os.path.dirname(os.path.abspath(__file__))
os.environ["PATH"] = r"C:\msys64\ucrt64\bin" + ";" + os.environ.get("PATH", "")

def run(cmd):
    print(">>> " + " ".join(cmd))
    p = subprocess.run(cmd, cwd=HERE, capture_output=True, text=True)
    if p.stdout:
        sys.stdout.write(p.stdout)
    if p.stderr:
        sys.stderr.write(p.stderr)
    print("exit=%d" % p.returncode)
    return p.returncode

def run_out(cmd):
    p = subprocess.run(cmd, cwd=HERE, capture_output=True)
    out = (p.stdout or b"").decode("utf-8", "replace") + (p.stderr or b"").decode("utf-8", "replace")
    return p.returncode, out

rc = run(["g++", "-std=c++17", "-I.", "-o", "example.exe", "example.cpp", "sml.cpp"])
if rc != 0:
    sys.exit(rc)
rc = run([os.path.join(HERE, "example.exe")])
if rc != 0:
    sys.exit(rc)

rc = run(["g++", "-std=c++17", "-I.", "-o", "t_contract.exe", "test_contracts.cpp", "sml.cpp"])
if rc == 0:
    rc2, out = run_out([os.path.join(HERE, "t_contract.exe")])
    with open(os.path.join(HERE, "t_contract_out.txt"), "w", encoding="utf-8") as f:
        f.write(out)
    print("CONTRACT rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)

rc = run(["g++", "-std=c++17", "-I.", "-o", "t_comments.exe", "test_comments.cpp", "sml.cpp"])
if rc == 0:
    rc2, out = run_out([os.path.join(HERE, "t_comments.exe")])
    with open(os.path.join(HERE, "t_comments_out.txt"), "w", encoding="utf-8") as f:
        f.write(out)
    print("COMMENTS rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)

# --- 桥接 Rust cdylib 的 v3 能力 (sml_rs.* 与原生 sml.cpp 并存) ---
RUST_LIB = os.environ.get("SML_RUST_LIB", r"E:/snoware-target/release")
rc = run(["g++", "-std=c++17", "-I.", "-o", "example_rs.exe",
          "example_rs.cpp", "sml_rs.cpp", "-L" + RUST_LIB, "-lsml"])
if rc == 0:
    rc2, out = run_out([os.path.join(HERE, "example_rs.exe")])
    with open(os.path.join(HERE, "example_rs_out.txt"), "w", encoding="utf-8") as f:
        f.write(out)
    print("RS-BRIDGE rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)
else:
    print("RS-BRIDGE 跳过: 未找到 Rust cdylib (设置 SML_RUST_LIB 指向 cargo target/release)")

sys.exit(0)
