# -*- coding: utf-8 -*-
"""逐码比对 `errors/codes.sml` 的 `impls` 与五端源码产出点。

由 `_scratch/audit_impls_all.py` 固化而来（2026-11-17，P15）：
保留报告模式（默认），新增 `--check`（存在**未解释差异**时非零退出，可进 CI）。

判据（与一次性探针同族）：
  · C / C++ 用具名宏 `SML_E_CONTRACT_017`（含 `E_CONTRACT_017`）；
  · Rust 用常量 `E_CONTRACT_017`；JS / Lua 用字面量 `"E-CONTRACT-017"`。
  · 只扫解析器 / 契约 / 词法 / include 的**源码**，排除生成物与测试目录。
  · 命中只说明「该端有产出点」，不等于「行为与 Rust 一致」—— 逐格一致由 conformance 管。

`ALLOWED_DRIFT` 里是**已解释**的差异：码表登记的 impls 不在五端源码扫描范围内
（CLI / derive 宏 / 绑定层 / 已废弃码的兼容保留）。新增条目必须写清理由；
不要因为「现在红了」就往里加一条 —— 先按源码证据修 `errors/codes.sml`。
"""
import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

ENDS = {
    "rust": ["rust/sml-parse/src", "rust/sml-contract/src", "rust/sml-feature/src",
             "rust/sml-include/src", "rust/sml-lex/src", "rust/src"],
    "c": ["c/src", "c/sml.c"],
    "cpp": ["cpp/src", "cpp/sml.cpp"],
    "js": ["js/sml.mjs"],
    "lua": ["lua/lib/sml.lua"],
}
ORDER = ("rust", "c", "cpp", "js", "lua")

# (code, direction, ends) → reason。direction = "extra"（登记了但源码没命中）
# 或 "miss"（源码命中但没登记）。
ALLOWED_DRIFT = {
    ("E-LIMIT-008", "extra", "rust,js"): "码在 CLI / 解析预算层产出，不在五端库源码扫描范围内",
    ("W-FEATURE-001", "extra", "rust,js"): "非致命诊断，CLI 侧经诊断通道产出",
    ("E-CONTRACT-014", "extra", "js"): "已废弃：JS 改报 E-CONTRACT-013，码保留供旧脚本识别",
    ("E-CONTRACT-015", "extra", "js"): "已废弃：JS 改报 E-CONTRACT-013，码保留供旧脚本识别",
    ("E-CONTRACT-017", "miss", "rust"): "已废弃：Rust 不再产出，码保留供旧脚本识别",
    ("E-EXT-001", "extra", "rust,js"): "未注册扩展的码尚未在 Rust / JS 库层统一落地（见 errors/codes.sml note）",
    ("E-IO-001", "extra", "lua"): "Lua 文件 I/O 由宿主给入，库内不直接产出",
    ("E-IO-002", "extra", "c"): "宿主侧文件错误，库内不直接产出",
    ("E-INCLUDE-011", "miss", "lua"): "Lua 有码字面量但 include 子文件错误仍落在码表主码，待逐端核对",
    ("E-DERIVE-001", "extra", "rust"): "derive 宏生成物，不在手写源码里",
    ("E-DERIVE-003", "extra", "rust"): "derive 宏生成物，不在手写源码里",
    ("E-DERIVE-004", "extra", "rust"): "derive 宏生成物，不在手写源码里",
    ("E-DERIVE-005", "extra", "rust"): "derive 宏生成物，不在手写源码里",
    ("E-DERIVE-006", "extra", "rust"): "derive 宏生成物，不在手写源码里",
    ("E-DERIVE-007", "extra", "rust"): "derive 宏生成物，不在手写源码里",
    ("E-DERIVE-008", "extra", "rust"): "derive 宏生成物，不在手写源码里",
    ("E-LINT-002", "extra", "smltools"): "CLI 专属码，impls 写的是工具名而非五端名",
    ("E-LINT-003", "extra", "smltools"): "CLI 专属码，impls 写的是工具名而非五端名",
    ("E-LINT-004", "extra", "smltools"): "CLI 专属码，impls 写的是工具名而非五端名",
    ("E-LINT-005", "extra", "smltools"): "CLI 专属码，impls 写的是工具名而非五端名",
    ("W-LINT-001", "extra", "smltools"): "CLI 专属码，impls 写的是工具名而非五端名",
    ("W-LINT-002", "extra", "smltools"): "CLI 专属码，impls 写的是工具名而非五端名",
    ("W-LINT-003", "extra", "smltools"): "CLI 专属码，impls 写的是工具名而非五端名",
    ("W-LINT-004", "extra", "smltools"): "CLI 专属码，impls 写的是工具名而非五端名",
    ("W-LINT-005", "extra", "smltools"): "CLI 专属码，impls 写的是工具名而非五端名",
}


def read_paths(paths):
    buf = []
    for p in paths:
        full = os.path.join(ROOT, p)
        if os.path.isdir(full):
            for dp, dn, fn in os.walk(full):
                dn[:] = [d for d in dn if d not in ("target", "build", "node_modules")]
                for f in fn:
                    if f.endswith((".rs", ".c", ".h", ".cpp", ".hpp", ".inc", ".lua", ".mjs")):
                        try:
                            buf.append(open(os.path.join(dp, f), encoding="utf-8", errors="replace").read())
                        except OSError:
                            pass
        elif os.path.isfile(full):
            try:
                buf.append(open(full, encoding="utf-8", errors="replace").read())
            except OSError:
                pass
    return "\n".join(buf)


def parse_codes():
    """从 errors/codes.sml 抽 (id, impls, status)。"""
    text = open(os.path.join(ROOT, "errors", "codes.sml"), encoding="utf-8", errors="replace").read()
    out = []
    for m in re.finditer(
            r"\{\s*id:\s*([EWI]-[A-Z]+-\d+)\b(.*?)(?:status:\s*(\w+))",
            text, re.S):
        cid, body, status = m.group(1), m.group(2), m.group(3)
        im = re.search(r"impls:\s*\[([^\]]*)\]", body)
        impls = [x for x in re.split(r"[,\s]+", im.group(1)) if x] if im else []
        out.append((cid, impls, status or "?"))
    return out


def main():
    check_mode = "--check" in sys.argv[1:]
    texts = {e: read_paths(p) for e, p in ENDS.items()}
    codes = parse_codes()
    unexplained = []
    if not check_mode:
        print("码表条目：%d\n" % len(codes))
        print("%-16s %-28s %-9s %s" % ("code", "impls(码表)", "status", "源码命中( rust c cpp js lua )"))
        print("-" * 100)
    for cid, impls, status in codes:
        macro = cid.replace("-", "_")
        hit = {e: (cid in texts[e] or macro in texts[e]) for e in ORDER}
        miss = [e for e in ORDER if e not in impls and hit[e]]
        extra = [e for e in ORDER if e in impls and not hit[e]]
        if check_mode:
            for direction, ends in (("miss", miss), ("extra", extra)):
                if not ends:
                    continue
                key = (cid, direction, ",".join(ends))
                if key not in ALLOWED_DRIFT:
                    unexplained.append("%s %s=%s（源码命中 %s）" %
                                       (cid, direction, ",".join(ends),
                                        " ".join("%s:%s" % (e, "Y" if hit[e] else ".") for e in ORDER)))
            continue
        row = " ".join(("Y" if hit[e] else ".").ljust(4) for e in ORDER)
        print("%-16s %-28s %-9s %s" % (cid, ",".join(impls) or "-", status, row))
        if miss or extra:
            print("    ↳ 已实现未登记：%s" % (", ".join(miss) or "无"))
            print("    ↳ 已登记未见命中：%s" % (", ".join(extra) or "无"))
    if check_mode:
        if unexplained:
            print("IMPLS DRIFT（未解释，须按源码证据修 errors/codes.sml 或补理由）：")
            for line in unexplained:
                print("  " + line)
            print("%d 条" % len(unexplained))
            return 1
        print("IMPLS CHECK OK（%d 条码，差异均已解释）" % len(codes))
        return 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
