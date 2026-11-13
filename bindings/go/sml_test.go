// SPDX-License-Identifier: MulanPSL-2.0
// SML Go 绑定自测。
// 仓库：https://gitee.com/snoware/sml
// @version 0.1.0 (swsml 0.6.3)

package sml

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

const golden = `server {
  host: "example.com"
  port: 8080
  ratio: 0.75
  tls: false
  tags: [ a b c ]
}
`

func expected() map[string]interface{} {
	return map[string]interface{}{
		"server": map[string]interface{}{
			"host":  "example.com",
			"port":  int64(8080),
			"ratio": 0.75,
			"tls":   false,
			"tags":  []interface{}{"a", "b", "c"},
		},
	}
}

var failures = 0

func check(t *testing.T, name string, cond bool, detail string, args ...interface{}) {
	if cond {
		t.Logf("  PASS  %s", name)
	} else {
		msg := detail
		if len(args) > 0 {
			msg = fmt.Sprintf(detail, args...)
		}
		t.Errorf("  FAIL  %s  %s", name, msg)
		failures++
	}
}

func TestSml(t *testing.T) {
	exp := expected()

	// 1. Loads 黄金向量
	obj, err := Loads(golden, 0)
	check(t, "1 Loads(golden)==expected (port int64, ratio float64, tls bool, tags 3)", Equal(obj, exp), "got=%v err=%v", obj, err)

	// 2. 往返
	js, err := Dumps(exp, 0)
	if err != nil {
		t.Fatalf("Dumps err: %v", err)
	}
	back, err := Loads(js, 0)
	check(t, "2 round-trip Dumps/Loads", err == nil && Equal(back, exp), "got=%v err=%v", back, err)

	// 3. 富错误：未闭合块 -> E-PARSE-001 + text 非空
	_, e3 := Loads("bad { a: 1", 0)
	if se, ok := e3.(*SmlError); ok {
		check(t, "3 rich error code_str==E-PARSE-001", se.CodeStr == "E-PARSE-001", "got=%q", se.CodeStr)
		check(t, "3 rich error text 非空", se.Text != "", "text=%q", se.Text)
		check(t, "3 不是通用 parse failed", !contains(se.Text, "parse failed"), se.Text)
	} else {
		check(t, "3 rich error (unclosed)", false, "未抛 SmlError")
	}

	// 4. 错误定位：第 2 行契约错误 -> line==2, E-CONTRACT-001
	_, e4 := Loads("a: 1\n@is NoSuchContract\n", 0)
	if se, ok := e4.(*SmlError); ok {
		check(t, "4 line==2", se.Line == 2, "line=%d", se.Line)
		check(t, "4 code_str==E-CONTRACT-001", se.CodeStr == "E-CONTRACT-001", "got=%q", se.CodeStr)
	} else {
		check(t, "4 line location", false, "未抛 SmlError")
	}

	// 5. LoadFile + include
	dir, err := os.MkdirTemp("", "sml_w28")
	if err != nil {
		t.Fatal(err)
	}
	defer os.RemoveAll(dir)
	sub := filepath.Join(dir, "_inc_sub.sml")
	main := filepath.Join(dir, "_inc_main.sml")
	if err := os.WriteFile(sub, []byte("k: 1\n"), 0644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(main, []byte("include \"_inc_sub.sml\"\nm: 2\n"), 0644); err != nil {
		t.Fatal(err)
	}
	inc, err := LoadFile(main, 0)
	if err != nil {
		t.Fatalf("LoadFile err: %v", err)
	}
	incm, ok := inc.(map[string]interface{})
	if !ok {
		t.Fatalf("inc not map: %T", inc)
	}
	check(t, "5 include: 子文件字段 k=1", eqInt64(incm["k"], 1), "got=%v", incm["k"])
	check(t, "5 include: 主文件字段 m=2", eqInt64(incm["m"], 2), "got=%v", incm["m"])

	// 6. flags 沙箱
	envDoc := "x: $env.NOPE\n"
	_, be := Loads(envDoc, 0)
	baseOk := be == nil || (be.(*SmlError).CodeStr != "E-FEATURE-002")
	check(t, "6 baseline(flags=0) 不因特性报错", baseOk, "baseOk=%v", baseOk)

	noEnv := SMLF_BASIC &^ SMLF_ENV
	_, se6 := Loads(envDoc, noEnv)
	if se, ok := se6.(*SmlError); ok {
		check(t, "6 sandbox code_str==E-FEATURE-002", se.CodeStr == "E-FEATURE-002", "got=%q", se.CodeStr)
	} else {
		check(t, "6 sandbox: 无 ENV 应报错", false, "未抛 SmlError")
	}

	// 7. version / features
	v := Version()
	check(t, "7 version() 非空", v != "", v)
	feats := Features()
	check(t, "7 features() 含 'include'", containsStr(feats, "include"), "%v", feats)
	check(t, "7 features() 含 'env'/'contract'", containsStr(feats, "env") && containsStr(feats, "contract"), "%v", feats)

	// 8. 内存稳定
	for i := 0; i < 1000; i++ {
		if _, e := Loads(golden, 0); e != nil {
			t.Fatalf("迭代 %d 失败: %v", i, e)
		}
	}
	check(t, "8 1000x Loads 不崩溃", true, "")

	// 9. GetIn
	check(t, "9 GetIn 路径取值", GetIn(obj, "server.host") == "example.com", "")

	// 10. 诊断出口（P7）：带码带行号、不阻断解析、值树与 Loads 相同
	dup, diags, derr := LoadsWithDiags("a: 1\na: 2\n", 0)
	check(t, "10 重复键 -> 1 条 W-PARSE-001 @ line 2",
		derr == nil && len(diags) == 1 && diags[0].Code == "W-PARSE-001" &&
			diags[0].Line == 2 && diags[0].Severity == 0, "%v err=%v", diags, derr)
	check(t, "10 值树不变（仍是数组提升）",
		Equal(dup, map[string]interface{}{"a": []interface{}{int64(1), int64(2)}}), "%v", dup)
	_, none, _ := LoadsWithDiags("k: 1\n", 0)
	check(t, "10 干净文档 -> 零诊断", len(none) == 0, "%v", none)
	for i := 0; i < 200; i++ {
		LoadsWithDiags("a: 1\na: 2\n", 0)
	}
	check(t, "10 200x LoadsWithDiags 不崩溃", true, "")

	// 11. 完整选项（P7 第二片）：内置契约表 + env 注入
	bt := NewBuiltin()
	check(t, "11 空表 Len()==0", bt.Len() == 0, "%d", bt.Len())
	built := bt.AddField("tls", "cert", "str", true) &&
		bt.AddField("tls", "port", "int", false) &&
		bt.SetRange("tls", "port", 1, 65535) &&
		bt.AddField("tls", "mode", "enum", false) &&
		bt.AddEnum("tls", "mode", "strict")
	check(t, "11 建表 -> 1 份契约", built && bt.Len() == 1, "%d", bt.Len())
	check(t, "11 未知类型被拒（表不动）", !bt.AddField("tls", "bad", "image", false), "")

	src2 := "@feature enable type.weak\n" +
		"srv {\n  @weakbind tls\n  web { cert: 1 port: 99999 mode: nope }\n}\n" +
		"@weakbind nosuch\nother { x: 1 }\n"
	obj2, diags2, derr2 := LoadsFull(src2, Opts{Flags: SMLF_BASIC | SMLF_WEAK, Builtins: bt})
	weak := 0
	for _, d := range diags2 {
		if d.Code == "W-CONTRACT-001" {
			weak++
		}
	}
	check(t, "11 内置表 -> 4 条 W-CONTRACT-001", derr2 == nil && weak == 4, "%d err=%v", weak, derr2)
	if m, ok := obj2.(map[string]interface{}); ok {
		srv, _ := m["srv"].(map[string]interface{})
		web, _ := srv["web"].(map[string]interface{})
		check(t, "11 软校验只读：cert 仍是 int64 1", eqInt64(web["cert"], 1), "%v", web["cert"])
	} else {
		check(t, "11 软校验只读：cert 仍是 int64 1", false, "obj2 不是 map")
	}

	envObj, _, eerr := LoadsFull("k: $env.SML_GO_PROBE\n", Opts{Env: []string{"SML_GO_PROBE=prod"}})
	check(t, "11 env 注入 -> k=prod", eerr == nil && Equal(envObj, map[string]interface{}{"k": "prod"}), "%v err=%v", envObj, eerr)
	if _, ok := os.LookupEnv("SML_GO_PROBE"); ok {
		check(t, "11 env 不改进程环境", false, "进程环境里有这个变量")
	} else {
		check(t, "11 env 不改进程环境", true, "")
	}

	if _, _, berr := LoadsFull("@feature enable type.weak\n@weakbind tls\ns { x: 1 }\n",
		Opts{Flags: 0}); berr != nil {
		if se, ok := berr.(*SmlError); ok {
			check(t, "11 基线不含 type.weak -> E-FEATURE-001", se.CodeStr == "E-FEATURE-001", "%q", se.CodeStr)
		} else {
			check(t, "11 基线不含 type.weak -> E-FEATURE-001", false, "%v", berr)
		}
	} else {
		check(t, "11 基线不含 type.weak -> E-FEATURE-001", false, "未报错")
	}
	bt.Close()

	// 12. 弱声明三表（P7 第四片）：都不进主树，从 LoadsFullLists 取出
	wsrc := "@feature enable type.weak\n" +
		"srv {\n  @weakbind tls\n  web { cert: \"x\" }\n  @weakdata limits { max: 10 }\n}\n" +
		"@weakdoc base\n"
	wobj, _, wl, werr := LoadsFullLists(wsrc, Opts{Flags: SMLF_BASIC | SMLF_WEAK})
	check(t, "12 三表计数 1/1/1",
		werr == nil && len(wl.Binds) == 1 && len(wl.Data) == 1 && len(wl.Docs) == 1,
		"%d/%d/%d err=%v", len(wl.Binds), len(wl.Data), len(wl.Docs), werr)
	if len(wl.Binds) == 1 {
		b := wl.Binds[0]
		check(t, "12 bind 的 path/name/line",
			b.Path == "srv.web" && b.Name == "tls" && b.Line > 0, "%+v", b)
	}
	if len(wl.Data) == 1 {
		d := wl.Data[0]
		payload, _ := d.Value.(map[string]interface{})
		check(t, "12 weakdata 归属 srv/limits 且载荷可读",
			d.Path == "srv" && d.Name == "limits" && eqInt64(payload["max"], 10), "%+v", d)
	}
	if len(wl.Docs) == 1 {
		check(t, "12 profile 在顶层（path 为空串）",
			wl.Docs[0].Path == "" && wl.Docs[0].Name == "base", "%+v", wl.Docs[0])
	}
	if m, ok := wobj.(map[string]interface{}); ok {
		srv, _ := m["srv"].(map[string]interface{})
		_, has := srv["limits"]
		check(t, "12 主树无痕迹", !has, "%v", m)
	} else {
		check(t, "12 主树无痕迹", false, "obj 不是 map")
	}
	for i := 0; i < 100; i++ {
		LoadsFullLists(wsrc, Opts{Flags: SMLF_BASIC | SMLF_WEAK})
	}
	check(t, "12 100x LoadsFullLists 不崩溃", true, "")

	// 13. 聚合结果（ParseAllOf）：契约映射 + 契约表捕获 + 多错误
	const SMLF_CONTRACT = 1 << 3
	pa, perr := ParseAllOf("@contract tls { cert: str }\nsrv { @is tls\n  cert: x\n}\n",
		Opts{Flags: SMLF_BASIC | SMLF_CONTRACT}, WantAll)
	check(t, "13 契约映射 srv -> tls",
		perr == nil && len(pa.ContractMap) == 1 && pa.ContractMap[0].Path == "srv" &&
			pa.ContractMap[0].Name == "tls", "%v err=%v", pa, perr)
	check(t, "13 契约表已捕获（1 份）",
		perr == nil && pa.Contracts != nil && pa.Contracts.Len() == 1, "%v", perr)

	if perr == nil {
		_, wd2, _, werr2 := LoadsFullLists(
			"@feature enable type.weak\n@weakbind nosuch\ns { x: 1 }\n",
			Opts{Flags: SMLF_BASIC | SMLF_WEAK, Builtins: pa.Contracts})
		check(t, "13 闭环：注入捕获的表后报 decl.unknown_contract",
			werr2 == nil && len(wd2) == 1 && wd2[0].Code == "W-CONTRACT-001",
			"%v err=%v", wd2, werr2)
		pa.Contracts.Close()
	}

	bad2 := "a { @is No1\n  x: 1 }\nb { @is No2\n  y: 2 }\n"
	ba, berr := ParseAllOf(bad2, Opts{Flags: SMLF_BASIC | SMLF_CONTRACT}, WantAll)
	check(t, "13 多错误：2 条且 severity=1、带行号",
		berr == nil && len(ba.Errors) == 2 && ba.Errors[0].Code == "E-CONTRACT-001" &&
			ba.Errors[0].Line == 1 && ba.Errors[0].Severity == 1 && ba.Errors[1].Line == 3,
		"%v err=%v", ba, berr)
	if m, ok := ba.Value.(map[string]interface{}); ok {
		check(t, "13 多错误：值树是尽力而为的部分结果",
			len(m) == 2 && m["a"] != nil && m["b"] != nil, "%v", m)
	} else {
		check(t, "13 多错误：值树是尽力而为的部分结果", false, "%v", ba.Value)
	}

	if _, nerr := ParseAllOf(bad2, Opts{Flags: SMLF_BASIC | SMLF_CONTRACT}, WantDiags); nerr == nil {
		check(t, "13 未请求 ERRORS -> 仍返回 error", false, "未报错")
	} else if se, ok := nerr.(*SmlError); ok {
		check(t, "13 未请求 ERRORS -> 仍返回 error", se.CodeStr == "E-CONTRACT-001", "%q", se.CodeStr)
	} else {
		check(t, "13 未请求 ERRORS -> 仍返回 error", false, "%v", nerr)
	}

	for i := 0; i < 100; i++ {
		ParseAllOf(wsrc, Opts{Flags: SMLF_BASIC | SMLF_WEAK}, WantAll)
	}
	check(t, "13 100x ParseAllOf 不崩溃", true, "")

	// 14. 文件入口（LoadFileFull）：展开 include + 诊断 + 弱表
	tdir := t.TempDir()
	if err := os.WriteFile(filepath.Join(tdir, "sub.sml"), []byte("k: 1\n"), 0o600); err != nil {
		t.Fatalf("写 sub.sml 失败: %v", err)
	}
	mainPath := filepath.Join(tdir, "main.sml")
	if err := os.WriteFile(mainPath,
		[]byte("@feature enable type.weak\ninclude \"sub.sml\"\n@weakdoc base\n"), 0o600); err != nil {
		t.Fatalf("写 main.sml 失败: %v", err)
	}
	fobj, fdiags, fwl, ferr := LoadFileFull(mainPath, Opts{Flags: SMLF_BASIC | SMLF_WEAK})
	check(t, "14 文件入口：include 已展开",
		ferr == nil && Equal(fobj, map[string]interface{}{"k": int64(1)}), "%v err=%v", fobj, ferr)
	check(t, "14 文件入口：弱表带 @weakdoc",
		len(fwl.Docs) == 1 && fwl.Docs[0].Name == "base", "%+v", fwl.Docs)
	check(t, "14 文件入口：干净文件 -> 零诊断", len(fdiags) == 0, "%v", fdiags)
	if _, _, _, nerr := LoadFileFull(filepath.Join(tdir, "nope.sml"), Opts{}); nerr == nil {
		check(t, "14 文件不存在 -> E-IO-001", false, "未报错")
	} else if se, ok := nerr.(*SmlError); ok {
		check(t, "14 文件不存在 -> E-IO-001", se.CodeStr == "E-IO-001", "%q", se.CodeStr)
	} else {
		check(t, "14 文件不存在 -> E-IO-001", false, "%v", nerr)
	}

	if failures > 0 {
		t.Fatalf("%d 项断言失败", failures)
	}
}

func contains(s, sub string) bool {
	for i := 0; i+len(sub) <= len(s); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}

func containsStr(ss []string, want string) bool {
	for _, s := range ss {
		if s == want {
			return true
		}
	}
	return false
}

func eqInt64(v interface{}, want int64) bool {
	i, ok := v.(int64)
	return ok && i == want
}
