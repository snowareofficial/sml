// SPDX-License-Identifier: MulanPSL-2.0
// SML 多语言宿主绑定（Go / cgo）。
// 仓库：https://gitee.com/snoware/sml
// @version 0.1.0 (swsml 0.6.3)

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
// 12..18 与 c/sml_rs.h 逐位一致（守护：tools/check/check_amberice_align.py）
SMLF_WHEN          = 1 << 12
SMLF_FOR           = 1 << 13
SMLF_TYPED_BLOCK   = 1 << 14
SMLF_CASE_INSENSITIVE = 1 << 15
SMLF_BLOCK_NAME    = 1 << 16
SMLF_FEATAG        = 1 << 17
SMLF_IDENT         = 1 << 18
	// SMLF_WEAK 弱声明族（@weakbind / @weakdata / @weakdoc）：opt-in、不在基线里。
	// LoadsFull 的 Flags 是**特性上限**，要校验弱绑定得显式带上这一位。
	SMLF_WEAK  = 1 << 19
	SMLF_BASIC = SMLF_BAREWORD_STR | SMLF_INCLUDE | SMLF_FRAGMENT
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

// Diagnostic 一条非致命诊断（键冲突 W-PARSE-001 / 片段覆盖 W-PARSE-002 / 弱声明不匹配 W-CONTRACT-001）。
type Diagnostic struct {
	Code     string // 稳定码；该档暂无码时为空串
	Text     string // 人读文案（不含码）
	Line     int    // 1 起；0 = 未知
	Severity int    // 0 = warning，1 = error（预留）
}

// LoadsWithDiags 文本 -> 原生对象 + 非致命诊断。
//
// 与 Loads 的差别只有返回值：值树逐字节相同，诊断只说明「数据被改写 / 覆盖过」，
// 不阻断解析。致命错误仍走 error。
func LoadsWithDiags(text string, flags int) (interface{}, []Diagnostic, error) {
	ctext := C.CString(text)
	defer C.free(unsafe.Pointer(ctext))
	var err C.sml_error
	var dl C.sml_diag_list
	root := C.sml_parse_with_diags(ctext, C.uint(flags), &dl, &err)
	if root == nil {
		C.sml_diag_list_free(&dl)
		return nil, nil, errorFrom(&err)
	}
	defer C.sml_free(root)
	defer C.sml_diag_list_free(&dl)
	diags := make([]Diagnostic, 0, int(dl.len))
	if dl.len > 0 {
		items := unsafe.Slice((*C.sml_diag)(unsafe.Pointer(dl.items)), int(dl.len))
		for _, d := range items {
			diags = append(diags, Diagnostic{
				Code:     C.GoString(d.code_str),
				Text:     C.GoString(d.text),
				Line:     int(d.line),
				Severity: int(d.severity),
			})
		}
	}
	return materialize(root), diags, nil
}

// Builtin 内置契约表（`@weakbind` 软校验用）；空表 ⇒ 软校验整段跳过。
//
// 形状与 C 端 `sml_builtin` 一致：AddField 遇到不存在的契约会自动建。
// 构造方法返回 false 表示参数非法（NULL / 未知类型 / 字段不存在），此时表不动。
type Builtin struct {
	p *C.sml_builtin
}

// NewBuiltin 新建空表。
func NewBuiltin() *Builtin {
	return &Builtin{p: C.sml_builtin_new()}
}

// adoptBuiltin 接管一个已归本进程所有的 sml_builtin*（如 sml_builtin_clone 的产物）。
func adoptBuiltin(p *C.sml_builtin) *Builtin {
	if p == nil {
		return nil
	}
	return &Builtin{p: p}
}

// Close 释放句柄（可重复调用）。
func (b *Builtin) Close() {
	if b != nil && b.p != nil {
		C.sml_builtin_free(b.p)
		b.p = nil
	}
}

// Len 表里有几份契约。
func (b *Builtin) Len() int {
	if b == nil || b.p == nil {
		return 0
	}
	return int(C.sml_builtin_count(b.p))
}

// AddField 声明字段；ty 取 any / str / int / num / bool / enum。
func (b *Builtin) AddField(contract, field, ty string, required bool) bool {
	if b == nil || b.p == nil {
		return false
	}
	cc, cf, ct := C.CString(contract), C.CString(field), C.CString(ty)
	defer C.free(unsafe.Pointer(cc))
	defer C.free(unsafe.Pointer(cf))
	defer C.free(unsafe.Pointer(ct))
	req := C.int(0)
	if required {
		req = 1
	}
	return C.sml_builtin_add_field(b.p, cc, cf, ct, req) == 0
}

// SetRange 给已有字段设数值区间（含端点）。
func (b *Builtin) SetRange(contract, field string, min, max float64) bool {
	if b == nil || b.p == nil {
		return false
	}
	cc, cf := C.CString(contract), C.CString(field)
	defer C.free(unsafe.Pointer(cc))
	defer C.free(unsafe.Pointer(cf))
	return C.sml_builtin_set_range(b.p, cc, cf, C.double(min), C.double(max)) == 0
}

// AddEnum 给 enum 字段追加合法取值。
func (b *Builtin) AddEnum(contract, field, value string) bool {
	if b == nil || b.p == nil {
		return false
	}
	cc, cf, cv := C.CString(contract), C.CString(field), C.CString(value)
	defer C.free(unsafe.Pointer(cc))
	defer C.free(unsafe.Pointer(cf))
	defer C.free(unsafe.Pointer(cv))
	return C.sml_builtin_add_enum(b.p, cc, cf, cv) == 0
}

// Opts LoadsFull 的选项：特性上限 + $env 覆盖表 + 内置契约表。
type Opts struct {
	// Flags 是**特性上限**（SMLF_* 位）；0 = 基线，不含 type.weak。
	Flags uint32
	// Env 是 "KEY=VALUE" 数组，供 $env.X 内联；只作用于本次解析。
	Env []string
	// Builtins 可空；非空时对 @weakbind 做软校验。
	Builtins *Builtin
}

// WeakBind 一条弱绑定（`@weakbind`）：目标块路径 + 契约名。
type WeakBind struct {
	Path string
	Name string
	Line int
}

// WeakData 一条弱数据（`@weakdata`）：载荷不在主树里，已物化成原生对象。
type WeakData struct {
	Path  string
	Name  string
	Line  int
	Value interface{}
}

// WeakDoc 一条弱 profile 引用（`@weakdoc`）：只有名字。
type WeakDoc struct {
	Path string
	Name string
	Line int
}

// WeakLists 弱声明三表；三者都不进主树。
type WeakLists struct {
	Binds []WeakBind
	Data  []WeakData
	Docs  []WeakDoc
}

// optsGuard 持有本次调用需要的 C 内存（env 串 + 指针数组），Close 时统一释放。
//
// 指针数组必须落在 **C 内存**里：cgo 的检查器不允许把「含指针的 Go 内存」交给 C
// （"Go pointer to unpinned Go pointer"）。
type optsGuard struct {
	o     C.sml_opts
	strs  []*C.char
	block unsafe.Pointer
}

func newOptsGuard(opts Opts) *optsGuard {
	g := &optsGuard{}
	g.o.struct_size = C.size_t(unsafe.Sizeof(g.o))
	g.o.flags = C.uint(opts.Flags)
	for _, kv := range opts.Env {
		g.strs = append(g.strs, C.CString(kv))
	}
	if len(g.strs) > 0 {
		g.block = C.malloc(C.size_t(len(g.strs)) * C.size_t(unsafe.Sizeof(uintptr(0))))
		arr := (*[1 << 20]*C.char)(g.block)[:len(g.strs):len(g.strs)]
		for i, p := range g.strs {
			arr[i] = p
		}
		g.o.env = (**C.char)(g.block)
		g.o.env_len = C.size_t(len(g.strs))
	}
	if opts.Builtins != nil && opts.Builtins.p != nil {
		g.o.builtins = opts.Builtins.p
	}
	return g
}

func (g *optsGuard) Close() {
	for _, p := range g.strs {
		C.free(unsafe.Pointer(p))
	}
	if g.block != nil {
		C.free(g.block)
	}
}

func readDiags(dl *C.sml_diag_list) []Diagnostic {
	diags := make([]Diagnostic, 0, int(dl.len))
	if dl.len > 0 {
		items := unsafe.Slice((*C.sml_diag)(unsafe.Pointer(dl.items)), int(dl.len))
		for _, d := range items {
			diags = append(diags, Diagnostic{
				Code:     C.GoString(d.code_str),
				Text:     C.GoString(d.text),
				Line:     int(d.line),
				Severity: int(d.severity),
			})
		}
	}
	return diags
}

func readWeakLists(wl *C.sml_weaklists) WeakLists {
	out := WeakLists{}
	if wl.n_binds > 0 && wl.binds != nil {
		items := unsafe.Slice((*C.sml_weakbind)(unsafe.Pointer(wl.binds)), int(wl.n_binds))
		for _, b := range items {
			out.Binds = append(out.Binds, WeakBind{
				Path: C.GoString(b.path), Name: C.GoString(b.name), Line: int(b.line),
			})
		}
	}
	if wl.n_data > 0 && wl.data != nil {
		items := unsafe.Slice((*C.sml_weakdata)(unsafe.Pointer(wl.data)), int(wl.n_data))
		for _, d := range items {
			out.Data = append(out.Data, WeakData{
				Path: C.GoString(d.path), Name: C.GoString(d.name), Line: int(d.line),
				Value: materialize(d.value),
			})
		}
	}
	if wl.n_docs > 0 && wl.docs != nil {
		items := unsafe.Slice((*C.sml_weakdoc)(unsafe.Pointer(wl.docs)), int(wl.n_docs))
		for _, d := range items {
			out.Docs = append(out.Docs, WeakDoc{
				Path: C.GoString(d.path), Name: C.GoString(d.name), Line: int(d.line),
			})
		}
	}
	return out
}

// LoadsFull 解析文本，带完整选项，返回原生对象 + 非致命诊断。
//
// 值树与 Loads(text, flags) 逐字节相同：选项只影响 $env 取值、特性上限与诊断内容。
func LoadsFull(text string, opts Opts) (interface{}, []Diagnostic, error) {
	obj, diags, _, err := LoadsFullLists(text, opts)
	return obj, diags, err
}

// LoadsFullLists 同 LoadsFull，另交出弱声明三表（@weakbind / @weakdata / @weakdoc）。
func LoadsFullLists(text string, opts Opts) (interface{}, []Diagnostic, WeakLists, error) {
	ctext := C.CString(text)
	defer C.free(unsafe.Pointer(ctext))

	g := newOptsGuard(opts)
	defer g.Close()

	var err C.sml_error
	var dl C.sml_diag_list
	var wl C.sml_weaklists
	root := C.sml_parse_full_lists(ctext, &g.o, &wl, &dl, &err)
	if root == nil {
		C.sml_weaklists_free(&wl)
		C.sml_diag_list_free(&dl)
		return nil, nil, WeakLists{}, errorFrom(&err)
	}
	defer C.sml_free(root)
	defer C.sml_diag_list_free(&dl)
	defer C.sml_weaklists_free(&wl)
	diags := readDiags(&dl)
	lists := readWeakLists(&wl)
	return materialize(root), diags, lists, nil
}

// parse_all 的 want 位（与 c/sml_rs.h 的 SML_WANT_* 逐位一致）。
const (
	WantDiags       = 1 << 0 // 非致命诊断
	WantErrors      = 1 << 1 // 多错误收集（语法错也返回结果）
	WantWeak        = 1 << 2 // 弱声明三表
	WantContractMap = 1 << 3 // 契约映射
	WantContracts   = 1 << 4 // 捕获的契约表（独立克隆，可再当内置表注入）
	WantAll         = 0x1F
)

// ContractRef 一条「某实体应用了某个契约」。
type ContractRef struct {
	Path string
	Name string
}

// ParseAll 一次解析的全部产物；未请求的部分是空表 / nil。
type ParseAll struct {
	Value       interface{}
	Diagnostics []Diagnostic
	Errors      []Diagnostic
	Weak        WeakLists
	ContractMap []ContractRef
	// Contracts 是**独立克隆**的表（调用方负责 Close）；未请求时为 nil。
	Contracts *Builtin
}

// ParseAllOf 一次拿全：值树 + 诊断 + 多错误 + 弱表 + 契约映射 + 契约表。
//
// 开了 WantErrors 后语法/契约错**不再返回 error**：值树是尽力而为的部分结果，
// 全部错误在 Errors 里（Severity == 1）。
func ParseAllOf(text string, opts Opts, want int) (*ParseAll, error) {
	ctext := C.CString(text)
	defer C.free(unsafe.Pointer(ctext))

	g := newOptsGuard(opts)
	defer g.Close()

	var err C.sml_error
	r := C.sml_parse_all(ctext, &g.o, C.uint(want), &err)
	if r == nil {
		return nil, errorFrom(&err)
	}
	defer C.sml_result_free(r)

	out := &ParseAll{Value: materialize(C.sml_result_value(r))}
	if want&WantDiags != 0 {
		out.Diagnostics = readDiags(C.sml_result_diags(r))
	}
	if want&WantErrors != 0 {
		out.Errors = readDiags(C.sml_result_errors(r))
	}
	if want&WantWeak != 0 {
		out.Weak = readWeakLists(C.sml_result_weak(r))
	}
	if want&WantContractMap != 0 {
		cm := C.sml_result_contract_map(r)
		if cm != nil && cm.len > 0 {
			items := unsafe.Slice((*C.sml_contract_ref)(unsafe.Pointer(cm.items)), int(cm.len))
			for _, it := range items {
				out.ContractMap = append(out.ContractMap, ContractRef{
					Path: C.GoString(it.path), Name: C.GoString(it.name),
				})
			}
		}
	}
	if want&WantContracts != 0 {
		// 借用的句柄不能活过结果 ⇒ 立刻克隆成宿主独占的一份。
		out.Contracts = adoptBuiltin(C.sml_builtin_clone(C.sml_result_contracts(r)))
	}
	return out, nil
}

// LoadFileFull 同 LoadFile，另交出诊断与弱声明三表（文件入口版的 LoadsFullLists）。
//
// 展开 include，相对路径以文件所在目录为基准；跨 include 时行号可能为 0。
func LoadFileFull(path string, opts Opts) (interface{}, []Diagnostic, WeakLists, error) {
	cpath := C.CString(path)
	defer C.free(unsafe.Pointer(cpath))

	g := newOptsGuard(opts)
	defer g.Close()

	var err C.sml_error
	var dl C.sml_diag_list
	var wl C.sml_weaklists
	root := C.sml_load_file_full(cpath, &g.o, &wl, &dl, &err)
	if root == nil {
		C.sml_weaklists_free(&wl)
		C.sml_diag_list_free(&dl)
		return nil, nil, WeakLists{}, errorFrom(&err)
	}
	defer C.sml_free(root)
	defer C.sml_diag_list_free(&dl)
	defer C.sml_weaklists_free(&wl)
	return materialize(root), readDiags(&dl), readWeakLists(&wl), nil
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
