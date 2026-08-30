import subprocess, os

os.chdir(r"c:/Users/sakeen/Desktop/sml")

# 读取 commit message
with open("_msg_vscode.txt", "r", encoding="utf-8") as f:
    msg = f.read()

# 写临时文件避免命令行编码问题
with open("_cm.txt", "w", encoding="utf-8") as f:
    f.write(msg)

r = subprocess.run(["git", "add", "-A"], capture_output=True, text=True)
print("add rc=", r.returncode, r.stderr)
r = subprocess.run(["git", "commit", "-F", "_cm.txt"], capture_output=True, text=True)
print("commit rc=", r.returncode)
print(r.stdout[-500:] if r.stdout else "")
print(r.stderr[-500:] if r.stderr else "")

try:
    os.remove("_cm.txt")
    os.remove("_msg_vscode.txt")
except Exception as e:
    print("cleanup warn:", e)

r = subprocess.run(["git", "log", "--oneline", "-2"], capture_output=True, text=True)
print(r.stdout)
