# SPDX-License-Identifier: MulanPSL-2.0
# SML 多语言宿主绑定（Python / ctypes）。
# 仓库：https://gitee.com/snoware/sml
# @version 0.1.0 (swsml 0.6.3)

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
  * sml_parse_with_diags 交出的 sml_diag_list：整体交给 sml_diag_list_free，
    其中每个 sml_diag 的 code_str / text 由该表独占，**不要**单独 free。
"""

import ctypes
import json
import os
from typing import NamedTuple

__all__ = [
    "loads", "loads_with_diags", "loads_full", "load_file", "dumps", "version", "features",
    "get_in", "SmlError", "Diagnostic", "Builtin",
    "loads_full_lists", "WeakLists", "WeakBind", "WeakData", "WeakDoc",
    "parse_all", "ParseAll", "ContractRef",
    "SML_WANT_DIAGS", "SML_WANT_ERRORS", "SML_WANT_WEAK", "SML_WANT_CONTRACT_MAP",
    "SML_WANT_CONTRACTS", "SML_WANT_ALL",
    "SML_F_BAREWORD_STR", "SML_F_INCLUDE", "SML_F_ENV", "SML_F_CONTRACT",
    "SML_F_FRAGMENT", "SML_F_TOP_ARRAY", "SML_F_NAMESPACE", "SML_F_IMPLICIT_NS",
    "SML_F_MULTI_INCLUDE", "SML_F_GLOB_INCLUDE", "SML_F_REGEX_INCLUDE",
    "SML_F_EXT_REWRITE", "SML_F_WEAK", "SML_F_BASIC",
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
# 弱声明族（`@weakbind` / `@weakdata` / `@weakdoc`）：opt-in，**不在**基线里。
# 要校验弱绑定，`flags` 得显式带上这一位（flags 是特性上限，不是「额外启用」）。
SML_F_WEAK = 1 << 19
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


class _SmlDiag(ctypes.Structure):
    """与 c/sml_rs.h 的 sml_diag 逐字节对齐（char* char* int int）。"""
    _fields_ = [
        ("code_str", ctypes.c_char_p),
        ("text", ctypes.c_char_p),
        ("line", ctypes.c_int),
        ("severity", ctypes.c_int),
    ]


class _SmlDiagList(ctypes.Structure):
    """与 c/sml_rs.h 的 sml_diag_list 逐字节对齐（items 指针 + len）。"""
    _fields_ = [
        ("items", ctypes.POINTER(_SmlDiag)),
        ("len", ctypes.c_size_t),
    ]


class _SmlOpts(ctypes.Structure):
    """与 c/sml_rs.h 的 sml_opts 逐字节对齐（ctypes 自动补 flags 后的 4 字节对齐）。"""
    _fields_ = [
        ("struct_size", ctypes.c_size_t),
        ("flags", ctypes.c_uint),
        ("env", ctypes.POINTER(ctypes.c_char_p)),
        ("env_len", ctypes.c_size_t),
        ("builtins", ctypes.c_void_p),
    ]


class _SmlWeakBind(ctypes.Structure):
    _fields_ = [("path", ctypes.c_char_p), ("name", ctypes.c_char_p),
                ("line", ctypes.c_int)]


class _SmlWeakData(ctypes.Structure):
    _fields_ = [("path", ctypes.c_char_p), ("name", ctypes.c_char_p),
                ("line", ctypes.c_int), ("value", ctypes.c_void_p)]


class _SmlWeakDoc(ctypes.Structure):
    _fields_ = [("path", ctypes.c_char_p), ("name", ctypes.c_char_p),
                ("line", ctypes.c_int)]


class _SmlWeakLists(ctypes.Structure):
    _fields_ = [
        ("binds", ctypes.POINTER(_SmlWeakBind)), ("n_binds", ctypes.c_size_t),
        ("data", ctypes.POINTER(_SmlWeakData)), ("n_data", ctypes.c_size_t),
        ("docs", ctypes.POINTER(_SmlWeakDoc)), ("n_docs", ctypes.c_size_t),
    ]


class _SmlContractRef(ctypes.Structure):
    _fields_ = [("path", ctypes.c_char_p), ("name", ctypes.c_char_p)]


class _SmlContractMap(ctypes.Structure):
    _fields_ = [("items", ctypes.POINTER(_SmlContractRef)), ("len", ctypes.c_size_t)]


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

_sml_parse_with_diags = _lib.sml_parse_with_diags
_sml_parse_with_diags.restype = ctypes.c_void_p
_sml_parse_with_diags.argtypes = [ctypes.c_char_p, ctypes.c_uint,
                                  ctypes.POINTER(_SmlDiagList),
                                  ctypes.POINTER(_SmlError)]

_sml_diag_list_free = _lib.sml_diag_list_free
_sml_diag_list_free.restype = None
_sml_diag_list_free.argtypes = [ctypes.POINTER(_SmlDiagList)]

_sml_builtin_new = _lib.sml_builtin_new
_sml_builtin_new.restype = ctypes.c_void_p
_sml_builtin_new.argtypes = []

_sml_builtin_free = _lib.sml_builtin_free
_sml_builtin_free.restype = None
_sml_builtin_free.argtypes = [ctypes.c_void_p]

_sml_builtin_count = _lib.sml_builtin_count
_sml_builtin_count.restype = ctypes.c_uint
_sml_builtin_count.argtypes = [ctypes.c_void_p]

_sml_builtin_add_field = _lib.sml_builtin_add_field
_sml_builtin_add_field.restype = ctypes.c_int
_sml_builtin_add_field.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p,
                                   ctypes.c_char_p, ctypes.c_int]

_sml_builtin_set_range = _lib.sml_builtin_set_range
_sml_builtin_set_range.restype = ctypes.c_int
_sml_builtin_set_range.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p,
                                   ctypes.c_double, ctypes.c_double]

_sml_builtin_add_enum = _lib.sml_builtin_add_enum
_sml_builtin_add_enum.restype = ctypes.c_int
_sml_builtin_add_enum.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p,
                                  ctypes.c_char_p]

_sml_parse_full = _lib.sml_parse_full
_sml_parse_full.restype = ctypes.c_void_p
_sml_parse_full.argtypes = [ctypes.c_char_p, ctypes.POINTER(_SmlOpts),
                            ctypes.POINTER(_SmlDiagList), ctypes.POINTER(_SmlError)]

_sml_parse_full_lists = _lib.sml_parse_full_lists
_sml_parse_full_lists.restype = ctypes.c_void_p
_sml_parse_full_lists.argtypes = [ctypes.c_char_p, ctypes.POINTER(_SmlOpts),
                                  ctypes.POINTER(_SmlWeakLists),
                                  ctypes.POINTER(_SmlDiagList), ctypes.POINTER(_SmlError)]

_sml_load_file_full = _lib.sml_load_file_full
_sml_load_file_full.restype = ctypes.c_void_p
_sml_load_file_full.argtypes = [ctypes.c_char_p, ctypes.POINTER(_SmlOpts),
                                ctypes.POINTER(_SmlWeakLists),
                                ctypes.POINTER(_SmlDiagList), ctypes.POINTER(_SmlError)]

_sml_weaklists_free = _lib.sml_weaklists_free
_sml_weaklists_free.restype = None
_sml_weaklists_free.argtypes = [ctypes.POINTER(_SmlWeakLists)]

_sml_parse_all = _lib.sml_parse_all
_sml_parse_all.restype = ctypes.c_void_p
_sml_parse_all.argtypes = [ctypes.c_char_p, ctypes.POINTER(_SmlOpts), ctypes.c_uint,
                           ctypes.POINTER(_SmlError)]

_sml_result_value = _lib.sml_result_value
_sml_result_value.restype = ctypes.c_void_p
_sml_result_value.argtypes = [ctypes.c_void_p]

_sml_result_diags = _lib.sml_result_diags
_sml_result_diags.restype = ctypes.POINTER(_SmlDiagList)
_sml_result_diags.argtypes = [ctypes.c_void_p]

_sml_result_errors = _lib.sml_result_errors
_sml_result_errors.restype = ctypes.POINTER(_SmlDiagList)
_sml_result_errors.argtypes = [ctypes.c_void_p]

_sml_result_weak = _lib.sml_result_weak
_sml_result_weak.restype = ctypes.POINTER(_SmlWeakLists)
_sml_result_weak.argtypes = [ctypes.c_void_p]

_sml_result_contract_map = _lib.sml_result_contract_map
_sml_result_contract_map.restype = ctypes.POINTER(_SmlContractMap)
_sml_result_contract_map.argtypes = [ctypes.c_void_p]

_sml_result_contracts = _lib.sml_result_contracts
_sml_result_contracts.restype = ctypes.c_void_p
_sml_result_contracts.argtypes = [ctypes.c_void_p]

_sml_result_free = _lib.sml_result_free
_sml_result_free.restype = None
_sml_result_free.argtypes = [ctypes.c_void_p]

_sml_builtin_clone = _lib.sml_builtin_clone
_sml_builtin_clone.restype = ctypes.c_void_p
_sml_builtin_clone.argtypes = [ctypes.c_void_p]

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


class Diagnostic:
    """一条非致命诊断（键冲突 W-PARSE-001 / 片段覆盖 W-PARSE-002 / 弱声明不匹配 W-CONTRACT-001）。

    code_str 为 None 表示该档暂无稳定码（Deprecated / 契约 warn 放行）。
    """

    __slots__ = ("code_str", "text", "line", "severity")

    def __init__(self, code_str, text, line, severity):
        self.code_str = code_str
        self.text = text
        self.line = line
        self.severity = severity

    def __repr__(self):
        loc = f" @{self.line}" if self.line else ""
        return f"Diagnostic({self.code_str or '<no code>'}{loc}: {self.text})"


def _take_diags(dl):
    """把 sml_diag_list 拷成 Python 列表（拷贝后即可整体释放）。"""
    out = []
    for i in range(int(dl.len)):
        d = dl.items[i]
        code = d.code_str.decode("utf-8") if d.code_str else None
        text = d.text.decode("utf-8", "replace") if d.text else ""
        out.append(Diagnostic(code, text, int(d.line), int(d.severity)))
    return out


def loads_with_diags(text, flags=0):
    """解析 SML 文本，返回 ``(对象, 诊断列表)``；失败抛 SmlError。

    与 :func:`loads` 的差别只有返回值：值树逐字节相同，诊断只说明
    「数据被改写 / 覆盖过」，不阻断解析。
    """
    if text is None:
        raise SmlError("E-INTERNAL-001", text="loads_with_diags: text is None")
    err = _SmlError()
    dl = _SmlDiagList()
    root = _sml_parse_with_diags(text.encode("utf-8"), ctypes.c_uint(flags),
                                 ctypes.byref(dl), ctypes.byref(err))
    if not root:
        _sml_diag_list_free(ctypes.byref(dl))
        _raise_from(err)
    try:
        diags = _take_diags(dl)
        return _materialize(root), diags
    finally:
        _sml_free(root)
        _sml_diag_list_free(ctypes.byref(dl))


class Builtin:
    """内置契约表（`@weakbind` 软校验用）；空表 ⇒ 软校验整段跳过。

    形状与 C 端 `sml_builtin` 一致：`add_field` 遇到不存在的契约会自动建。
    各构造方法返回 ``bool``：``False`` 表示参数非法（NULL / 未知类型 / 字段不存在），
    此时表**不动**。
    """

    def __init__(self):
        self._p = _sml_builtin_new()
        if not self._p:
            raise MemoryError("sml_builtin_new 失败")

    @classmethod
    def _from_ptr(cls, p):
        """接管一个已归本进程所有的 `sml_builtin*`（如 `sml_builtin_clone` 的产物）。"""
        self = cls.__new__(cls)
        self._p = p
        return self

    def __len__(self):
        return int(_sml_builtin_count(self._p)) if self._p else 0

    def add_field(self, contract, field, ty, required=False):
        """声明字段；`ty` 取 any / str / int / num / bool / enum。"""
        if not self._p:
            return False
        return _sml_builtin_add_field(
            self._p, _utf8(contract), _utf8(field), _utf8(ty), 1 if required else 0
        ) == 0

    def set_range(self, contract, field, min_value, max_value):
        """给已有字段设数值区间（含端点）。"""
        if not self._p:
            return False
        return _sml_builtin_set_range(
            self._p, _utf8(contract), _utf8(field), float(min_value), float(max_value)
        ) == 0

    def add_enum(self, contract, field, value):
        """给 enum 字段追加合法取值。"""
        if not self._p:
            return False
        return _sml_builtin_add_enum(self._p, _utf8(contract), _utf8(field),
                                     _utf8(value)) == 0

    @property
    def raw(self):
        return self._p

    def close(self):
        if self._p:
            _sml_builtin_free(self._p)
            self._p = None

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
        return False

    def __del__(self):
        self.close()


def _utf8(s):
    return s.encode("utf-8") if isinstance(s, str) else s


class WeakBind(NamedTuple):
    """一条弱绑定：`@weakbind` 指向的块路径 + 契约名。"""
    path: str
    name: str
    line: int


class WeakData(NamedTuple):
    """一条弱数据：`@weakdata` 的载荷（已物化成 Python 对象，**不在主树里**）。"""
    path: str
    name: str
    line: int
    value: object


class WeakDoc(NamedTuple):
    """一条弱 profile 引用（`@weakdoc`）：只有名字，库不读文件系统。"""
    path: str
    name: str
    line: int


class WeakLists(NamedTuple):
    """弱声明三表；三者都不进主树。"""
    binds: list
    data: list
    docs: list


def _encode_opts(flags, env, builtins):
    """把 (flags, env, builtins) 编成 _SmlOpts；返回值里带着 env 数组以维持生命周期。"""
    pairs = []
    if env:
        pairs = [f"{k}={v}" for k, v in env.items()] if isinstance(env, dict) else list(env)
    env_arr = (ctypes.c_char_p * len(pairs))(*[_utf8(p) for p in pairs]) if pairs else None
    opts = _SmlOpts()
    opts.struct_size = ctypes.sizeof(_SmlOpts)
    opts.flags = ctypes.c_uint(flags)
    opts.env = ctypes.cast(env_arr, ctypes.POINTER(ctypes.c_char_p)) if env_arr else None
    opts.env_len = len(pairs)
    opts.builtins = builtins.raw if builtins else None
    return opts, env_arr


def _dec(b):
    return b.decode("utf-8") if b else ""


def _take_weaklists(raw):
    """把 C 侧三表拷成 Python 对象（拷完即可让调用方整体释放）。"""
    binds = [WeakBind(_dec(p.path), _dec(p.name), int(p.line)) for p in raw.binds[: raw.n_binds]] \
        if raw.n_binds else []
    data = [WeakData(_dec(p.path), _dec(p.name), int(p.line), _materialize(p.value))
            for p in raw.data[: raw.n_data]] if raw.n_data else []
    docs = [WeakDoc(_dec(p.path), _dec(p.name), int(p.line)) for p in raw.docs[: raw.n_docs]] \
        if raw.n_docs else []
    return WeakLists(binds, data, docs)


def loads_full(text, flags=0, env=None, builtins=None):
    """解析文本，带完整选项（特性上限 / ``$env`` 注入 / 内置契约表）。

    返回 ``(对象, 诊断列表)``；值树与 :func:`loads` 逐字节相同。

    ``flags`` 是**特性上限**（与 :func:`loads` 同义）：``type.weak`` 是 opt-in、
    不在基线里 ⇒ 要校验弱绑定请显式带上 ``SML_F_WEAK``。``env`` 可为 dict 或
    ``"KEY=VALUE"`` 列表，只作用于本次解析，**不改进程环境**。
    """
    if text is None:
        raise SmlError("E-INTERNAL-001", text="loads_full: text is None")
    opts, _keep = _encode_opts(flags, env, builtins)
    err = _SmlError()
    dl = _SmlDiagList()
    root = _sml_parse_full(text.encode("utf-8"), ctypes.byref(opts),
                           ctypes.byref(dl), ctypes.byref(err))
    if not root:
        _sml_diag_list_free(ctypes.byref(dl))
        _raise_from(err)
    try:
        return _materialize(root), _take_diags(dl)
    finally:
        _sml_free(root)
        _sml_diag_list_free(ctypes.byref(dl))


def loads_full_lists(text, flags=0, env=None, builtins=None):
    """同 :func:`loads_full`，另交出弱声明三表。

    返回 ``(对象, 诊断列表, WeakLists)``。三者**都不进主树**：`@weakdata` 的载荷
    只在 ``lists.data[i].value`` 里，``@weakbind`` / ``@weakdoc`` 同理。
    """
    if text is None:
        raise SmlError("E-INTERNAL-001", text="loads_full_lists: text is None")
    opts, _keep = _encode_opts(flags, env, builtins)
    err = _SmlError()
    dl = _SmlDiagList()
    wl = _SmlWeakLists()
    root = _sml_parse_full_lists(text.encode("utf-8"), ctypes.byref(opts),
                                 ctypes.byref(wl), ctypes.byref(dl), ctypes.byref(err))
    if not root:
        _sml_diag_list_free(ctypes.byref(dl))
        _sml_weaklists_free(ctypes.byref(wl))
        _raise_from(err)
    try:
        return _materialize(root), _take_diags(dl), _take_weaklists(wl)
    finally:
        _sml_free(root)
        _sml_diag_list_free(ctypes.byref(dl))
        _sml_weaklists_free(ctypes.byref(wl))


SML_WANT_DIAGS = 1 << 0
"""要非致命诊断（`W-PARSE-001` / `W-PARSE-002` / `W-CONTRACT-001` …）。"""
SML_WANT_ERRORS = 1 << 1
"""要**多错误收集**：语法/契约错也返回结果，值树是尽力而为的部分结果。"""
SML_WANT_WEAK = 1 << 2
"""要弱声明三表（`@weakbind` / `@weakdata` / `@weakdoc`）。"""
SML_WANT_CONTRACT_MAP = 1 << 3
"""要契约映射（哪个实体应用了哪些契约）。"""
SML_WANT_CONTRACTS = 1 << 4
"""要本次解析收集到的契约表（以独立 `Builtin` 交出，可再当内置表注入）。"""
SML_WANT_ALL = 0x1F


class ContractRef(NamedTuple):
    """一条「某实体应用了某个契约」。"""
    path: str
    name: str


class ParseAll(NamedTuple):
    """`parse_all` 的全部产物；未请求的部分是空表 / None。"""
    value: object
    diagnostics: list
    errors: list
    weak: WeakLists
    contract_map: list
    contracts: "Builtin"


def parse_all(text, flags=0, env=None, builtins=None, want=SML_WANT_ALL):
    """一次解析拿到全部产物：值树 + 诊断 + 多错误 + 弱表 + 契约映射 + 契约表。

    与 :func:`loads_with_diags` / :func:`loads_full_lists` 的区别只是「一次拿全」；
    值树仍与 :func:`loads` 逐字节相同。

    ``SML_WANT_ERRORS`` 打开后，语法/契约错**不再抛异常**：值树是尽力而为的部分结果，
    全部错误在 ``errors`` 里（``severity == 1``）。其余情况失败仍抛 :class:`SmlError`。

    ``contracts`` 是**独立克隆**的 :class:`Builtin`（调用方负责 ``close()``），
    可直接传给 :func:`loads_full` 的 ``builtins`` —— 这就是
    「用 SML 写契约表、再注入回去」的完整闭环。
    """
    if text is None:
        raise SmlError("E-INTERNAL-001", text="parse_all: text is None")
    opts, _keep = _encode_opts(flags, env, builtins)
    err = _SmlError()
    r = _sml_parse_all(text.encode("utf-8"), ctypes.byref(opts), ctypes.c_uint(want),
                       ctypes.byref(err))
    if not r:
        _raise_from(err)
    try:
        diags = _take_diags(_sml_result_diags(r).contents) if want & SML_WANT_DIAGS else []
        errs = _take_diags(_sml_result_errors(r).contents) if want & SML_WANT_ERRORS else []
        weak = _take_weaklists(_sml_result_weak(r).contents) if want & SML_WANT_WEAK \
            else WeakLists([], [], [])
        cmap = []
        if want & SML_WANT_CONTRACT_MAP:
            raw = _sml_result_contract_map(r).contents
            cmap = [ContractRef(_dec(p.path), _dec(p.name)) for p in raw.items[: raw.len]] \
                if raw.len else []
        contracts = None
        if want & SML_WANT_CONTRACTS:
            # 借用的句柄不能活过结果，所以**立刻克隆**成宿主独占的一份。
            contracts = Builtin._from_ptr(_sml_builtin_clone(_sml_result_contracts(r)))
        return ParseAll(_materialize(_sml_result_value(r)), diags, errs, weak, cmap, contracts)
    finally:
        _sml_result_free(r)


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


def load_file_full(path, flags=0, env=None, builtins=None):
    """同 :func:`load_file`，另交出诊断与弱声明三表。

    返回 ``(对象, 诊断列表, WeakLists)``；是 :func:`loads_full_lists` 的**文件对应物**
    （展开 `include`，相对路径以文件所在目录为基准）。跨 include 时行号可能为 0
    —— token 跨文件合并，没有可映射的单一原文。
    """
    if path is None:
        raise SmlError("E-INTERNAL-001", text="load_file_full: path is None")
    opts, _keep = _encode_opts(flags, env, builtins)
    err = _SmlError()
    dl = _SmlDiagList()
    wl = _SmlWeakLists()
    root = _sml_load_file_full(os.fspath(path).encode("utf-8"), ctypes.byref(opts),
                               ctypes.byref(wl), ctypes.byref(dl), ctypes.byref(err))
    if not root:
        _sml_diag_list_free(ctypes.byref(dl))
        _sml_weaklists_free(ctypes.byref(wl))
        _raise_from(err)
    try:
        return _materialize(root), _take_diags(dl), _take_weaklists(wl)
    finally:
        _sml_free(root)
        _sml_diag_list_free(ctypes.byref(dl))
        _sml_weaklists_free(ctypes.byref(wl))


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
