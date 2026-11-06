# `sml/amberice` — Go reader for `.ambr`

Reads the amberice binary envelope and returns **canonical text**. Value trees come from
the SML parser; the FFI surface is deliberately two symbols.

读取 amberice 二进制信封，返回 **canonical 文本**。值树交给 SML 解析器；FFI 面刻意只有两个符号。

```go
import "sml/amberice"

text, err := amberice.LoadTextFile("data.ambr")
if err != nil {
    // err.Error() starts with "AMBR:" — passed through from Rust, not translated here.
}
value, err := sml.Loads(text)
```

## Why a separate package

`.ambr` never depended on SML's parser and `amberice` is its own product line; importing
`amberice` must not pull in `sml`.

`.ambr` 从不依赖 SML 解析器，`amberice` 是独立产品线；导入 `amberice` 不应拖进 `sml`。

## Build

The C ABI lives in the `amberice-ffi` cdylib (the `amberice` crate is a pure rlib):

C ABI 在 `amberice-ffi` 这个 cdylib 里（`amberice` crate 是纯 rlib）：

```bash
cargo build -p amberice-ffi
```

Then make the artifact visible to both the linker and the loader:

然后让产物对链接器和运行期都可见：

| Platform | Link-time | Run-time |
|---|---|---|
| Linux | `CGO_LDFLAGS=-L<dir>` | `LD_LIBRARY_PATH=<dir>` |
| macOS | `CGO_LDFLAGS=-L<dir>` | `DYLD_LIBRARY_PATH=<dir>` |
| Windows + MinGW | see below | `PATH=<dir>` |

Windows: rustc emits an MSVC-style `amberice_ffi.dll.lib`, which GNU ld cannot read.
Generate a MinGW import library once:

Windows：rustc 产出 MSVC 风格 `amberice_ffi.dll.lib`，GNU ld 认不了；生成一次 import lib：

```bash
gendef amberice_ffi.dll
dlltool -d amberice_ffi.def -l libamberice_ffi.dll.a -D amberice_ffi.dll
CGO_LDFLAGS=-L. go test ./...
```

Point `AMBR_LIB` at the full dll path to override the name the binding searches for.

用 `AMBR_LIB` 指完整 dll 路径可覆盖绑定搜索的名字。

## Verify

```bash
go test ./amberice -v
```

The test decodes all 21 vectors in `tests/vectors/amberice-container.v1.json` and compares
the result to `canonical_text` **byte for byte**. A missing vector file skips loudly instead
of faking green.

测试解码 `tests/vectors/amberice-container.v1.json` 的全部 21 例，与 `canonical_text`
**逐字符**比较；向量文件缺失时显式跳过，不假绿。
