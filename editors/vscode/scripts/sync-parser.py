#!/usr/bin/env python3
"""把仓库的 SML JS 实现同步到扩展目录（打包前必须运行）。

背景：
  桥接层 src/sml-parse.mjs 需要复用 ../../../js/sml.mjs 以保证「插件与语言
  实现行为一致」。但 VSIX 只包含扩展目录内的文件——跨目录 import 的模块
  **不会被打进包**，装到别的机器上会因找不到模块而完全失效。

  因此在打包前把解析器复制到 src/vendor/ 下，桥接层改为引用该副本。

  解析器不止一个文件：`js/sml.mjs` 末尾 import 了 `./src/dump.mjs`（dump 与解析
  同包不同层）。所以同步要**连同 `js/src/` 一起**搬到 `vendor/src/`，保持相对路径
  原样 —— 只搬 sml.mjs 会让副本在加载时 ERR_MODULE_NOT_FOUND。

  代价：副本会与上游不同步。故打包脚本必须自动执行本同步，且 sync 会校验
  内容一致（若检测到 import 仍指向外部，会直接报错退出）。

用法：
  python scripts/sync-parser.py
"""
import hashlib
import shutil
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]          # sml/
EXT = Path(__file__).resolve().parents[1]           # editors/vscode/
SRC = ROOT / "js" / "sml.mjs"
SRC_SUB = ROOT / "js" / "src"                       # sml.mjs 里 ./src/… 的子模块
DST_DIR = EXT / "src" / "vendor"
DST = DST_DIR / "sml.mjs"
BRIDGE = EXT / "src" / "sml-parse.mjs"


def _import_lines(path: Path):
    text = path.read_text(encoding="utf-8")
    return [
        ln for ln in text.splitlines()
        if ln.lstrip().startswith("import") or "from \"" in ln or "from '" in ln
    ]


def main() -> int:
    if not SRC.is_file():
        print(f"错误：找不到解析器实现 {SRC}")
        return 1

    DST_DIR.mkdir(parents=True, exist_ok=True)
    shutil.copy2(SRC, DST)
    print(f"已同步: {SRC.relative_to(ROOT)} -> {DST.relative_to(EXT)}")

    # 子模块：保持 ./src/ 相对路径不变，故落到 vendor/src/ 下同名文件。
    n_sub = 0
    if SRC_SUB.is_dir():
        (DST_DIR / "src").mkdir(parents=True, exist_ok=True)
        for f in sorted(SRC_SUB.glob("*.mjs")):
            shutil.copy2(f, DST_DIR / "src" / f.name)
            n_sub += 1
            print(f"已同步: {f.relative_to(ROOT)} -> {(DST_DIR / 'src' / f.name).relative_to(EXT)}")
    if n_sub == 0:
        print("提示：js/src/ 下没有子模块（解析器是单文件）")

    # 校验桥接层引用的是副本，而非扩展目录之外的文件。
    # 只检查**真正的 import 语句**：注释里会提到旧路径用于说明原因，不能误判。
    bridge_src = BRIDGE.read_text(encoding="utf-8")
    if any("../../../js/sml.mjs" in ln for ln in _import_lines(BRIDGE)):
        print(
            "错误：桥接层仍 import 扩展目录之外的 ../../../js/sml.mjs。\n"
            "      该文件不会被打进 VSIX，装到别的机器上会找不到模块。\n"
            "      请把 import 改为 './vendor/sml.mjs'。"
        )
        return 1
    if "./vendor/sml.mjs" not in bridge_src:
        print("警告：桥接层未发现 './vendor/sml.mjs' 引用，请确认 import 路径。")

    # 副本自身也不许指回扩展目录之外（`./src/…` 在 vendor 内，合法；`../` 越界即错）。
    escape = []
    for f in sorted(DST_DIR.rglob("*.mjs")):
        for ln in _import_lines(f):
            if "../" in ln and "./src/" not in ln:
                escape.append(f"{f.relative_to(EXT)}: {ln.strip()}")
    if escape:
        print("错误：副本里的 import 越出扩展目录，装到别的机器会失效：")
        for e in escape:
            print("      " + e)
        return 1

    h = hashlib.sha256(DST.read_bytes()).hexdigest()[:16]
    print(f"副本 sha256 前 16 位: {h}（{n_sub} 个子模块）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
