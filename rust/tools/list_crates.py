# 列出 rust/ 下全部 crate 的发布状态，用于核对「哪些会发到 crates.io」。
#
# 用法：python rust/tools/list_crates.py
#
# 约定：对外只发布 sml 系（swsml / swsml-derive / sml-* / smlconv）；
# qsm 与 crystalic 是独立 workspace，其 crate 均设 publish = false（内部 / 在研）。

import os
import re
import sys

sys.stdout.reconfigure(encoding="utf-8")

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
SKIP_DIRS = {"target", "node_modules", ".git", "target_verify"}

def package_seg(text):
    m = re.search(r"^\[package\](.*?)(?=^\[|\Z)", text, re.M | re.S)
    return m.group(1) if m else None

def field(seg, key):
    m = re.search(rf"^{key}\s*=\s*(.+)$", seg, re.M)
    return m.group(1).strip().strip('"') if m else ""

rows = []
for dirpath, dirnames, filenames in os.walk(ROOT):
    dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
    if "Cargo.toml" not in filenames:
        continue
    text = open(os.path.join(dirpath, "Cargo.toml"), encoding="utf-8").read()
    seg = package_seg(text)
    if seg is None:
        continue  # 纯 workspace 清单
    name = field(seg, "name") or "<no name>"
    ver = field(seg, "version")
    publish = field(seg, "publish")
    rows.append((name, ver, publish or "true(default)", os.path.relpath(dirpath, ROOT)))

rows.sort(key=lambda r: (not r[0].startswith(("swsml", "sml")), r[0]))

print(f"{'crate':<18} {'version':<18} {'publish':<16} 路径")
print("-" * 90)
for name, ver, pub, rel in rows:
    print(f"{name:<18} {ver or '(workspace继承)':<18} {pub:<16} {rel}")

pub_yes = [r for r in rows if r[2] in ("true", "true(default)")]
pub_no = [r for r in rows if r[2] not in ("true", "true(default)")]
print(f"\n会发布: {len(pub_yes)}    不发布: {len(pub_no)}")
print("会发布:", ", ".join(r[0] for r in pub_yes))
print("不发布:", ", ".join(r[0] for r in pub_no))
