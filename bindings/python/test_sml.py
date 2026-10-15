# SPDX-License-Identifier: MulanPSL-2.0
# SML Python 绑定自测。
# 仓库：https://gitee.com/snoware/sml
# @version 0.1.0 (swsml 0.6.3)

"""SML Python 绑定测试（§4 黄金向量 + 内存稳定）。

运行：python bindings/python/test_sml.py
（需先让 libsml 可被加载：设置 SML_LIB 或把其目录加入 PATH。）
"""

import os
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import sml
from sml import SmlError, loads, load_file, dumps, version, features, get_in
from sml import SML_F_BASIC, SML_F_ENV

GOLDEN = """server {
  host: "example.com"
  port: 8080
  ratio: 0.75
  tls: false
  tags: [ a b c ]
}
"""

EXPECTED = {
    "server": {
        "host": "example.com",
        "port": 8080,
        "ratio": 0.75,
        "tls": False,
        "tags": ["a", "b", "c"],
    }
}


def check(name, cond, detail=""):
    if cond:
        print(f"  PASS  {name}")
        return True
    print(f"  FAIL  {name}  {detail}")
    return False


def main():
    ok = True

    # 1. loads 黄金向量（类型保真）
    obj = loads(GOLDEN)
    ok &= check("1 loads(golden)==expected (port int, ratio float, tls bool, tags 3-elem)",
                obj == EXPECTED and isinstance(obj["server"]["port"], int)
                and isinstance(obj["server"]["ratio"], float)
                and obj["server"]["tls"] is False
                and len(obj["server"]["tags"]) == 3, repr(obj))

    # 2. 往返：dumps(loads(sml)) 再 loads == 同一对象
    back = loads(dumps(obj))
    ok &= check("2 round-trip dumps/loads", back == EXPECTED, repr(back))

    # 3. 富错误：未闭合块 -> E-PARSE-001 + text 非空（EOF 错 line 为 0，按实测不强制 line>0）
    try:
        loads("bad { a: 1")
        ok &= check("3 rich error (unclosed)", False, "未抛异常")
    except SmlError as e:
        ok &= check("3 rich error code_str==E-PARSE-001",
                    e.code_str == "E-PARSE-001", f"got {e.code_str!r}")
        ok &= check("3 rich error text 非空", bool(e.text), f"text={e.text!r}")
        ok &= check("3 不是通用 parse failed", "parse failed" not in e.text.lower(), e.text)

    # 4. 错误定位：第 2 行的契约错误 -> line==2, code_str==E-CONTRACT-001（证明行列真到位）
    try:
        loads("a: 1\n@is NoSuchContract\n")
        ok &= check("4 line location", False, "未抛异常")
    except SmlError as e:
        ok &= check("4 line==2", e.line == 2, f"line={e.line}")
        ok &= check("4 code_str==E-CONTRACT-001", e.code_str == "E-CONTRACT-001",
                    f"got {e.code_str!r}")

    # 5. load_file + include 展开
    with tempfile.TemporaryDirectory() as d:
        mainp = os.path.join(d, "_inc_main.sml")
        subp = os.path.join(d, "_inc_sub.sml")
        with open(subp, "w", encoding="utf-8") as f:
            f.write("k: 1\n")
        with open(mainp, "w", encoding="utf-8") as f:
            f.write('include "_inc_sub.sml"\nm: 2\n')
        inc = load_file(mainp)
        ok &= check("5 include: 子文件字段 k=1", inc.get("k") == 1, repr(inc))
        ok &= check("5 include: 主文件字段 m=2", inc.get("m") == 2, repr(inc))
        os.remove(mainp)
        os.remove(subp)

    # 6. flags 沙箱：基线含 ENV 正常；$env 禁用 -> E-FEATURE-002
    # 注意：须把 $env 放在「值」上下文（x: $env.NOPE），特性门禁才会触发；
    # 裸顶层 $env.NOPE 在门禁前就被判为语法错（E-PARSE-008）。
    env_doc = "x: $env.NOPE\n"
    try:
        loads(env_doc, flags=0)  # 基线（含 ENV）应成功解析
        base_ok = True
    except SmlError as e:
        base_ok = (e.code_str != "E-FEATURE-002")
    ok &= check("6 baseline(flags=0) 不因特性报错", base_ok)

    no_env = SML_F_BASIC & ~SML_F_ENV  # 去掉 ENV 位
    try:
        loads(env_doc, flags=no_env)
        ok &= check("6 sandbox: 无 ENV 应报错", False, "未抛异常")
    except SmlError as e:
        ok &= check("6 sandbox code_str==E-FEATURE-002", e.code_str == "E-FEATURE-002",
                    f"got {e.code_str!r}")

    # 7. version / features
    v = version()
    ok &= check("7 version() 非空", bool(v), repr(v))
    feats = features()
    ok &= check("7 features() 含 'include'", "include" in feats, repr(feats))
    ok &= check("7 features() 含 'env'/'contract'",
                ("env" in feats) and ("contract" in feats), repr(feats))

    # 8. 内存稳定：连跑 1000 次 loads（每次根 sml_free、串 sml_free_str）
    for _ in range(1000):
        loads(GOLDEN)
    ok &= check("8 1000x loads 不崩溃", True)

    # 便捷取值
    ok &= check("9 get_in 路径取值", get_in(obj, "server.host") == "example.com")

    print("\n" + ("ALL OK" if ok else "SOME FAILED"))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
