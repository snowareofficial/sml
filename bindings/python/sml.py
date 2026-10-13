"""SML 宿主绑定（Python / ctypes）—— 经 C-ABI 调 Rust cdylib（sml / libsml）。

读路径（loads / load_file / 取值）**直接遍历 v3 值树**，不经 JSON：类型保真
（int / float / bool 精确），富错误直接到手。仅 dumps 因 ABI 无值树构建器，
按文档走官方桥 `obj -> JSON -> sml_dump`（其余功能零 JSON 依赖）。

ABI 宪法 = c/sml_rs.h（Rust cdylib v3 全集）。不重写 SML 解析器。

内存纪律（§0.3）：
  * sml_loads / sml_load_file 返回的根：用完 sml_free。
  * sml_str_dup / sml_key_at / sml_dumps / sml_dump / sml_version_str 返回的
    char*：读后 sml_free_str（sml_key_at 返回的键也是堆副本，必须 free）。
  * sml_get / sml_at / sml_get_path 返回**借用**指针：不释放、不跨根释放使用。
  * sml_version 返回**静态**串：只读，不释放。
"""

import ctypes
import json
import os

__all__ = [
    "loads", "load_file", "dumps", "version", "features", "get_in", "SmlError",
    "SML_F_BAREWORD_STR", "SML_F_INCLUDE", "SML_F_ENV", "SML_F_CONTRACT",
    "SML_F_FRAGMENT", "SML_F_TOP_ARRAY", "SML_F_NAMESPACE", "SML_F_IMPLICIT_NS",
    "SML_F_MULTI_INCLUDE", "SML_F_GLOB_INCLUDE", "SML_F_REGEX_INCLUDE",
    "SML_F_EXT_REWRITE", "SML_F_BASIC",
]

# ---- 特性位常量（与 c/sml_rs.h 逐位一致） ----
SML_F_BAREWORD_STR = 1 << 0
SML_F_INCLUDE = 1 << 1
SML_F_ENV = 1 << 2
SML_F_CONTRACT = 1 << 3
SML_F_FRAGMENT = 1 << 4
SML_F_TOP_ARRAY = 1 << 5
SML_F_NAMESPACE = 1 << 6
SML_F_IMPLICIT_NS = 1 << 7
SML_F_MULTI_INCLUDE = 1 << 8
SML_F_GLOB_INCLUDE = 1 << 9
SML_F_REGEX_INCLUDE = 1 << 10
SML_F_EXT_REWRITE = 1 << 11
SML_F_BASIC = SML_F_BAREWORD_STR | SML_F_INCLUDE | SML_F_FRAGMENT

# ---- 值类型（sml_type） ----
_T_NULL = 0
_T_BOOL = 1
_T_INT = 2
_T_FLOAT = 3
_T_STR = 4
_T_ARRAY = 5
_T_OBJECT = 6


class SmlError(Exception):
    """富错误：失败抛此异常，含稳定码 code_str 与定位（line/column/position/source/text）。"""

    def __init__(self, code_str, code=0, line=0, column=0, position=0,
                 source="", text=""):
        self.code_str = code_str
        self.code = code
        self.line = line
        self.column = column
        self.position = position
        self.source = source
        self.text = text
        super().__init__(self.__str__())

    def __str__(self):
        loc = ""
        if self.line or self.column:
            loc = f" (line {self.line}, column {self.column})"
        return f"{self.code_str}: {self.text}{loc}"


class _SmlError(ctypes.Structure):
    """与 c/sml_rs.h 的 sml_error 逐字节对齐（字段顺序 + 类型 + 数组长度）。

    布局：int code, int line, int column, size_t position(8, 此处需 4 字节填充),
    char source[128], char text[256], char code_str[16]。"""
    _fields_ = [
        ("code", ctypes.c_int),
        ("line", ctypes.c_int),
        ("column", ctypes.c_int),
        ("position", ctypes.c_size_t),
        ("source", ctypes.c_char * 128),
        ("text", ctypes.c_char * 256),
        ("code_str", ctypes.c_char * 16),
    ]


# ---- 加载库 ----
def _load_lib():
    name = os.environ.get("SML_LIB")
    candidates = []
    if name:
        candidates.append(name)
    candidates += ["sml", "libsml", "sml.dll", "libsml.dll", "libsml.so", "libsml.dylib"]
    for c in candidates:
        try:
            return ctypes.CDLL(c)
        except OSError:
            continue
    raise OSError(
        "无法加载 SML cdylib（sml / libsml）。请设置环境变量 SML_LIB 指向 "
        "libsml.{dll,so,dylib}，或将其所在目录加入 PATH。"
    )


_lib = _load_lib()

# ---- 函数签名 ----
_sml_loads = _lib.sml_loads
_sml_loads.restype = ctypes.c_void_p
_sml_loads.argtypes = [ctypes.c_char_p, ctypes.c_uint, ctypes.POINTER(_SmlError)]

_sml_load_file = _lib.sml_load_file
_sml_load_file.restype = ctypes.c_void_p
_sml_load_file.argtypes = [ctypes.c_char_p, ctypes.c_uint, ctypes.POINTER(_SmlError)]

_sml_free = _lib.sml_free
_sml_free.argtypes = [ctypes.c_void_p]
_sml_free.restype = None

_sml_free_str = _lib.sml_free_str
_sml_free_str.argtypes = [ctypes.c_void_p]
_sml_free_str.restype = None

_sml_typeof = _lib.sml_typeof
_sml_typeof.restype = ctypes.c_int
_sml_typeof.argtypes = [ctypes.c_void_p]

_sml_get = _lib.sml_get
_sml_get.restype = ctypes.c_void_p
_sml_get.argtypes = [ctypes.c_void_p, ctypes.c_char_p]

_sml_at = _lib.sml_at
_sml_at.restype = ctypes.c_void_p
_sml_at.argtypes = [ctypes.c_void_p, ctypes.c_size_t]

_sml_size = _lib.sml_size
_sml_size.restype = ctypes.c_size_t
_sml_size.argtypes = [ctypes.c_void_p]

_sml_key_at = _lib.sml_key_at
_sml_key_at.restype = ctypes.c_void_p
_sml_key_at.argtypes = [ctypes.c_void_p, ctypes.c_size_t]

_sml_str_dup = _lib.sml_str_dup
_sml_str_dup.restype = ctypes.c_void_p
_sml_str_dup.argtypes = [ctypes.c_void_p]

_sml_int_value = _lib.sml_int_value
_sml_int_value.restype = ctypes.c_int64
_sml_int_value.argtypes = [ctypes.c_void_p]

_sml_real_value = _lib.sml_real_value
_sml_real_value.restype = ctypes.c_double
_sml_real_value.argtypes = [ctypes.c_void_p]

_sml_bool_value = _lib.sml_bool_value
_sml_bool_value.restype = ctypes.c_int
_sml_bool_value.argtypes = [ctypes.c_void_p]

_sml_dumps = _lib.sml_dumps
_sml_dumps.restype = ctypes.c_void_p
_sml_dumps.argtypes = [ctypes.c_void_p, ctypes.c_uint]

_sml_dump = _lib.sml_dump
_sml_dump.restype = ctypes.c_void_p
_sml_dump.argtypes = [ctypes.c_char_p]

_sml_dump_err = _lib.sml_dump_err
_sml_dump_err.restype = ctypes.c_void_p
_sml_dump_err.argtypes = [ctypes.c_char_p, ctypes.POINTER(_SmlError)]

_sml_version = _lib.sml_version
_sml_version.restype = ctypes.c_void_p
_sml_version.argtypes = []

_sml_features_mask = _lib.sml_features_mask
_sml_features_mask.restype = ctypes.c_uint
_sml_features_mask.argtypes = []

_sml_feature_name = _lib.sml_feature_name
_sml_feature_name.restype = ctypes.c_void_p
_sml_feature_name.argtypes = [ctypes.c_uint]


# ---- 字符串读 / 释放 ----
def _read_cstr(p):
    if not p:
        return None
    raw = ctypes.cast(p, ctypes.c_char_p).value
    return raw.decode("utf-8") if raw is not None else None


def _take(p):
    """读走并立即释放（用于 sml_str_dup / sml_key_at / sml_dumps / sml_dump 的返回值）。"""
    if not p:
        return None
    s = _read_cstr(p)
    _sml_free_str(p)
    return s


def _raise_from(err):
    def _field(b):
        if not b:
            return ""
        return b.split(b"\x00")[0].decode("utf-8", "replace")

    cs = _field(err.code_str)
    src = _field(err.source)
    txt = _field(err.text)
    raise SmlError(
        code_str=cs,
        code=int(err.code),
        line=int(err.line),
        column=int(err.column),
        position=int(err.position),
        source=src,
        text=txt,
    )


def _materialize(v):
    """把值树节点递归物化为原生 Python 对象（读后即拷；借用指针在根释放前全部读完）。"""
    t = _sml_typeof(v)
    if t == _T_NULL:
        return None
    if t == _T_BOOL:
        return _sml_bool_value(v) != 0
    if t == _T_INT:
        return _sml_int_value(v)
    if t == _T_FLOAT:
        return _sml_real_value(v)
    if t == _T_STR:
        return _take(_sml_str_dup(v))
    if t == _T_ARRAY:
        return [_materialize(_sml_at(v, i)) for i in range(_sml_size(v))]
    if t == _T_OBJECT:
        d = {}
        n = _sml_size(v)
        for i in range(n):
            kp = _sml_key_at(v, i)
            key = _take(kp)
            if key is None:
                continue
            d[key] = _materialize(_sml_get(v, key.encode("utf-8")))
        return d
    return None


def loads(text, flags=0):
    """解析 SML 文本为原生对象（dict/list/int/float/str/bool/None）。失败抛 SmlError。"""
    if text is None:
        raise SmlError("E-INTERNAL-001", text="loads: text is None")
    err = _SmlError()
    root = _sml_loads(text.encode("utf-8"), ctypes.c_uint(flags), ctypes.byref(err))
    if not root:
        _raise_from(err)
    try:
        return _materialize(root)
    finally:
        _sml_free(root)


def load_file(path, flags=0):
    """解析 SML 文件为原生对象（展开 include；相对路径以文件所在目录为基准）。"""
    if path is None:
        raise SmlError("E-INTERNAL-001", text="load_file: path is None")
    err = _SmlError()
    root = _sml_load_file(os.fspath(path).encode("utf-8"), ctypes.c_uint(flags),
                          ctypes.byref(err))
    if not root:
        _raise_from(err)
    try:
        return _materialize(root)
    finally:
        _sml_free(root)


def dumps(obj, flags=0):
    """原生对象 -> SML 文本（官方桥：obj -> JSON -> sml_dump）。失败抛 SmlError。"""
    js = json.dumps(obj, ensure_ascii=False).encode("utf-8")
    out = _sml_dump(js)
    if not out:
        err = _SmlError()
        _sml_dump_err(js, ctypes.byref(err))
        _raise_from(err)
    return _take(out)


def version():
    """底层库版本串（静态指针，不释放）。"""
    return _read_cstr(_sml_version()) or ""


def features():
    """支持的特性名列表（sml_features_mask + sml_feature_name）。"""
    mask = _sml_features_mask()
    names = []
    b = 0
    while (1 << b) <= mask and b < 64:
        p = _sml_feature_name(ctypes.c_uint(b))
        n = _read_cstr(p)
        if n:
            names.append(n)
        b += 1
    return names


def get_in(obj, path):
    """在已物化的原生对象上按 'a.b.c' / 'a.0' 路径便利取值（宿主侧，不走 ABI）。"""
    cur = obj
    for seg in path.split("."):
        if cur is None:
            return None
        if isinstance(cur, dict):
            cur = cur.get(seg)
        elif isinstance(cur, list):
            try:
                cur = cur[int(seg)]
            except (ValueError, IndexError):
                return None
        else:
            return None
    return cur
