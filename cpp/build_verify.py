# SPDX-License-Identifier: MulanPSL-2.0
# build_verify.py - compile and run the C++ SML demo, capture output.
import argparse
import subprocess
import sys
import os

HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(HERE)  # cpp/ 的上级 = 仓库根
os.environ["PATH"] = r"C:\msys64\ucrt64\bin" + ";" + os.environ.get("PATH", "")

# RS-BRIDGE 需要 Rust cdylib；CI 里要先 `cargo build --release` 再显式传 SML_RUST_LIB。
# 默认用仓库内相对推导（不再写死本机盘符：审计曾发现 E:/snoware-target 这类硬编码）。
# 找不到库时：默认整体失败（不再假绿）；除非显式 --allow-skip-rs-bridge。
AP = argparse.ArgumentParser(description="C++ SML 构建/运行验证")
AP.add_argument("--allow-skip-rs-bridge", action="store_true",
                help="Rust cdylib 缺失时跳过 RS-BRIDGE 而不报错（仅本地缺库排查用；CI 不带）")
ARGS = AP.parse_args()

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

# --- 深度守卫回归用例：纯块嵌套曾绕过守卫打穿栈，必须报错而不是崩 ---
rc = run(["g++", "-std=c++17", "-I.", "-o", "t_limits.exe", "test_limits.cpp", "sml.cpp"])
if rc == 0:
    rc2, out = run_out([os.path.join(HERE, "t_limits.exe")])
    with open(os.path.join(HERE, "t_limits_out.txt"), "w", encoding="utf-8") as f:
        f.write(out)
    print("LIMITS rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)

# --- 错误码回归（W10，C++ 侧）：触发条件 → 期望码，码是跨端契约 ---
rc = run(["g++", "-std=c++17", "-I.", "-o", "t_codes.exe", "test_codes.cpp", "sml.cpp"])
if rc == 0:
    rc2, out = run_out([os.path.join(HERE, "t_codes.exe")])
    with open(os.path.join(HERE, "t_codes_out.txt"), "w", encoding="utf-8") as f:
        f.write(out)
    print("CODES rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)
else:
    sys.exit(rc)

# --- 桥接 Rust cdylib 的 v3 能力 (sml_rs.* 与原生 sml.cpp 并存) ---
SML_RUST_LIB = os.environ.get("SML_RUST_LIB")
if SML_RUST_LIB is None:
    SML_RUST_LIB = os.path.join(REPO_ROOT, "rust", "target", "release")
    print("SML_RUST_LIB 未设置，默认用仓库内相对路径: %s" % SML_RUST_LIB)

# 不同平台 cdylib 产物名不同：Linux libsml.so / Windows sml.lib(+sml.dll) / macOS libsml.dylib
_RS_CANDIDATES = ["libsml.so", "sml.lib", "libsml.dylib", "sml.dll"]
_rs_present = os.path.isdir(SML_RUST_LIB) and any(
    os.path.exists(os.path.join(SML_RUST_LIB, n)) for n in _RS_CANDIDATES
)

if not _rs_present:
    if ARGS.allow_skip_rs_bridge:
        print("RS-BRIDGE 跳过（--allow-skip-rs-bridge）: 在 %s 下找不到 Rust cdylib" % SML_RUST_LIB)
    else:
        print("RS-BRIDGE 失败: 在 %s 下找不到 Rust cdylib（先 `cargo build --release` 并设 SML_RUST_LIB）" % SML_RUST_LIB)
        sys.exit(1)

rc = run(["g++", "-std=c++17", "-I.", "-o", "example_rs.exe",
          "example_rs.cpp", "sml_rs.cpp", "-L" + SML_RUST_LIB, "-lsml"])
if rc == 0:
    rc2, out = run_out([os.path.join(HERE, "example_rs.exe")])
    with open(os.path.join(HERE, "example_rs_out.txt"), "w", encoding="utf-8") as f:
        f.write(out)
    print("RS-BRIDGE rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)
else:
    # 编译/链接失败（含缺库）→ 整体失败，绝不允许静默跳过
    sys.exit(rc)

sys.exit(0)
