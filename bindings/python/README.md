# SML — Python 绑定（ctypes）

经 C-ABI 调 Rust cdylib（`sml` / `libsml`），暴露 SML v3 全集能力。
**不重写 SML 解析器**：只做 FFI 与物化桥接。

## 能力面（与 §2.2 一致）

| 函数 | 说明 |
|---|---|
| `loads(text, flags=0)` | 文本 → 原生对象（dict/list/int/float/str/bool/None），类型精确 |
| `load_file(path, flags=0)` | 文件 → 原生对象（展开 include；相对路径以文件所在目录为基准） |
| `dumps(obj, flags=0)` | 原生对象 → SML 文本 |
| `version()` | 版本串（静态，不释放） |
| `features()` | 支持的特性名列表 |
| `get_in(obj, "a.b.c")` | 在已物化对象上按路径便利取值（宿主侧） |
| `SmlError` | 富错误异常：`code_str` / `code` / `line` / `column` / `position` / `source` / `text` |

## 实现路径：值树直访（§2.3-A，零 JSON 依赖的读路径）

- **读**（`loads` / `load_file`）：直接遍历 v3 值树
  （`sml_loads` / `sml_load_file` → `sml_typeof` / `sml_get` / `sml_at` /
  `sml_size` / `sml_key_at` / `sml_str_dup` / 标量取值），**不经 JSON**，
  int / float / bool 类型保真，富错误直接到手。
- **写**（`dumps`）：ABI 没有值树构建器，按文档走官方桥
  `obj → JSON（标准库 json）→ sml_dump`。除 `dumps` 外，功能零 JSON 依赖。

为支持「不经 JSON 完整物化对象」，本任务给 Rust cdylib 增补了一个极小符号
`sml_key_at`（见 `rust/src/c_abi.rs` 与 `c/sml_rs.h`），加法兼容、未破坏旧符号。

## 构建 / 加载 cdylib

1. 构建 Rust cdylib（产物 `sml.dll` / `libsml.so` / `libsml.dylib`）：
   ```bash
   cd rust
   cargo build --release -p swsml
   ```
2. 让绑定能加载它：设置环境变量 `SML_LIB` 指向 `.dll/.so/.dylib` 全路径，
   或将其所在目录加入 `PATH`（也可直接放进程 cwd）。
   ```bash
   export SML_LIB=/path/to/libsml.so
   ```

## 内存纪律（§0.3，三条）

- `sml_loads` / `sml_load_file` 返回的根：物化完成后 `sml_free`。
- `sml_str_dup` / `sml_key_at` / `sml_dumps` / `sml_dump` / `sml_version_str`
  返回的 `char*`：读后 `sml_free_str`（**`sml_key_at` 返回的键也是堆副本，必须 free**）。
- `sml_get` / `sml_at` / `sml_get_path` 返回**借用**指针：不释放、不跨根释放使用。
- `sml_version` 返回**静态**串：只读，**不释放**。

## `sml_error` 镜像

`SmlErrorStruct`（ctypes `Structure`）逐字节对齐 `c/sml_rs.h` 的 `sml_error`：
`c_int`×3 + `c_size_t` + `c_char*128/256/16`。`size_t` 在 64 位为 8 字节，
ctypes 会在 `position` 前自动插入 4 字节填充，与 Rust `#[repr(C)]` 一致。

## 运行测试

```bash
SML_LIB=/path/to/libsml.so python bindings/python/test_sml.py
```

§4 黄金向量 + 内存稳定循环（1000 次 loads），全绿。

## 已知事实（实现行为，非 bug）

- **行列上报**：`line` / `column` 只对「带源码位置的错误」
  （契约 `@contract` / `@feature` / `@version`）填充；通用语法错误与 EOF 收尾错误
  （`E-PARSE-001` 未闭合块、`E-PARSE-003` 多余 `}`、`E-PARSE-012`）一律
  `line=0, col=0, position=0`。读路径与物化不受影响。
- 对象键序：底层对象为 `BTreeMap`，键按字典序稳定排列（物化后宿主 dict 顺序无关）。
