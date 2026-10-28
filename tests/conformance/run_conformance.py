#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""W5：跨实现一致性套件（conformance）驱动。

一份共享用例（`tests/conformance/cases/*.sml`）被 **Rust / C / C++ / JS / Lua** 各解析一遍，
比较解析出的**值树**是否一致（而不是比较各自 dump 出来的字节 —— 那是 W4
`tools/check_dump_parity.py` 的 C↔Rust 逐字节装置，本套件与它分工不同）。

------------------------------------------------------------------------------
一、为什么不是「各自 dump 成 JSON 再比」
------------------------------------------------------------------------------
`docs/capability-matrix.md` 记着一批**已知的历史差异**：
  - JS 的 `\\uXXXX` 少跳一位（末位十六进制数字被当普通字符拼回）；
  - C++ 只认 `\\u{...}`、不认 `\\uXXXX`，且不支持裸 `include`；
  - Lua 的大整数（>2^53）转 float 丢精度；
  - `1.0` 只有 Rust 保留为浮点；
  - `$env` 在引号内有三种口径。

这些差异**不该**由「换一个比对载体」来消掉，也不该用「跳过该用例」藏起来 ——
它们必须跑出来，然后逐条写进 `known_divergences.md`。所以本脚本的设计是：

  每端只负责「用本端的解析器解析，并 dump 成本端的 SML 文本」（这是五端本来就有的能力：
  Rust `to_sml` / C `sml_dump` / C++ `to_sml` / JS `stringify` / Lua `Sml.dump`），
  **归一化交给唯一的 Rust 侧**：把该文本再喂给 `smltools` 换成
  （① 结构化 JSON 值树，② 重新 dump 出的规范 SML 文本），两层比较。

为什么要有两层：
  - **JSON 层**看结构与类型（对象/数组/字符串/整数），键序已由 Rust 侧 `BTreeMap` 统一，
    所以「C 保源序、Rust 排序」这类**纯渲染差异不会误报**；
  - **规范文本层**补 JSON 的短板 —— `smltools --to json` 会把浮点 `1.0` 压成 `1`、
    会把 `1e10` 展开成 `10000000000`。若只比 JSON，`1.0` 这条**点名要钉的**差异
    就会被载体静默吃掉。文本层是 Rust 自己的 dump，键序 / 引号 / 缩进都已规范，
    不会引入渲染噪声。

⇒ 判据：两端**值树**相同 ⇔ 两层都相同。差异一律显式报出，不静默。

------------------------------------------------------------------------------
二、用例协议
------------------------------------------------------------------------------
普通用例：五端都必须解析成功，且值树一致。

负向用例：文件头写一行

    # conformance: expect-error E-PARSE-008

则要求**可用端全部以该错误码失败**（`errors/codes.sml` 的码是稳定契约，文案不是）。
任一端「解析成功」或报了别的码 ⇒ DIVERGE。这样 W3（顶层标量五端统一显式报错）
这类「必须一起报错」的口径才有钉子。

------------------------------------------------------------------------------
三、分类与退出码
------------------------------------------------------------------------------
  AGREE       五端值树一致（负向用例：五端同码失败）
  DIVERGE     真差异（含「某端解析失败而另一端成功」）
  SKIP        该端工具链缺失（**明确写出缺什么，绝不伪造通过**）
  PARSE_FAIL  值树用例里**所有**可用端都解析失败 ⇒ 多半是用例本身写坏了

退出码：0 = 没有 DIVERGE 也没有 PARSE_FAIL；1 = 有（有 SKIP 不算失败）。

------------------------------------------------------------------------------
四、用法
------------------------------------------------------------------------------
  python tests/conformance/run_conformance.py
  python tests/conformance/run_conformance.py --case nest_array --show 3
  python tests/conformance/run_conformance.py --end rust --end js
  python tests/conformance/run_conformance.py --no-build      # 不自动构建 smltools

环境变量：
  SMLTOOLS   指定 Rust 侧 `smltools` 可执行文件（不指定则自动探测，见 find_smltools）
  SML_REPO   覆盖仓库根（默认由本文件位置推出）
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unicodedata

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    sys.stderr.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

ROOT = os.environ.get("SML_REPO") or os.path.dirname(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CONF = os.path.join(ROOT, "tests", "conformance")
CASES = os.path.join(CONF, "cases")
ADAPTERS = os.path.join(CONF, "adapters")
ENDPOINTS = ["rust", "c", "cpp", "js", "lua"]

CODE_RE = re.compile(r"E-[A-Z0-9]+-\d{3}")
DIRECTIVE_RE = re.compile(r"^\s*#\s*conformance:\s*([A-Za-z_-]+)\s*(.*?)\s*$")

# 复现用的固定环境变量（用例里写 `$env.SML_CONF_TOKEN`）。
# 值刻意写成明显的占位符 —— 不得出现任何真实凭据。
CONF_ENV = {"SML_CONF_TOKEN": "conf-token-1"}

TIMEOUT = 120


# ---------------------------------------------------------------------------
# 小工具
# ---------------------------------------------------------------------------
def disp_width(s):
    w = 0
    for ch in s:
        w += 2 if unicodedata.east_asian_width(ch) in ("W", "F") else 1
    return w


def pad(s, width):
    return s + " " * max(0, width - disp_width(s))


def run(cmd, cwd=None, env=None, timeout=TIMEOUT):
    """返回 (rc, stdout, stderr)。编码统一按 UTF-8 宽松解码，避免 GBK 平台炸掉。"""
    try:
        p = subprocess.run(cmd, cwd=cwd, env=env, timeout=timeout,
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    except FileNotFoundError:
        return (127, "", "找不到可执行文件: %s" % cmd[0])
    except subprocess.TimeoutExpired:
        return (124, "", "超时 %ds: %s" % (timeout, cmd[0]))
    out = (p.stdout or b"").decode("utf-8", "replace")
    err = (p.stderr or b"").decode("utf-8", "replace")
    return (p.returncode, out, err)


def first_code(text):
    m = CODE_RE.search(text or "")
    return m.group(0) if m else ""


def first_line(text, n=140):
    for ln in (text or "").splitlines():
        ln = ln.strip()
        if ln:
            return ln[:n]
    return ""


# ---------------------------------------------------------------------------
# Rust 侧（参考实现 + 归一化器）
# ---------------------------------------------------------------------------
def cargo_target_dir():
    """从 `cargo metadata` 取真实 target 目录。

    为什么不能只认 `rust/target`：本机 `~/.cargo/config.toml` 把 `target-dir`
    设到了别的盘（C 盘空间紧张），产物根本不在仓库里。`--no-deps` 很快。
    """
    rc, out, _ = run(["cargo", "metadata", "--no-deps", "--format-version", "1"],
                     cwd=os.path.join(ROOT, "rust"), timeout=180)
    if rc != 0:
        return None
    try:
        return json.loads(out).get("target_directory")
    except Exception:
        return None


def find_smltools():
    env = os.environ.get("SMLTOOLS")
    if env and os.path.isfile(env):
        return env
    cands = []
    for sub in ("release", "debug"):
        cands.append(os.path.join(ROOT, "rust", "target", sub, "smltools.exe"))
        cands.append(os.path.join(ROOT, "rust", "target", sub, "smltools"))
    tdir = cargo_target_dir()
    if tdir:
        for sub in ("release", "debug"):
            cands.append(os.path.join(tdir, sub, "smltools.exe"))
            cands.append(os.path.join(tdir, sub, "smltools"))
    for c in cands:
        if os.path.isfile(c):
            return c
    return None


def build_smltools():
    """缺二进制时现编一次（只为跑套件，不产物入库）。返回 (exe, 说明)。"""
    if shutil.which("cargo") is None:
        return None, "缺 cargo，且找不到已构建的 smltools"
    print("[..] 未找到 smltools，尝试 `cargo build -p smltools`（首次可能较慢）")
    rc, out, err = run(["cargo", "build", "-p", "smltools"],
                       cwd=os.path.join(ROOT, "rust"), timeout=1800)
    if rc != 0:
        return None, "cargo build -p smltools 失败: " + first_line(err or out)
    exe = find_smltools()
    if not exe:
        return None, "构建成功但找不到 smltools 产物"
    return exe, ""


# ---------------------------------------------------------------------------
# 各端适配器
# ---------------------------------------------------------------------------
class Endpoint(object):
    def __init__(self, name):
        self.name = name
        self.ok = False
        self.why = ""
        self.cmd = None      # 不含文件路径的命令前缀
        self.suffix = []     # 文件路径之后的参数（Rust 的 `--to sml` 走这里）
        self.env = None
        self.tmp = None      # 需要写文件的端（Lua）用
        self.outfile = False
        self.kind = name     # rust / c / cpp / js / lua —— 决定输出协议细节

    def run_case(self, path):
        """返回 (kind, payload)；kind ∈ ok / err / crash。"""
        if self.outfile:
            outp = os.path.join(self.tmp, "adapter_out_%s.txt" % self.name)
            if os.path.exists(outp):
                os.remove(outp)
            rc, out, err = run(self.cmd + [outp, path] + self.suffix, env=self.env)
            if os.path.isfile(outp):
                with open(outp, "rb") as f:
                    out = f.read().decode("utf-8", "replace")
        else:
            rc, out, err = run(self.cmd + [path] + self.suffix, env=self.env)
        if rc in (124, 127):
            return ("crash", err.strip() or ("rc=%d" % rc))
        if self.kind == "rust":
            # Rust 端没有适配器外壳：`smltools -i <f> --to sml` 的 stdout 就是 dump 原文，
            # 失败时 rc != 0 且信息在 stderr（这一端与 C / Lua 不同，后两者失败也是 rc=0）。
            if rc != 0:
                return ("err", (err or out).strip())
            return ("ok", out)
        line0 = out.split("\n", 1)[0] if out else ""
        body = out.split("\n", 1)[1] if "\n" in out else ""
        for tag, good in (("CDUMP ", True), ("DUMP ", True), ("CERR ", False), ("ERR ", False)):
            if line0.startswith(tag):
                if good:
                    return ("ok", body)
                rest = line0[len(tag):]
                if rest.startswith(path):
                    rest = rest[len(path):]
                return ("err", rest.strip() or err.strip())
        # 谁也没打印协议行 ⇒ 适配器自己崩了（不是解析失败）——必须区分开
        detail = first_line(err or out) or ("rc=%d 无输出" % rc)
        return ("crash", detail)


def prep_rust(allow_build):
    ep = Endpoint("rust")
    exe = find_smltools()
    if not exe and allow_build:
        exe, why = build_smltools()
        if not exe:
            ep.why = why
            return ep
    if not exe:
        ep.why = "找不到 smltools（先 `cd rust && cargo build -p smltools`，或设 SMLTOOLS）"
        return ep
    ep.ok = True
    # Rust 端**不需要适配器**：`smltools -i <file> --to sml` 就是它的 to_sml 出口。
    # 解析失败时 rc != 0、信息走 stderr（与 C / Lua 不同，这两端失败也是 rc=0）。
    ep.cmd = [exe, "-i"]
    ep.suffix = ["--to", "sml"]
    ep.exe = exe
    return ep


def prep_c(tmp):
    ep = Endpoint("c")
    cc = shutil.which("gcc") or shutil.which("cc")
    if not cc:
        ep.why = "缺 gcc"
        return ep
    exe = os.path.join(tmp, "dump_c.exe")
    src = os.path.join(ROOT, "tools", "dump_c.c")     # 复用 W4 的 C 侧 dumper
    rc, out, err = run([cc, "-std=c99", "-I" + os.path.join(ROOT, "c"),
                        "-o", exe, src, os.path.join(ROOT, "c", "sml.c")])
    if rc != 0:
        ep.why = "编译 C dumper 失败: " + first_line(err or out)
        return ep
    ep.ok = True
    ep.cmd = [exe]
    return ep


def prep_cpp(tmp):
    ep = Endpoint("cpp")
    cxx = shutil.which("g++") or shutil.which("c++")
    if not cxx:
        ep.why = "缺 g++"
        return ep
    exe = os.path.join(tmp, "dump_cpp.exe")
    base = [cxx, "-std=c++17", "-I" + os.path.join(ROOT, "cpp"), "-o", exe,
            os.path.join(ADAPTERS, "dump_cpp.cpp"), os.path.join(ROOT, "cpp", "sml.cpp")]
    # 先试**全静态**：MSYS 的 g++ 产出的 exe 依赖 PATH 上的 libstdc++-6.dll，而 PATH 里
    # 可能先命中另一份（实测 `-static-libstdc++ -static-libgcc` 仍会 0xC0000139
    # ENTRYPOINT_NOT_FOUND；`-static` 才行）。这类失败表现成「适配器异常」而不是编译错误，
    # 很容易被误判成 C++ 实现坏了 —— 故这里必须把两条路都试过。
    rc, out, err = run(base + ["-static"])
    if rc != 0:
        rc, out, err = run(base)
    if rc != 0:
        ep.why = "编译 C++ dumper 失败: " + first_line(err or out)
        return ep
    ep.ok = True
    ep.cmd = [exe]
    return ep


def prep_js():
    ep = Endpoint("js")
    node = shutil.which("node")
    if not node:
        ep.why = "缺 node"
        return ep
    ep.ok = True
    ep.cmd = [node, os.path.join(ADAPTERS, "dump_js.mjs")]
    return ep


def prep_lua(tmp):
    ep = Endpoint("lua")
    lua = shutil.which("luajit") or shutil.which("lua")
    if not lua:
        ep.why = "缺 luajit（或 lua）"
        return ep
    env = dict(os.environ)
    lua_dir = os.path.join(ROOT, "lua")
    # `?.soup` 是仓库里的实际扩展名（纯 Lua 源码，不是编译产物）；`?.lua` 兜底
    env["LUA_PATH"] = os.path.join(lua_dir, "?.soup") + ";" + \
        os.path.join(lua_dir, "?.lua") + ";" + env.get("LUA_PATH", ";;")
    for k, v in CONF_ENV.items():
        env[k] = v
    ep.ok = True
    ep.env = env
    ep.tmp = tmp
    ep.outfile = True     # Windows 上 Lua 的 stdout 是文本模式，会把 \n 翻成 \r\n
    ep.cmd = [lua, os.path.join(ADAPTERS, "dump_lua.lua")]
    return ep


def build_endpoints(tmp, allow_build):
    eps = []
    for prep in (lambda: prep_rust(allow_build), lambda: prep_c(tmp), lambda: prep_cpp(tmp),
                 lambda: prep_js(), lambda: prep_lua(tmp)):
        ep = prep()
        ep.env = ep.env or dict(os.environ, **CONF_ENV)
        eps.append(ep)
    return eps


# ---------------------------------------------------------------------------
# 归一化（唯一的 Rust 侧）
# ---------------------------------------------------------------------------
def canon(exe, dump_text, tmp, tag):
    """把某端的 dump 文本归一化成 (json 值树, 规范 SML 文本)。

    失败返回 (None, 原因) —— 归一到一半失败本身也是**跨端问题**
    （某端 dump 出来的东西 Rust 读不回去），必须报出来，不能吞。
    """
    body = dump_text
    if body.startswith("\n"):
        body = body[1:]
    if body.strip() == "":
        # 空文档：dump 为空串是合法形态（空块 / 只有注释的文档），
        # 用空 token 流表示，别让它掉进「取值失败」。
        return ({"json": None, "sml": "", "tokens": [], "raw": ""}, None)
    p = os.path.join(tmp, "canon_%s.sml" % tag)
    with open(p, "w", encoding="utf-8", newline="\n") as f:
        f.write(body if body.endswith("\n") else body + "\n")
    rc1, jout, jerr = run([exe, "-i", p, "--to", "json"])
    if rc1 != 0:
        return (None, "该端 dump 不能被 Rust 回读（--to json）: " + first_line(jerr or jout))
    rc2, tout, terr = run([exe, "-i", p, "--to", "sml"])
    if rc2 != 0:
        return (None, "该端 dump 不能被 Rust 回读（--to sml）: " + first_line(terr or tout))
    try:
        tree = json.loads(jout)
    except Exception as e:
        return (None, "Rust --to json 输出不是合法 JSON: %s" % e)
    sml_text = tout.rstrip("\n")
    return ({"json": tree, "sml": sml_text, "tokens": tokenize_dump(sml_text),
             "raw": body.rstrip("\n")}, None)


RE_INT = re.compile(r"^-?\d+$")
RE_FLOAT = re.compile(r"^-?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][-+]?\d+)?$")


def tokenize_dump(text):
    """把 Rust 的规范 SML dump 切成 token 流，用于**忽略数字写法**的比对。

    为什么不能直接比文本：Rust 的 `Value` 保留了原始字面量（`raw`），于是
    `1e10` / `1e+10` / `10000000000` 会被 dump 成三种写法，而它们是**同一个值**；
    `3.14` 与 `3.1400000000000001` 同理。那是渲染差异（capability-matrix §3 #7/#8），
    不该判成 DIVERGE。反过来 `1` 与 `1.0` 是**真差异**（int vs float），必须要留住。

    ⇒ 规则：数字按「整数 / 浮点 + 数值」归一（`1` → int 1，`1.0` → float 1.0，
    `1e10` 与 `10000000000` → 同一个 float）；字符串按**解码后的内容**比；
    标点与裸词按字面比。空白全部丢掉。
    """
    toks = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c in " \t\r\n":
            i += 1
            continue
        if c == '"':
            j, buf = i + 1, []
            while j < n:
                if text[j] == "\\" and j + 1 < n:
                    buf.append(text[j:j + 2])
                    j += 2
                    continue
                if text[j] == '"':
                    break
                buf.append(text[j])
                j += 1
            toks.append(("str", "".join(buf)))
            i = j + 1
            continue
        if c in "{}[]:,":
            toks.append(("punct", c))
            i += 1
            continue
        j = i
        while j < n and text[j] not in ' \t\r\n{}[]:,"':
            j += 1
        w = text[i:j]
        if RE_INT.match(w):
            toks.append(("int", w.lstrip("+")))
        elif RE_FLOAT.match(w):
            try:
                toks.append(("float", repr(float(w))))
            except ValueError:
                toks.append(("word", w))
        elif w in ("true", "false"):
            toks.append(("bool", w))
        elif w == "null":
            toks.append(("null", w))
        else:
            toks.append(("word", w))
        i = j
    return toks


def first_tok_diff(a, b):
    for i in range(min(len(a), len(b))):
        if a[i] != b[i]:
            return i
    if len(a) != len(b):
        return min(len(a), len(b))
    return -1


def fmt_toks(toks, hi=None):
    parts = []
    for k, (kind, val) in enumerate(toks):
        s = val if kind in ("word", "punct", "int") else "%s:%s" % (kind, val)
        parts.append("[%s]" % s if k == hi else s)
    return " ".join(parts)


def type_name(v):
    if v is None:
        return "null"
    if isinstance(v, bool):
        return "bool"
    if isinstance(v, int):
        return "int"
    if isinstance(v, float):
        return "float"
    if isinstance(v, str):
        return "str"
    if isinstance(v, list):
        return "array"
    if isinstance(v, dict):
        return "object"
    return type(v).__name__


def diff_values(a, b, path="", out=None, limit=8):
    """**类型严格**的深比较：`1`（int）与 `1.0`（float）、`1` 与 `true` 都算不同。

    Python 的 `1 == 1.0` 为真、`True == 1` 也为真 —— 直接 `==` 会把
    「`1.0` 只有 Rust 保留为浮点」这条已知差异静默吃掉，正是不能接受的那种「放宽判据」。
    """
    if out is None:
        out = []
    if len(out) >= limit:
        return out
    ta, tb = type_name(a), type_name(b)
    if ta != tb:
        out.append((path or "$", "%s %r" % (ta, a), "%s %r" % (tb, b)))
        return out
    if ta == "object":
        ka, kb = set(a.keys()), set(b.keys())
        for k in sorted(ka - kb):
            out.append((path + "/" + k, json.dumps(a[k], ensure_ascii=False), "<该端缺失>"))
            if len(out) >= limit:
                return out
        for k in sorted(kb - ka):
            out.append((path + "/" + k, "<该端缺失>", json.dumps(b[k], ensure_ascii=False)))
            if len(out) >= limit:
                return out
        for k in sorted(ka & kb):
            diff_values(a[k], b[k], path + "/" + k, out, limit)
            if len(out) >= limit:
                return out
        return out
    if ta == "array":
        if len(a) != len(b):
            out.append((path, "长度 %d" % len(a), "长度 %d" % len(b)))
            if len(out) >= limit:
                return out
        for i in range(min(len(a), len(b))):
            diff_values(a[i], b[i], "%s[%d]" % (path, i), out, limit)
            if len(out) >= limit:
                return out
        return out
    if a != b:
        out.append((path or "$", "%s %r" % (ta, a), "%s %r" % (tb, b)))
    return out


def sig_key(c):
    """归一化结果的可哈希签名（分类用）。

    两层都要进签名：JSON 层管结构（`--to json` 会把 1.0 写成 1，故它单独不够），
    token 层管 int/float 身份（数字按值归一，写法差异不参与）。
    """
    return json.dumps(c["json"], sort_keys=True, ensure_ascii=False) + "\x00" + repr(c["tokens"])


# ---------------------------------------------------------------------------
# 用例
# ---------------------------------------------------------------------------
def read_directives(path):
    d = {}
    try:
        with open(path, "r", encoding="utf-8-sig") as f:
            head = f.read(2048)
    except Exception:
        return d
    for line in head.splitlines():
        m = DIRECTIVE_RE.match(line)
        if m:
            d[m.group(1)] = m.group(2)
    return d


def discover(filter_pat):
    if not os.path.isdir(CASES):
        return []
    names = sorted(n for n in os.listdir(CASES)
                   if n.endswith(".sml") and not n.startswith("_"))
    if filter_pat:
        names = [n for n in names if any(p.lower() in n.lower() for p in filter_pat)]
    return [os.path.join(CASES, n) for n in names]


# ---------------------------------------------------------------------------
# 主流程
# ---------------------------------------------------------------------------
def main():
    ap = argparse.ArgumentParser(description="SML 跨实现一致性套件（五端）")
    ap.add_argument("--case", action="append", default=None,
                    help="只跑文件名含该子串的用例（可重复）")
    ap.add_argument("--end", action="append", default=None, choices=ENDPOINTS,
                    help="只跑指定端（可重复）")
    ap.add_argument("--show", type=int, default=2, help="每个 DIVERGE 打印几处差异（默认 2）")
    ap.add_argument("--no-build", action="store_true", help="不自动构建 smltools")
    ap.add_argument("--verbose", action="store_true", help="打印每端原始 dump")
    args = ap.parse_args()

    files = discover(args.case)
    if not files:
        print("[!] 没找到用例（%s）。用 --case 过滤时确认子串对得上。" % CASES)
        return 1
    # 空用例是**假绿**：五端都产出空树，于是全员 AGREE。
    # （本目录真的被并发写入清空过一次 ⇒ 加这道闸，别让「用例坏了」表现成「五端一致」。）
    empty = [os.path.basename(p) for p in files if os.path.getsize(p) == 0]
    if empty:
        print("[!] 以下用例是 **0 字节**（空文档会让五端一起 AGREE，属假绿）：")
        for n in empty:
            print("      " + n)
        print("    先补回内容再跑；确实要测空文档，请显式写成注释用例并在文件名上标明。")
        return 1

    tmp = tempfile.mkdtemp(prefix="sml_conformance_")
    try:
        eps = build_endpoints(tmp, not args.no_build)
        wanted = args.end or ENDPOINTS
        eps = [e for e in eps if e.name in wanted]

        print("== 工具链 ==")
        for e in eps:
            if e.ok:
                print("  %-4s OK   %s" % (e.name, " ".join(e.cmd)))
            else:
                print("  %-4s SKIP %s" % (e.name, e.why))
        rust_ep = next((e for e in eps if e.name == "rust" and e.ok), None)
        if not rust_ep:
            print("\n[!] Rust 侧不可用 ⇒ **归一化器缺失**，套件无法判定。"
                  "这不是「五端一致」，请先修 Rust 侧（见上面的 SKIP 原因）。")
            return 1

        active = [e for e in eps if e.ok]
        print("\n用例 %d 个，可用端 %d 个（%s）\n"
              % (len(files), len(active), ", ".join(e.name for e in active)))

        rows = []
        per_end = {e.name: {"AGREE": 0, "DIVERGE": 0, "SKIP": 0, "PARSE_FAIL": 0} for e in eps}
        diverged = []

        name_w = max([disp_width(os.path.basename(f)[:-4]) for f in files] + [4])

        for idx, path in enumerate(files, 1):
            stem = os.path.basename(path)[:-4]
            meta = read_directives(path)
            expect_err = (meta.get("expect-error") or "").strip()
            tag = "%02d" % idx

            results = {}     # end -> dict(kind, dump|msg, canon, canon_err)
            for e in eps:
                if not e.ok:
                    results[e.name] = {"kind": "skip", "msg": e.why}
                    continue
                kind, payload = e.run_case(path)
                r = {"kind": kind, "msg": payload if kind != "ok" else ""}
                if kind == "ok":
                    c, cerr = canon(rust_ep.exe, payload, tmp, tag + "_" + e.name)
                    r["canon"], r["canon_err"] = c, cerr
                    r["dump"] = payload
                results[e.name] = r

            cells = {}
            if expect_err:
                # —— 负向用例：五端必须以同一个码失败 ——
                for e in eps:
                    r = results[e.name]
                    if r["kind"] == "skip":
                        cells[e.name] = "skip"
                    elif r["kind"] == "err":
                        code = first_code(r["msg"])
                        cells[e.name] = "e" if code == expect_err else (code or "err")
                    elif r["kind"] == "crash":
                        cells[e.name] = "!"
                    else:
                        cells[e.name] = "ok"       # 该端**没有**报错 —— 与其余端相反
                bad = [n for n in cells if cells[n] not in ("skip", "e")]
                verdict = "AGREE" if not bad else "DIVERGE"
                detail = ("期望全部以 %s 失败；不符合的端：%s"
                          % (expect_err, ", ".join(
                              "%s(%s: %s)" % (n, cells[n], first_line(results[n]["msg"], 90))
                              for n in bad)))
            else:
                classes = {}
                for e in eps:
                    r = results[e.name]
                    if r["kind"] == "skip":
                        cells[e.name] = "skip"
                    elif r["kind"] == "err":
                        cells[e.name] = "ERR"      # 该端解析失败（原因见下面的明细）
                    elif r["kind"] == "crash":
                        cells[e.name] = "!"        # 适配器自己崩了，与「实现差异」区分开
                    elif r.get("canon") is None:
                        cells[e.name] = "NC"       # 解析成功，但 dump 不能被 Rust 回读
                    else:
                        k = sig_key(r["canon"])
                        if k not in classes:
                            classes[k] = (chr(ord("A") + len(classes)), [])
                        classes[k][1].append(e.name)
                        cells[e.name] = classes[k][0]
                groups = list(classes.values())
                if len(groups) == 1 and all(cells[n] in ("A", "skip") for n in cells):
                    verdict = "AGREE"
                    detail = ""
                elif not groups:
                    verdict = "PARSE_FAIL"
                    detail = "所有可用端都解析失败 —— 先用 Rust 单独核对该用例本身"
                else:
                    verdict = "DIVERGE"
                    detail = "分组：" + " | ".join(
                        "%s = %s" % (letter, ",".join(names)) for letter, names in groups)
                    for n in cells:
                        c = cells[n]
                        if c in ("skip",) or (len(c) == 1 and c.isalpha() and c.isupper()):
                            continue
                        detail += "；%s=%s" % (n, c)
                        if c == "NC":
                            detail += "（%s）" % first_line(results[n].get("canon_err") or "", 130)
                        elif c in ("ERR", "!"):
                            detail += "（%s）" % first_line(results[n].get("msg") or "", 130)

            rows.append((stem, cells, verdict, detail, results))
            if verdict == "DIVERGE":
                diverged.append((stem, detail, results))

            # —— 每端计分（参考实现 = Rust；负向用例的参考 = 期望码）——
            ref_cell = cells.get("rust")
            for e in eps:
                c = cells[e.name]
                bucket = per_end[e.name]
                if c == "skip":
                    bucket["SKIP"] += 1
                elif verdict == "AGREE":
                    bucket["AGREE"] += 1
                elif verdict == "PARSE_FAIL":
                    bucket["PARSE_FAIL"] += 1
                elif c in ("ERR", "NC", "!") or (expect_err and c == "ok"):
                    # 「两端都失败」不算一致 —— 必须先判「有没有产出值树」，
                    # 否则 ref 也是 ERR 时会被算成 AGREE（本轮实测踩过）。
                    bucket["PARSE_FAIL"] += 1
                elif c == ref_cell:
                    bucket["AGREE"] += 1
                else:
                    bucket["DIVERGE"] += 1

        # —— 表格 ——
        head = pad("用例", name_w + 2) + "".join(pad(e.name, 6) for e in eps) + "判定"
        print(head)
        print("-" * disp_width(head))
        for stem, cells, verdict, detail, _ in rows:
            line = pad(stem, name_w + 2) + "".join(pad(str(cells.get(e.name, "-")), 6) for e in eps)
            print(line + verdict)
        # 表格只放短记号，原因见下面的逐条明细
        print("\n图例：A/B/C = 值树分组（同字母即完全一致）；e = 按期望码失败；"
              "ERR = 该端解析失败；NC = 该端 dump 不能被 Rust 回读；"
              "! = 适配器异常（不是实现差异）；skip = 该端缺工具链")

        print("\n== 分类统计（按端；参考实现 = Rust）==")
        print(pad("端", 8) + pad("AGREE", 8) + pad("DIVERGE", 10) + pad("PARSE_FAIL", 12) + "SKIP")
        for e in eps:
            b = per_end[e.name]
            print(pad(e.name, 8) + pad(str(b["AGREE"]), 8) + pad(str(b["DIVERGE"]), 10)
                  + pad(str(b["PARSE_FAIL"]), 12) + str(b["SKIP"]))

        agg = {}
        for stem, cells, verdict, detail, _ in rows:
            agg[verdict] = agg.get(verdict, 0) + 1
        print("\n== 用例判定 ==")
        for k in ("AGREE", "DIVERGE", "PARSE_FAIL", "SKIP"):
            if agg.get(k):
                print("  %-11s %d" % (k, agg[k]))

        if diverged:
            print("\n== DIVERGE 明细（每条都该进 known_divergences.md）==")
            for stem, detail, results in diverged:
                print("\n  [%s]" % stem)
                print("    " + detail)
                # 取两个分组的代表做值树 diff
                groups = {}
                for n, r in results.items():
                    if r["kind"] == "ok" and r.get("canon"):
                        groups.setdefault(sig_key(r["canon"]), []).append(n)
                glist = list(groups.values())
                if len(glist) >= 2:
                    left, right = glist[0], glist[1]
                    a = results[left[0]]["canon"]
                    b = results[right[0]]["canon"]
                    diffs = diff_values(a["json"], b["json"], limit=args.show)
                    print("    %s vs %s：JSON 值树差异 %d 处%s"
                          % (",".join(left), ",".join(right), len(diffs),
                             "" if diffs else "（JSON 层一致 ⇒ 差异在 int/float 身份上，看下面 token 层）"))
                    for p, x, y in diffs:
                        print("      %-28s %-30s | %s" % (p, x, y))
                    ta, tb = a["tokens"], b["tokens"]
                    k = first_tok_diff(ta, tb)
                    if k >= 0:
                        lo = max(0, k - 3)
                        print("      token 层差异起于第 %d 个 token（数字按值归一，故这里只剩"
                              " int/float 身份与字符串内容）" % k)
                        print("        %-10s … %s" % (left[0], fmt_toks(ta[lo:k + 3], k - lo)))
                        print("        %-10s … %s" % (right[0], fmt_toks(tb[lo:k + 3], k - lo)))
                elif len(glist) == 1:
                    others = [n for n, r in results.items()
                              if r["kind"] != "ok" and r["kind"] != "skip"]
                    for n in others:
                        print("    %s 没有产出值树：%s" % (n, first_line(results[n]["msg"], 160)))

        if args.verbose:
            print("\n== 原始 dump ==")
            for stem, cells, verdict, detail, results in rows:
                for n, r in results.items():
                    if r["kind"] == "ok":
                        print("\n--- %s / %s ---\n%s" % (stem, n, r.get("dump", "").strip()))
                    else:
                        print("\n--- %s / %s --- %s" % (stem, n, first_line(r["msg"], 160)))

        print()
        if diverged or agg.get("PARSE_FAIL"):
            print("结论：存在 DIVERGE / PARSE_FAIL ⇒ 退出码 1。"
                  "差异请逐条登记进 tests/conformance/known_divergences.md（不许静默忽略）。")
            return 1
        print("结论：全部非 SKIP 用例 AGREE ⇒ 退出码 0。")
        return 0
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
