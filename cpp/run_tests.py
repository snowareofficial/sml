# SPDX-License-Identifier: MulanPSL-2.0
# 产物一律落 cpp/build/（临时与源码分开；见 build_verify.py 顶部说明）。
import os
import subprocess

HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = os.path.join(HERE, "build")
os.makedirs(BUILD, exist_ok=True)
os.environ["PATH"] = r"C:\msys64\ucrt64\bin" + ";" + os.environ.get("PATH", "")


def run(cmd):
    p = subprocess.run(cmd, cwd=HERE, capture_output=True, text=True)
    return p.returncode, (p.stdout or "") + (p.stderr or "")


exe = os.path.join(BUILD, "t_contract.exe")
rc, out = run(["g++", "-std=c++17", "-I", HERE, "-o", exe,
               os.path.join(HERE, "tests", "test_contracts.cpp"), os.path.join(HERE, "sml.cpp")])
if rc != 0:
    print("COMPILE FAIL")
    print(out)
    raise SystemExit(1)
rc, out = run([exe])
with open(os.path.join(BUILD, "t_contract_out.txt"), "w", encoding="utf-8") as f:
    f.write(out)
# do not print output to stdout (avoids AMSI scan of binary stdout)
print("CONTRACT rc=%d written" % rc)
