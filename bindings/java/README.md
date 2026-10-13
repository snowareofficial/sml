# SML — Java 绑定（JDK Panama FFM）

经 C-ABI 调 Rust cdylib（`sml` / `libsml`），暴露 SML v3 全集能力。
**不重写 SML 解析器**：只做 FFI 与物化桥接。

## 能力面（与 §2.2 一致）

| 成员 | 说明 |
|---|---|
| `Sml.loads(text, flags=0)` | 文本 → 原生对象（`Map`/`List`/`Long`/`Double`/`Boolean`/`String`/`null`） |
| `Sml.loadFile(path, flags=0)` | 文件 → 原生对象（展开 include） |
| `Sml.dumps(obj, flags=0)` | 原生对象 → SML 文本 |
| `Sml.version()` | 版本串（静态，不释放） |
| `Sml.features()` | 支持的特性名列表 |
| `Sml.getIn(obj, "a.b.c")` | 在已物化对象上按路径便利取值（宿主侧） |
| `Sml.SmlError` | 富错误异常：`codeStr` / `code` / `line` / `column` / `position` / `source` / `text` |

## 实现路径：值树直访（§2.3-A，零 JSON 依赖的读路径）

- **读**：`loads` / `loadFile` 直接遍历 v3 值树（不经 JSON），类型保真、富错误直接到手。
- **写**：`dumps` 按文档走官方桥 `obj → JSON（极简序列化器）→ sml_dump`。
- 为完整物化对象，Rust cdylib 提供 `sml_key_at`（见 `rust/src/c_abi.rs`、`c/sml_rs.h`）。

## 加载 cdylib

设环境变量 `SML_LIB` 指向 `.dll/.so/.dylib` 全路径；否则按名 `sml` / `libsml` 经
`System.loadLibrary` 加载（需其在库搜索路径）。

## 内存纪律（§0.3）

- `sml_loads` / `sml_load_file` 返回的根：物化后 `sml_free`。
- `sml_str_dup` / `sml_key_at` / `sml_dumps` / `sml_dump` / `sml_version_str` 返回的 char*：读后 `sml_free_str`。
- `sml_get` / `sml_at` / `sml_get_path` 返回**借用**指针：不释放、不跨根释放使用。
- `sml_version` 返回**静态**串：只读，不释放。

## `sml_error` 镜像

用 FFM `MemoryLayout.structLayout` 逐字段对齐 `c/sml_rs.h`：`int code(0)` / `int line(4)` /
`int column(8)` / **4 字节 padding** / `long position(16,8)` / `byte[128](24)` / `byte[256](152)` /
`byte[16](408)`，共 424 字节。注意 `position` 为 8 字节，FFM 要求在其前显式插入 4 字节 padding。

## 运行测试

```bash
export SML_LIB=/path/to/libsml.so
javac -d out bindings/java/src/main/java/sml/*.java bindings/java/src/test/java/sml/*.java
java --enable-native-access=ALL-UNNAMED -cp out sml.SmlTest
```

§4 黄金向量 + 内存稳定循环（1000 次 loads），全绿。

## 已知事实（实现行为，非 bug）

- 行列上报：仅「带源码位置的错误」填充 `line`/`column`；通用语法与 EOF 收尾错误一律 `0`。
- 对象键序：`BTreeMap` 字典序稳定。
