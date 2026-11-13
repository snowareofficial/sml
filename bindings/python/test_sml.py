# SPDX-License-Identifier: MulanPSL-2.0
# SML Python 绑定自测。
# 仓库：https://gitee.com/snoware/sml
# @version 由 sml_version() 提供（不在此写死包版本）

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

    # 10. 诊断出口（P7）：带码带行号、不阻断解析、值树与 loads 相同
    dup, diags = sml.loads_with_diags("a: 1\na: 2\n")
    ok &= check("10 重复键 -> 1 条 W-PARSE-001 @ line 2",
                len(diags) == 1 and diags[0].code_str == "W-PARSE-001"
                and diags[0].line == 2 and diags[0].severity == 0, repr(diags))
    ok &= check("10 值树不变（仍是数组提升）", dup == {"a": [1, 2]}, repr(dup))
    ok &= check("10 与 loads 逐字节相同", dup == loads("a: 1\na: 2\n"), repr(dup))

    clean, no_diags = sml.loads_with_diags("k: 1\n")
    ok &= check("10 干净文档 -> 零诊断", clean == {"k": 1} and no_diags == [],
                repr(no_diags))

    try:
        sml.loads_with_diags("bad { a: 1")
        ok &= check("10 语法错误仍抛 SmlError", False, "未抛异常")
    except SmlError as e:
        ok &= check("10 语法错误 code_str 非空", bool(e.code_str), repr(e.code_str))

    # 连跑 200 次：诊断表每次都整体释放（漏释放会在这里露出来）
    for _ in range(200):
        sml.loads_with_diags("a: 1\na: 2\n")
    ok &= check("10 200x loads_with_diags 不崩溃", True)

    # 11. 完整选项（P7 第二片）：内置契约表 + env 注入
    with sml.Builtin() as bt:
        ok &= check("11 空表 len==0", len(bt) == 0)
        built = (bt.add_field("tls", "cert", "str", True)
                 and bt.add_field("tls", "port", "int")
                 and bt.set_range("tls", "port", 1, 65535)
                 and bt.add_field("tls", "mode", "enum")
                 and bt.add_enum("tls", "mode", "strict"))
        ok &= check("11 建表 -> 1 份契约", built and len(bt) == 1, str(len(bt)))
        ok &= check("11 未知类型被拒（表不动）", not bt.add_field("tls", "bad", "image"))

        src = ("@feature enable type.weak\n"
               "srv {\n  @weakbind tls\n"
               "  web { cert: 1 port: 99999 mode: nope }\n}\n"
               "@weakbind nosuch\nother { x: 1 }\n")
        obj2, diags2 = sml.loads_full(src, flags=sml.SML_F_BASIC | sml.SML_F_WEAK,
                                      builtins=bt)
        weak = [d for d in diags2 if d.code_str == "W-CONTRACT-001"]
        ok &= check("11 内置表 -> 4 条 W-CONTRACT-001", len(weak) == 4, repr(diags2))
        ok &= check("11 软校验只读：cert 仍是 int 1",
                    obj2["srv"]["web"]["cert"] == 1, repr(obj2))
        ok &= check("11 值树与 loads 相同",
                    sml.loads(src, flags=sml.SML_F_BASIC | sml.SML_F_WEAK) == obj2)

    env_obj, _ = sml.loads_full("k: $env.SML_PY_PROBE\n", env={"SML_PY_PROBE": "prod"})
    ok &= check("11 env 注入 -> k=prod", env_obj == {"k": "prod"}, repr(env_obj))
    ok &= check("11 env 不改进程环境", "SML_PY_PROBE" not in os.environ)

    try:
        sml.loads_full("@feature enable type.weak\n@weakbind tls\ns { x: 1 }\n", flags=0)
        ok &= check("11 基线不含 type.weak -> E-FEATURE-001", False, "未抛异常")
    except SmlError as e:
        ok &= check("11 基线不含 type.weak -> E-FEATURE-001",
                    e.code_str == "E-FEATURE-001", repr(e.code_str))

    # 12. 弱声明三表（P7 第四片）：都不进主树，从 loads_full_lists 取出
    wsrc = ("@feature enable type.weak\n"
            "srv {\n  @weakbind tls\n  web { cert: \"x\" }\n"
            "  @weakdata limits { max: 10 }\n}\n"
            "@weakdoc base\n")
    wobj, wdiags, wl = sml.loads_full_lists(wsrc, flags=sml.SML_F_BASIC | sml.SML_F_WEAK)
    ok &= check("12 三表计数 1/1/1",
                (len(wl.binds), len(wl.data), len(wl.docs)) == (1, 1, 1),
                repr((len(wl.binds), len(wl.data), len(wl.docs))))
    if wl.binds:
        b = wl.binds[0]
        ok &= check("12 bind 的 path/name/line",
                    b.path == "srv.web" and b.name == "tls" and b.line > 0, repr(b))
    if wl.data:
        d = wl.data[0]
        ok &= check("12 weakdata 归属 srv/limits 且载荷可读",
                    d.path == "srv" and d.name == "limits" and d.value == {"max": 10},
                    repr(d))
    ok &= check("12 profile 在顶层（path 为空串）",
                bool(wl.docs) and wl.docs[0].path == "" and wl.docs[0].name == "base",
                repr(wl.docs))
    ok &= check("12 主树无痕迹", "limits" not in wobj["srv"], repr(wobj))
    for _ in range(100):
        sml.loads_full_lists(wsrc, flags=sml.SML_F_BASIC | sml.SML_F_WEAK)
    ok &= check("12 100x loads_full_lists 不崩溃", True)

    # 13. 聚合结果（parse_all）：契约映射 + 契约表捕获 + 多错误，一次拿全
    csrc, _, _, _, cmap, contracts = sml.parse_all(
        '@contract tls { cert: str }\nsrv { @is tls\n  cert: "x"\n}\n',
        flags=sml.SML_F_BASIC | sml.SML_F_CONTRACT)
    ok &= check("13 契约映射 srv -> tls",
                [(c.path, c.name) for c in cmap] == [("srv", "tls")], repr(cmap))
    ok &= check("13 契约表已捕获（1 份）", len(contracts) == 1, str(len(contracts)))
    # 闭环：捕获的表直接当内置表注入
    _, wdiags, _ = sml.loads_full_lists(
        "@feature enable type.weak\n@weakbind nosuch\ns { x: 1 }\n",
        flags=sml.SML_F_BASIC | sml.SML_F_WEAK, builtins=contracts)
    ok &= check("13 闭环：注入捕获的表后报 decl.unknown_contract",
                [d.code_str for d in wdiags] == ["W-CONTRACT-001"], repr(wdiags))
    contracts.close()

    bad = "a { @is No1\n  x: 1 }\nb { @is No2\n  y: 2 }\n"
    ba = sml.parse_all(bad, flags=sml.SML_F_BASIC | sml.SML_F_CONTRACT)
    ok &= check("13 多错误：2 条且 severity=1、带行号",
                [(d.code_str, d.line, d.severity) for d in ba.errors]
                == [("E-CONTRACT-001", 1, 1), ("E-CONTRACT-001", 3, 1)], repr(ba.errors))
    ok &= check("13 多错误：值树是尽力而为的部分结果",
                ba.value == {"a": {"x": 1}, "b": {"y": 2}}, repr(ba.value))

    try:
        sml.parse_all(bad, flags=sml.SML_F_BASIC | sml.SML_F_CONTRACT,
                      want=sml.SML_WANT_DIAGS)
        ok &= check("13 未请求 ERRORS -> 仍抛 SmlError", False, "未抛异常")
    except SmlError as e:
        ok &= check("13 未请求 ERRORS -> 仍抛 SmlError",
                    e.code_str == "E-CONTRACT-001", repr(e.code_str))

    for _ in range(100):
        sml.parse_all(wsrc, flags=sml.SML_F_BASIC | sml.SML_F_WEAK)
    ok &= check("13 100x parse_all 不崩溃", True)

    # 14. 文件入口（load_file_full）：展开 include + 诊断 + 弱表
    with tempfile.TemporaryDirectory(prefix="smlpy_") as d:
        with open(os.path.join(d, "sub.sml"), "w", encoding="utf-8", newline="\n") as f:
            f.write("k: 1\n")
        main = os.path.join(d, "main.sml")
        with open(main, "w", encoding="utf-8", newline="\n") as f:
            f.write('@feature enable type.weak\ninclude "sub.sml"\n@weakdoc base\n')
        # flags 是特性**上限**：`type.weak` 是 opt-in，必须显式允许
        fobj, fdiags, fwl = sml.load_file_full(
            main, flags=sml.SML_F_BASIC | sml.SML_F_WEAK)
        ok &= check("14 文件入口：include 已展开", fobj == {"k": 1}, repr(fobj))
        ok &= check("14 文件入口：弱表带 @weakdoc",
                    [x.name for x in fwl.docs] == ["base"], repr(fwl.docs))
        ok &= check("14 文件入口：干净文件 -> 零诊断", fdiags == [], repr(fdiags))

        try:
            sml.load_file_full(os.path.join(d, "nope.sml"))
            ok &= check("14 文件不存在 -> E-IO-001", False, "未抛异常")
        except SmlError as e:
            ok &= check("14 文件不存在 -> E-IO-001",
                        e.code_str == "E-IO-001", repr(e.code_str))

    print("\n" + ("ALL OK" if ok else "SOME FAILED"))
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
