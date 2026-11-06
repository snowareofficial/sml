# SPDX-License-Identifier: MulanPSL-2.0
# test_ambr.py — `.ambr` FFI 端对拍容器向量（T7 / N03，Python/ctypes）。
#
# 判据：把 fixture 的 21 个 `ambr_hex` 解成 canonical 文本，与 `canonical_text` **逐字符相同**。
# 这比「解成 Python 对象再比」更强：同时钉住容器解码与 canonical 文本发射。
#
# 运行：AMBR_LIB=/path/to/amberice.dll python bindings/python/test_ambr.py [fixture.json]

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import ambr

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


def main() -> int:
    path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        ROOT, "tests", "vectors", "amberice-container.v1.json"
    )
    with open(path, encoding="utf-8") as f:
        fx = json.load(f)

    fails = 0
    for case in fx["cases"]:
        got = ambr.load_text(bytes.fromhex(case["ambr_hex"]))
        want = case["canonical_text"]
        if got == want:
            print("  ok: %s" % case["id"])
        else:
            print("  FAIL: %s\n    期望 %r\n    实得 %r" % (case["id"], want, got))
            fails += 1

    print("[ambr] %d 例，%d 个失败" % (len(fx["cases"]), fails))
    print("[ambr] ALL OK" if fails == 0 else "[ambr] FAILED")
    return 0 if fails == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
