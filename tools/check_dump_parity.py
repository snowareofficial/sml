# -*- coding: utf-8 -*-
"""W4：C 的 `sml_dump` 与 Rust 的 `to_sml` **逐字节**比对。

为什么要有这个脚本：C 与 Rust 各有一套序列化器，两边都能「解析成功」，但**输出可以不一样** ——
这种差异不会让任何一端的测试变红（各自看着都对），只会等到跨端交换数据时才炸。

用法：
    python tools/check_dump_parity.py                 # 全仓 *.sml（排除 `_*` 临时探针）
    python tools/check_dump_parity.py --file a.sml b.sml
    python tools/check_dump_parity.py --show 3        # 每个不一致文件打印前 N 处差异（默认 1）

退出码：0 = 全部一致（或有差异但都来自「任一端解析失败」之外的可接受项 —— 见输出判定）
        1 = 存在**真正的**输出差异（两端都解析成功、但输出不同）
"""
import argparse
import os
import subprocess
import sys

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SMLTOOLS = os.environ.get("SMLTOOLS", os.path.join(ROOT, "rust", "target", "release", "smltools"))


def find_smltools():
    if os.path.isfile(SMLTOOLS):
        return SMLTOOLS
    for cand in [os.path.join(ROOT, "rust", "target", "debug", "smltools.exe"),
                 r"E:\snoware-target\release\smltools.exe",
                 r"E:\snoware-target\debug\smltools.exe",
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


def corpus():
    out = []
    for b, d, fs in os.walk(ROOT):
        d[:] = [x for x in d if x not in (".git", "node_modules", "target", ".codebuddy")]
        for f in sorted(fs):
            if f.endswith(".sml") and not f.startswith("_"):
                out.append(os.path.join(b, f))
    return sorted(out)


def build_dump_c():
    src = os.path.join(ROOT, "tools", "dump_c.c")
    exe = os.path.join(os.environ.get("TEMP", "."), "dump_c_parity.exe")
    p = subprocess.run(["gcc", "-std=c99", "-I" + os.path.join(ROOT, "c"), "-o", exe,
                        src, os.path.join(ROOT, "c", "sml.c")],
                       capture_output=True, text=True, encoding="utf-8", errors="replace")
    if p.returncode != 0:
        print("[!] 编译 C dumper 失败:\n" + (p.stderr or "")[:800])
        return None
    return exe


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--file", nargs="*", help="指定文件；不给则扫全仓 *.sml")
    ap.add_argument("--show", type=int, default=1, help="每个不一致文件打印几处差异")
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
            if len(norm_tail(cbody).splitlines()) == len(norm_tail(rbody).splitlines()):
                lsame += 1
            diff.append((rel, cbody, rbody))

    print("== 汇总 ==")
    print("  完全一致 : %d" % len(same))
    print("  **不一致**: %d" % len(diff))
    print("  仅尾部换行差异（已按 CLI 打印差异忽略）: %d" % len(tail_only))
    print("  其中行数已一致（② 行内/展开已对齐，剩余差异来自引号/键序）: %d" % lsame)
    print("  C 解析失败: %d（这些**不算**序列化差异，是解析层面的已知缺口）" % len(cerr))
    print("  Rust 失败 : %d" % len(rerr))

    if diff:
        print("\n== 不一致清单 ==")
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
    if cerr:
        print("\n== C 解析失败（解析层缺口，非本脚本判定对象）==")
        for rel, m in cerr:
            print("   %-46s %s" % (rel, m))
    if rerr:
        print("\n== Rust 失败 ==")
        for rel, m in rerr:
            print("   %-46s %s" % (rel, m))
    return 1 if diff else 0


if __name__ == "__main__":
    sys.exit(main())
