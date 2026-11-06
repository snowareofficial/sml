# SPDX-License-Identifier: MulanPSL-2.0
# `.ambr` cross-end vectors: read + write round-trip.
# `.ambr` 跨端对拍：读 + 写往返。
#
# Run / 运行: python amberice/bindings/python/test_ambr.py
# Needs a built cdylib; set AMBR_LIB to its full path or put it on the loader path.
# 需要已构建的 cdylib；用 AMBR_LIB 指路径，或放进库搜索路径。

from __future__ import annotations

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ambr  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
VECTORS = os.path.join(ROOT, "tests", "vectors", "amberice-container.v1.json")


def main() -> int:
    with open(VECTORS, encoding="utf-8") as f:
        cases = json.load(f)["cases"]

    caps = ambr.capabilities()
    print(f"capabilities={caps} (read={bool(caps & ambr.CAP_READ)}, write={bool(caps & ambr.CAP_WRITE)})")

    read_ok = rt_ok = 0
    failures = 0
    for c in cases:
        data = bytes.fromhex(c["ambr_hex"])
        try:
            if ambr.load_text(data) == c["canonical_text"]:
                read_ok += 1
            else:
                failures += 1
                print(f"FAIL  {c['id']}  read mismatch / 读不一致")
        except ambr.AmbrError as e:
            failures += 1
            print(f"FAIL  {c['id']}  read: {e}")

        if not caps & ambr.CAP_WRITE:
            continue
        try:
            if ambr.load_text(ambr.pack_text(c["canonical_text"])) == c["canonical_text"]:
                rt_ok += 1
            else:
                failures += 1
                print(f"FAIL  {c['id']}  round-trip mismatch / 往返不一致")
        except ambr.AmbrError as e:
            failures += 1
            print(f"FAIL  {c['id']}  pack: {e}")

    total = len(cases)
    if caps & ambr.CAP_WRITE:
        print(f"{read_ok}/{total} read, {rt_ok}/{total} round-trip")
    else:
        print(f"{read_ok}/{total} read (read-only build / 只读构建)")

    return 0 if failures == 0 and read_ok == total and (not caps & ambr.CAP_WRITE or rt_ok == total) else 1


if __name__ == "__main__":
    raise SystemExit(main())
