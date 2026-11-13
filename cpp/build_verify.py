# SPDX-License-Identifier: MulanPSL-2.0
# build_verify.py - compile and run the C++ SML demo, capture output.
#
# 目录约定（2026-11-06 起）：源码在 cpp/（库）、cpp/examples/、cpp/tests/；
# **所有产物一律落到 cpp/build/**（被 .gitignore 忽略）——
# 不再把 .exe / *_out.txt 丢在源码旁边，临时与永久分开。
import argparse
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO_ROOT = os.path.dirname(HERE)  # cpp/ 的上级 = 仓库根
BUILD = os.path.join(HERE, "build")
os.makedirs(BUILD, exist_ok=True)
if sys.platform == "win32":
    os.environ["PATH"] = r"C:\msys64\ucrt64\bin" + ";" + os.environ.get("PATH", "")

# RS-BRIDGE 需要 Rust cdylib；CI 里要先 `cargo build --release` 再显式传 SML_RUST_LIB。
# 默认用仓库内相对推导（不再写死本机盘符：审计曾发现 <target-dir> 这类硬编码）。
# 找不到库时：默认整体失败（不再假绿）；除非显式 --allow-skip-rs-bridge。
AP = argparse.ArgumentParser(description="C++ SML 构建/运行验证")
AP.add_argument("--allow-skip-rs-bridge", action="store_true",
                help="Rust cdylib 缺失时跳过 RS-BRIDGE 而不报错（仅本地缺库排查用；CI 不带）")
ARGS = AP.parse_args()


# Windows 上 PATH 里的 libstdc++ 可能与编译期版本不匹配：新代码用 <fstream> 后会
# 报 0xC0000139（ENTRYPOINT_NOT_FOUND）。静态链接这两个运行时库即可，Linux 不需要。
EXTRA = ["-static-libstdc++", "-static-libgcc"] if sys.platform == "win32" else []

def _child_env():
    """子进程环境：非 Windows 上把 Rust cdylib 目录塞进**运行期**动态库搜索路径。

    `-L` 只影响链接；`example_rs.exe` 启动时由 loader 找 `libsml.so`，没有
    `LD_LIBRARY_PATH` 就会 exit 127（CI 上就踩过）。
    ｜ `-L` is link-time only; the loader needs LD_LIBRARY_PATH at runtime.
    """
    env = dict(os.environ)
    lib = env.get("SML_RUST_LIB")
    if lib and sys.platform != "win32":
        key = "DYLD_LIBRARY_PATH" if sys.platform == "darwin" else "LD_LIBRARY_PATH"
        old = env.get(key)
        env[key] = lib + ((os.pathsep + old) if old else "")
    return env


def run(cmd):
    print(">>> " + " ".join(cmd))
    # 子进程输出含中文 / 箭头：强制 UTF-8 解码，别让 locale(GBK) 把测试判成失败。
    p = subprocess.run(cmd, cwd=HERE, env=_child_env(), capture_output=True, text=True,
                       encoding="utf-8", errors="replace")
    if p.stdout:
        sys.stdout.write(p.stdout)
    if p.stderr:
        sys.stderr.write(p.stderr)
    print("exit=%d" % p.returncode)
    return p.returncode


def run_out(cmd):
    p = subprocess.run(cmd, cwd=HERE, env=_child_env(), capture_output=True)
    out = (p.stdout or b"").decode("utf-8", "replace") + (p.stderr or b"").decode("utf-8", "replace")
    return p.returncode, out


def exe(name):
    return os.path.join(BUILD, name)


def write_log(name, text):
    with open(os.path.join(BUILD, name), "w", encoding="utf-8") as f:
        f.write(text)


rc = run(["g++", "-std=c++17", *EXTRA, "-I", HERE, "-o", exe("example.exe"),
          os.path.join(HERE, "examples", "example.cpp"), os.path.join(HERE, "sml.cpp")])
if rc != 0:
    sys.exit(rc)
rc = run([exe("example.exe")])
if rc != 0:
    sys.exit(rc)

rc = run(["g++", "-std=c++17", *EXTRA, "-I", HERE, "-o", exe("t_contract.exe"),
          os.path.join(HERE, "tests", "test_contracts.cpp"), os.path.join(HERE, "sml.cpp")])
if rc == 0:
    rc2, out = run_out([exe("t_contract.exe")])
    write_log("t_contract_out.txt", out)
    print("CONTRACT rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)

rc = run(["g++", "-std=c++17", *EXTRA, "-I", HERE, "-o", exe("t_comments.exe"),
          os.path.join(HERE, "tests", "test_comments.cpp"), os.path.join(HERE, "sml.cpp")])
if rc == 0:
    rc2, out = run_out([exe("t_comments.exe")])
    write_log("t_comments_out.txt", out)
    print("COMMENTS rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)

# --- 深度守卫回归用例：纯块嵌套曾绕过守卫打穿栈，必须报错而不是崩 ---
rc = run(["g++", "-std=c++17", *EXTRA, "-I", HERE, "-o", exe("t_limits.exe"),
          os.path.join(HERE, "tests", "test_limits.cpp"), os.path.join(HERE, "sml.cpp")])
if rc == 0:
    rc2, out = run_out([exe("t_limits.exe")])
    write_log("t_limits_out.txt", out)
    print("LIMITS rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)

# --- 错误码回归（W10，C++ 侧）：触发条件 → 期望码，码是跨端契约 ---
rc = run(["g++", "-std=c++17", *EXTRA, "-I", HERE, "-o", exe("t_codes.exe"),
          os.path.join(HERE, "tests", "test_codes.cpp"), os.path.join(HERE, "sml.cpp")])
if rc == 0:
    rc2, out = run_out([exe("t_codes.exe")])
    write_log("t_codes_out.txt", out)
    print("CODES rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)
else:
    sys.exit(rc)

# --- `.ambr` 只读解码器对拍容器向量（T7 / N03）：21 例 + 语义抽检 ---
rc = run(["g++", "-std=c++17", *EXTRA, "-I", HERE, "-o", exe("t_ambr.exe"),
          os.path.join(HERE, "tests", "test_ambr.cpp"), os.path.join(HERE, "ambr.cpp")])
if rc == 0:
    rc2, out = run_out([exe("t_ambr.exe")])
    write_log("t_ambr_out.txt", out)
    print("AMBR rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)
else:
    sys.exit(rc)

# --- D45：SCHEMA/TYPED 动态渲染（fixture 与 C / JS 共用） ---
rc = run(["g++", "-std=c++17", *EXTRA, "-I", HERE, "-o", exe("t_ambr_typed.exe"),
          os.path.join(HERE, "tests", "test_ambr_typed.cpp"), os.path.join(HERE, "ambr.cpp")])
if rc == 0:
    rc2, out = run_out([exe("t_ambr_typed.exe")])
    write_log("t_ambr_typed_out.txt", out)
    print("AMBR-TYPED rc=%d" % rc2)
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
        # 显式允许跳过：**整段**跳过（原先只打印一句仍然去编译，必然失败 —— 假跳过）。
        print("RS-BRIDGE 跳过（--allow-skip-rs-bridge）: 在 %s 下找不到 Rust cdylib" % SML_RUST_LIB)
        sys.exit(0)
    print("RS-BRIDGE 失败: 在 %s 下找不到 Rust cdylib（先 `cargo build --release` 并设 SML_RUST_LIB）" % SML_RUST_LIB)
    sys.exit(1)

rc = run(["g++", "-std=c++17", *EXTRA, "-I", HERE, "-o", exe("example_rs.exe"),
          os.path.join(HERE, "examples", "example_rs.cpp"), os.path.join(HERE, "sml_rs.cpp"),
          "-L" + SML_RUST_LIB, "-lsml"])
if rc == 0:
    rc2, out = run_out([exe("example_rs.exe")])
    write_log("example_rs_out.txt", out)
    print("RS-BRIDGE rc=%d" % rc2)
    if rc2 != 0: sys.exit(rc2)
else:
    # 编译/链接失败（含缺库）→ 整体失败，绝不允许静默跳过
    sys.exit(rc)

# --- P7：诊断出口（sml::loads_with_diags）—— C-ABI 功能对等的最小钉子 ---
rc = run(["g++", "-std=c++17", *EXTRA, "-I", HERE, "-o", exe("t_rs_diag.exe"),
          os.path.join(HERE, "tests", "test_rs_diag.cpp"), os.path.join(HERE, "sml_rs.cpp"),
          "-L" + SML_RUST_LIB, "-lsml"])
if rc != 0:
    sys.exit(rc)
rc2, out = run_out([exe("t_rs_diag.exe")])
write_log("t_rs_diag_out.txt", out)
print("RS-DIAG rc=%d" % rc2)
if rc2 != 0:
    sys.exit(rc2)

sys.exit(0)
