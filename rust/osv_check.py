"""用 OSV.dev API 检查 Cargo.lock 中所有依赖的已知漏洞。"""
import json, re, urllib.request, sys

ROOT = "C:/Users/sakeen/Desktop/sml/rust"
lock = open(ROOT + "/Cargo.lock", encoding="utf-8").read()

pkgs = []
for block in lock.split("[[package]]")[1:]:
    name = re.search(r'^name = "(.*?)"', block, re.M)
    ver = re.search(r'^version = "(.*?)"', block, re.M)
    src = re.search(r'^source = "(.*?)"', block, re.M)
    if name and ver:
        pkgs.append((name.group(1), ver.group(1), src.group(1) if src else "local"))

print(f"共 {len(pkgs)} 个包，查询 OSV ...\n")
vulns = []
for name, ver, src in pkgs:
    if src == "local":
        continue
    body = json.dumps({
        "package": {"name": name, "ecosystem": "crates.io"},
        "version": ver,
    }).encode()
    req = urllib.request.Request(
        "https://api.osv.dev/v1/query",
        data=body,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            res = json.loads(r.read().decode())
    except Exception as e:
        print(f"  [!] {name} {ver} 查询失败: {e}")
        continue
    for v in res.get("vulns", []):
        aliases = ",".join(v.get("aliases", [])[:3])
        sev = "?"
        for s in v.get("severity", []):
            sev = s.get("score", "?")
        db = v.get("database_specific", {})
        vulns.append((name, ver, v.get("id"), aliases, v.get("summary", "")[:110],
                      db.get("severity", "")))

if not vulns:
    print("OK: OSV 未发现任何已知漏洞")
else:
    print(f"WARN: 发现 {len(vulns)} 条：\n")
    for n, ver, vid, al, summ, sev in vulns:
        print(f"  - {n} {ver}: {vid} ({al}) [{sev}]\n    {summ}")
