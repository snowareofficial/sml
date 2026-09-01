"""运行测试并只打印失败详情（规避 PowerShell 输出截断与 AMSI 崩溃）。"""
import subprocess, sys

sys.stdout.reconfigure(encoding="utf-8")
target = sys.argv[1] if len(sys.argv) > 1 else ""

cmd = ["cargo", "test"]
if target:
    cmd += ["--test", target]

p = subprocess.run(cmd, cwd=".", capture_output=True, text=True,
                   encoding="utf-8", errors="replace")
out = (p.stdout or "") + (p.stderr or "")

lines = out.splitlines()
# 输出结果摘要
for l in lines:
    if l.startswith("test result") or l.startswith("running "):
        print(l)

if p.returncode != 0:
    print("\n================ FAILURES ================")
    keep = []
    grab = False
    for l in lines:
        if l.startswith("---- "):
            grab = True
        if grab:
            keep.append(l)
        if l.startswith("failures:") :
            grab = False
    print("\n".join(keep[:120]))
else:
    print("ALL GREEN")

print("rc =", p.returncode)
