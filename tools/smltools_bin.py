#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""tools/smltools_bin.py —— 定位「已构建的 smltools」，**不写死本机盘符**。

为什么要单独抽出来：审计（2026-10）发现仓库里多处把 `<target-dir>/...`、
`c:\\Users\\<某人>\\...` 这类**本机绝对路径**写进脚本 —— 换台机器、或清一次构建缓存，
就报「找不到 smltools」，而错误信息还指向一个与仓库毫无关系的盘符。
更糟的是它**静默降级**：站点构建只是打印一行 `!!`，页面照旧产出，只是错误码表变成加载失败。

定位顺序（本机无关 → 本机相关）：

    1. 环境变量 SMLTOOLS_BIN（显式指定，最高优先）
    2. 环境变量 SMLCONV（老脚本沿用过的旧名字，仍然认）
    3. `cargo metadata` 报出的 target_directory —— **尊重用户的全局 .cargo/config.toml
       与 CARGO_TARGET_DIR**（所以 target-dir 被指到 E:/… 也照样找得到），
       再拼 `release|debug/smltools[.exe]`
    4. 仓库内相对路径 `rust/target/release|debug/smltools[.exe]`（未改过 target-dir 时命中）
    5. PATH 上的 smltools

只依赖标准库；cargo 不在、或 metadata 解析失败时**静默跳过**第 3 步（不抛异常）。

用法::

    import os, sys
    sys.path.insert(0, os.path.join(ROOT, "tools"))
    from smltools_bin import find_smltools, require_smltools
"""

import json
import os
import shutil
import subprocess

#: 仓库根（本文件在 <root>/tools/ 下）
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

#: Windows 上带 .exe；其它平台不带
EXE = [".exe", ""] if os.name == "nt" else [""]


def _cargo_target_dirs():
    """问 cargo 要 target_directory（含全局配置与环境变量的影响）。失败就返回空表。

    ⚠️ 必须显式给 `--manifest-path`：`cargo metadata` 默认看**当前目录**有没有 Cargo.toml，
    而调用方常常在**仓库根**运行（如 `python site/build_site.py`），根目录没有 Cargo.toml
    ⇒ metadata 直接失败、候选表为空 ⇒ 报「找不到 smltools」。这个坑实测踩过。
    """
    manifest = os.path.join(ROOT, "rust", "Cargo.toml")
    if not os.path.isfile(manifest):
        return []
    try:
        p = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1",
             "--manifest-path", manifest],
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=120,
        )
        if p.returncode == 0:
            td = json.loads(p.stdout or "{}").get("target_directory")
            if td:
                return [td]
    except (OSError, subprocess.SubprocessError, ValueError):
        pass
    return []


def candidates():
    """按优先级给出候选路径（可能含不存在的项；调用方逐个验证）。"""
    out = []
    for env in ("SMLTOOLS_BIN", "SMLCONV"):
        v = os.environ.get(env)
        if v:
            out.append(v)
    for td in _cargo_target_dirs():
        for prof in ("release", "debug"):
            for ext in EXE:
                out.append(os.path.join(td, prof, "smltools" + ext))
    for prof in ("release", "debug"):
        for ext in EXE:
            out.append(os.path.join(ROOT, "rust", "target", prof, "smltools" + ext))
    out.append(shutil.which("smltools") or "smltools")
    return out


def find_smltools():
    """返回可用的二进制路径；找不到返回 None。

    判据是「真能跑起来」（`--help` 不抛异常），而不是「文件存在」——
    半截的产物、架构不对的可执行文件都会在这一步露出来。
    """
    for c in candidates():
        if not c:
            continue
        has_sep = (os.path.sep in c) or (os.altsep is not None and os.altsep in c)
        if has_sep and not os.path.isfile(c):
            continue
        try:
            subprocess.run([c, "--help"], capture_output=True, timeout=60)
        except (OSError, subprocess.SubprocessError):
            continue
        return c
    return None


def require_smltools(what="smltools"):
    """找不到就带着**可执行的**指引退出（别只说「找不到」）。"""
    b = find_smltools()
    if not b:
        raise SystemExit(
            "找不到 %s：设 SMLTOOLS_BIN 指向它，或先构建 —— `cd rust && cargo build --release -p %s`"
            % (what, what)
        )
    return b
