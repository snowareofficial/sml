# -*- coding: utf-8 -*-
"""守卫：私有资产（报送件 / 内部报告）**不许**出现在公开主库里。

背景：这些文件的正主是内网私库 `CrystalicCore/sml_secret`
（`http://10.16.144.2:3000/CrystalicCore/sml_secret.git`，工作副本在仓库**之外**）。
主库 `.gitignore` 里有对应规则挡着，但「`git add -f` 手滑」「改名后不再匹配」
这类事故 gitignore 挡不住 —— 所以再加一道**可执行的**检查。

检查三项：
1. 任何提交（含已删除文件）的历史里**没有**这些名字；
2. 当前索引里**没有**它们；
3. `.gitignore` 里那几条规则**还在**（防止有人顺手删掉）。

用法：`python tools/check_private_assets.py`（rc=0 通过，rc=1 报警）。
"""
import subprocess
import sys

# ⚠️ 这里只写**文件名模式**，不写文件内容、更不写任何个人信息
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
]

try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass


def git(*args):
    out = subprocess.run(["git"] + list(args), capture_output=True, text=True,
                         encoding="utf-8", errors="replace").stdout or ""
    return out


fails = []

# 1) 历史
hist = [l for l in git("log", "--all", "--name-only", "--pretty=format:").splitlines() if l.strip()]
for pat in PATTERNS:
    bad = sorted({h for h in hist if pat in h})
    if bad:
        fails.append("历史里出现了 %s：%s" % (pat, bad))

# 2) 索引
indexed = [l for l in git("ls-files").splitlines() if l.strip()]
for pat in PATTERNS:
    bad = [i for i in indexed if pat in i]
    if bad:
        fails.append("索引里出现了 %s：%s" % (pat, bad))

# 3) ignore 规则还在
gi = git("show", "HEAD:.gitignore") if git("rev-parse", "--verify", "HEAD") else ""
missing = [l for l in IGNORE_LINES if l not in gi]
if missing:
    fails.append(".gitignore 的私有件规则缺失：%s" % missing)

if fails:
    print("FAIL: 私有资产守卫未通过")
    for f in fails:
        print("  -", f)
    print("\n这些文件的正主是内网私库 sml_secret；见 HANDOFF §18。")
    sys.exit(1)

print("OK: 私有资产未进入主库（历史 / 索引），.gitignore 规则在位")
print("    正主：http://10.16.144.2:3000/CrystalicCore/sml_secret.git")
