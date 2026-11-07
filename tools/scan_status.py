#!/usr/bin/env python3
"""可重跑的文档状态扫描器（TODO §9 第三条）。

产出即清单：`docs/repo/ref/scan.md` 由本脚本生成，**不要手改**。
`docs/repo/ref/unfinished.md` 的 §0 只保留结论与指向本文件的链接。

用法：
    python tools/scan_status.py            # 打印到 stdout
    python tools/scan_status.py --write    # 写入 docs/repo/ref/scan.md

为什么需要它（TODO §9 的症状 ⑤）：上一份「自动扫描」的文档自称自动生成，
仓库里却没有生成脚本，数字漂移无人发现。这个脚本就是那个缺失的脚本。
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs" / "repo" / "ref" / "scan.md"


def scan_todo() -> dict:
    """TODO.md: 勾选统计 + 按章节分组。"""
    text = (ROOT / "TODO.md").read_text(encoding="utf-8", errors="replace")
    open_n = done_n = 0
    sections: dict[str, int] = {}
    cur = "(无章节)"
    for line in text.splitlines():
        if line.startswith("## "):
            cur = line[3:].strip()
        elif re.match(r"^\s*- \[ \]", line):
            open_n += 1
            sections[cur] = sections.get(cur, 0) + 1
        elif re.match(r"^\s*- \[x\]", line, re.IGNORECASE):
            done_n += 1
    return {"open": open_n, "done": done_n, "sections": sections}


def scan_ideas() -> dict:
    """IDEAS.md: Status 行统计。"""
    text = (ROOT / "IDEAS.md").read_text(encoding="utf-8", errors="replace")
    counts: dict[str, int] = {}
    for m in re.finditer(r"Status / 状态\*\*：`(\w+)`", text):
        counts[m.group(1)] = counts.get(m.group(1), 0) + 1
    return counts


def scan_codes() -> dict:
    """errors/codes.sml: impls 覆盖度。"""
    text = (ROOT / "errors" / "codes.sml").read_text(encoding="utf-8", errors="replace")
    total = only_rust = 0
    missing: dict[str, int] = {}
    for m in re.finditer(r"impls:\s*\[([^\]]*)\]", text):
        impls = m.group(1).split()
        total += 1
        if impls == ["rust"]:
            only_rust += 1
        for end in ("cpp", "js", "c", "lua"):
            if end not in impls:
                missing[end] = missing.get(end, 0) + 1
    return {"total": total, "only_rust": only_rust, "missing": missing}


def scan_divergences() -> dict:
    """known_divergences.md: 台账里的 AGREE / DIVERGE 计数（最近一次实测）。"""
    p = ROOT / "tests" / "conformance" / "known_divergences.md"
    if not p.is_file():
        return {"agree": None, "diverge": None}
    text = p.read_text(encoding="utf-8", errors="replace")
    # 台账约定：**第一节**写当前实测，后面是历史段落。故取第一个匹配。
    m = re.search(r"AGREE (\d+) / DIVERGE (\d+)", text)
    if not m:
        return {"agree": None, "diverge": None}
    return {"agree": int(m.group(1)), "diverge": int(m.group(2))}


def scan_ci() -> dict:
    """ci.yml: 是否跑 fuzz / bench。"""
    p = ROOT / ".github" / "workflows" / "ci.yml"
    if not p.is_file():
        return {"fuzz": False, "bench": False}
    text = p.read_text(encoding="utf-8", errors="replace")
    return {"fuzz": "fuzz" in text, "bench": "bench" in text}


def scan_marks() -> int:
    """docs/**/*.md 里未完成标记（⬜）的数量。"""
    n = 0
    docs = ROOT / "docs"
    if docs.is_dir():
        for p in docs.rglob("*.md"):
            try:
                n += p.read_text(encoding="utf-8", errors="ignore").count("⬜")
            except OSError:
                pass
    return n


def render(d: dict) -> str:
    stamp = dt.datetime.now().strftime("%Y-%m-%d %H:%M")
    lines = [
        "# 文档状态扫描（生成物，勿手改）",
        "",
        f"> 生成：`python tools/scan_status.py --write` · {stamp}",
        "> 手工编辑会在下次扫描时被覆盖；结论写进 `unfinished.md`。",
        "",
        "## 汇总",
        "",
        "| 维度 | 数量 |",
        "|---|---|",
        f"| `TODO.md` 未完成 | **{d['todo']['open']}** |",
        f"| `TODO.md` 已完成 | {d['todo']['done']} |",
    ]

    ideas = d["ideas"]
    for k in ("Done", "Partial", "Todo", "Blocked", "TBD", "Rejected"):
        if k in ideas:
            lines.append(f"| `IDEAS.md` {k} | {ideas[k]} |")

    codes = d["codes"]
    lines += [
        f"| 错误码总数 | {codes['total']} |",
        f"| 其中只有 Rust | **{codes['only_rust']}** |",
    ]
    for end, n in codes["missing"].items():
        if n:
            lines.append(f"| 缺 `{end}` | {n} |")

    div = d["divergences"]
    if div["agree"] is not None:
        lines.append(f"| conformance | AGREE {div['agree']} / DIVERGE {div['diverge']} |")

    ci = d["ci"]
    lines.append(f"| CI 跑 fuzz / bench | {ci['fuzz']} / {ci['bench']} |")
    lines.append(f"| `docs/**` 未完成标记 ⬜ | {d['marks']} |")

    sections = d["todo"]["sections"]
    if sections:
        lines += ["", "## TODO 未完成 · 按章节", "", "| 章节 | 未完成 |", "|---|---|"]
        for name, n in sorted(sections.items(), key=lambda kv: -kv[1]):
            lines.append(f"| {name} | {n} |")

    return "\n".join(lines) + "\n"


def main() -> int:
    # Windows 控制台默认 GBK，输出 `⬜` 会 UnicodeEncodeError；强制 UTF-8。
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")

    ap = argparse.ArgumentParser(description="可重跑的文档状态扫描器")
    ap.add_argument("--write", action="store_true", help="写入 docs/repo/ref/scan.md")
    ap.add_argument("--json", action="store_true", help="只输出 JSON")
    args = ap.parse_args()

    data = {
        "todo": scan_todo(),
        "ideas": scan_ideas(),
        "codes": scan_codes(),
        "divergences": scan_divergences(),
        "ci": scan_ci(),
        "marks": scan_marks(),
    }

    if args.json:
        print(json.dumps(data, ensure_ascii=False, indent=2))
        return 0

    text = render(data)
    if args.write:
        OUT.parent.mkdir(parents=True, exist_ok=True)
        OUT.write_text(text, encoding="utf-8", newline="\n")
        print(f"written {OUT.relative_to(ROOT)}")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
