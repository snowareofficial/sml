import os, shutil
base = r"c:\Users\sakeen\Desktop\sml"
candidates = [
    os.path.join(base, "_t.sml"),
    os.path.join(base, "_run_test.py"),
    os.path.join(base, "_smktest.py"),
    os.path.join(base, "_out.xml"),
    os.path.join(base, "_err.txt"),
    os.path.join(base, "_j.json"),
    os.path.join(base, "_je.txt"),
    os.path.join(base, "_o.txt"),
    os.path.join(base, "rust", "_t.sml"),
    os.path.join(r"E:\snoware-target", "_hugotest"),
]
for c in candidates:
    try:
        if os.path.isdir(c):
            shutil.rmtree(c)
            print("rmdir", c)
        elif os.path.exists(c):
            os.remove(c)
            print("rm", c)
    except Exception as e:
        print("skip", c, e)
print("done")
