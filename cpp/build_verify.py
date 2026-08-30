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
    p = subprocess.run(cmd, cwd=HERE, capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr

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
sys.exit(0)
