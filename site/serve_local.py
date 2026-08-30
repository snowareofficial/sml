#!/usr/bin/env python3
import subprocess, os, time, re, urllib.request, ctypes

SITE = r"c:\Users\sakeen\Desktop\sml\site"
PUB = os.path.join(SITE, "public")
DEVLOG = os.path.join(SITE, "dev.log")
WRANGLER = r"C:\Users\sakeen\AppData\Roaming\npm\wrangler.cmd"

# 清空日志
open(DEVLOG, "wb").close()

# 直接拉起 wrangler.cmd 作为后台子进程，stdout/stderr 写入 dev.log
# 用 CREATE_NEW_PROCESS_GROUP (0x200) + DETACHED_PROCESS (0x8) 彻底脱钩
DETACHED = 0x00000008
NEW_PG   = 0x00000200
CREATE_NO_WINDOW = 0x08000000

logf = open(DEVLOG, "ab")
p = subprocess.Popen(
    [WRANGLER, 'pages', 'dev', PUB, '--port', '8788'],
    cwd=SITE,
    stdout=logf, stderr=subprocess.STDOUT,
    stdin=subprocess.DEVNULL,
    close_fds=True,
    creationflags=DETACHED | NEW_PG | CREATE_NO_WINDOW,
)
print("launched pid=%s" % p.pid)
logf.flush()

# 等 URL 出现（最多 120s）
url = None
for i in range(240):
    time.sleep(0.5)
    try:
        d = open(DEVLOG, "rb").read()
    except OSError:
        continue
    txt = d.decode("utf-8", "replace")
    m = re.search(r"https?://(?:localhost|127\.0\.0\.1):8788[^\s]*", txt)
    if m:
        url = m.group(0); break
    # 也检测端口直接被监听
    try:
        urllib.request.urlopen("http://localhost:8788/", timeout=1)
        url = "http://localhost:8788/"; break
    except Exception:
        pass

if url:
    print("PREVIEW READY:", url)
else:
    d = open(DEVLOG, "rb").read()
    print("not ready after 120s; log size=%d tail:" % len(d))
    print(d[-1500:].decode("utf-8", "replace"))

# 最终探活
try:
    r = urllib.request.urlopen("http://localhost:8788/", timeout=5)
    print("PORT CHECK: HTTP", r.status)
except Exception as e:
    print("PORT CHECK:", str(e)[:120])
