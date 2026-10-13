# SML — Ruby 绑定（fiddle）

经 C-ABI 调 Rust cdylib（`sml` / `libsml`），暴露 SML v3 全集能力。
**不重写 SML 解析器**：只做 FFI 与物化桥接。

> 依赖：标准库 `fiddle`（零第三方依赖；`ffi` gem 本机未装时自动回退）。

## 能力面（与 §2.2 一致）

| 函数 | 说明 |
|---|---|
| `Sml.loads(text, flags=0)` | 文本 → 原生对象（Hash/Array/Integer/Float/String/true-false/nil） |
| `Sml.load_file(path, flags=0)` | 文件 → 原生对象（展开 include） |
| `Sml.dumps(obj, flags=0)` | 原生对象 → SML 文本 |
| `Sml.version` | 版本串（静态，不释放） |
| `Sml.features` | 支持的特性名数组 |
| `Sml.get_in(obj, "a.b.c")` | 在已物化对象上按路径便利取值（宿主侧） |
| `Sml::SmlError` | 富错误异常：`code_str` / `code` / `line` / `column` / `position` / `source` / `text` |

## 实现路径：值树直访（§2.3-A，零 JSON 依赖的读路径）

- **读**：`loads` / `load_file` 直接遍历 v3 值树（不经 JSON），类型保真、富错误直接到手。
- **写**：`dumps` 按文档走官方桥 `obj → JSON（标准库 json）→ sml_dump`。除 `dumps` 外功能零 JSON 依赖。
- 为完整物化对象，本任务给 Rust cdylib 增补 `sml_key_at`（见 `rust/src/c_abi.rs`、`c/sml_rs.h`）。

## 加载 cdylib

设置环境变量 `SML_LIB` 指向 `.dll/.so/.dylib` 全路径，或将其目录加入 `PATH`：
```bash
export SML_LIB=/path/to/libsml.so
ruby -e 'require "sml"; ...'
```

## 内存纪律（§0.3）

- `sml_loads` / `sml_load_file` 返回的根：物化后 `sml_free`。
- `sml_str_dup` / `sml_key_at` / `sml_dumps` / `sml_dump` / `sml_version_str` 返回的 `char*`：读后 `sml_free_str`
  （`sml_key_at` 的键也是堆副本，必须 free）。
- `sml_get` / `sml_at` / `sml_get_path` 返回**借用**指针：不释放、不跨根释放使用。
- `sml_version` 返回**静态**串：只读，不释放。

## `sml_error` 镜像

`SmlErrorStruct` 按字节偏移读取（小端）：`int code(0)` / `int line(4)` / `int column(8)` /
`size_t position(16,8)` / `char source[128](24)` / `char text[256](152)` / `char code_str[16](408)`，
共 424 字节。字段读取用 `bytes.index(0)` 定位 NUL（注意：Ruby 的 `String#index` 不接受 Integer，
须用 `bytes.index("\x00")` 或 `bytes.bytes.index(0)`）。

## 运行测试

```bash
SML_LIB=/path/to/libsml.so ruby bindings/ruby/test_sml.rb
```

§4 黄金向量 + 内存稳定循环（1000 次 loads），全绿。

## 已知事实（实现行为，非 bug）

- **行列上报**：`line` / `column` 只对「带源码位置的错误」（契约 / `@feature` / `@version`）填充；
  通用语法错误与 EOF 收尾错误（`E-PARSE-001/003/012`）一律 `line=0, col=0, position=0`。
- 对象键序：`BTreeMap` 字典序稳定。
