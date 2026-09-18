"""errors/codes.sml -> site/static/errors.json

为什么用 `smltools --to json` 而不是在 Python 里再写一个 SML 解析器：
  1. 生成走**真实工具链**，官网展示的码表与各实现引用的是同一份事实；
  2. 顺带自检 —— 这份登记表本身必须是合法 SML，写坏了这里就报错；
  3. 少一份「Python 眼里的 SML」语义副本（这类副本迟早与实现漂移）。

用法：
    python errors/gen_json.py            # 生成 + 校验，失败退出码非 0
    python errors/gen_json.py --check    # 只校验不写文件（给 CI 用）

输出**不含时间戳**：同样的输入必须产出逐字节相同的文件，否则每次构建都脏一个文件。
"""

import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REGISTRY = os.path.join(ROOT, "errors", "codes.sml")
OUT = os.path.join(ROOT, "site", "static", "errors.json")

ID_RE = re.compile(r"^(E|W|I)-([A-Z]+)-(\d{3})$")
SEVERITY_OF = {"E": "error", "W": "warning", "I": "info"}


def find_smltools():
    """按「显式指定 > 环境变量 > 常见产物路径」的顺序找二进制。"""
    cands = []
    env = os.environ.get("SMLTOOLS_BIN")
    if env:
        cands.append(env)
    for d in (os.environ.get("CARGO_TARGET_DIR"), "E:/snoware-target", "D:/snoware-target"):
        if d:
            cands.append(os.path.join(d, "release", "smltools.exe"))
            cands.append(os.path.join(d, "debug", "smltools.exe"))
    cands += [
        os.path.join(ROOT, "rust", "target", "release", "smltools.exe"),
        os.path.join(ROOT, "rust", "target", "debug", "smltools.exe"),
        "smltools",
    ]
    for c in cands:
        if os.path.sep not in c or os.path.exists(c):
            try:
                subprocess.run([c, "--help"], capture_output=True, timeout=30)
                return c
            except (OSError, subprocess.SubprocessError):
                continue
    return None


def load_registry(binary):
    p = subprocess.run(
        [binary, "-i", REGISTRY, "--to", "json"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=120,
    )
    if p.returncode != 0:
        print("!! 登记表解析失败（errors/codes.sml 不是合法 SML，或工具链报错）：")
        print((p.stderr or "").strip()[:2000])
        sys.exit(1)
    return json.loads(p.stdout)


def check(reg):
    """校验：码的形状、唯一性、领域已声明、级别与码前缀一致。"""
    errs = []
    domains = reg.get("domains") or {}
    codes = reg.get("codes") or []
    if not codes:
        errs.append("codes 为空")
    seen = {}
    for idx, c in enumerate(codes):
        cid = c.get("id")
        where = cid or ("codes[%d]" % idx)
        m = ID_RE.match(cid or "")
        if not m:
            errs.append("%s：id 形状非法（应为 E-LEX-001 这种）" % where)
            continue
        sev, dom, _num = m.groups()
        if dom not in domains:
            errs.append("%s：领域 %s 未在 domains 里声明" % (where, dom))
        if c.get("domain") != dom:
            errs.append("%s：domain 字段(%s)与 id 里的领域(%s)不一致" % (where, c.get("domain"), dom))
        if c.get("severity") != sev:
            errs.append("%s：severity(%s)与 id 前缀(%s)不一致" % (where, c.get("severity"), sev))
        if cid in seen:
            errs.append("%s：id 重复" % where)
        seen[cid] = True
        for need in ("title", "msg", "impls", "status"):
            if not c.get(need):
                errs.append("%s：缺字段 %s" % (where, need))
    if "coverage" not in reg:
        errs.append("缺 coverage 字段（用来标明录了多少、还剩多少，别让人误以为是全量）")
    return errs


def main():
    check_only = "--check" in sys.argv
    binary = find_smltools()
    if not binary:
        print("!! 找不到 smltools：设 SMLTOOLS_BIN 或先 `cargo build --release -p smltools`")
        return 1
    reg = load_registry(binary)
    errs = check(reg)
    if errs:
        print("!! 登记表校验失败：")
        for e in errs:
            print("   -", e)
        return 1

    codes = reg["codes"]
    # 按 id 排序，保证产物稳定（与书写顺序无关）
    codes.sort(key=lambda c: c["id"])
    out = {
        "version": reg.get("version"),
        "coverage": reg.get("coverage"),
        "domains": reg.get("domains"),
        "count": len(codes),
        "codes": [
            {
                "id": c["id"],
                "severity": SEVERITY_OF.get(c["severity"], "error"),
                "domain": c["domain"],
                "title": c["title"],
                "msg": c["msg"],
                "impls": sorted(c["impls"]),
                "status": c["status"],
                "note": c.get("note", ""),
            }
            for c in codes
        ],
    }
    if check_only:
        print("校验通过：%d 条码（--check 模式，未写文件）" % len(codes))
        return 0
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, ensure_ascii=False, indent=2)
        f.write("\n")
    print("已生成 %s（%d 条码）" % (os.path.relpath(OUT, ROOT), len(codes)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
