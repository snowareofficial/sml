import subprocess, os

BIN = "E:/smv-target/debug/smlconv.exe"
EX = "c:/Users/sakeen/Desktop/sml/examples"

def run(args, inp=None):
    p = subprocess.run([BIN] + args, input=inp, capture_output=True,
                       cwd="c:/Users/sakeen/Desktop/sml/rust")
    return p.returncode, p.stdout.decode("utf-8","replace"), p.stderr.decode("utf-8","replace")

rc, out, err = run(["--to", "md"], inp='h1 { text: "Hello" }\np { text: "World" }\n')
print("T1 rc=", rc, "OUT:", repr(out[:300]), "ERR:", err[:200])

rc, out, err = run(["-i", f"{EX}/app.sml", "--to", "xml", "-o", "e:/o_xml_out.xml"])
print("T2 rc=", rc, "ERR:", err[:200])
if os.path.exists("e:/o_xml_out.xml"):
    print("T2 xml head:", open("e:/o_xml_out.xml",encoding="utf-8").read()[:200])

rc, out, err = run(["-i", f"{EX}/app.sml", "--to", "md", "--hugo", "e:/hugo_out", "--hugo-lang", "zh", "--hugo-section", "docs"])
print("T3 rc=", rc, "ERR:", err[:300])
import os as _os
for root,_,files in _os.walk("e:/hugo_out"):
    for f in files:
        print("  hugo file:", _os.path.join(root,f))
