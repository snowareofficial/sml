"""errors/codes.sml -> 各端错误码常量

为什么要有这一步：错误码的**唯一事实来源**是 `errors/codes.sml`。
如果各端各自手抄一份常量表，迟早漂移 —— 而「同因同码」是码表存在的全部意义。
所以把码表**生成**成各语言能直接引用的文件，各端只写「用哪个码」，不写「码长什么样」。

产物（全部是生成物，**不要手改**）：
    rust/sml-codes/src/codes.rs     Rust 常量（`pub const E_LEX_001: &str = "E-LEX-001";`）
    js/sml-codes.mjs                JS 常量（ESM 具名导出 + `ALL`）
    c/sml_codes.h                   C / C++ 宏（`SML_E_LEX_001`，Lua 侧亦可用字符串字面量）

用法：
    python errors/gen_codes.py            # 生成，失败退出码非 0
    python errors/gen_codes.py --check    # 只校验产物是否与码表一致（给 CI 用）

产物**不含时间戳**：同样的码表必须产出逐字节相同的文件，否则每次构建都脏一个文件。
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import gen_json  # noqa: E402  （同目录，复用「找工具链 + 解析 codes.sml」的逻辑）

ROOT = gen_json.ROOT
RUST_OUT = os.path.join(ROOT, "rust", "sml-codes", "src", "codes.rs")
JS_OUT = os.path.join(ROOT, "js", "sml-codes.mjs")
C_OUT = os.path.join(ROOT, "c", "sml_codes.h")

# 生成物统一带这一行「勿手改」抬头，--check 时也靠它自我说明
GEN_NOTE = "本文件由 errors/gen_codes.py 从 errors/codes.sml 生成，请勿手改"


def ident(code):
    """`E-LEX-001` -> `E_LEX_001`（Rust / C 标识符）。"""
    return code.replace("-", "_")


def rust_src(codes):
    lines = [
        "// SPDX-License-Identifier: MulanPSL-2.0",
        "//! SML 错误码常量。",
        "//!",
        "//! %s。" % GEN_NOTE,
        "//! 唯一事实来源是 `errors/codes.sml`（见 `errors/README.md`）。",
        "//!",
        "//! 三条纪律：**码是稳定契约，文案不是**；同一触发条件各端必须同码；序号只增不回收。",
        "",
        "#![allow(dead_code)]",
        "",
    ]
    for c in codes:
        lines.append("/// [%s] %s：%s" % (c["id"], c["title"], c["msg"]))
        lines.append('pub const %s: &str = "%s";' % (ident(c["id"]), c["id"]))
    lines += [
        "",
        "/// 全部错误码，按 id 升序（码表里就是按 id 排序的）。",
        "pub const ALL: &[&str] = &[",
    ]
    for c in codes:
        lines.append("    %s," % ident(c["id"]))
    lines += [
        "];",
        "",
        "/// 码总数（与 `errors/codes.sml` 的 `count` 对齐）。",
        "pub const COUNT: usize = %d;" % len(codes),
        "",
    ]
    return "\n".join(lines)


def js_src(codes):
    lines = [
        "// SPDX-License-Identifier: MulanPSL-2.0",
        "// %s。" % GEN_NOTE,
        "// 唯一事实来源是 errors/codes.sml（见 errors/README.md）。",
        "",
    ]
    for c in codes:
        lines.append("/// [%s] %s：%s" % (c["id"], c["title"], c["msg"]))
        lines.append('export const %s = "%s";' % (ident(c["id"]), c["id"]))
    lines += [
        "",
        "/// 全部错误码，按 id 升序。",
        "export const ALL = Object.freeze([",
    ]
    for c in codes:
        lines.append("  %s," % ident(c["id"]))
    lines += [
        "]);",
        "",
        "export const COUNT = %d;" % len(codes),
        "",
    ]
    return "\n".join(lines)


def c_src(codes):
    lines = [
        "/* SPDX-License-Identifier: MulanPSL-2.0 */",
        "/* %s。" % GEN_NOTE,
        " * 唯一事实来源是 errors/codes.sml（见 errors/README.md）。",
        " *",
        " * 纯 C 与 C++ 实现都 include 本文件；Lua 侧走 C-ABI，用到的码同样出自这里。",
        " * 只提供宏，不提供查找函数 —— 码是编译期常量，运行时不需要表。",
        " */",
        "#ifndef SML_CODES_H",
        "#define SML_CODES_H",
        "",
    ]
    for c in codes:
        lines.append("/* [%s] %s：%s */" % (c["id"], c["title"], c["msg"]))
        lines.append('#define SML_%s "%s"' % (ident(c["id"]), c["id"]))
    lines += [
        "",
        "#define SML_CODES_COUNT %d" % len(codes),
        "",
        "#endif /* SML_CODES_H */",
        "",
    ]
    return "\n".join(lines)


def main():
    check_only = "--check" in sys.argv
    binary = gen_json.find_smltools()
    if not binary:
        print("!! 找不到 smltools：设 SMLTOOLS_BIN 或先 `cargo build --release -p smltools`")
        return 1
    reg = gen_json.load_registry(binary)
    errs = gen_json.check(reg)
    if errs:
        print("!! 登记表校验失败：")
        for e in errs:
            print("   -", e)
        return 1

    codes = reg["codes"]
    codes = sorted(codes, key=lambda c: c["id"])  # 按 id 排序，与书写顺序无关

    targets = [
        (RUST_OUT, rust_src(codes)),
        (JS_OUT, js_src(codes)),
        (C_OUT, c_src(codes)),
    ]

    if check_only:
        bad = []
        for path, want in targets:
            rel = os.path.relpath(path, ROOT)
            if not os.path.exists(path):
                bad.append("%s：缺失（请跑 python errors/gen_codes.py）" % rel)
                continue
            with open(path, "r", encoding="utf-8", newline="") as f:
                got = f.read()
            # 行尾差异不算漂移（Windows 检出可能是 CRLF）
            if got.replace("\r\n", "\n") != want:
                bad.append("%s：与码表不一致（请跑 python errors/gen_codes.py）" % rel)
        if bad:
            print("!! 错误码产物已漂移：")
            for b in bad:
                print("   -", b)
            return 1
        print("校验通过：%d 条码，3 份产物均与码表一致（--check 模式，未写文件）" % len(codes))
        return 0

    for path, want in targets:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        # newline="" + 显式 \n：产物一律 LF，避免 Windows 检出把生成物写成 CRLF
        # 之后 --check 每次都说「漂移」。
        with open(path, "w", encoding="utf-8", newline="") as f:
            f.write(want)
        print("已生成 %s" % os.path.relpath(path, ROOT))
    print("共 %d 条码" % len(codes))
    return 0


if __name__ == "__main__":
    sys.exit(main())
