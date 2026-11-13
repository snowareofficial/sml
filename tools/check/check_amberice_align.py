#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# SPDX-License-Identifier: MulanPSL-2.0
"""amberice ↔ swsml 对齐守护（纯标准库、只读、零依赖）。

背景：`docs/amberice/ref/ports.md` / `decisions.md` 里多处写着「v5 发布时要手动跟一手」，
而 `rust/amberice/src/container.rs` 的 `EDITION`、Lua 的 `EDITION`、冻结向量的头字节
是**三份手抄副本**。手工对齐迟早漂：本脚本把这些「手抄 → 手工同步」的点变成
**一条可跑命令**，任一副本与单一事实来源不符即退出码 1。

判据（详见 `docs/amberice/ref/swsml-alignment.md`）：
  1. EDITIONS 最后一个 `Supported` 的 major ⇔ `Edition::CURRENT`
     ⇔ `amberice::container::EDITION` ⇔ Lua `EDITION` ⇔ 容器向量头 `04 00`；
  2. `c/sml_rs.h` 的 `SML_F_*` 20 位（0..19）齐全，且五门绑定各有一套 0..19 的位常量；
  3. `amberice-ffi` 的 `AMBR_CAP_READ/WRITE` 与各端副本一致（能力位是 amberice 自己的，
     **不是** swsml 的 FEATURES；刻意分开，见对照表）；
  4. 错误码条目数在 `errors/codes.sml` 与三份生成物里一致。

用法：`python tools/check/check_amberice_align.py`（CI 见 `.github/workflows/ci.yml` 的 guards job）
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FAILURES = []


def read(rel):
    return (ROOT / rel).read_text(encoding="utf-8", errors="replace")


def check(label, ok, detail=""):
    status = "ok" if ok else "FAIL"
    print(f"  {status:4} {label}" + (f"  {detail}" if detail else ""))
    if not ok:
        FAILURES.append(label)


def one(pattern, text, label, cast=str):
    m = re.search(pattern, text, re.MULTILINE | re.DOTALL)
    check(label, m is not None, "" if m else f"未匹配 {pattern[:48]}")
    return cast(m.group(1)) if m else None


# ---------------------------------------------------------------------------
# 1. edition 单一来源链
# ---------------------------------------------------------------------------
feature_src = read("rust/sml-feature/src/lib.rs")
block = re.search(r"pub static EDITIONS:[^=]*=\s*&\[(.*?)\n\];", feature_src, re.S)
rows = re.findall(r"EditionRow\s*\{(.*?)\}", block.group(1), re.S) if block else []
supported_majors = []
for row in rows:
    m = re.search(r"edition:\s*Edition::(?:V(\d+)|new\(\s*(\d+)\s*,)", row)
    status = re.search(r"status:\s*EditionStatus::(\w+)", row)
    if m and status and status.group(1) == "Supported":
        supported_majors.append(int(m.group(1) or m.group(2)))
check("EDITIONS 表可解析且有 Supported 行", bool(supported_majors),
      f"Supported={supported_majors}")
major = max(supported_majors) if supported_majors else -1

cur = one(r"pub const CURRENT:\s*Edition\s*=\s*Edition::V(\d+)", feature_src,
          "Edition::CURRENT 与最后一个 Supported 一致", int)
check("CURRENT 对齐 EDITIONS", cur == major, f"CURRENT={cur} EDITIONS.max={major}")

rust_ed = one(r"pub const EDITION:\s*u16\s*=\s*0x([0-9A-Fa-f]{4})\s*;",
              read("rust/amberice/src/container.rs"),
              "amberice::container::EDITION 可读", lambda s: int(s, 16))
check("Rust EDITION 对齐 swsml edition", rust_ed == major,
      f"amberice=0x{rust_ed:04x} swsml major={major}" if rust_ed is not None else "")

lua_ed = one(r"local EDITION\s*=\s*0x([0-9A-Fa-f]{4})", read("lua/lib/ambr_pack.lua"),
             "Lua EDITION 可读", lambda s: int(s, 16))
check("Lua EDITION 对齐 swsml edition", lua_ed == major,
      f"lua=0x{lua_ed:04x} swsml major={major}" if lua_ed is not None else "")

vectors = json.loads(read("tests/vectors/amberice-container.v1.json"))
cases = [c for c in vectors.get("cases", []) if c.get("ambr_hex")]
bad_hex = []
for c in cases:
    h = c["ambr_hex"]
    # 头 4 字节 "AMBR"，随后是 **little-endian u16** edition（`04 00` ⇒ 4）
    raw = bytes.fromhex(h[:12]) if len(h) >= 12 else b""
    if len(raw) < 6 or raw[:4] != b"AMBR" or int.from_bytes(raw[4:6], "little") != major:
        bad_hex.append(c.get("id", "?"))
check("容器向量头部 edition 对齐", bool(cases) and not bad_hex,
      f"{len(cases)} 例；不符={bad_hex[:4]}")

# ---------------------------------------------------------------------------
# 2. 特性位表：C-ABI 20 位 + 五门绑定各 20 位
# ---------------------------------------------------------------------------
c_hdr = read("c/sml_rs.h")
c_bits = [int(n) for n in re.findall(r"#define\s+SML_F_\w+\s+\(1u\s*<<\s*(\d+)\)", c_hdr)]
check("c/sml_rs.h 特性位恰好 0..19", sorted(c_bits) == list(range(20)),
      f"count={len(c_bits)} bits={sorted(c_bits)}")

BINDINGS = [
    ("Python", "bindings/python/sml.py", r"^\s*SML_F_\w+\s*=\s*1\s*<<\s*(\d+)"),
    ("Ruby", "bindings/ruby/lib/sml.rb", r"^\s*SML_F_\w+\s*=\s*1\s*<<\s*(\d+)"),
    ("Go", "bindings/go/sml.go", r"^\s*SMLF_\w+\s*=\s*1\s*<<\s*(\d+)"),
    ("Java", "bindings/java/src/main/java/sml/Sml.java",
     r"public static final int SML_F_\w+\s*=\s*1\s*<<\s*(\d+)\s*;"),
    ("C#", "bindings/csharp/Sml.cs",
     r"public const (?:int|uint) F_\w+\s*=\s*1u?\s*<<\s*(\d+)\s*;"),
]
for name, path, pattern in BINDINGS:
    bits = [int(n) for n in re.findall(pattern, read(path), re.MULTILINE)]
    check(f"{name} 绑定特性位 0..19 齐全", sorted(bits) == list(range(20)),
          f"count={len(bits)} bits={sorted(bits)}")

# ---------------------------------------------------------------------------
# 3. amberice-ffi 能力位（amberice 自己的，不是 swsml FEATURES）
# ---------------------------------------------------------------------------
ffi = read("rust/amberice-ffi/src/lib.rs")
cap_r = one(r"pub const AMBR_CAP_READ:\s*u32\s*=\s*1\s*<<\s*(\d+)", ffi,
            "AMBR_CAP_READ 可读", int)
cap_w = one(r"pub const AMBR_CAP_WRITE:\s*u32\s*=\s*1\s*<<\s*(\d+)", ffi,
            "AMBR_CAP_WRITE 可读", int)
check("AMBR_CAP_* 位定义", (cap_r, cap_w) == (0, 1), f"read={cap_r} write={cap_w}")

c_caps = read("c/ambr_write.h")
check("C 写入端 caps 值对齐",
      "SML_AMBR_CAP_READ 1u" in c_caps and "SML_AMBR_CAP_WRITE 2u" in c_caps)
js_caps = read("js/ambr_write.mjs")
check("JS 写入端 caps 值对齐",
      "CAP_READ = 1 << 0" in js_caps and "CAP_WRITE = 1 << 1" in js_caps)
lua_caps = read("lua/lib/ambr_write.lua")
check("Lua 写入端 caps 值对齐",
      "M.CAP_READ = 1" in lua_caps and "M.CAP_WRITE = 2" in lua_caps)

# ---------------------------------------------------------------------------
# 4. 错误码单一来源（swsml 侧生成物；amberice 刻意不复制码表）
# ---------------------------------------------------------------------------
codes = read("errors/codes.sml")
n_codes = len(re.findall(r"^\s*\{ id: [^}]*impls:", codes, re.MULTILINE))
counts = {
    "rust/sml-codes/src/codes.rs": r"pub const COUNT:\s*usize\s*=\s*(\d+)",
    "js/sml-codes.mjs": r"export const COUNT\s*=\s*(\d+)",
    "c/sml_codes.h": r"#define SML_CODES_COUNT\s+(\d+)",
}
check("errors/codes.sml 条目可读", n_codes > 0, f"count={n_codes}")
for path, pattern in counts.items():
    got = one(pattern, read(path), f"{path} COUNT 可读", int)
    check(f"{path} COUNT 对齐码表", got == n_codes, f"生成物={got} 码表={n_codes}")

# ---------------------------------------------------------------------------
# 5. AMBR 错误码域：码表 ⇔ Rust 错误类型 ⇔ 各端 wrapper 文本
# ---------------------------------------------------------------------------
ambr_codes = set(re.findall(r"\{ id: (E-AMBR-\d{3}) domain: AMBR", read("errors/codes.sml")))
rust_codes = set(re.findall(r'"(E-AMBR-\d{3})"', read("rust/amberice/src/error.rs")))
check("AMBR 码表与 Rust 错误类型一一对应", ambr_codes and ambr_codes == rust_codes,
      f"码表={sorted(ambr_codes)} Rust={sorted(rust_codes)}")

wrappers = [
    "rust/amberice-ffi/src/lib.rs", "cpp/ambr_write.cpp", "js/ambr_write.mjs",
    "lua/lib/ambr_write.lua", "bindings/csharp/Amberice.cs",
]
bare = []
for path in wrappers:
    for m in re.finditer(r'"AMBR: (?!\[E-AMBR-\d{3}\])', read(path)):
        bare.append(f"{path}@{m.start()}")
check("wrapper 的 AMBR 错误全部带码", not bare, f"未带码={bare[:4]}")

en_msgs = json.loads(read("errors/msg.en.json"))
missing_en = sorted(c for c in ambr_codes if c not in en_msgs)
check("AMBR 码有英文 title/msg（i18n）", not missing_en, f"缺英文={missing_en}")

# ---------------------------------------------------------------------------
# 6. §20 分组命名空间与预留位（代码 ⇔ 规范）
# ---------------------------------------------------------------------------
hash_src = read("rust/amberice/src/hash.rs")
spec = read("docs/amberice/spec/format.md")

# 组内 16 个号全部登记：3 个实现 + 13 个预留（含具名 / 转义 / 私有 / 未分配槽）
check("哈希组常量 0x30..0x3F 已声明",
      "pub const HASH_GROUP_FIRST: u8 = 0x30;" in hash_src
      and "pub const HASH_GROUP_LAST: u8 = 0x3F;" in hash_src)
check("0x33 国密 SM 抗量子位已登记",
      "0x33 => Some(HashAlgo::SmPqcDigest)" in hash_src
      and "HashAlgo::SmPqcDigest => 0x33" in hash_src)
check("多预留：0x34 通用 PQC / 0x35–0x3D 未分配槽",
      "0x34 => Some(HashAlgo::PqcDigest)" in hash_src
      and "0x35..=0x3D => Some(HashAlgo::Reserved(id))" in hash_src)
check("组内末两号：0x3E 转义 / 0x3F 私有",
      "pub const HASH_ESCAPE: u8 = 0x3E;" in hash_src
      and "pub const HASH_PRIVATE: u8 = 0x3F;" in hash_src
      and "HASH_ESCAPE => Some(HashAlgo::ExtendedNamespace)" in hash_src)
check("所有预留位统一明确拒绝（不退回 SHA-256）",
      "other => Err(HashError::NotImplemented(other.id()))" in hash_src
      and "pub const fn is_reserved(self) -> bool" in hash_src)
check("组边界仍是 Unknown",
      "assert_eq!(digest_by_id(0x2F, b\"x\"), Err(HashError::Unknown(0x2F)));" in hash_src
      and "assert_eq!(digest_by_id(0x40, b\"x\"), Err(HashError::Unknown(0x40)));" in hash_src)

# 规范同步：分组命名空间 + 各组预留位
check("规范 §20.3 分组命名空间已落地",
      "### 20.3 分组命名空间与预留编号" in spec
      and "#### 20.3.1 组 = 命名空间" in spec
      and "#### 20.3.3 组内末两号：转义（多级元组）与私有" in spec)
check("规范里哈希组 16 号明细齐全",
      "| `0x35–0x3D` | 组内未分配槽（9 个）" in spec
      and "| `0x3E` | **扩展命名空间转义**" in spec
      and "| `0x3F` | 私有 / 实验" in spec)
check("其它组也留了国密 PQC 位",
      "`0x21` 国密 PQC 对称（预留）" in spec
      and "`0x42` 国密 PQC 签名（预留）" in spec
      and "`0x62` 国密 PQC 签名（预留）" in spec
      and "`0x83` 国密 PQC KEM（预留）" in spec
      and "`0xA2` 国密 PQC 混合（预留）" in spec)
# 区间重叠的处置（用户 2026-11-13 裁决 KDF 迁走）也要一直被登记着。
check("规范登记了 KDF 迁移（0xB0–0xBF，旧 0x51 只读兼容）",
      "既有区间重叠已裁决（2026-11-13）" in spec
      and "`0xB0–0xBF`" in spec
      and "`0xB1 Argon2id`" in spec
      and "KDF 语境下 `0x51` **只读兼容**" in spec)

# 多级转义元组：[0x3E, ns_version, sub…]，版本号在前、每级 2 字节、最多 4 级。
check("代码实现多级 hash 元组（HashPath）",
      "pub struct HashPath" in hash_src
      and "pub const MAX_HASH_SUBS: usize = 4;" in hash_src
      and "pub ns_version: u8," in hash_src
      and "pub subs: Vec<u16>," in hash_src)
check("manifest 能往返多级 hash_id",
      "pub hash_path: Option<crate::hash::HashPath>," in read("rust/amberice/src/manifest.rs")
      and "crate::hash::HashPath::from_value(v)" in read("rust/amberice/src/manifest.rs"))
# 已经补了 hash_id 分派的端：代码里必须真的有「解 manifest + 按号分派」的实现。
c_src = read("c/ambr.c")
js_src = read("js/ambr.mjs")
check("C 端按 hash_id 分派（manifest + digest_check）",
      "static int manifest_hash_id(" in c_src
      and "digest_check(out->hash_id" in c_src
      and "sm3_digest(" in c_src
      and "b3_digest(" in c_src)
cpp_src = read("cpp/ambr.cpp")
check("C++ 端按 hash_id 分派（manifest + digest_check）",
      "bool manifest_hash_id(" in cpp_src
      and "digest_check(out->hash_id" in cpp_src
      and "ambr_sm3_digest(" in cpp_src
      and "ambr_b3_digest(" in cpp_src)
lua_hash_src = read("lua/lib/ambr_hash.lua")
lua_sml_src = read("lua/lib/sml.lua")
check("Lua 端按 hash_id 分派（ambr_hash + manifest）",
      "function M.sm3(" in lua_hash_src
      and "function M.blake3(" in lua_hash_src
      and "function _ambr_manifest_hash_id(" in lua_sml_src
      and "local digest_fn = _ambr_hash_by_id(hash_id)" in lua_sml_src)
java_ambr_src = read("bindings/java/src/main/java/sml/Ambr.java")
check("Java 端按 hash_id 分派（manifest + digestFor）",
      "static byte[] sm3(" in java_ambr_src
      and "static byte[] blake3(" in java_ambr_src
      and "private static Integer manifestHashId(" in java_ambr_src
      and "digestFor(p.hashId" in java_ambr_src)
check("JS 端按 hash_id 分派（manifest + digestFnFor）",
      "function readHashId(" in js_src
      and "function digestFnFor(" in js_src
      and "digestFn(body)" in js_src)
# 多级转义的 fmt 门 + 各端「是否验过」的可观测出参
check("fmt 门已实现（HashPathRequiresFmt + 纯函数）",
      "HashPathRequiresFmt" in read("rust/amberice/src/container.rs")
      and "fn check_hash_path_fmt(" in read("rust/amberice/src/container.rs")
      and "C::HashPathRequiresFmt { .. }" in read("rust/amberice/src/error.rs"))
check("规范写明 USER 区（0xF0）与应用标识",
      "### 19.15 `USER`" in spec
      and "魔数**不可替换**" in spec
      and "BIFC" in spec and "PTCH" in spec)
check("规范写明 fmt 门",
      "`fmt` 门（D5，硬约束）" in spec and "HashPathRequiresFmt" in spec)
check("五端公开「是否验过」",
      "sml_ambr_load_ex" in read("c/ambr.h")
      and "struct AmbrInfo" in read("cpp/ambr.hpp")
      and "digest_skipped" in read("lua/lib/sml.lua")
      and "loadWithInfo" in read("bindings/java/src/main/java/sml/Ambr.java")
      and "report.skipped" in read("js/ambr.mjs"))
check("规范写明元组形态与版本号在前",
      "hash_id: [ 0x3E, 1, 0x0001 ]" in spec
      and "**第一个子号是命名空间版本号**" in spec
      and "**最多 4 级**" in spec)

print()
if FAILURES:
    print(f"{len(FAILURES)} 项未对齐：" + "、".join(FAILURES))
    sys.exit(1)
print("AMBERICE/SWSML ALIGN OK")
