import subprocess, os, sys

BIN = "E:/smv-target/debug/smlconv.exe"
EX = "c:/Users/sakeen/Desktop/sml/examples"

def run(args, inp=None):
    p = subprocess.run([BIN] + args, input=inp, capture_output=True,
                       cwd="c:/Users/sakeen/Desktop/sml/rust")
    return p.returncode, p.stdout.decode("utf-8","replace"), p.stderr.decode("utf-8","replace")

# 测试 1: stdin -> markdown
rc, out, err = run(["--to", "md"], inp='h1 { text: "Hello" }\np { text: "World" }\n')
print("=== T1 stdin->md rc=", rc, "===")
print(out)
print("ERR:", err[:500])

# 测试 2: file -> xml
rc, out, err = run(["-i", f"{EX}/app.sml", "--to", "xml"])
print("=== T2 file->xml rc=", rc, "===")
print(out[:600])
print("ERR:", err[:300])

# 测试 3: hugo
rc, out, err = run(["-i", f"{EX}/app.sml", "--to", "md", "--hugo", "e:/hugo_out", "--hugo-lang", "zh", "--hugo-section", "docs"])
print("=== T3 hugo rc=", rc, "===")
print("ERR:", err[:500])

# 测试 4: svg on full.sml
rc, out, err = run(["-i", f"{EX}/full.sml", "--to", "svg"])
print("=== T4 svg rc=", rc, "===")
print("ERR:", err[:500], "OUT_LEN", len(out))

# 测试 5: 错误输入 (孤立 @)
rc, out, err = run(["--to", "md"], inp='@\nfoo: 1\n')
print("=== T5 error rc=", rc, "===")
print("ERR:", err[:300])

# 测试 6: slint
rc, out, err = run(["-i", f"{EX}/app.sml", "--to", "slint"])
print("=== T6 slint rc=", rc, "===")
print("ERR:", err[:300], "OUT_LEN", len(out))

# 测试 7: latex
rc, out, err = run(["-i", f"{EX}/app.sml", "--to", "latex"])
print("=== T7 latex rc=", rc, "===")
print("ERR:", err[:300], "OUT_LEN", len(out))
