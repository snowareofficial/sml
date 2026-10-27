# -*- coding: utf-8 -*-
"""守卫：私有资产、隐私串**不许**出现在公开主库里。

背景：私有报送件的正主是**仓库之外**的私有库；地址与库名只写在本地
`.git/info/exclude` 与私有库里，**不写进公开仓库**（2026-10-28 脱敏）。

两道检查：
A. 私有报送件（文件名，硬编码在下方 PATTERNS）—— 历史 / 索引都不能有；
B. 隐私串 —— 本机绝对路径、真实邮箱、令牌形状、内网地址（RFC1918）。
   具体的内网主机名 / 组织名 / 内部代号不放代码里（否则守卫自己就泄密），
   只从本地未入库的 `tools/.private_patterns` 读，一行一个**字面串**。

用法：
  python tools/check_private_assets.py            # 快：索引内容 + 提交信息 + 路径
  python tools/check_private_assets.py --history  # 慢：再把全部历史 blob 内容扫一遍
rc=0 通过，rc=1 报警。
"""
import os
import re
import subprocess
import sys

PATTERNS = [
    "SML_政务数据密级标注规范_报送稿",
    "SML图形管线项目_全量文档合集.md",
    "SML_数字字面量保真性审计报告.md",
    "报送邮件.txt",
]
IGNORE_LINES = [
    "SML_政务数据密级标注规范_报送稿.*",
    "SML图形管线项目_全量文档合集.md",
    "SML_数字字面量保真性审计报告.md",
    "报送邮件.txt",
    "tools/.private_patterns",
]

# 通用隐私串：本身不含秘密，写死没问题
GENERIC = [
    (r"[A-Za-z]:[\\/]Users[\\/]", "本机绝对路径（盘符 + Users）"),
    (r"/home/[a-z_][\w-]*/", "本机绝对路径（/home/<user>）"),
    (r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}", "GitHub 令牌"),
    (r"\bglpat-[A-Za-z0-9_-]{20,}", "GitLab 令牌"),
    (r"\bAKIA[0-9A-Z]{16}\b", "AWS Access Key"),
    (r"-----BEGIN [A-Z ]*PRIVATE KEY-----", "私钥"),
    (r"\b(?:10|192\.168|172\.(?:1[6-9]|2\d|3[01]))\.\d{1,3}\.\d{1,3}\.\d{1,3}\b", "内网地址（RFC1918）"),
    (r"[\w.+-]+@(?!users\.noreply\.github\.com\b|noreply\.gitee\.com\b)[\w-]+(?:\.[\w-]+)*\.[A-Za-z]{2,}",
     "真实邮箱（noreply 除外）"),
]
# 允许出现的占位符 / 公开地址（命中行含这些就不算）
ALLOW = ["<user>", "<用户名>", "<你>", "<target-dir>", "<tmp-dir>", "<node-dir>", "<vscode-dir>",
         "<private-repo-url>", "<gitea-host>", "<private-org>", "<private-repo>",
         "example.com", "localhost", "127.0.0.1", "@users.noreply.github.com",
         # 项目官方联系邮箱：README / Cargo.toml / debian / 站点都公开发布，属于有意公开
         "@mail.swebase.cn",
         # 示例文档里的保留网段与演示主机（demo 数据，非真实内网）
         "10.0.0.0/8", "172.16.4.0/24", "10.20.0.0/16", "10.20.9.31",
         # GitHub Actions 运行器路径
         "/home/runner/"]

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def git(*args, stdin=None):
    p = subprocess.run(["git"] + list(args), capture_output=True, text=True,
                       encoding="utf-8", errors="replace", input=stdin)
    return p.stdout or ""


def load_local_patterns():
    path = os.path.join(ROOT, "tools", ".private_patterns")
    if not os.path.exists(path):
        return []
    out = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line and not line.startswith("#"):
                out.append(line)
    return out


def allowed(line):
    return any(a in line for a in ALLOW)


fails = []


def scan_text(label, rev, path, lineno, line, patterns):
    if allowed(line):
        return
    for rx, desc in patterns:
        if rx.search(line):
            fails.append("%s %s:%s 命中「%s」" % (label, path, lineno, desc))


def generic_res():
    return [(re.compile(rx), desc) for rx, desc in GENERIC]


def local_res(literals):
    return [(re.compile(re.escape(s), re.IGNORECASE), "本地敏感串") for s in literals]


# ── 1) 私有报送件：历史路径 + 索引 ────────────────────────────────────────
hist = [l for l in git("log", "--all", "--name-only", "--pretty=format:").splitlines() if l.strip()]
indexed = [l for l in git("ls-files").splitlines() if l.strip()]
for pat in PATTERNS:
    bad = sorted({h for h in hist if pat in h})
    if bad:
        fails.append("历史里出现了 %s：%s" % (pat, bad))
    bad = [i for i in indexed if pat in i]
    if bad:
        fails.append("索引里出现了 %s：%s" % (pat, bad))

# ── 2) .gitignore 规则还在 ───────────────────────────────────────────────
gi = git("show", "HEAD:.gitignore") if git("rev-parse", "--verify", "HEAD") else ""
missing = [l for l in IGNORE_LINES if l not in gi]
if missing:
    fails.append(".gitignore 的私有件规则缺失：%s" % missing)

# ── 3) 索引内容：通用隐私串 + 本地敏感串 ─────────────────────────────────
rules = generic_res() + local_res(load_local_patterns())
for path in indexed:
    fs = os.path.join(ROOT, path)
    try:
        with open(fs, encoding="utf-8", errors="replace") as f:
            for i, line in enumerate(f, 1):
                scan_text("索引", "HEAD", path, i, line, rules)
    except (OSError, UnicodeError):
        pass

# ── 4) 提交信息 + 历史路径 ───────────────────────────────────────────────
msgs = git("log", "--all", "--format=%H%x00%B%x00").split("\x00")
for i in range(0, len(msgs) - 1, 2):
    sha, body = msgs[i].strip(), msgs[i + 1]
    for j, line in enumerate(body.splitlines(), 1):
        scan_text("提交信息", sha[:10], "msg", j, line, rules)
for p in sorted(set(hist)):
    for rx, desc in rules:
        if rx.search(p) and not allowed(p):
            fails.append("历史路径 %s 命中「%s」" % (p, desc))

# ── 5) --history：全部历史 blob 内容 ─────────────────────────────────────
if "--history" in sys.argv:
    revs = git("rev-list", "--all").split()
    pats = subprocess.run(
        ["git", "grep", "-I", "-n", "-E", "-f", "-"] + revs,
        capture_output=True, text=True, encoding="utf-8", errors="replace",
        input="\n".join(rx.pattern for rx, _ in rules) + "\n").stdout or ""
    for line in pats.splitlines():
        parts = line.split(":", 3)
        if len(parts) < 4:
            continue
        rev, path, lineno, content = parts
        scan_text("历史", rev[:10], path, lineno, content, rules)

if fails:
    uniq = sorted(set(fails))
    print("FAIL: 隐私 / 私有资产守卫未通过（%d 条）" % len(uniq))
    for f in uniq[:60]:
        print("  -", f)
    if len(uniq) > 60:
        print("  ... 另有 %d 条" % (len(uniq) - 60))
    print("\n私有报送件的正主在仓库之外的私有库；内网主机名见本地 tools/.private_patterns。")
    sys.exit(1)

print("OK: 索引内容 / 提交信息 / 历史路径均无隐私串，私有报送件未入库，.gitignore 规则在位")
print("    正主在仓库之外的私有库（地址见本地 .git/info/exclude）")
