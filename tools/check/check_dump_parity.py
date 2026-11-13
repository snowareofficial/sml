# -*- coding: utf-8 -*-
"""W4：C 的 `sml_dump` 与 Rust 的 `to_sml` 输出比对（**分类**，不再要求逐字节相同）。

为什么要有这个脚本：C 与 Rust 各有一套序列化器，两边都能「解析成功」，但**输出可以不一样** ——
这种差异不会让任何一端的测试变红（各自看着都对），只会等到跨端交换数据时才炸。

2026-11-06（T05 裁决「留路」）：两类差异**登记为允许**，不再要求逐字节一致 ——
  ① **键序**：C 保源序，Rust 默认 `BTreeMap` 排序（canonical 才强制排序）；
  ② **引号策略**：两端 `needs_quote` 的判据不同（如 `ops@example.com` / `+86-…`）。
其余差异仍判红。判据写在 `docs/swsml/spec/value-layout.md` 的「渲染差异（允许）」一节；
将来要统一，出口只有 `to_sml_canonical`，不做第二套引号规则。

用法：
    python tools/check_dump_parity.py                 # 全仓 *.sml（排除 `_*` 临时探针）
    python tools/check_dump_parity.py --file a.sml b.sml
    python tools/check_dump_parity.py --show 3        # 每个未分类差异打印前 N 处（默认 1）
    python tools/check_dump_parity.py --strict        # 旧口径：必须逐字节一致（含两类差异也红）

退出码：0 = 无**未分类**差异（允许差异按类计数后忽略）
        1 = 存在未分类差异（两端都解析成功、但差异不属①/②）
"""
import argparse
import collections
import os
import re
import subprocess
import sys

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SMLTOOLS = os.environ.get("SMLTOOLS", os.path.join(ROOT, "rust", "target", "release", "smltools"))


def find_smltools():
    if os.path.isfile(SMLTOOLS):
        return SMLTOOLS
    # 原先这里还有两个候选是「本机盘符路径」被脱敏成占位符后留下的字面量
    # （`<target-dir>\release\...` 之类）。它们是死候选：永远匹配不到真实文件，
    # 只会让读代码的人以为「这里支持别的盘」（2026-10-28 删除）。
    for cand in [os.path.join(ROOT, "rust", "target", "debug", "smltools.exe"),
                 os.path.join(ROOT, "rust", "target", "release", "smltools.exe")]:
        if os.path.isfile(cand):
            return cand
    return None


def norm_tail(t):
    """去掉输出**末尾**的空行再比。

    为什么要这一步：Rust 侧走的是 `smltools --to sml`，它在正文后还会多打一个换行
    （实测 `a: []` → Rust `"a: []\\n\\n"`、C `"a: []\\n"`）。那是 **CLI 打印**的差异，
    不是 `to_sml` 与 `sml_dump` 的差异；不归一化的话每一个文件都会判成不一致。
    """
    return t.rstrip("\n") + ("\n" if t.strip() else "")


def strip_quotes(line):
    """去掉行内所有双引号 —— 用来判断「差异是否只来自引号策略」。

    ⚠️ 这会把 `s: "1"` 与 `s: 1` 也视作同一行。**类型/结构差异由
    `tests/conformance` 的 JSON 层守着**（那里比的是解析后的值树），本脚本只管渲染。
    """
    return line.replace('"', "")


_TOK = re.compile(r"[{}]|\[|\]|,|:|[^\s{}[\],:]+")


def sig(line):
    """行签名：去引号 → 切成原子 → 排序。

    原子含 `{ } [ ] , :`，故签名相同 = 这一行（行内多字段）只是**字段顺序**或**引号**不同。
    排序会把「键序」与「引号」两个维度同时抹平 —— 这正是 T05 允许的两类。
    ⚠️ 别的差异（值、类型、元素个数）会改变原子集合，签名不同 ⇒ 仍判真问题。
    """
    return tuple(sorted(_TOK.findall(strip_quotes(line))))


def classify(clines, rlines):
    """把一对输出归到 same / order / quote / layout / other。

    - `order`：同一批行、顺序不同（C 保源序 vs Rust 排序）；
    - `quote`：逐行同签名（含行内字段顺序与引号），且**行数相同**；
    - `layout`：签名相同但行数不同 ⇒ 行内/展开的排版差异，**不属 T05 的两类**，仍报出来；
    - `other`：原子集合都不同 ⇒ 真差异。
    """
    if clines == rlines:
        return "same"
    if collections.Counter(clines) == collections.Counter(rlines):
        return "order"
    if collections.Counter(map(sig, clines)) == collections.Counter(map(sig, rlines)):
        return "quote" if len(clines) == len(rlines) else "layout"
    # 行被拆得不一样（行内 vs 展开）时，逐行签名必然不同 ⇒ 退到**全文原子多重集**：
    # 仍相同就说明只是排版与顺序，不是值变了。
    if collections.Counter(_TOK.findall(strip_quotes("\n".join(clines)))) == \
            collections.Counter(_TOK.findall(strip_quotes("\n".join(rlines)))):
        return "layout"
    return "other"


def corpus():
    out = []
    for b, d, fs in os.walk(ROOT):
        d[:] = [x for x in d if x not in (".git", "node_modules", "target", ".codebuddy")]
        for f in sorted(fs):
            if f.endswith(".sml") and not f.startswith("_"):
                out.append(os.path.join(b, f))
    return sorted(out)


def build_dump_c():
    src = os.path.join(ROOT, "tools", "gen", "dump_c.c")
    exe = os.path.join(os.environ.get("TEMP", "."), "dump_c_parity.exe")
    # `-lm`：`sml.c` 的 `add_float` 用了 `floor()`，Linux 上 libm 不自动链接。
    p = subprocess.run(["gcc", "-std=c99", "-I" + os.path.join(ROOT, "c"), "-o", exe,
                        src, os.path.join(ROOT, "c", "sml.c"), "-lm"],
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    if p.returncode != 0:
        print("[!] 编译 C dumper 失败:\n" + (p.stderr or "")[:800])
        return None
    return exe


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", nargs="*", help="指定文件；不给则扫全仓 *.sml")
    ap.add_argument("--show", type=int, default=1, help="每个未分类差异文件打印几处")
    ap.add_argument("--strict", action="store_true",
                    help="旧口径：逐字节一致才算过（键序/引号两类也判红）")
    args = ap.parse_args()

    exe = build_dump_c()
    sml = find_smltools()
    if not exe or not sml:
        print("[!] 缺少 C dumper 或 smltools，无法比对")
        return 1

    files = args.file if args.file else corpus()
    print("C 侧 dumper : %s" % exe)
    print("Rust 侧工具 : %s" % sml)
    print("语料 %d 个\n" % len(files))

    same, diff, cerr, rerr, tail_only = [], [], [], [], []
    order_only, quote_only, layout_diff = [], [], []
    # 行数一致计数：② 的「行内 vs 展开」只影响**行数**，而 ③ 引号 / ④ 键顺序
    # 几乎每个文件都命中（Rust 用 BTreeMap 排序、C 保源序），会把「完全一致」
    # 压成 0，看不出 ② 有没有收敛。故单列一项：归一化尾部后行数是否相同。
    lsame = 0
    for f in files:
        rel = os.path.relpath(f, ROOT)
        p = subprocess.run([exe, f], capture_output=True, timeout=180)
        cout = p.stdout.decode("utf-8", "replace")
        if cout.startswith("CERR "):
            cerr.append((rel, cout[5:].strip()[:100]))
            continue
        cbody = cout.split("\n", 1)[1] if cout.startswith("CDUMP ") else cout
        q = subprocess.run([sml, "-i", f, "--to", "sml"], capture_output=True, timeout=180)
        if q.returncode != 0:
            msg = ((q.stderr or b"") + (q.stdout or b"")).decode("utf-8", "replace").strip()
            rerr.append((rel, msg.splitlines()[0][:100] if msg else "rc=%d" % q.returncode))
            continue
        rbody = (q.stdout or b"").decode("utf-8", "replace")
        if cbody == rbody:
            same.append((rel, cbody, rbody))
        elif norm_tail(cbody) == norm_tail(rbody):
            tail_only.append(rel)
        else:
            cl = norm_tail(cbody).splitlines()
            rl = norm_tail(rbody).splitlines()
            if len(cl) == len(rl):
                lsame += 1
            kind = "other" if args.strict else classify(cl, rl)
            if kind == "order":
                order_only.append(rel)
            elif kind == "quote":
                quote_only.append(rel)
            elif kind == "layout":
                layout_diff.append((rel, len(cl), len(rl)))
            else:
                diff.append((rel, cbody, rbody))

    print("== 汇总 ==")
    print("  完全一致 : %d" % len(same))
    print("  允许差异·仅键序（C 源序 / Rust 排序）: %d" % len(order_only))
    print("  允许差异·引号策略（可叠加键序）: %d" % len(quote_only))
    print("  **行内/展开布局差异（不属 T05 两类）**: %d" % len(layout_diff))
    print("  **未分类差异（真问题）**: %d" % len(diff))
    print("  仅尾部换行差异（已按 CLI 打印差异忽略）: %d" % len(tail_only))
    print("  其中行数已一致（② 行内/展开已对齐）: %d" % lsame)
    print("  C 解析失败: %d（这些**不算**序列化差异，是解析层面的已知缺口）" % len(cerr))
    print("  Rust 失败 : %d" % len(rerr))
    if args.strict:
        print("  ⚠️ --strict：键序/引号两类也计入未分类差异")

    for label, bucket in (("仅键序", order_only), ("引号策略", quote_only)):
        if bucket and args.show:
            print("\n== 允许差异·%s（%d 个文件，最多列 20）==" % (label, len(bucket)))
            for rel in bucket[:20]:
                print("   " + rel)
            if len(bucket) > 20:
                print("   …… 其余 %d 个未列出" % (len(bucket) - 20))

    if diff:
        print("\n== 未分类差异清单 ==")
        for rel, c, r in diff:
            # 用归一化后的文本取行：Rust CLI 的尾部多一个换行会让 Rust 恒多 1 行
            cl, rl = norm_tail(c).splitlines(), norm_tail(r).splitlines()
            strip_eq = [x.rstrip() for x in cl] == [x.rstrip() for x in rl]
            print("   %-46s 行数 C=%-5d Rust=%-5d 仅行尾空白差异:%s"
                  % (rel, len(cl), len(rl), "是" if strip_eq else "否"))
            n, i = 0, 0
            while n < args.show and i < max(len(cl), len(rl)):
                a = cl[i] if i < len(cl) else "<无此行>"
                b = rl[i] if i < len(rl) else "<无此行>"
                if a != b:
                    print("        第 %d 行  C: %r" % (i + 1, a[:78]))
                    print("                 Rust: %r" % b[:78])
                    n += 1
                i += 1
    if layout_diff:
        print("\n== 行内/展开布局差异（行数不同、签名相同；不属 T05 两类，需单独定性）==")
        for rel, nc, nr in layout_diff:
            print("   %-46s 行数 C=%-5d Rust=%-5d" % (rel, nc, nr))
    if cerr:
        print("\n== C 解析失败（解析层缺口，非本脚本判定对象）==")
        for rel, m in cerr:
            print("   %-46s %s" % (rel, m))
    if rerr:
        print("\n== Rust 失败 ==")
        for rel, m in rerr:
            print("   %-46s %s" % (rel, m))
    return 1 if (diff or layout_diff) else 0


if __name__ == "__main__":
    sys.exit(main())
