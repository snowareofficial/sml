"""运行 cargo clippy 并按类别汇总告警/错误（规避 PowerShell 输出截断与 AMSI 崩溃）。

用法：
    python clippy_report.py                 # 全部类别计数 + 明细
    python clippy_report.py --show <关键字>  # 只打印含关键字的原始块
"""
import re
import subprocess
import sys
from collections import Counter

sys.stdout.reconfigure(encoding="utf-8")


def run():
    p = subprocess.run(
        ["cargo", "clippy", "--all-features", "--all-targets", "--message-format=short"],
        cwd=".",
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return (p.stdout or "") + (p.stderr or "")


def main() -> int:
    out = run()

    if "--show" in sys.argv:
        kw = sys.argv[sys.argv.index("--show") + 1]
        for line in out.splitlines():
            if kw in line:
                print(line)
        return 0

    counts: Counter = Counter()
    locs: dict = {}
    # short 格式：`src/c_abi.rs:34:5: error: 说明`
    pat = re.compile(r"^(\S+\.rs):(\d+):(\d+):\s*(error|warning):\s*(.*)$")
    for line in out.splitlines():
        m = pat.match(line.strip())
        if not m:
            continue
        f, ln, _col, sev, msg = m.groups()
        # 归一：去掉借用/泛型等噪音，取前 70 字符
        key = (sev, msg[:70])
        counts[key] += 1
        locs.setdefault(key, []).append(f"{f}:{ln}")

    if not counts:
        print("clippy: 无告警无错误")
        return 0

    for sev in ("error", "warning"):
        items = [(k, c) for k, c in counts.items() if k[0] == sev]
        items.sort(key=lambda x: -x[1])
        print(f"\n===== {sev.upper()} ({sum(c for _, c in items)}) =====")
        for (_, msg), c in items:
            print(f"  [{c}x] {msg}")
            for loc in locs[(_sev := sev, msg)][:6]:
                print(f"        {loc}")
    return 1 if counts.get(("error", "")) or any(k[0] == "error" for k in counts) else 0


if __name__ == "__main__":
    sys.exit(main())
