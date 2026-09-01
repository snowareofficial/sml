"""用 Miri 跑 C-ABI 测试（解释执行，检测 UB）。

Miri 极慢，且是**内存开销放大器**：50000 层嵌套这类用例在 native 下毫秒级，
在 Miri 下会直接 OOM / 挂死。故本脚本：
  1. 只跑指定的测试目标（默认 tests/c_abi.rs）；
  2. 逐个用例运行（`--exact`），每个用例单独计时并设超时，避免一个用例拖垮全局；
  3. 结果与耗时写入 miri_result.txt，便于复看。

用法：
    python miri_check.py                      # 跑 tests/c_abi.rs 全部用例
    python miri_check.py --test security      # 跑 tests/security.rs
    python miri_check.py --timeout 300        # 单用例超时（秒），默认 600
    python miri_check.py --skip 名字1 名字2    # 跳过指定用例
"""
import argparse
import os
import re
import subprocess
import sys
import time

sys.stdout.reconfigure(encoding="utf-8")

# Miri 下这些用例会 OOM / 挂死（native 下是压测用例，非 UB 检测目标）
DEFAULT_SKIP = {
    "deep",  # 深层嵌套压测（50000 层）
    "fuzz",  # 模糊测试循环
    "stress",
    "huge",
    "50000",
}


def list_tests(test_target: str) -> list:
    """列出某个集成测试目标里的所有 #[test] 用例名。"""
    src = f"tests/{test_target}.rs"
    if not os.path.isfile(src):
        print(f"[!] 找不到 {src}")
        return []
    names = []
    for line in open(src, encoding="utf-8"):
        m = re.match(r"\s*fn\s+([A-Za-z0-9_]+)\s*\(", line)
        if m:
            names.append(m.group(1))
    return names


def run_one(test_target: str, name: str, timeout: int) -> tuple:
    """返回 (状态, 耗时秒, 关键输出)。状态：PASS / FAIL / TIMEOUT / SKIP"""
    env = dict(os.environ)
    # Miri 默认开启隔离，文件 / 环境变量访问会直接报 unsupported；
    # C-ABI 测试含临时文件与 env 读写，故关闭隔离。
    env.setdefault("MIRIFLAGS", "-Zmiri-disable-isolation")
    # 关掉栈溢出的人为限制提示噪音
    cmd = [
        "cargo", "+nightly", "miri", "test",
        "--test", test_target,
        "--", "--exact", name, "--nocapture",
    ]
    t0 = time.time()
    try:
        p = subprocess.run(cmd, cwd=".", env=env, capture_output=True,
                           text=True, encoding="utf-8", errors="replace",
                           timeout=timeout)
    except subprocess.TimeoutExpired:
        return "TIMEOUT", time.time() - t0, f"超过 {timeout}s"
    out = (p.stdout or "") + (p.stderr or "")
    dt = time.time() - t0
    if p.returncode == 0:
        return "PASS", dt, ""
    # 提取 Miri 的 UB 报告段
    keep = []
    grab = False
    for l in out.splitlines():
        if "error: Undefined Behavior" in l or "error[E" in l:
            grab = True
        if grab:
            keep.append(l)
        if l.strip() == "" and keep and len(keep) > 40:
            break
    if not keep:
        keep = [l for l in out.splitlines() if "error" in l.lower()][:20]
    return "FAIL", dt, "\n".join(keep[:40])


def setup() -> int:
    print("== cargo miri setup ==")
    p = subprocess.run(["cargo", "+nightly", "miri", "setup"], cwd=".",
                       capture_output=True, text=True, encoding="utf-8",
                       errors="replace")
    print((p.stdout or "")[-1500:])
    if p.returncode != 0:
        print((p.stderr or "")[-2000:])
    return p.returncode


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--test", default="c_abi", help="tests/ 下的测试目标名（不含 .rs）")
    ap.add_argument("--timeout", type=int, default=600)
    ap.add_argument("--skip", nargs="*", default=[])
    ap.add_argument("--no-setup", action="store_true")
    args = ap.parse_args()

    if not args.no_setup:
        rc = setup()
        if rc != 0:
            print("miri setup 失败")
            return rc

    names = list_tests(args.test)
    if not names:
        return 1

    skips = set(args.skip) | DEFAULT_SKIP
    results = []
    print(f"\n== Miri: tests/{args.test}.rs 共 {len(names)} 个用例 ==\n")
    for n in names:
        if any(k.lower() in n.lower() for k in skips):
            results.append((n, "SKIP", 0.0, ""))
            print(f"  [SKIP] {n}")
            continue
        st, dt, msg = run_one(args.test, n, args.timeout)
        results.append((n, st, dt, msg))
        print(f"  [{st}] {n}  ({dt:.1f}s)")
        if msg:
            print("      " + msg.replace("\n", "\n      "))

    lines = [f"{st:8s} {dt:8.1f}s  {n}" for n, st, dt, _ in results]
    with open("miri_result.txt", "w", encoding="utf-8") as f:
        f.write("\n".join(lines))
        for n, st, _, msg in results:
            if msg:
                f.write(f"\n\n---- {n} ({st}) ----\n{msg}")

    bad = [r for r in results if r[1] in ("FAIL", "TIMEOUT")]
    print(f"\n合计 {len(results)}："
          f"PASS {sum(1 for r in results if r[1] == 'PASS')} / "
          f"FAIL {sum(1 for r in results if r[1] == 'FAIL')} / "
          f"TIMEOUT {sum(1 for r in results if r[1] == 'TIMEOUT')} / "
          f"SKIP {sum(1 for r in results if r[1] == 'SKIP')}")
    print("明细见 miri_result.txt")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
