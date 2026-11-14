#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""按语法特性切片探针：逐端定位「第一处拒绝」与静默错树 ｜ Per-feature probe per end.

`tests/conformance/run_conformance.py` 判的是「一组共享用例的五端值树是否一致」；
本脚本判的是**单个特性**：把某个语法特性切成最小片段，逐端跑一遍，记录

  - 该端是否解析成功；
  - 失败时的**第一个错误码 + 文案**（码是稳定契约，文案不是）；
  - 成功但与 Rust 值树不同 ⇒ `DIFF`（这才是「静默差异」：不报错，结果却不一样）。

用法：

  python tools/check/probe_grammar.py grammar.sml
  python tools/check/probe_grammar.py --md            # 只跑切片，输出可粘进文档的表格
  python tools/check/probe_grammar.py grammar.sml --end js --end lua
  python tools/check/probe_grammar.py --list          # 列出切片清单

------------------------------------------------------------------------------
为什么不自己写四个探针程序
------------------------------------------------------------------------------
探针程序已经存在，且是**套件在用的那一份**（改一处两边同时生效）：

  C    tools/gen/dump_c.c                          (sml_parse_file + sml_dump)
  C++  tests/conformance/adapters/dump_cpp.cpp      (sml::Parser::parse + to_sml)
  JS   tests/conformance/adapters/dump_js.mjs       (parse + stringify)
  Lua  tests/conformance/adapters/dump_lua.lua      (Sml.load + Sml.dump)
  Rust smltools -i <f> --to sml                     (不需要外壳)

判据也复用套件那一份（`canon` / `sig_key` / `diff_values`）：JSON 值树 + token 层
两层比较，数字按「整数 / 浮点 + 数值」归一。另写一份只会让两处判据漂移。

退出码：0 = 切片全部与 Rust 一致；1 = 有拒绝或静默错树（明细逐条打印，不静默）。
SKIP 不算失败（缺工具链时如实说出缺什么），但 `--require-all` 下算。
"""
import argparse
import importlib.util
import os
import shutil
import sys
import tempfile

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

ROOT = os.environ.get("SML_REPO") or os.path.dirname(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CONF_DRIVER = os.path.join(ROOT, "tests", "conformance", "run_conformance.py")
ENDPOINTS = ["rust", "c", "cpp", "js", "lua"]

# ---------------------------------------------------------------------------
# 切片：每个片段只钉**一个**语法特性，尽量短。
# 出处一栏指向 grammar.sml 的小节，便于回写时对照。
# ---------------------------------------------------------------------------
SLICES = [
    {
        "id": "contract-nest",
        "what": "契约当字段类型（契约嵌套）",
        "where": "grammar.sml 0 / 6（Address、Node）",
        "code": """\
@contract Address {
    city: str
    country: str default CN
}
@contract Node {
    host: str
    address: Address
}
node {
    @is Node
    host: db1.internal
    address { city: Beijing }
}
""",
    },
    {
        "id": "contract-multi",
        "what": "多契约叠加 + 就地严格度",
        "where": "grammar.sml 6b（pair）",
        "code": """\
@contract Left { l: int }
@contract Right { r: int }
pair {
    @is Left loose Right loose
    l: 1
    r: 2
}
""",
    },
    {
        "id": "contract-requires",
        "what": "契约级 @requires",
        "where": "grammar.sml 6（CONTRACT-ITEM）",
        "code": """\
@contract Gated {
    @requires when
    flag: bool default true
}
gated {
    @is Gated
}
""",
    },
    {
        "id": "contract-requires-on",
        "what": "契约级 @requires 且要求已满足",
        "where": "grammar.sml 6（@requires when，已开 when）",
        "code": """\
@version v1
@feature enable when
@contract Gated {
    @requires when
    flag: bool default true
}
gated {
    @is Gated
}
""",
    },
    {
        "id": "type-dir",
        "what": "@type 模式类型（中文类型名）",
        "where": "grammar.sml 6b（时刻、Window）",
        "code": """\
@type name: 时刻 {
    序列: [
        { 名: 时, 类: 数字, 次: 2 }
        { 字面: ":" }
        { 名: 分, 类: 数字, 次: 2 }
    ]
}
@contract Window {
    start: 时刻
    end: 时刻 default "23:59"
}
window {
    @is Window
    start: "09:30"
}
""",
    },
    {
        "id": "featag",
        "what": "featag 声明 + 块头使用（三个特性齐开）",
        "where": "grammar.sml 8（Service !critical !obey[4], prod）",
        "code": """\
@version v1
@feature enable type.featag, typed-block, block-name
@contract Service {
    name: str
    port: int default 8080
    !critical
    !obey: [ int ]
}
Service !critical !obey[4], prod {
    name: gateway
}
""",
    },
    {
        "id": "featag-gate",
        "what": "只开 type.featag（带点特性名）",
        "where": "grammar.sml 2 / 8",
        "code": """\
@version v1
@feature enable type.featag
k: v
""",
    },
    {
        "id": "type-dir-alone",
        "what": "@type 只声明不使用（是否静默忽略）",
        "where": "grammar.sml 6b（@type 块本身）",
        "code": """\
@type name: 时刻 {
    序列: [
        { 名: 时, 类: 数字, 次: 2 }
        { 字面: ":" }
    ]
}
k: v
""",
    },
    {
        "id": "featag-decl",
        "what": "featag 只在契约体内声明（是否静默接受）",
        "where": "grammar.sml 8（Service 的 !critical / !obey）",
        "code": """\
@version v1
@feature enable type.featag, typed-block, block-name
@contract Service {
    name: str
    !critical
    !obey: [ int ]
}
svc {
    @is Service
    name: gateway
}
""",
    },
    {
        "id": "feature-unknown",
        "what": "未注册特性名是否显式拒绝",
        "where": "grammar.sml 4（@feature 未知名 ⇒ E-FEATURE-003）",
        "code": """\
@version v1
@feature enable no.such
k: v
""",
    },
    {
        "id": "type-dir-check",
        "what": "@type 的模式约束是否真的校验",
        "where": "grammar.sml 6b（时刻：两位数字 冒号 两位数字）",
        "code": """\
@type name: 时刻 {
    序列: [
        { 名: 时, 类: 数字, 次: 2 }
        { 字面: ":" }
        { 名: 分, 类: 数字, 次: 2 }
    ]
}
@contract Window {
    start: 时刻
}
window {
    @is Window
    start: "9:30"
}
""",
    },
    {
        "id": "typed-block-default",
        "what": "块级类型标注是否真的应用契约（默认值回填）",
        "where": "grammar.sml 3（bare-block）+ 契约 default",
        "code": """\
@version v1
@feature enable typed-block
@contract Service {
    name: str
    port: int default 8080
}
Service {
    name: gateway
}
""",
    },
    {
        "id": "nested-block-meta",
        "what": "嵌套带名裸块里的 __name 是否参与严格校验",
        "where": "Rust 实测：报 E-CONTRACT-004 `addr.__name`",
        "code": """\
@contract Sub { city: str }
@contract A { addr: Sub }
x {
    @is A
    addr Sub { city: B }
}
""",
    },
    {
        "id": "typed-block",
        "what": "块级类型标注（无块名）",
        "where": "grammar.sml 3（bare-block）",
        "code": """\
@version v1
@feature enable typed-block
@contract Service {
    name: str
}
Service {
    name: gateway
}
""",
    },
    {
        "id": "block-name",
        "what": "块级类型标注 + 块名",
        "where": "grammar.sml 8（BLOCK-HEAD）",
        "code": """\
@version v1
@feature enable typed-block, block-name
@contract Service {
    name: str
}
Service prod {
    name: gateway
}
""",
    },
    {
        "id": "when",
        "what": "@when 解析期裁剪",
        "where": "grammar.sml 13（live_parsing）",
        "code": """\
@version v1
@feature enable when
@when $env.SML_PROBE_ENV == prod
prodOnly: true
keep: 1
""",
    },
    {
        "id": "for",
        "what": "@for 有界展开 + ${} 循环变量",
        "where": "grammar.sml 13（hosts）",
        "code": """\
@version v1
@feature enable for
hosts: @for h in web api {
    name: "${h}"
    port: 8080
}
""",
    },
    {
        "id": "weak-star",
        "what": "weakbind / weakdata / weakdoc",
        "where": "grammar.sml 13（live_parsing 尾部）",
        "code": """\
@version v1
@feature enable type.weak
@weakdata fmt {
    theme: snow
}
@weakdoc api-docs
@weakbind Service
soft { name: x }
""",
    },
    {
        "id": "type-weak-gate",
        "what": "只开 type.weak（带点特性名 + 普通块）",
        "where": "grammar.sml 2",
        "code": """\
@version v1
@feature enable type.weak
soft { name: x }
""",
    },
]


def load_conformance():
    """把套件驱动当模块加载：判据、适配器、编译参数都只有那一份。"""
    spec = importlib.util.spec_from_file_location("sml_conformance", CONF_DRIVER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def cell_of(conf, res):
    """把一个端的结果压成表格里的一格。"""
    if res["kind"] == "skip":
        return "skip"
    if res["kind"] == "crash":
        return "!"
    if res["kind"] == "err":
        return res["code"] or "ERR"
    if res["kind"] == "nc":
        return "NC"
    return "OK"


def probe(conf, eps, rust_exe, path, tmp, tag):
    """跑一个文件的所有端；返回 {端名: 结果}。"""
    out = {}
    for e in eps:
        if not e.ok:
            out[e.name] = {"kind": "skip", "why": e.why}
            continue
        kind, payload = e.run_case(path)
        if kind in ("err", "crash"):
            out[e.name] = {
                "kind": kind,
                "code": conf.first_code(payload) if kind == "err" else "",
                "msg": conf.first_line(payload, 150),
            }
            continue
        canon, cerr = conf.canon(rust_exe, payload, tmp, tag + "_" + e.name)
        if canon is None:
            out[e.name] = {"kind": "nc", "msg": conf.first_line(cerr or "", 150)}
            continue
        out[e.name] = {"kind": "ok", "sig": conf.sig_key(canon), "canon": canon,
                       "dump": payload}
    return out


def verdict_of(cells, names):
    """与 Rust 对照：全一致才算 agree。Rust 自己失败时，同码的端仍算 agree。"""
    ref = cells.get("rust")
    if ref is None or ref["kind"] == "skip":
        return "NO-REF"
    bad = []
    for n in names:
        c = cells.get(n)
        if c is None or c["kind"] == "skip":
            continue
        if ref["kind"] == "ok":
            if c["kind"] != "ok" or c["sig"] != ref["sig"]:
                bad.append(n)
        else:
            # Rust 也拒绝：只有「同码拒绝」才算一致
            if c["kind"] != "err" or c["code"] != ref["code"]:
                bad.append(n)
    return "AGREE" if not bad else "DIVERGE:" + ",".join(bad)


def main():
    ap = argparse.ArgumentParser(description="按语法特性切片，逐端探针（第一处拒绝 / 静默错树）")
    ap.add_argument("file", nargs="?", default=None,
                    help="额外探一份完整文档（如 grammar.sml）；切片总会跑")
    ap.add_argument("--end", action="append", default=None, choices=ENDPOINTS,
                    help="只跑指定端（可重复）")
    ap.add_argument("--only", action="append", default=None,
                    help="只跑 id 含该子串的切片（可重复）")
    ap.add_argument("--list", action="store_true", help="列出切片清单后退出")
    ap.add_argument("--md", action="store_true", help="输出 markdown 表格（粘文档用）")
    ap.add_argument("--show-ok", action="store_true", help="明细里也打印成功的格")
    ap.add_argument("--no-build", action="store_true", help="不自动构建 smltools")
    ap.add_argument("--require-all", action="store_true", help="有 SKIP 即判失败")
    args = ap.parse_args()

    if args.list:
        for s in SLICES:
            print("%-16s %-34s %s" % (s["id"], s["what"], s["where"]))
        return 0

    if not os.path.isfile(CONF_DRIVER):
        print("[!] 找不到套件驱动：%s" % CONF_DRIVER)
        return 1
    conf = load_conformance()

    cases = []
    if args.file:
        cases.append({"id": "whole:" + os.path.basename(args.file),
                      "what": "整份文档", "where": args.file,
                      "path": os.path.abspath(args.file)})
    for s in SLICES:
        if args.only and not any(k.lower() in s["id"].lower() for k in args.only):
            continue
        cases.append(s)
    if not cases:
        print("[!] 没有要跑的切片。")
        return 1

    tmp = tempfile.mkdtemp(prefix="sml_probe_grammar_")
    try:
        eps = conf.build_endpoints(tmp, not args.no_build)
        wanted = args.end or ENDPOINTS
        eps = [e for e in eps if e.name in wanted]
        names = [e.name for e in eps]
        rust_ep = next((e for e in eps if e.name == "rust" and e.ok), None)

        print("== 工具链 ==")
        for e in eps:
            print("  %-4s %-4s %s" % (e.name, "OK" if e.ok else "SKIP",
                                      " ".join(e.cmd) if e.ok else e.why))
        if rust_ep is None:
            print("\n[!] Rust 侧不可用 ⇒ 没有参照，也无法归一化，判不了。")
            return 1
        if args.require_all and any(not e.ok for e in eps):
            print("\n[!] --require-all：有端 SKIP，判失败。")
            return 1

        # 切片落成临时文件（统一 UTF-8 + LF，避免平台换行混进词法）
        results = []
        for idx, s in enumerate(cases, 1):
            path = s.get("path")
            if path is None:
                path = os.path.join(tmp, "%02d_%s.sml" % (idx, s["id"]))
                with open(path, "w", encoding="utf-8", newline="\n") as f:
                    f.write(s["code"])
            cells = probe(conf, eps, rust_ep.exe, path, tmp, "%02d%s" % (idx, s["id"]))
            results.append((s, cells, verdict_of(cells, names)))

        if args.md:
            print("\n| 特性 | 说明 | " + " | ".join(names) + " |")
            print("|---|---|" + "---|" * len(names))
            for s, cells, _ in results:
                row = [s["id"], s["what"]] + [cell_of(conf, cells.get(n) or
                                                      {"kind": "skip"}) for n in names]
                print("| " + " | ".join(row) + " |")
            return 0

        widths = {n: 16 for n in names}
        idw = max([len(s["id"]) for s, _, _ in results] + [8]) + 2
        head = conf.pad("切片", idw) + "".join(conf.pad(n, widths[n]) for n in names)
        print("\n" + head + "判定")
        print("-" * conf.disp_width(head + "判定"))
        for s, cells, verdict in results:
            line = conf.pad(s["id"], idw)
            for n in names:
                line += conf.pad(cell_of(conf, cells.get(n) or {"kind": "skip"}), widths[n])
            print(line + verdict)

        print("\n图例：OK = 与 Rust 值树一致；DIFF = 解析成功但值树不同（静默差异）；"
              "其余为错误码；NC = dump 不能被 Rust 回读；! = 适配器异常；skip = 缺工具链")

        print("\n== 明细 ==")
        for s, cells, verdict in results:
            print("\n  [%s] %s  (%s)" % (s["id"], s["what"], s["where"]))
            for n in names:
                c = cells.get(n) or {"kind": "skip"}
                if c["kind"] == "ok" and not args.show_ok:
                    continue
                if c["kind"] == "skip":
                    print("    %-4s skip  %s" % (n, c["why"]))
                elif c["kind"] == "err":
                    print("    %-4s %-15s %s" % (n, c["code"] or "ERR", c["msg"]))
                elif c["kind"] == "crash":
                    print("    %-4s !     %s" % (n, c["msg"]))
                elif c["kind"] == "nc":
                    print("    %-4s NC    %s" % (n, c["msg"]))
                else:
                    print("    %-4s OK    解析成功，值树与 Rust 一致" % n)

        # 静默错树单独列：这是本次清查的重点
        silent = []
        for s, cells, _ in results:
            ref = cells.get("rust") or {}
            if ref.get("kind") != "ok":
                continue
            for n in names:
                c = cells.get(n) or {"kind": "skip"}
                if c["kind"] == "ok" and c["sig"] != ref["sig"]:
                    silent.append((s, n, c, ref))
        if silent:
            print("\n== 静默错树（解析成功但值树与 Rust 不同）==")
            for s, n, c, ref in silent:
                diffs = conf.diff_values(ref["canon"]["json"], c["canon"]["json"], limit=4)
                print("\n  [%s] %s vs rust：JSON 值树差异 %d 处%s"
                      % (s["id"], n, len(diffs),
                         "" if diffs else "（JSON 层一致 ⇒ 差在 int/float 身份上）"))
                for p, x, y in diffs:
                    print("    %-30s %-28s | %s" % (p, x, y))

        bad = [s["id"] for s, _, v in results if v.startswith("DIVERGE")]
        print()
        if bad:
            print("结论：%d/%d 切片与 Rust 不一致（%s）⇒ 退出码 1。"
                  "差异请登记进 docs/swsml/ref/grammar-divergences.md。"
                  % (len(bad), len(results), ", ".join(bad)))
            return 1
        print("结论：%d 个切片五端一致 ⇒ 退出码 0。" % len(results))
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
