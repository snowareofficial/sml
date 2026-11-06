// SPDX-License-Identifier: MulanPSL-2.0
// amberice — reader for `.ambr`, the general-purpose binary envelope.
// amberice —— `.ambr`（通用二进制信封）读取器。
//
// Repository / 仓库：https://gitee.com/snoware/sml
//
// This package is independent on purpose: `.ambr` never depended on SML's parser,
// and `amberice` is its own product line. Importing it must not pull in `sml`.
// 本包刻意独立：`.ambr` 从不依赖 SML 解析器，`amberice` 是独立产品线；导入它不应拖进 `sml`。
//
// Only "bytes in, canonical text out" is promised — the two symbols of `amberice-ffi`.
// Materialise a value tree with sml.Loads(amberice.LoadText(data)).
// 只承诺「字节进、canonical 文本出」，即 `amberice-ffi` 的两个符号；
// 要值树用 sml.Loads(amberice.LoadText(data))。
//
// Messages are not translated here: `swi18n` is not built yet, and a per-binding copy
// would drift. Error text is passed through verbatim from Rust (prefixed "AMBR:").
// 文案不在绑定层翻译：`swi18n` 尚未建立，各端自造会漂移；错误串原样透传 Rust。
package amberice

import (
	"fmt"
	"io"
	"os"
)

// Capability bits — same values as `ambr_caps()` in amberice-ffi.
// 能力位 —— 与 FFI 的 `ambr_caps()` 逐位一致。
const (
	CapRead  uint32 = 1 << 0
	CapWrite uint32 = 1 << 1
)

// Error reports a failed `.ambr` decode. Message starts with "AMBR:" and comes from Rust.
// Error 表示 `.ambr` 解码失败；Message 以 "AMBR:" 开头，来自 Rust 侧。
type Error struct {
	Message string
}

func (e *Error) Error() string { return e.Message }

// LoadText decodes `.ambr` bytes into canonical text (UTF-8).
// LoadText 把 `.ambr` 字节解成 canonical 文本（UTF-8）。
//
// The library is `amberice_ffi`, not `amberice` (the Rust crate is a pure rlib):
// build with `cargo build -p amberice-ffi`, then either put the artifact on the
// loader path or set AMBR_LIB to its full path.
//
// 库名是 `amberice_ffi`（Rust 的 `amberice` crate 是纯 rlib）：
// `cargo build -p amberice-ffi`，再把产物放进库搜索路径，或用 AMBR_LIB 指全路径。
func LoadText(data []byte) (string, error) {
	if len(data) == 0 {
		return "", &Error{Message: "AMBR: empty input"}
	}
	return decode(data)
}

// PackText packs SML source text into `.ambr` bytes. Requires the cdylib built with
// `--features sml` — the write path needs a parser; the read path does not.
//
// PackText 把 SML 源文本打成 `.ambr` 字节。需要带 `--features sml` 构建的 cdylib
// —— 写路径需要解析器，读路径不需要。
//
// Objects keep source order, which is what canonical bytes require.
// Round-trip: LoadText(PackText(text)) == the canonical form of text.
//
// 对象保留源序（canonical 字节的前提）。往返判据：
// LoadText(PackText(text)) == text 的 canonical 形式。
func PackText(text string) ([]byte, error) {
	if text == "" {
		return nil, &Error{Message: "AMBR: empty input"}
	}
	return encode(text)
}

// LoadTextFile decodes a `.ambr` file.
// LoadTextFile 解一个 `.ambr` 文件。
func LoadTextFile(path string) (string, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return "", fmt.Errorf("AMBR: read %s: %w", path, err)
	}
	return LoadText(data)
}

// LoadReader decodes from any io.Reader (HTTP body, embed.FS, archive member).
// The reader is consumed fully and not closed.
//
// LoadReader 从任意 io.Reader 解码（HTTP body、embed.FS、压缩包成员）；读到底，不负责关闭。
func LoadReader(r io.Reader) (string, error) {
	if r == nil {
		return "", &Error{Message: "AMBR: nil reader"}
	}
	data, err := io.ReadAll(r)
	if err != nil {
		return "", fmt.Errorf("AMBR: read stream: %w", err)
	}
	return LoadText(data)
}
