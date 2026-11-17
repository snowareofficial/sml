#!/usr/bin/env python3
"""全仓 Cargo 依赖漏洞审计：把每个独立 lock 都过一遍 `cargo audit`。

为什么不是一条 `cargo audit`：
  本仓有多个互不隶属的 workspace（`rust/`、`rust/qsm`、`rust/fuzz`、示例 crate…），
  每个各有自己的 `Cargo.lock`。在主 workspace 跑一次只覆盖它自己，别的工程里的
  依赖树完全不在扫描范围内 —— 真正的漏点就在那些地方。

用法：
    python tools/check/check_cargo_audit.py                 # 汇总，命中漏洞即非零退出
    python tools/check/check_cargo_audit.py --include-unlocked
                                                            # 给「没有 lock」的工程在临时目录
                                                            # 生成锁再审计（覆盖只在库外维护的工程）
    python tools/check/check_cargo_audit.py --deny-warnings  # unmaintained/unsound 也算失败
    python tools/check/check_cargo_audit.py --no-fetch       # 只用本地 advisory-db（离线）
    python tools/check/check_cargo_audit.py --strict         # 没有 lock 的工程也算失败

退出码：
    0   所有 lock 干净（或仅有允许的 warning）
    1   命中漏洞，或 `cargo audit` 本身跑失败（查不到 ≠ 没漏洞，故 fail-closed）

设计取舍：
  * **不留下痕迹**。已入库的 lock 只读；无 lock 的工程就地生成锁，审完即删（finally）。
  * 无 lock 的工程默认只列出、不算失败 —— 锁文件可能故意不入库（示例 / 内部工具）。
  * advisory-db 只拉一次：首个 lock 正常联网刷新，其后的调用一律 `--no-fetch`。
"""
import argparse
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SKIP_DIRS = {"target", "node_modules", ".git", ".hg", ".svn", "__pycache__", "corpus", "artifacts"}
WORKSPACE_TABLE = re.compile(r"^\[workspace\]\s*$", re.M)


def has_workspace_table(manifest):
    """清单是否自带 [workspace] 表 —— 是则它自己就是 workspace 根，不归属上游。"""
    try:
        with open(manifest, encoding="utf-8", errors="replace") as f:
            return bool(WORKSPACE_TABLE.search(f.read()))
    except OSError:
        return False


def find_targets(root):
    """返回 (locks, manifests_without_lock)，都按路径排序。"""
    locks, manifests = [], []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIRS and not d.startswith(".cache"))
        if "Cargo.toml" in filenames:
            manifests.append(os.path.join(dirpath, "Cargo.toml"))
        if "Cargo.lock" in filenames:
            locks.append(os.path.join(dirpath, "Cargo.lock"))

    locked = {os.path.dirname(p) for p in locks}
    # workspace 成员的 Cargo.toml 不该被算成「独立工程」：成员没有自己的 lock，
    # 归属看祖先目录里的 lock。但「自带 [workspace] 表的清单」自己就是 workspace 根
    # （如 rust/fuzz），哪怕上游也有 lock 也不归属它 —— 否则它的依赖树会被静默跳过。
    orphans = []
    for m in manifests:
        d = os.path.dirname(m)
        if d in locked:
            continue
        p = d
        owned = False
        while True:
            if has_workspace_table(os.path.join(p, "Cargo.toml")):
                break
            parent = os.path.dirname(p)
            if parent == p:
                break
            p = parent
            if p in locked:
                owned = True
                break
        if not owned:
            orphans.append(m)
    return sorted(locks), sorted(orphans)


def audit(lock, no_fetch):
    cmd = ["cargo", "audit", "--file", lock, "--json"]
    if no_fetch:
        cmd.append("--no-fetch")
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, errors="replace", cwd=os.path.dirname(lock))
    except FileNotFoundError:
        return None, "cargo-audit 未安装（cargo install cargo-audit）"
    if p.returncode not in (0, 1):
        tail = (p.stderr or p.stdout).strip().splitlines()
        return None, tail[-1] if tail else ("cargo audit 退出码 %d" % p.returncode)
    try:
        return json.loads(p.stdout), ""
    except json.JSONDecodeError:
        return None, "cargo audit 输出无法解析为 JSON"


def audit_unlocked(crate_dir, no_fetch):
    """就地 generate-lockfile → 审计 → 无论成败都把锁删掉。

    不复制到临时目录：这类工程常带 `path = ".."` 之类的出目录依赖，单独复制会让 cargo
    解析失败。生成出来的锁本来就被 .gitignore 忽略，且只在本次运行期间存在；
    已存在的锁一律不碰（这只处理「没有锁」的工程）。
    """
    lock = os.path.join(crate_dir, "Cargo.lock")
    if os.path.exists(lock):
        return audit(lock, no_fetch)
    try:
        gen = subprocess.run(["cargo", "generate-lockfile"], cwd=crate_dir,
                             capture_output=True, text=True, errors="replace")
        if gen.returncode != 0:
            tail = (gen.stderr or gen.stdout).strip().splitlines()
            return None, "generate-lockfile 失败：%s" % (tail[-1] if tail else gen.returncode)
        return audit(lock, no_fetch)
    finally:
        if os.path.exists(lock):
            try:
                os.remove(lock)
            except OSError:
                pass


def main():
    # Windows 控制台常是 GBK：cargo 输出里若有无法编码的字符（如 U+FFFD），
    # print 会抛 UnicodeEncodeError 把整次审计打断。宁可显示成 ? 也不能崩。
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(errors="replace")
        except (AttributeError, ValueError):
            pass

    ap = argparse.ArgumentParser()
    ap.add_argument("--deny-warnings", action="store_true", help="把 unmaintained/unsound/notice 也判为失败")
    ap.add_argument("--no-fetch", action="store_true", help="完全不联网，只用本地 advisory-db")
    ap.add_argument("--strict", action="store_true", help="没有 lock 的 Rust 工程也判为失败")
    ap.add_argument("--include-unlocked", action="store_true",
                    help="给没有 lock 的工程就地生成锁后审计（审完即删）")
    args = ap.parse_args()

    locks, orphans = find_targets(ROOT)
    if not locks and not (args.include_unlocked and orphans):
        print("没有找到任何 Cargo.lock")
        return 1

    bad, failed, warned = [], [], []
    first = True
    total = len(locks) + (len(orphans) if args.include_unlocked else 0)
    print("Cargo 依赖审计（%d 个 lock%s）\n" % (total, "，含临时生成" if args.include_unlocked else ""))

    def show(rel, data, err):
        if data is None:
            failed.append((rel, err))
            print("  FAIL  %-46s %s" % (rel, err))
            return
        n = data.get("lockfile", {}).get("dependency-count", 0)
        v = data.get("vulnerabilities", {}).get("count", 0)
        warns = data.get("warnings", {}) or {}
        nwarn = sum(len(items) for items in warns.values() if isinstance(items, list))
        print("  %s %-46s %3d 依赖  漏洞 %d  警告 %d" % ("OK   " if v == 0 else "VULN ", rel, n, v, nwarn))
        for item in data.get("vulnerabilities", {}).get("list", []):
            adv, pkg = item.get("advisory", {}), item.get("package", {})
            print("          %s %s  %s  %s" % (pkg.get("name"), pkg.get("version"), adv.get("id"), adv.get("title")))
            if adv.get("patched_versions"):
                print("          修复：>= %s" % ", ".join(adv["patched_versions"]))
            bad.append((rel, adv.get("id"), pkg.get("name")))
        for kind, items in warns.items():
            if not isinstance(items, list):
                continue
            for item in items:
                adv, pkg = item.get("advisory", {}), item.get("package", {})
                print("          [%s] %s %s  %s  %s" % (kind, pkg.get("name"), pkg.get("version"),
                                                       adv.get("id"), adv.get("title")))
                warned.append((rel, kind, adv.get("id"), pkg.get("name")))

    for lock in locks:
        rel = os.path.relpath(lock, ROOT).replace("\\", "/")
        data, err = audit(lock, args.no_fetch or not first)
        first = False
        show(rel, data, err)

    if args.include_unlocked:
        for m in orphans:
            rel = os.path.relpath(os.path.dirname(m), ROOT).replace("\\", "/")
            data, err = audit_unlocked(os.path.dirname(m), args.no_fetch or not first)
            first = False
            show(rel + "  [临时生成]", data, err)
    elif orphans:
        print("\n无 lock、未审计的 Rust 工程（%d，加 --include-unlocked 可审计）：" % len(orphans))
        for m in orphans:
            print("  %s" % os.path.relpath(m, ROOT).replace("\\", "/"))

    print()
    if failed:
        print("查询失败 %d 个 lock —— 查不到不等于没漏洞：%s" % (len(failed), ", ".join(r for r, _ in failed)))
    if bad:
        print("发现漏洞 %d 条" % len(bad))
    if warned:
        print("警告 %d 条（默认不判失败）" % len(warned))
    if not (failed or bad or (args.deny_warnings and warned) or (args.strict and orphans)):
        print("全部干净")
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
