# SML — Go 绑定（cgo）

经 C-ABI 调 Rust cdylib（`sml` / `libsml`），暴露 SML v3 全集能力。
**不重写 SML 解析器**：只做 FFI 与物化桥接。

> ⚠️ 构建需 CGO + 链接 libsml。例：
> `CGO_CFLAGS=-I/path/to/c CGO_LDFLAGS="-L/path/to/libsml -lsml" go build ./...`
> 运行期把 libsml 所在目录加入 PATH / LD_LIBRARY_PATH / DYLD_LIBRARY_PATH。

## 能力面

| 成员 | 说明 |
|---|---|
| `sml.Loads(text, flags)` | 文本 → 原生对象（`map`/`[]interface{}`/`int64`/`float64`/`bool`/`string`/`nil`） |
| `sml.LoadFile(path, flags)` | 文件 → 原生对象（展开 include） |
| `sml.Dumps(obj, flags)` | 原生对象 → SML 文本 |
| `sml.Version()` / `sml.Features()` | 版本 / 特性名列表 |
| `sml.GetIn(obj, "a.b.c")` | 已物化对象上按路径取值 |
| `sml.SmlError` | 富错误：`CodeStr` / `Code` / `Line` / `Column` / `Position` / `Source` / `Text` |

## 路径：值树直访（§2.3-A，零 JSON 依赖的读路径）

读路径直接遍历 v3 值树（不经 JSON）；仅 `Dumps` 走官方桥 `obj → JSON → sml_dump`。
为完整物化对象，Rust cdylib 提供 `sml_key_at`（见 `rust/src/c_abi.rs`、`c/sml_rs.h`）。

## 内存纪律

`sml_loads`/`sml_load_file` 根用 `sml_free`；`sml_str_dup`/`sml_key_at`/`sml_dumps`/
`sml_dump` 返回的堆副本用 `sml_free_str`；`sml_get`/`sml_at` 借用指针不释放；
`sml_version` 静态串不释放。

## 已知事实

行列上报仅「带源码位置的错误」（契约 / `@feature` / `@version`）填充；通用语法与 EOF
收尾错误（`E-PARSE-001/003/012`）一律 `line=0, col=0, position=0`。
