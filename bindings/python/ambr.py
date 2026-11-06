# SPDX-License-Identifier: MulanPSL-2.0
# ambr.py — FFI binding for `.ambr` (ctypes → amberice cdylib).
# ambr.py —— `.ambr` 的 Python FFI 绑定（ctypes 调 amberice 的 cdylib）。
#
# Why text / 为什么返回文本:
#   `bindings/python/sml.py` already wraps the SML parser, so materialising a tree is
#   `loads(load_text(bytes))`. Keeping the FFI to two symbols avoids duplicating the value
#   model in Python. ｜ 值树用现有 `loads` 物化，FFI 只负责把容器解成 canonical 文本。
#
# Load path / 库路径: env `AMBR_LIB` (full path) wins; otherwise `amberice` on the loader path.
#   Build it with: cargo build -p amberice --features ffi
"""`.ambr` reader backed by the `amberice` cdylib. ｜ 基于 amberice cdylib 的 `.ambr` 读取器。"""

from __future__ import annotations

import ctypes
import os

__all__ = ["AmbrError", "load_text", "load_text_file"]

_lib = None


class AmbrError(RuntimeError):
    """Decoder failure; message starts with `AMBR:`. ｜ 解码失败，消息以 `AMBR:` 开头。"""


def _lib_path() -> str | None:
    return os.environ.get("AMBR_LIB") or None


def _load_lib():
    global _lib
    if _lib is not None:
        return _lib
    path = _lib_path()
    lib = ctypes.CDLL(path) if path else ctypes.CDLL("amberice")
    lib.ambr_text.restype = ctypes.c_void_p
    lib.ambr_text.argtypes = [
        ctypes.c_char_p,
        ctypes.c_size_t,
        ctypes.c_char_p,
        ctypes.c_size_t,
    ]
    lib.ambr_free_str.argtypes = [ctypes.c_void_p]
    lib.ambr_free_str.restype = None
    _lib = lib
    return lib


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
