"""用 OSV.dev API 检查 Cargo.lock 中所有依赖的已知漏洞。

退出码语义（CI 门禁用）：
  rc=0   干净（或显式豁免）
  rc=1   发现已知漏洞，或存在「查询失败」的包（查不到 ≠ 没漏洞）

默认是 fail-closed：任一「查询失败」的包都会让门禁变红，因为那只是「没查到」，
不代表「没有漏洞」。CI 网络偶发抖动时可显式加 --allow-network-error 豁免，
但漏洞（vulns）默认**不可**豁免 —— 那才是门禁要挡的东西。
"""
import argparse
import json
import os
import re
import sys
import urllib.request

# 路径基于本文件位置推导（不写死开发机绝对路径：审计曾发现硬编码 C:\Users\<用户名>）
ROOT = os.path.dirname(os.path.abspath(__file__))
lock = open(os.path.join(ROOT, "Cargo.lock"), encoding="utf-8").read()


def parse_packages(lock_text):
    """从 Cargo.lock 文本里解析出 (name, version, source)。"""
    pkgs = []
    for block in lock_text.split("[[package]]")[1:]:
        name = re.search(r'^name = "(.*?)"', block, re.M)
        ver = re.search(r'^version = "(.*?)"', block, re.M)
        src = re.search(r'^source = "(.*?)"', block, re.M)
        if name and ver:
            pkgs.append((name.group(1), ver.group(1), src.group(1) if src else "local"))
    return pkgs


def query_osv(name, ver):
    """查某个包的已知漏洞，返回 (vulns, failure)。failure 非空表示查询失败。"""
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
    except Exception as e:  # noqa: BLE001 —— 网络/HTTP 任何异常都是「查不到」，不算没漏洞
        return [], str(e)
    vulns = []
    for v in res.get("vulns", []):
        aliases = ",".join(v.get("aliases", [])[:3])
        sev = "?"
        for s in v.get("severity", []):
            sev = s.get("score", "?")
        db = v.get("database_specific", {})
        vulns.append((name, ver, v.get("id"), aliases, v.get("summary", "")[:110],
                      db.get("severity", "")))
    return vulns, ""


def decide_exit(vulns, query_failures, allow_vuln, allow_network_error):
    """纯逻辑：给定结果返回 (rc, 原因)。便于本地单元测试，不必真联网。"""
    if vulns and not allow_vuln:
        return 1, "发现 %d 条已知漏洞" % len(vulns)
    if query_failures and not allow_network_error:
        return 1, "%d 个包查询失败（查不到 ≠ 没漏洞）" % len(query_failures)
    return 0, "OK"


def main():
    ap = argparse.ArgumentParser(description="OSV 依赖漏洞门禁")
    ap.add_argument("--allow-vuln", action="store_true",
                    help="发现漏洞时只警告、不退出失败（本地排查用；CI 不带）")
    ap.add_argument("--allow-network-error", action="store_true",
                    help="个别包 OSV 查询失败时不退出失败（CI 网络抖动豁免；默认严格）")
    args = ap.parse_args()

    pkgs = parse_packages(lock)
    print("共 %d 个包，查询 OSV ...\n" % len(pkgs))
    vulns = []
    query_failures = []
    for name, ver, src in pkgs:
        if src == "local":
            continue
        found, failure = query_osv(name, ver)
        if failure:
            query_failures.append((name, ver, failure))
            print("  [!] %s %s 查询失败: %s" % (name, ver, failure))
            continue
        vulns.extend(found)

    if not vulns:
        print("OK: OSV 未发现任何已知漏洞")
    else:
        print("WARN: 发现 %d 条：\n" % len(vulns))
        for n, ver, vid, al, summ, sev in vulns:
            print("  - %s %s: %s (%s) [%s]\n    %s" % (n, ver, vid, al, sev, summ))

    if query_failures:
        print("WARN: %d 个包查询失败（查不到 ≠ 没漏洞）：" % len(query_failures))
        for n, ver, _f in query_failures:
            print("  - %s %s" % (n, ver))

    code, reason = decide_exit(vulns, query_failures,
                               args.allow_vuln, args.allow_network_error)
    if code != 0:
        print("FAIL: %s（可加 --allow-vuln / --allow-network-error 豁免）" % reason)
    sys.exit(code)


if __name__ == "__main__":
    sys.exit(main())
