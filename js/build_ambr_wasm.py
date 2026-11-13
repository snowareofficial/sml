# -*- coding: utf-8 -*-
"""把 amberice-ffi 编成 wasm，产物落到 js/build/amberice_ffi.wasm。

为什么是 `cargo build` 而不是 `wasm-pack`：
    `wasm-pack build` 在这里直接拒绝执行 —— 它要求 crate 依赖 `wasm-bindgen`

        Error: Ensure that you have "wasm-bindgen" as a dependency in your Cargo.toml file

    而 `amberice-ffi` 是纯 C-ABI 的 cdylib（导出 `ambr_pack_text` / `ambr_text` /
    `ambr_caps` / `ambr_alloc` …，零 `wasm-bindgen` 注解）。给它加 wasm-bindgen
    只为换一层胶水，既多一个依赖，也和 `site/static/sml-rs.js` 已确立的做法相反
    （那份也是裸 `WebAssembly.instantiate` + C-ABI，浏览器与 Node 共用）。

    ⇒ 直接用 cargo 出 wasm32-unknown-unknown 的 cdylib：模块无任何 import，
      `memory` 与全部 `ambr_*` 符号都是普通 wasm export，宿主裸实例化即可。

用法::

    python js/build_ambr_wasm.py              # 构建（缺 wasm32 目标会明确报错）
    python js/build_ambr_wasm.py --skip-build # 只把已有产物拷到 js/build/
    python js/build_ambr_wasm.py --check      # 只检查 js/build/ 里的产物在不在

产物一律进 `js/build/`（已 gitignore），不落源码目录。
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUST = os.path.join(ROOT, "rust")
OUT_DIR = os.path.join(ROOT, "js", "build")
OUT = os.path.join(OUT_DIR, "amberice_ffi.wasm")
TARGET = "wasm32-unknown-unknown"


def fail(msg):
    print("[!] " + msg)
    return 1


def target_dir():
    """`cargo metadata` 的 target_directory —— 用户的 ~/.cargo/config.toml 可能把
    target-dir 指到仓库外（本机就指到了仓库外），不能假设 rust/target。"""
    try:
        raw = subprocess.check_output(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            cwd=RUST, stderr=subprocess.STDOUT,
        )
        return json.loads(raw.decode("utf-8", "replace"))["target_directory"]
    except Exception as exc:  # noqa: BLE001 - 报错要带上原因，别静默走默认路径
        print("[!] 取 target_directory 失败：%s" % exc)
        return os.path.join(RUST, "target")


def build():
    # `allow-32bit`：wasm32 是 32 位目标，`sml-value` 默认拒编（u64→usize 截断风险）。
    # 只在这条 wasm 构建命令里显式开，native 构建一律不带。
    # ｜ wasm32 needs the explicit 32-bit risk opt-in; native builds never pass it.
    cmd = ["cargo", "build", "-p", "amberice-ffi", "--features", "sml,allow-32bit",
           "--release", "--target", TARGET]
    print("[..] " + " ".join(cmd) + "   (cwd=rust)")
    rc = subprocess.call(cmd, cwd=RUST)
    if rc != 0:
        print("[!] cargo 构建失败（rc=%d）。缺目标时先跑："
              "rustup target add %s" % (rc, TARGET))
        return rc
    return 0


def publish():
    src = os.path.join(target_dir(), TARGET, "release", "amberice_ffi.wasm")
    if not os.path.isfile(src):
        return fail("找不到 wasm 产物：%s" % src)
    os.makedirs(OUT_DIR, exist_ok=True)
    shutil.copyfile(src, OUT)
    data = open(OUT, "rb").read()
    print("[ok] %s  %d bytes  sha256=%s" % (
        os.path.relpath(OUT, ROOT).replace("\\", "/"), len(data),
        hashlib.sha256(data).hexdigest()[:16]))
    return 0


def check():
    if not os.path.isfile(OUT):
        return fail("缺 %s —— 先跑 python js/build_ambr_wasm.py"
                    % os.path.relpath(OUT, ROOT).replace("\\", "/"))
    print("[ok] 产物在：%s  %d bytes"
          % (os.path.relpath(OUT, ROOT).replace("\\", "/"), os.path.getsize(OUT)))
    return 0


def main():
    ap = argparse.ArgumentParser(description="build .ambr wasm for JS")
    ap.add_argument("--skip-build", action="store_true", help="不编译，只拷贝已有产物")
    ap.add_argument("--check", action="store_true", help="只检查产物是否存在")
    args = ap.parse_args()

    if args.check:
        return check()
    if not args.skip_build:
        rc = build()
        if rc != 0:
            return rc
    return publish()


if __name__ == "__main__":
    sys.exit(main())
