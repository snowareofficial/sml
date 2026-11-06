// SPDX-License-Identifier: MulanPSL-2.0
// cgo bridge to amberice-ffi.
// 到 amberice-ffi 的 cgo 桥。
//
// Build prerequisites, per platform / 各平台构建前提：
//
//	Linux / macOS: cargo build -p amberice-ffi  → libamberice_ffi.so|dylib
//	Windows + MinGW: cargo build -p amberice-ffi → amberice_ffi.dll
//	  Rust 产出的是 MSVC 风格 .dll.lib，GNU ld 认不了，先生成 import lib:
//	    gendef amberice_ffi.dll
//	    dlltool -d amberice_ffi.def -l libamberice_ffi.dll.a -D amberice_ffi.dll
//
// 然后 CGO_LDFLAGS="-L<该目录>" 构建，运行期把同一目录放进 PATH / LD_LIBRARY_PATH。
// Why cgo and not syscall: `go vet` rejects every uintptr→unsafe.Pointer conversion
// on the return path, and an unvet-able binding is worse than a build step.
// 为什么用 cgo 不用 syscall：vet 会拒绝返回值上的 uintptr→unsafe.Pointer 转换；
// 一个过不了 vet 的绑定比多一步构建更糟。

package amberice

/*
#cgo LDFLAGS: -lamberice_ffi
#include <stdlib.h>

char *ambr_text(const unsigned char *buf, size_t len, char *err, size_t errsz);
void ambr_free_str(char *p);

// write path — present only when the cdylib was built with `--features sml`.
// 写路径 —— 只有带 `--features sml` 构建的 cdylib 才有。
unsigned char *ambr_pack_text(const char *text, size_t len, size_t *out_len,
                              char *err, size_t errsz);
void ambr_free_bytes(unsigned char *p, size_t len);

unsigned ambr_caps(void);
*/
import "C"

import "unsafe"

// Capabilities of the cdylib this package was linked against.
//
// Note: cgo links statically, so a read-only cdylib fails at **build** time here rather
// than at run time. The query still matters when one binary may talk to either build.
// ｜ cgo 是静态链接：只读 cdylib 在**构建期**就失败，而不是运行期。
// 当同一个程序可能对接两种构建时，这个查询仍然有用。
func Capabilities() uint32 { return uint32(C.ambr_caps()) }

func decode(data []byte) (string, error) {
	errbuf := make([]byte, 256)

	out := C.ambr_text(
		(*C.uchar)(unsafe.Pointer(&data[0])),
		C.size_t(len(data)),
		(*C.char)(unsafe.Pointer(&errbuf[0])),
		C.size_t(len(errbuf)),
	)
	if out == nil {
		return "", &Error{Message: C.GoString((*C.char)(unsafe.Pointer(&errbuf[0])))}
	}
	defer C.ambr_free_str(out)
	return C.GoString(out), nil
}

func encode(text string) ([]byte, error) {
	raw := []byte(text)
	var outLen C.size_t
	errbuf := make([]byte, 256)

	p := C.ambr_pack_text(
		(*C.char)(unsafe.Pointer(&raw[0])),
		C.size_t(len(raw)),
		&outLen,
		(*C.char)(unsafe.Pointer(&errbuf[0])),
		C.size_t(len(errbuf)),
	)
	if p == nil {
		return nil, &Error{Message: C.GoString((*C.char)(unsafe.Pointer(&errbuf[0])))}
	}
	defer C.ambr_free_bytes(p, outLen)
	return C.GoBytes(unsafe.Pointer(p), C.int(outLen)), nil
}
