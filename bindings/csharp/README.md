# SML — C# 绑定（P/Invoke）

经 C-ABI 调 Rust cdylib（`sml` / `libsml`），暴露 SML v3 全集能力。
**不重写 SML 解析器**：只做 FFI 与物化桥接。

> ⚠️ **状态：未在本机实地验证**。本机未安装 `dotnet` SDK，代码按 §2.3-A 值树路径与
> UTF-8 内存纪律编写，逻辑与其余四门（Python/Ruby/Java/Go，均已跑绿）严格对齐。
> 请提供 `dotnet` 与 `libsml` 后按下方命令验证。

## 能力面（与 §2.2 一致）

| 成员 | 说明 |
|---|---|
| `Sml.Loads(text, flags=0)` | 文本 → 原生对象（`Dictionary`/`List`/`long`/`double`/`bool`/`string`/`null`） |
| `Sml.LoadFile(path, flags=0)` | 文件 → 原生对象（展开 include） |
| `Sml.Dumps(obj, flags=0)` | 原生对象 → SML 文本 |
| `Sml.Version()` | 版本串（静态，不释放） |
| `Sml.Features()` | 支持的特性名列表 |
| `Sml.GetIn(obj, "a.b.c")` | 在已物化对象上按路径便利取值（宿主侧） |
| `Sml.SmlException` | 富错误异常：`CodeStr` / `Code` / `Line` / `Column` / `Position` / `Source` / `Text` |

## 实现路径：值树直访（§2.3-A，零 JSON 依赖的读路径）

- **读**：`Loads` / `LoadFile` 直接遍历 v3 值树（不经 JSON），类型保真、富错误直接到手。
- **写**：`Dumps` 按文档走官方桥 `obj → JSON（System.Text.Json）→ sml_dump`。
- 为完整物化对象，Rust cdylib 提供 `sml_key_at`（见 `rust/src/c_abi.rs`、`c/sml_rs.h`，加法兼容）。

## 关键实现注意（与其余语言一致，避免踩坑）

- **UTF-8 而非 ANSI**：Rust 的 `char*` 一律 UTF-8。入参用 `Marshal.StringToCoTaskMemUTF8`
  分配 UTF-8 字节缓冲（调用后 `ZeroFreeCoTaskMemUTF8` 释放）；出参用 `Marshal.PtrToStringUTF8`
  读取。**切勿**用 `LPStr`/`LPWStr` 默认编解码，否则中文乱码。
- **字段补齐**：`sml_error` 在 3 个 `int` 与 8 字节 `size_t position` 之间需 4 字节补齐，
  C# 结构体用显式 `_pad` 字段保证 `position` 落在偏移 16；字符数组字段用 `ByValArray`
  （而非 `ByValTStr`），回程手动按 UTF-8 解码，避免 ANSI 解码乱码。
- **内存纪律**：`sml_loads`/`sml_load_file` 根用 `sml_free`；`sml_str_dup`/`sml_key_at`/
  `sml_dumps`/`sml_dump` 返回的堆副本用 `sml_free_str`；`sml_get`/`sml_at` 借用指针不释放；
  `sml_version` 静态串不释放。

## 构建 / 运行

1. 构建 Rust cdylib 得 `sml.dll` / `libsml.so` / `libsml.dylib`。
2. 让运行时能找到它：把其所在目录加入 `PATH`（或把 `sml.dll` 放到生成 exe 旁）。
3. `dotnet run --project bindings/csharp` 跑 §4 八断言。

## 已知事实（实现行为，非 bug）

- 行列上报：仅「带源码位置的错误」（契约 / `@feature` / `@version`）填充 `line`/`column`；
  通用语法与 EOF 收尾错误（`E-PARSE-001/003/012`）一律 `line=0, col=0, position=0`。
