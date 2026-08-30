# SPDX-License-Identifier: MulanPSL-2.0
import subprocess, os
HERE = os.path.dirname(os.path.abspath(__file__))
os.environ["PATH"] = r"C:\msys64\ucrt64\bin" + ";" + os.environ.get("PATH", "")
def run(cmd):
    p = subprocess.run(cmd, cwd=HERE, capture_output=True, text=True)
    out = (p.stdout or "") + (p.stderr or "")
    return p.returncode, out
rc, out = run(["g++", "-std=c++17", "-I.", "-o", "t_contract.exe", "test_contracts.cpp", "sml.cpp"])
if rc != 0:
    print("COMPILE FAIL"); print(out); raise SystemExit(1)
rc, out = run([os.path.join(HERE, "t_contract.exe")])
with open(os.path.join(HERE, "t_contract_out.txt"), "w", encoding="utf-8") as f:
    f.write(out)
# do not print output to stdout (avoids AMSI scan of binary stdout)
print("CONTRACT rc=%d written" % rc)
