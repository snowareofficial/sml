#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""套件检不出的差异：错误码 / 特性门控 / 值身份 / 警告 ｜ Divergences the suite cannot see.

`tests/conformance/run_conformance.py` 用「各端 dump → Rust 归一化 → 比值树」判定，
这套判据在结构上有四块盲区，本脚本各给一组最小探针：

  errors   套件只有十来条 `expect-error` 用例。其余错误条件下的**码是否同**没人管 ——
           同一条件报不同的码，值树用例是看不见的（它们只跑成功路径）。
  gates    `@feature disable X` 之后再用 X：Rust 拒，某端若**不门控**就静默放行。
           放行后值树往往与「没写 @feature」时一模一样，套件照样 AGREE。
  values   值身份要穿过「文本载体」才能比。`"42"` 与 `42`、`1.0` 与 `1`、2^53 以上的整数，
           一旦某端 dump 时丢掉类型，Rust 回读后与自己的值树一致 ⇒ AGREE 是假的。
  warnings 警告既不影响值树也不影响退出码，套件**完全不判**。本组直接调各端的诊断 API
           （C `sml_parse_ex` / C++ `Diag` / JS warningsOut / Lua parse_with_warnings，
           Rust 用 `smltools --lint`），只比码、不比文案。

用法：

  python tools/check/probe_matrix.py                 # 四组全跑
  python tools/check/probe_matrix.py --suite errors
  python tools/check/probe_matrix.py --suite warnings
  python tools/check/probe_matrix.py --suite gates --end js --end lua
  python tools/check/probe_matrix.py --csv out.csv   # 逐行落盘，便于 diff

判据：
  errors  期望码写在行里（出处是 errors/codes.sml）。
  gates   以 **Rust 的实际行为**为准：Rust 拒绝而某端解析成功 ⇒ 门控缺失。
  values  以 **Rust 的 dump 字面量**为准：字面量不同 ⇒ 类型或精度在载体上丢了。
  warnings 以 **Rust 的警告码集合**为准：少一条或多一条都算差异。

退出码：0 = 无差异；1 = 有差异（明细逐条打印，不静默）。
"""
import argparse
import importlib.util
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

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
# errors：最小坏文档 → 期望码（出处 errors/codes.sml）
# ---------------------------------------------------------------------------
ERRORS = [
    ("lex-unclosed-string", "E-LEX-001", 's: "abc\n'),
    ("lex-unclosed-comment", "E-LEX-002", "/* x\n"),
    ("lex-unclosed-comment-u", "E-LEX-003", "_* x\n"),
    ("lex-bad-escape", "E-LEX-004", 's: "a\\q"\n'),
    ("lex-bad-unicode", "E-LEX-005", 's: "\\uZZZZ"\n'),
    ("lex-eof-after-backslash", "E-LEX-006", 's: "abc\\'),
    ("parse-unclosed-block", "E-PARSE-001", "a { b: 1\n"),
    ("parse-mismatch", "E-PARSE-002", "a { b: 1 ]\n"),
    ("parse-extra-close", "E-PARSE-003", "a: 1 }\n"),
    # 值位置的孤立 `@`：Rust 走兜底 012（004 只覆盖「行首孤立的 @」，见下一行钉住）。
    ("parse-lone-at-value", "E-PARSE-012", "a: @ b\n"),
    ("parse-unknown-directive", "E-PARSE-005", "@nope\n"),
    ("parse-double-colon", "E-PARSE-006", "a: B::C\n"),
    ("parse-comma-bare", "E-PARSE-007", "a: x,y\n"),
    ("parse-top-scalar", "E-PARSE-008", "42\n"),
    ("parse-top-scalar-block", "E-PARSE-008", "42\na { b: 1 }\n"),
    ("parse-stray-colon", "E-PARSE-012", "a: :\n"),
    ("par-default-missing", "E-PARSE-021", "@contract C { a: str default }\nx { @is C }\n"),
    ("par-min-not-number", "E-PARSE-022", "@contract C { a: int min abc }\nx { @is C a: 1 }\n"),
    ("par-min-on-str", "E-PARSE-022", "@contract C { a: str min abc }\nx { @is C a: 1 }\n"),
    ("par-enum-not-array", "E-PARSE-023", "@contract C { a: enum x }\nx { @is C a: x }\n"),
    # 类型为空：Rust 报 006（「期望类型」）。024 专指「数组/枚举缺少闭合」那一族。
    ("par-field-spec", "E-PARSE-006", "@contract C { a: }\nx { @is C a: 1 }\n"),
    ("parse-lone-at-line", "E-PARSE-004", "a: 1\n@\nk: 2\n"),
    # S5：featag 的作用域是契约私有的 —— 块头写契约里没声明的名字必须拒（021）。
    # 改前四端只收集不校验，拼错的 `!nope` 会被静默记进 `__featags`。
    ("featag-undeclared", "E-CONTRACT-021",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S { name: str !critical }\nS !nope, prod { name: a }\n"),
    # S5b：个数不符（无参声明带参数 / 带参声明不写 `[…]`）都是 021。
    ("featag-args-on-tag", "E-CONTRACT-021",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S {\n    name: str\n    !critical\n}\nS !critical[1], prod { name: a }\n"),
    ("featag-args-missing", "E-CONTRACT-021",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S {\n    name: str\n    !obey: [ int ]\n}\nS !obey, prod { name: a }\n"),
    # `!any` / `!strict-paths` 是**契约级开关**，不占 featag 名：块头拿它当 featag 用 ⇒ 021。
    ("featag-switch-not-tag", "E-CONTRACT-021",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract C {\n    name: str\n    !any\n}\nC !any, prod { name: a }\n"),
    # 开关不接受类型 / 参数 / 落点 ⇒ 021（不是「未知类型」那类 006）。
    ("featag-switch-has-type", "E-CONTRACT-021",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract C {\n    name: str\n    !any: str\n}\nC, prod { name: a }\n"),
    # S6 落点填充：五端同一套规则（禁止覆盖 / strict / 越界 / auto 无默认）。
    ("featag-target-overwrite", "E-CONTRACT-022",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S loose {\n    name: str\n    !tls: str -> tls.cert\n}\n"
     'S !tls["abc"], prod { name: a tls { cert: x } }\n'),
    ("featag-target-strict", "E-CONTRACT-023",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S loose {\n    name: str\n    !strict-paths\n    !tls: str -> tls.cert\n}\n"
     'S !tls["abc"], prod { name: a }\n'),
    ("featag-target-overrange", "E-CONTRACT-025",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S loose {\n    name: str\n    !tag: str -> list[5]\n}\n"
     'S !tag["x"], prod { name: a list: [ 1 2 ] }\n'),
    ("featag-target-auto-none", "E-CONTRACT-022",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S loose {\n    name: str\n    !r: int -> spec.r\n}\n"
     "S !r[auto], prod { name: a }\n"),
    # S8：`any` 类型默认禁用 —— 契约里写 `!any` 才放行；报码复用 E-FEATURE-001。
    ("featag-any-bare", "E-FEATURE-001",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S {\n    x: any\n}\nS prod { x: 1 }\n"),
    # 递归：嵌套契约里的 any 也要查（错误指向内层契约名）。
    ("featag-any-nested", "E-FEATURE-001",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract Sub {\n    y: any\n}\n@contract S {\n    s: Sub\n}\nS prod { s { y: 1 } }\n"),
    # S7 `from-featag`：该键只能由 featag 落点写入，文档直接赋值 ⇒ 024。
    # 判据在**填充之前**跑，填进去的键与手写的键长得一样，来源就分不出来了。
    ("featag-from-direct", "E-CONTRACT-024",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S {\n    name: str\n    tls: str from-featag\n}\n"
     "S prod { name: a tls: x }\n"),
    # 递归：嵌套契约里的 from-featag 字段也要查（路径为 parent.child）。
    ("featag-from-nested", "E-CONTRACT-024",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract Sub {\n    cert: str from-featag\n}\n"
     "@contract S {\n    name: str\n    sub: Sub\n}\n"
     "S prod { name: a sub { cert: x } }\n"),
    # S5b：类型不符走 002 —— Rust 那里复用 check_value，产出的是字段类型码。
    ("featag-arg-type", "E-CONTRACT-002",
     "@version v1\n@feature enable type.featag, typed-block, block-name\n"
     "@contract S {\n    name: str\n    !cert: str\n}\nS !cert[1], prod { name: a }\n"),
    ("contract-undefined", "E-CONTRACT-001", "x { @is Nope }\n"),
    ("contract-type", "E-CONTRACT-002", "@contract C { a: int }\nx { @is C a: abc }\n"),
    ("contract-required", "E-CONTRACT-003", "@contract C { a: int }\nx { @is C }\n"),
    ("contract-strict-extra", "E-CONTRACT-004", "@contract C { a: int }\nx { @is C a: 1 b: 2 }\n"),
    ("contract-range", "E-CONTRACT-005", "@contract C { a: int min 1 max 3 }\nx { @is C a: 5 }\n"),
    ("contract-enum", "E-CONTRACT-006", "@contract C { a: enum [ x y ] }\nx { @is C a: z }\n"),
    ("contract-array-enum", "E-CONTRACT-012", "@contract C { a: [enum x y] }\nx { @is C a: [ x ] }\n"),
    # `@is` 后跟第二个名字：Rust 把它当**另一个契约**并报「未定义」；四端先撞上
    # 「多契约未实现」⇒ 017。同属拒绝，但码不同（语义差：Rust 走到「名字是否存在」）。
    ("is-two-contracts", "E-CONTRACT-001",
     "@contract C { a: int }\nx { @is C bogus }\n"),
    ("feature-version", "E-FEATURE-004", "@version v9\na: 1\n"),
    ("feature-unknown-sub", "E-FEATURE-006", "@feature bogus\n"),
    ("feature-missing-arg", "E-FEATURE-007", "@feature\n"),
    ("feature-mode-arg", "E-FEATURE-008", "@feature mode xyz\n"),
    ("when-missing-value", "E-PARSE-015", "@feature enable when\n@when $env.X ==\na: 1\n"),
    ("when-twice", "E-PARSE-016",
     "@feature enable when\n@when $env.X == y\n@when $env.X == z\na: 1\n"),
    ("when-eof", "E-PARSE-017", "@feature enable when\n@when $env.X == y\n"),
    ("for-bad", "E-PARSE-018", "@feature enable for\nh: @for x web {}\n"),
    ("include-missing", "E-INCLUDE-001", 'include "nope.sml"\n'),
    ("include-unquoted", "E-INCLUDE-012", "include x.sml\n"),
    ("include-unquoted-bom", "E-INCLUDE-012", "include x.sml\n", {"bom": True}),
    ("fragment-undefined", "E-INCLUDE-006", "a: &nope\n"),
    ("include-cycle", "E-INCLUDE-002", 'include "b.sml"\n',
     {"b.sml": 'include "main.sml"\n'}),
]

# ---------------------------------------------------------------------------
# gates：`@feature disable X` 之后再用 X。期望 = Rust 的行为
# ---------------------------------------------------------------------------
GATES = [
    ("gate-contract", "contract",
     "@feature disable contract\n@contract C { a: int }\nx { @is C a: 1 }\n"),
    ("gate-fragment", "fragment", "@feature disable fragment\n@f { a: 1 }\n"),
    ("gate-env", "env", "@feature disable env\na: $env.HOME\n"),
    ("gate-env-quoted", "env", '@feature disable env\na: "$env.HOME"\n'),
    ("gate-bareword", "bareword-string", "@feature disable bareword-string\na: hello\n"),
    ("gate-top-array", "top-level-array", "@feature disable top-level-array\n[ 1 2 ]\n"),
    ("gate-when", "when", "@feature disable when\n@when $env.X == y\na: 1\n"),
    ("gate-for", "for", "@feature disable for\nh: @for x in web {}\n"),
    ("gate-typed-block", "typed-block",
     "@feature disable typed-block\n@contract C { a: int }\nC { a: 1 }\n"),
    ("gate-block-name", "block-name",
     "@feature disable block-name\n@contract C { a: int }\nC nm { a: 1 }\n"),
    ("gate-weak", "type.weak", "@feature disable type.weak\n@weakdata f { a: 1 }\n"),
]

# ---------------------------------------------------------------------------
# values：值身份。期望 = Rust 的 dump 字面量
# ---------------------------------------------------------------------------
VALUES = [
    ("str-numeric", 'a: "42"\n'),
    ("str-bool", 'a: "true"\n'),
    ("str-null", 'a: "null"\n'),
    ("str-float", 'a: "1.5"\n'),
    ("str-leading-zero", 'a: "007"\n'),
    ("str-empty", 'a: ""\n'),
    ("str-space", 'a: "a b"\n'),
    ("str-newline", 'a: "a\\nb"\n'),
    ("str-nul", 'a: "a\\0b"\n'),
    ("str-unicode", 'a: "\\u00e9"\n'),
    ("int-big", "a: 9007199254740993\n"),
    ("int-i64max", "a: 9223372036854775807\n"),
    ("int-over-i64", "a: 9223372036854775808\n"),
    ("num-one-dot-zero", "a: 1.0\n"),
    ("num-sci", "a: 1e3\n"),
    ("num-negzero", "a: -0.0\n"),
    ("int-leading-zero", "a: 007\n"),
    ("int-hex", "a: 0x20\n"),
    ("bool-true", "a: true\n"),
    ("null-value", "a: null\n"),
]


# ---------------------------------------------------------------------------
# warnings：警告码集合。套件只比值树，诊断层从来没进过判据。
# ---------------------------------------------------------------------------
WARNINGS = [
    ("dup-key", "k: 1\nk: 2\n"),
    ("dup-key-block", "a { x: 1 x: 2 }\n"),
    ("dup-key-triple", "k: 1\nk: 2\nk: 3\n"),
    ("fragment-redefine", "@f { a: 1 }\n@f { b: 2 }\nk: 1\n"),
    ("weakbind-unknown", "@feature enable type.weak\n@weakbind Nope\nk: 1\n"),
    ("clean-doc", "a: 1\nb: 2\n"),
]

# 各端「只打印警告码」的小适配器。写在临时目录、不进仓库；协议：WARN <码> / ERR <信息> / OK。
WARN_C = r"""
#include <stdio.h>
#include <stdlib.h>
#include "sml.h"
int main(int argc, char **argv) {
    if (argc < 2) return 2;
    FILE *f = fopen(argv[1], "rb");
    if (!f) { printf("ERR E-IO-001\n"); return 0; }
    fseek(f, 0, SEEK_END); long sz = ftell(f); fseek(f, 0, SEEK_SET);
    char *text = (char *)malloc((size_t)sz + 1);
    if (!text) { fclose(f); return 2; }
    size_t got = fread(text, 1, (size_t)sz, f);
    text[got] = '\0';
    fclose(f);
    char err[512] = {0};
    sml_diag *diags = NULL; size_t n = 0;
    sml_value *v = sml_parse_ex(text, err, sizeof(err), &diags, &n);
    if (!v) printf("ERR %s\n", err);
    else {
        for (size_t i = 0; i < n; i++) printf("WARN %s\n", diags[i].code);
        printf("OK\n");
        sml_free(v);
    }
    if (diags) sml_diag_free(diags, n);
    free(text);
    return 0;
}
"""
WARN_CPP = r"""
#include <fstream>
#include <iostream>
#include <sstream>
#include <vector>
#include "sml.hpp"
int main(int argc, char **argv) {
    if (argc < 2) return 2;
    std::ifstream f(argv[1], std::ios::binary);
    std::stringstream ss; ss << f.rdbuf();
    std::string err;
    std::vector<sml::Diag> diags;
    sml::ValuePtr v = sml::Parser::parse(ss.str(), &err, "", &diags);
    if (!v) std::cout << "ERR " << err << "\n";
    else {
        for (const auto &d : diags) std::cout << "WARN " << d.code << "\n";
        std::cout << "OK\n";
    }
    return 0;
}
"""
WARN_JS = r"""
import fs from "node:fs";
const { parse } = await import(process.argv[2]);
const w = [];
try {
  parse(fs.readFileSync(process.argv[3], "utf8"), {}, undefined, undefined, undefined, w);
} catch (e) {
  console.log("ERR " + String((e && (e.code || e.message)) || e));
  process.exit(0);
}
for (const d of w) console.log("WARN " + d.code);
console.log("OK");
"""
WARN_LUA = r"""
local Sml = require("lib.sml")
local f = io.open(arg[1], "rb")
if not f then print("ERR E-IO-001"); return end
local text = f:read("a"); f:close()
local v, err, w = Sml.parse_with_warnings(text)
if not v then print("ERR " .. tostring(err)); return end
if type(w) == "table" then
  for _, d in ipairs(w) do print("WARN " .. tostring(type(d) == "table" and (d.code or d.message) or d)) end
end
print("OK")
"""


def load_conformance():
    """复用套件的适配器与探针协议：编译参数、错误协议、归一化只有那一份。"""
    spec = importlib.util.spec_from_file_location("sml_conformance", CONF_DRIVER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def literal_of(dump):
    """从 dump 文本里取单字段字面量：`a: <literal>`。

    多行（引号串里带真换行、或块）不硬拼，按 `\n` 转义展示，避免把「换行」看漏。
    """
    body = (dump or "").replace("\r\n", "\n").strip()
    if not body:
        return "<空>"
    body = body.replace("\n", "\\n")
    # 本组的文档都是单字段 `a:`；把键前缀去掉再比，否则「多行/单行」的排版差会冒充值差
    if body.startswith("a:"):
        body = body[2:].strip()
    return body[:44]


def write_case(tmp, idx, doc, extras=None):
    """落成临时文件；附带文件写在同一目录（include 用）。返回主文件路径。

    extras["bom"] = True 时给主文件加 UTF-8 BOM（BOM 会影响「行首指令」判定）。
    """
    extras = extras or {}
    d = os.path.join(tmp, "case_%02d" % idx)
    os.makedirs(d, exist_ok=True)
    for name, text in (extras.get("files") or {}).items():
        with open(os.path.join(d, name), "w", encoding="utf-8", newline="\n") as f:
            f.write(text)
    path = os.path.join(d, "main.sml")
    enc = "utf-8-sig" if extras.get("bom") else "utf-8"
    with open(path, "w", encoding=enc, newline="\n") as f:
        f.write(doc)
    return path


def run_row(conf, eps, path):
    """跑一行，返回 {端: (kind, payload)}。"""
    out = {}
    for e in eps:
        if not e.ok:
            out[e.name] = ("skip", e.why)
            continue
        out[e.name] = e.run_case(path)
    return out


WCODE_RE = re.compile(r"W-[A-Z]+-\d{3}")


def build_warn_probes(tmp, rust_exe):
    """编译/拼出各端「只给警告码」的探针命令。返回 {端: (命令前缀, env, 路径位置)}。

    路径位置 = 命令里该插文件路径的下标（-1 表示追加到末尾）。
    """
    # Rust 的 `--lint` 会把诊断写到 stderr；额外参数要放在文件之后。
    cmds = {"rust": ([rust_exe, "-i"], None, "lint")}

    def compile_probe(cc, src, exe, extra, static_first=False):
        with open(src, "w", encoding="utf-8", newline="\n") as f:
            f.write(extra[0])
        base = cc + ["-o", exe, src] + extra[1]
        # MSYS 的 g++ 产出的 exe 依赖 PATH 上的 libstdc++-6.dll，命中另一份会
        # 0xC0000139（ENTRYPOINT_NOT_FOUND）—— 与 conformance 的 prep_cpp 同坑。
        variants = (base + ["-static"], base) if static_first else (base,)
        for cmd in variants:
            p = subprocess.run(cmd, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, timeout=180)
            if p.returncode == 0:
                return True
        return False

    cc = shutil.which("gcc") or shutil.which("cc")
    if cc:
        src = os.path.join(tmp, "warn_c.c")
        exe = os.path.join(tmp, "warn_c.exe")
        if compile_probe([cc, "-std=c99", "-I" + os.path.join(ROOT, "c")],
                         src, exe,
                         (WARN_C, [os.path.join(ROOT, "c", "sml.c"), "-lm"])):
            cmds["c"] = ([exe], None, -1)
    cxx = shutil.which("g++") or shutil.which("c++")
    if cxx:
        src = os.path.join(tmp, "warn_cpp.cpp")
        exe = os.path.join(tmp, "warn_cpp.exe")
        if compile_probe([cxx, "-std=c++17", "-I" + os.path.join(ROOT, "cpp")],
                         src, exe,
                         (WARN_CPP, [os.path.join(ROOT, "cpp", "sml.cpp")]),
                         static_first=True):
            cmds["cpp"] = ([exe], None, -1)
    node = shutil.which("node")
    if node:
        src = os.path.join(tmp, "warn_js.mjs")
        with open(src, "w", encoding="utf-8", newline="\n") as f:
            f.write(WARN_JS)
        mod = Path(os.path.join(ROOT, "js", "sml.mjs")).as_uri()
        cmds["js"] = ([node, src, mod], None, -1)
    lua = shutil.which("luajit") or shutil.which("lua")
    if lua:
        src = os.path.join(tmp, "warn_lua.lua")
        with open(src, "w", encoding="utf-8", newline="\n") as f:
            f.write(WARN_LUA)
        env = dict(os.environ)
        env["LUA_PATH"] = (os.path.join(ROOT, "lua", "?.lua") + ";" +
                           os.path.join(ROOT, "lua", "?.soup") + ";" +
                           env.get("LUA_PATH", ";;"))
        cmds["lua"] = ([lua, src], env, -1)
    return cmds


def run_warn_cmds(cmds, path):
    """跑一份文档的所有端，返回 {端: 排序后的警告码列表 或 'ERR:<码>'}。"""
    out = {}
    for name, (prefix, env, pos) in cmds.items():
        cmd = list(prefix)
        if pos == "lint":
            cmd += [path, "--lint"]
        elif pos == -1:
            cmd += [path]
        else:
            cmd.insert(pos, path)
        try:
            p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               timeout=120, env=env)
        except Exception as e:
            out[name] = "CRASH:" + str(e)[:60]
            continue
        text = (p.stdout or b"").decode("utf-8", "replace") + "\n" + \
               (p.stderr or b"").decode("utf-8", "replace")
        if re.search(r"\bERR\b", text):
            out[name] = "ERR:" + (WCODE_RE.search(text).group(0) if WCODE_RE.search(text)
                                  else (conf_first_code(text) or "?"))
            continue
        # Rust 走 `--lint`，它还带 CLI 的 lint 规则（W-LINT-* / W-CLI-*）；比的是
        # **解析期诊断**，故这两族不计入，避免把「Rust 多了个 CLI 功能」当成跨端差异。
        out[name] = sorted(c for c in WCODE_RE.findall(text)
                           if not c.startswith(("W-LINT-", "W-CLI-")))
    return out


def conf_first_code(text):
    m = re.search(r"E-[A-Z0-9]+-\d{3}", text or "")
    return m.group(0) if m else ""


def main():
    ap = argparse.ArgumentParser(description="套件检不出的差异：错误码 / 门控 / 值身份 / 警告")
    ap.add_argument("--suite", action="append", default=None,
                    choices=["errors", "gates", "values", "warnings"],
                    help="只跑指定组（可重复）")
    ap.add_argument("--end", action="append", default=None, choices=ENDPOINTS)
    ap.add_argument("--no-build", action="store_true")
    ap.add_argument("--csv", default=None, help="把逐行结果写成 CSV")
    args = ap.parse_args()

    conf = load_conformance()
    tmp = tempfile.mkdtemp(prefix="sml_probe_matrix_")
    rows_csv = []
    try:
        eps = conf.build_endpoints(tmp, not args.no_build)
        wanted = args.end or ENDPOINTS
        eps = [e for e in eps if e.name in wanted]
        names = [e.name for e in eps]
        rust = next((e for e in eps if e.name == "rust" and e.ok), None)
        print("== 工具链 ==")
        for e in eps:
            print("  %-4s %-4s %s" % (e.name, "OK" if e.ok else "SKIP",
                                      " ".join(e.cmd) if e.ok else e.why))
        if not rust:
            print("\n[!] 无 Rust 参照，判不了。")
            return 1
        suites = args.suite or ["errors", "gates", "values", "warnings"]
        idw = 26
        colw = 15

        # ---------------- errors ----------------
        if "errors" in suites:
            print("\n== 错误码矩阵（期望码出自 errors/codes.sml）==")
            head = conf.pad("条件", idw) + conf.pad("期望", 15) + \
                "".join(conf.pad(n, colw) for n in names) + "判定"
            print(head)
            print("-" * conf.disp_width(head))
            for idx, row in enumerate(ERRORS, 1):
                name, expect, doc = row[0], row[1], row[2]
                extras = dict(row[3]) if len(row) > 3 else None
                if extras and not ({"files", "bom"} & set(extras)):
                    # 旧写法 {"b.sml": "..."}：整张表就是附带文件
                    extras = {"files": extras}
                path = write_case(tmp, idx, doc, extras)
                res = run_row(conf, eps, path)
                cells, bad = {}, []
                for n in names:
                    kind, payload = res[n]
                    code = conf.first_code(payload) if kind == "err" else \
                        ("OK" if kind == "ok" else kind)
                    cells[n] = code
                    if kind == "skip":
                        continue
                    if code != expect:
                        bad.append(n)
                rust_note = "" if cells.get("rust") == expect else \
                    "（Rust 实为 %s，期望待核）" % cells.get("rust")
                verdict = "AGREE" if not bad else "DIVERGE:" + ",".join(bad)
                print(conf.pad(name, idw) + conf.pad(expect, 15) +
                      "".join(conf.pad(cells.get(n, "-"), colw) for n in names) +
                      verdict + rust_note)
                rows_csv.append(["errors", name, expect] + [cells.get(n, "") for n in names])
            print("\n  说明：`期望待核` 表示 Rust 的实际码与码表不一致 —— 先定码表，再谈四端。")

        # ---------------- gates ----------------
        if "gates" in suites:
            print("\n== 特性门控（期望 = Rust 的实际行为）==")
            head = conf.pad("构造", idw) + conf.pad("Rust", 15) + \
                "".join(conf.pad(n, colw) for n in names[1:]) + "判定"
            print(head)
            print("-" * conf.disp_width(head))
            for idx, (name, feat, doc) in enumerate(GATES, 100):
                path = write_case(tmp, idx, doc)
                res = run_row(conf, eps, path)
                kind, payload = res["rust"]
                ref = conf.first_code(payload) if kind == "err" else \
                    ("OK" if kind == "ok" else kind)
                bad = []
                cells = {}
                for n in names[1:]:
                    k, p = res[n]
                    code = conf.first_code(p) if k == "err" else \
                        ("OK" if k == "ok" else k)
                    cells[n] = code
                    if k == "skip":
                        continue
                    if code != ref:
                        bad.append(n)
                verdict = "AGREE" if not bad else "DIVERGE:" + ",".join(bad)
                print(conf.pad(name, idw) + conf.pad(ref, 15) +
                      "".join(conf.pad(cells.get(n, "-"), colw) for n in names[1:]) + verdict)
                rows_csv.append(["gates", name, ref] + [cells.get(n, "") for n in names[1:]])
            print("\n  说明：Rust 拒绝而某端 OK ⇒ 该端的 `@feature disable` 没有生效（静默放行）。")

        # ---------------- values ----------------
        if "values" in suites:
            print("\n== 值身份（期望 = Rust 的 dump 字面量）==")
            head = conf.pad("用例", 20) + conf.pad("rust", 22) + \
                "".join(conf.pad(n, 22) for n in names[1:]) + "判定"
            print(head)
            print("-" * conf.disp_width(head))
            for idx, (name, doc) in enumerate(VALUES, 200):
                path = write_case(tmp, idx, doc)
                res = run_row(conf, eps, path)
                kind, payload = res["rust"]
                if kind != "ok":
                    print(conf.pad(name, 20) + "Rust 侧未解析成功：%s" % conf.first_line(payload, 80))
                    continue
                ref = literal_of(payload)
                bad = []
                cells = {}
                for n in names[1:]:
                    k, p = res[n]
                    if k == "skip":
                        cells[n] = "skip"
                        continue
                    if k != "ok":
                        cells[n] = conf.first_code(p) or k
                        bad.append(n)
                        continue
                    lit = literal_of(p)
                    cells[n] = lit[:20]
                    if lit != ref:
                        bad.append(n)
                verdict = "AGREE" if not bad else "DIVERGE:" + ",".join(bad)
                print(conf.pad(name, 20) + conf.pad(ref[:20], 22) +
                      "".join(conf.pad(cells.get(n, "-"), 22) for n in names[1:]) + verdict)
                rows_csv.append(["values", name, ref] + [cells.get(n, "") for n in names[1:]])
            print("\n  说明：字面量不同 ⇒ 类型或精度在 dump 载体上丢了；套件把该端 dump 交回 Rust")
            print("        回读，若恰好落回同一个值树，就会判 AGREE（假绿）。")

        # ---------------- warnings ----------------
        if "warnings" in suites:
            print("\n== 警告码（期望 = Rust 的警告码集合）==")
            cmds = build_warn_probes(tmp, rust.exe)
            wnames = [n for n in names if n in cmds]
            missing = [n for n in names if n not in cmds]
            if missing:
                print("  缺工具链：%s" % ", ".join(missing))
            head = conf.pad("用例", 22) + "".join(conf.pad(n, 20) for n in wnames) + "判定"
            print(head)
            print("-" * conf.disp_width(head))
            for idx, (name, doc) in enumerate(WARNINGS, 300):
                path = write_case(tmp, idx, doc)
                res = run_warn_cmds(cmds, path)
                ref = res.get("rust")
                if not isinstance(ref, list):
                    print(conf.pad(name, 22) + "Rust 侧未产出诊断：%s" % ref)
                    continue
                bad, cells = [], {}
                for n in wnames:
                    r = res.get(n)
                    cells[n] = ",".join(r) if isinstance(r, list) else str(r)
                    if r != ref:
                        bad.append(n)
                verdict = "AGREE" if not bad else "DIVERGE:" + ",".join(bad)
                print(conf.pad(name, 22) +
                      "".join(conf.pad(cells.get(n, "-")[:18], 20) for n in wnames) + verdict)
                rows_csv.append(["warnings", name, ",".join(ref)] +
                                [cells.get(n, "") for n in wnames])
            print("\n  说明：Rust 走 `smltools --lint`（含 lint 规则，可能比各端多出 W-LINT-*）；")
            print("        其余四端只交解析期诊断。少一条或多一条都记差异，不静默。")

        if args.csv:
            import csv as _csv
            with open(args.csv, "w", encoding="utf-8-sig", newline="") as f:
                w = _csv.writer(f)
                w.writerow(["suite", "case", "expect"] + names)
                w.writerows(rows_csv)
            print("\n已写出 %s" % args.csv)

        total = sum(1 for r in rows_csv if len(set(r[3:])) > 1)
        print("\n结论：%d 行里 %d 行各端不一致。" % (len(rows_csv), total))
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
