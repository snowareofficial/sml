# SPDX-License-Identifier: MulanPSL-2.0
# ambr.py — FFI binding for `.ambr` (ctypes → amberice cdylib).
# ambr.py —— `.ambr` 的 Python FFI 绑定（ctypes 调 amberice 的 cdylib）。
#
# Why text / 为什么返回文本:
#   `bindings/python/sml.py` already wraps the SML parser, so materialising a tree is
#   `loads(load_text(bytes))`. Keeping the FFI to two symbols avoids duplicating the value
#   model in Python. ｜ 值树用现有 `loads` 物化，FFI 只负责把容器解成 canonical 文本。
#
# Two build modes / 两种构建:
#   `cargo build -p amberice-ffi`            → read-only   (LOAD_TEXT only)
#   `cargo build -p amberice-ffi --features sml` → read + write (PACK_TEXT)
#   Call `capabilities()` to branch instead of guessing whether a symbol exists.
#   ｜ 用 `capabilities()` 判断，而不是猜符号在不在。
#
# Load path / 库路径: env `AMBR_LIB` (full path) wins; otherwise `amberice_ffi` on the loader path.
#   Build it with: cargo build -p amberice-ffi [--features sml]
"""`.ambr` reader/writer backed by the `amberice` cdylib. ｜ 基于 amberice cdylib 的 `.ambr` 读写器。"""

from __future__ import annotations

import ctypes
import os

__all__ = [
    "AmbrError",
    "CAP_READ",
    "CAP_WRITE",
    "capabilities",
    "load_text",
    "load_text_file",
    "pack_text",
    "pack_text_file",
]

# Capability bits — same values as `ambr_caps()` in amberice-ffi.
# 能力位 —— 与 amberice-ffi 的 `ambr_caps()` 逐位一致。
CAP_READ = 1 << 0
CAP_WRITE = 1 << 1

_lib = None


class AmbrError(RuntimeError):
    """Decoder/encoder failure; message starts with `AMBR:`. ｜ 失败，消息以 `AMBR:` 开头。"""


def _lib_path() -> str | None:
    return os.environ.get("AMBR_LIB") or None


def _load_lib():
    global _lib
    if _lib is not None:
        return _lib

    path = _lib_path()
    lib = ctypes.CDLL(path) if path else ctypes.CDLL("amberice_ffi")

    lib.ambr_text.restype = ctypes.c_void_p
    lib.ambr_text.argtypes = [
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.c_char_p,
        ctypes.c_size_t,
    ]
    lib.ambr_free_str.argtypes = [ctypes.c_void_p]
    lib.ambr_free_str.restype = None

    # Optional symbols: a read-only build simply does not have them.
    # 可选符号：只读构建里没有，故用 getattr 探测而不是硬访问。
    if hasattr(lib, "ambr_caps"):
        lib.ambr_caps.restype = ctypes.c_uint32
        lib.ambr_caps.argtypes = []

    if hasattr(lib, "ambr_pack_text"):
        lib.ambr_pack_text.restype = ctypes.c_void_p
        lib.ambr_pack_text.argtypes = [
            ctypes.c_char_p,
            ctypes.c_size_t,
            ctypes.POINTER(ctypes.c_size_t),
            ctypes.c_char_p,
            ctypes.c_size_t,
        ]
        lib.ambr_free_bytes.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        lib.ambr_free_bytes.restype = None

    _lib = lib
    return lib


def capabilities() -> int:
    """Capability bits of the loaded build (`CAP_READ` / `CAP_WRITE`).
    ｜ 当前构建的能力位（`CAP_READ` / `CAP_WRITE`）。
    """
    lib = _load_lib()
    caps = getattr(lib, "ambr_caps", None)
    if caps is None:
        return CAP_READ
    return int(caps())


def load_text(data: bytes) -> str:
    """Decode `.ambr` bytes into canonical text. ｜ 把 `.ambr` 解成 canonical 文本。"""
    lib = _load_lib()
    err = ctypes.create_string_buffer(256)
    ptr = lib.ambr_text(bytes(data), len(data), err, len(err))
    if not ptr:
        raise AmbrError(err.value.decode("utf-8", "replace"))
    try:
        return ctypes.string_at(ptr).decode("utf-8")
    finally:
        lib.ambr_free_str(ptr)


def load_text_file(path: str) -> str:
    """Decode a `.ambr` file into canonical text. ｜ 解一个 `.ambr` 文件。"""
    with open(path, "rb") as f:
        return load_text(f.read())


def pack_text(text: str) -> bytes:
    """Pack SML source text into `.ambr` bytes (needs `CAP_WRITE`).
    ｜ 把 SML 源文本打成 `.ambr` 字节（需要 `CAP_WRITE`）。

    Objects keep source order, which is what canonical bytes require. A lone top-level
    scalar (`42`, `null`) round-trips too — the encoder opts into the parser switch that
    allows it. ｜ 对象保源序；顶层标量也能往返（编码器显式打开了对应的解析开关）。
    """
    lib = _load_lib()
    if not capabilities() & CAP_WRITE:
        raise AmbrError(
            "AMBR: this build is read-only — rebuild amberice-ffi with --features sml / "
            "本次构建只读：请用 --features sml 重新构建 amberice-ffi"
        )

    src = text.encode("utf-8")
    err = ctypes.create_string_buffer(256)
    out_len = ctypes.c_size_t(0)
    ptr = lib.ambr_pack_text(src, len(src), ctypes.byref(out_len), err, len(err))
    if not ptr:
        raise AmbrError(err.value.decode("utf-8", "replace"))
    try:
        return ctypes.string_at(ptr, out_len.value)
    finally:
        lib.ambr_free_bytes(ptr, out_len.value)


def pack_text_file(text: str, path: str) -> int:
    """Pack `text` and write it to `path`; returns the byte count.
    ｜ 打包并写入文件，返回字节数。
    """
    data = pack_text(text)
    with open(path, "wb") as f:
        f.write(data)
    return len(data)
