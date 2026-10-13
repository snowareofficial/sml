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
