// Package sml 提供 SML 的 Go 宿主绑定（经 cgo 调用 Rust cdylib sml / libsml）。
//
// 读路径（Loads / LoadFile / 取值）直接遍历 v3 值树，不经 JSON：类型保真、富错误直接到手。
// 仅 Dumps 按文档走官方桥 obj -> JSON -> sml_dump（ABI 无值树构建器）。
// ABI 宪法 = c/sml_rs.h（Rust cdylib v3 全集）。不重写 SML 解析器。
//
// 内存纪律（§0.3）：
//   * sml_loads / sml_load_file 返回的根：物化后 sml_free。
//   * sml_str_dup / sml_key_at / sml_dumps / sml_dump / sml_version_str 返回的 char*：读后 sml_free_str
//     （sml_key_at 的键也是堆副本，必须 free）。
//   * sml_get / sml_at / sml_get_path 返回借用指针：不释放、不跨根释放使用。
//   * sml_version 返回静态串：只读，不释放。
//
// 构建：需 CGO_ENABLED=1，且链接 sml/libsml。例如：
//   CGO_CFLAGS=-I/path/to/c CGO_LDFLAGS="-L/path/to/libsml/dir -lsml" go build ./...
// 运行期把 libsml.{dll,so,dylib} 所在目录加入 PATH / LD_LIBRARY_PATH / DYLD_LIBRARY_PATH。
package sml

/*
#cgo CFLAGS: -I${SRCDIR}/../../c
#cgo LDFLAGS: -lsml
#include <stdlib.h>
#include "sml_rs.h"
*/
import "C"

import (
	"encoding/json"
	"fmt"
	"reflect"
	"unsafe"
)

// 特性位常量（与 c/sml_rs.h 逐位一致）
const (
	SMLF_BAREWORD_STR = 1 << 0
	SMLF_INCLUDE      = 1 << 1
	SMLF_ENV          = 1 << 2
	SMLF_CONTRACT     = 1 << 3
	SMLF_FRAGMENT     = 1 << 4
	SMLF_TOP_ARRAY    = 1 << 5
	SMLF_NAMESPACE    = 1 << 6
	SMLF_IMPLICIT_NS  = 1 << 7
	SMLF_MULTI_INC    = 1 << 8
	SMLF_GLOB_INC     = 1 << 9
	SMLF_REGEX_INC    = 1 << 10
	SMLF_EXT_REWRITE  = 1 << 11
	SMLF_BASIC        = SMLF_BAREWORD_STR | SMLF_INCLUDE | SMLF_FRAGMENT
)

// 值类型
const (
	tNull = 0
	tBool = 1
	tInt  = 2
	tReal = 3
	tStr  = 4
	tArr  = 5
	tObj  = 6
)

// SmlError 富错误（字段与 c/sml_rs.h 的 sml_error 对齐）。
type SmlError struct {
	CodeStr  string
	Code     int
	Line     int
	Column   int
	Position int64
	Source   string
	Text     string
}

func (e *SmlError) Error() string {
	loc := ""
	if e.Line != 0 || e.Column != 0 {
		loc = fmt.Sprintf(" (line %d, column %d)", e.Line, e.Column)
	}
	return fmt.Sprintf("%s: %s%s", e.CodeStr, e.Text, loc)
}

func errorFrom(err *C.sml_error) error {
	return &SmlError{
		CodeStr:  C.GoString(&err.code_str[0]),
		Code:     int(err.code),
		Line:     int(err.line),
		Column:   int(err.column),
		Position: int64(err.position),
		Source:   C.GoString(&err.source[0]),
		Text:     C.GoString(&err.text[0]),
	}
}

// takeStr 读走并释放堆副本字符串（sml_str_dup / sml_key_at / sml_dumps / sml_dump）。
func takeStr(p *C.char) string {
	if p == nil {
		return ""
	}
	s := C.GoString(p)
	C.sml_free_str(p)
	return s
}

// materialize 递归遍历值树（借用指针，不释放；根由调用方 sml_free）。
func materialize(v *C.sml_value) interface{} {
	if v == nil {
		return nil
	}
	switch int(C.sml_typeof(v)) {
	case tNull:
		return nil
	case tBool:
		return int(C.sml_bool_value(v)) != 0
	case tInt:
		return int64(C.sml_int_value(v))
	case tReal:
		return float64(C.sml_real_value(v))
	case tStr:
		return takeStr(C.sml_str_dup(v))
	case tArr:
		n := int(C.sml_size(v))
		arr := make([]interface{}, 0, n)
		for i := 0; i < n; i++ {
			arr = append(arr, materialize(C.sml_at(v, C.size_t(i))))
		}
		return arr
	case tObj:
		n := int(C.sml_size(v))
		m := make(map[string]interface{}, n)
		for i := 0; i < n; i++ {
			kp := C.sml_key_at(v, C.size_t(i))
			if kp == nil {
				continue
			}
			key := takeStr(kp)
			ck := C.CString(key)
			cv := C.sml_get(v, ck)
			C.free(unsafe.Pointer(ck))
			m[key] = materialize(cv)
		}
		return m
	default:
		return nil
	}
}

// Loads 文本 -> 原生对象（map/[]interface{}/int64/float64/string/bool/nil）。
func Loads(text string, flags int) (interface{}, error) {
	ctext := C.CString(text)
	defer C.free(unsafe.Pointer(ctext))
	var err C.sml_error
	root := C.sml_loads(ctext, C.uint(flags), &err)
	if root == nil {
		return nil, errorFrom(&err)
	}
	defer C.sml_free(root)
	return materialize(root), nil
}

// LoadFile 文件 -> 原生对象（展开 include；相对路径以文件所在目录为基准）。
func LoadFile(path string, flags int) (interface{}, error) {
	cpath := C.CString(path)
	defer C.free(unsafe.Pointer(cpath))
	var err C.sml_error
	root := C.sml_load_file(cpath, C.uint(flags), &err)
	if root == nil {
		return nil, errorFrom(&err)
	}
	defer C.sml_free(root)
	return materialize(root), nil
}

// Dumps 原生对象 -> SML 文本（文档桥：obj -> JSON -> sml_dump）。
func Dumps(obj interface{}, flags int) (string, error) {
	js, err := json.Marshal(obj)
	if err != nil {
		return "", err
	}
	cjs := C.CString(string(js))
	defer C.free(unsafe.Pointer(cjs))
	out := C.sml_dump(cjs)
	if out == nil {
		var e C.sml_error
		C.sml_dump_err(cjs, &e)
		return "", errorFrom(&e)
	}
	return takeStr(out), nil
}

// Version 版本串（静态，不释放）。
func Version() string {
	p := C.sml_version()
	if p == nil {
		return ""
	}
	return C.GoString(p)
}

// Features 支持的特性名列表。
func Features() []string {
	mask := uint(C.sml_features_mask())
	var names []string
	for b := 0; b < 64; b++ {
		bit := uint(1) << uint(b)
		if bit > mask {
			break
		}
		p := C.sml_feature_name(C.uint(b))
		if p == nil {
			continue
		}
		n := C.GoString(p) // 静态，不释放
		if n != "" {
			names = append(names, n)
		}
	}
	return names
}

// GetIn 在已物化对象上按 "a.b.c" / "a.0" 路径便利取值（宿主侧，不走 ABI）。
func GetIn(obj interface{}, path string) interface{} {
	cur := obj
	for _, seg := range splitDot(path) {
		if cur == nil {
			return nil
		}
		switch c := cur.(type) {
		case map[string]interface{}:
			cur = c[seg]
		case []interface{}:
			idx, ok := atoi(seg)
			if !ok || idx < 0 || idx >= len(c) {
				return nil
			}
			cur = c[idx]
		default:
			return nil
		}
	}
	return cur
}

func splitDot(s string) []string {
	var out []string
	cur := ""
	for _, r := range s {
		if r == '.' {
			out = append(out, cur)
			cur = ""
		} else {
			cur += string(r)
		}
	}
	out = append(out, cur)
	return out
}

func atoi(s string) (int, bool) {
	if s == "" {
		return 0, false
	}
	n := 0
	for _, r := range s {
		if r < '0' || r > '9' {
			return 0, false
		}
		n = n*10 + int(r-'0')
	}
	return n, true
}

// Equal 深比较物化对象（便于测试断言）。
func Equal(a, b interface{}) bool {
	return reflect.DeepEqual(a, b)
}
