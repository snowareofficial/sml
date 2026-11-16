# -*- coding: utf-8 -*-
"""守卫：私有资产、隐私串**不许**出现在公开主库里。

背景：私有报送件的正主是**仓库之外**的私有库；地址与库名只写在本地
`.git/info/exclude` 与私有库里，**不写进公开仓库**（2026-10-28 脱敏）。

四道检查：
A. 私有报送件（文件名，硬编码在下方 PATTERNS）—— 历史 / 索引都不能有；
B. 隐私串 —— 本机绝对路径、真实邮箱、令牌形状、内网地址（RFC1918）。
   具体的内网主机名 / 组织名 / 内部代号不放代码里（否则守卫自己就泄密），
   只从本地未入库的 `tools/.private_patterns` 读，一行一个**字面串**；
C. 图片资产 —— 每张都必须**逐张登记**（用途 + 指纹），指纹变了也要复查。
   图片是二进制，B 那道逐行文本扫描对它等于没扫：截图里的一条本机路径、
   一个真实邮箱、一段作者 / 软件名，都能原样进库而不触发任何告警。
   这里既扫图片里**能读的那部分**（PNG 文本块 / JPEG APP 段 / GIF 注释），
   也用指纹钉住「新图没登记」「老图被换掉」两种情况；
D. **全部非文本文件** —— 索引里所有二进制（vsix / zip / deb / ambr / wasm / 字体 /
   图片本体）逐个抽可读字符串（ASCII + UTF-16），并递归读容器成员，跑同一套隐私串规则。
   A 只认文件名、B 只逐行读文本、C 只读图片元数据和指纹；D 是最后一层兜底。

用法：
  python tools/check_private_assets.py            # 快：索引内容 + 提交信息 + 路径 + 图片
  python tools/check_private_assets.py --history  # 慢：再把全部历史 blob 内容扫一遍
rc=0 通过，rc=1 报警。
"""
import hashlib
import io
import os
import re
import subprocess
import sys
import tarfile
import zipfile
import zlib

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

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))


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

# ── 6) 图片资产：逐张白名单 + 指纹复查 + 元数据扫描 ─────────────────────
IMAGE_EXT = (".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".bmp", ".tif", ".tiff", ".avif")

# 每一张入库图片都必须在这里**逐张列出**：size + sha256 前 16 位 + 用途。
# 指纹不是防篡改，是**逼复查**：新图没登记 ⇒ 红；老图被替换（差一个字节）⇒ 红。
# 改图后请先确认它不含隐私（截图尤其要看：本机路径、真实邮箱、别人的文件名），
# 再把新指纹填进来。
IMAGES = {
    "editors/vscode/images/icon.png": (
        20020, "aa6b9495ff77d301",
        "VSCode 扩展图标（Marketplace 必需；与 logo/sml.png 是同一张图）"),
    "logo/sml.png": (
        20020, "aa6b9495ff77d301",
        "仓库 Logo（README / 站点共用）"),
}


def image_text_pieces(path):
    """图片里**人能读**的那部分（PNG 文本块 / JPEG APP 段 / GIF 注释），供隐私串扫描。"""
    pieces = []
    try:
        with open(path, "rb") as f:
            data = f.read()
    except OSError:
        return pieces
    if data.startswith(b"\x89PNG\r\n\x1a\n"):
        i = 8
        while i + 8 <= len(data):
            ln = int.from_bytes(data[i:i + 4], "big")
            typ = data[i + 4:i + 8]
            body = data[i + 8:i + 8 + ln]
            if typ in (b"tEXt", b"iTXt"):
                pieces.append(body.decode("latin-1", "replace"))
            elif typ == b"zTXt":
                kw, _, rest = body.partition(b"\x00")
                pieces.append(kw.decode("latin-1", "replace"))
                try:
                    pieces.append(zlib.decompress(rest[1:]).decode("utf-8", "replace"))
                except Exception:
                    pass
            i += 12 + ln
    elif data[:2] == b"\xff\xd8":  # JPEG：APPn（EXIF / IPTC / XMP）里全是可读文本
        i = 2
        while i + 4 <= len(data) and data[i] == 0xFF:
            marker = data[i + 1]
            ln = int.from_bytes(data[i + 2:i + 4], "big")
            if 0xE0 <= marker <= 0xEF:
                pieces.append(data[i + 4:i + 2 + ln].decode("latin-1", "replace"))
            if marker == 0xDA:  # 之后是压缩图像数据，不再有文本段
                break
            i += 2 + ln
    elif data[:6] in (b"GIF87a", b"GIF89a"):
        idx = data.find(b"\x21\xfe")  # 注释扩展 0x21 0xFE
        while idx >= 0:
            end = data.find(b"\x00", idx + 2)
            pieces.append(data[idx + 2:end if end > 0 else len(data)].decode("latin-1", "replace"))
            idx = data.find(b"\x21\xfe", idx + 2)
    return pieces


tracked_images = [p for p in indexed if p.lower().endswith(IMAGE_EXT)]
for p in sorted(tracked_images):
    ent = IMAGES.get(p)
    if not ent:
        fails.append("图片未登记：%s —— 复查后加进 tools/check/check_private_assets.py 的 IMAGES（用途 + 指纹）" % p)
        continue
    size, sha16, _why = ent
    try:
        with open(os.path.join(ROOT, p), "rb") as f:
            blob = f.read()
    except OSError:
        fails.append("图片读不到：%s" % p)
        continue
    got_size, got_sha = len(blob), hashlib.sha256(blob).hexdigest()[:16]
    if (got_size, got_sha) != (size, sha16):
        fails.append("图片已变：%s（登记 %d/%s，实际 %d/%s）—— 复查后更新 IMAGES 指纹"
                     % (p, size, sha16, got_size, got_sha))
    for n, piece in enumerate(image_text_pieces(os.path.join(ROOT, p)), 1):
        for line in (piece.splitlines() or [piece]):
            scan_text("图片", p, "%s#%d" % (p, n), n, line, rules)
for p in sorted(set(IMAGES) - set(tracked_images)):
    fails.append("IMAGES 里的 %s 已不在索引里（白名单僵尸条目，删掉）" % p)

# ── 7) 全部非文本文件：可读字符串 + 容器成员全量扫描 ─────────────────────
def is_binary(data):
    if b"\0" in data:
        return True
    try:
        data.decode("utf-8")
        return False
    except UnicodeDecodeError:
        return True


def printable_runs(data, min_len=4, max_len=4096):
    """从任意字节里抽可读 ASCII 串，返回 [(偏移, 文本)]。UTF-16 另走隔字节路径。"""
    out = []
    cur = bytearray()
    start = 0
    for i, b in enumerate(data):
        if 0x20 <= b <= 0x7E:
            if not cur:
                start = i
            cur.append(b)
            if len(cur) >= max_len:
                out.append((start, cur.decode("ascii")))
                cur.clear()
        else:
            if len(cur) >= min_len:
                out.append((start, cur.decode("ascii")))
            cur.clear()
    if len(cur) >= min_len:
        out.append((start, cur.decode("ascii")))
    return out


def nested_payloads(label, data, depth=0):
    """逐层产出 (位置标签, 字节)：zip/vsix、deb 的 data.tar.* 会再拆一层。"""
    yield label, data
    if depth >= 2:
        return
    if data[:2] == b"PK" and zipfile.is_zipfile(io.BytesIO(data)):
        try:
            with zipfile.ZipFile(io.BytesIO(data)) as z:
                for info in z.infolist():
                    sub = label + "!" + info.filename
                    yield sub, info.filename.encode("utf-8", "replace")
                    if info.file_size <= 20_000_000:
                        try:
                            yield from nested_payloads(sub, z.read(info), depth + 1)
                        except (OSError, zipfile.BadZipFile, RuntimeError):
                            pass
        except (OSError, zipfile.BadZipFile):
            pass
    elif data[:8] == b"!<arch>\n":  # deb 是 ar 归档；正文在 data.tar.*
        i = 8
        while i + 60 <= len(data):
            name = data[i:i + 16].decode("ascii", "replace").strip()
            size_field = data[i + 48:i + 58].decode("ascii", "replace").strip()
            if not size_field.isdigit():
                break
            size = int(size_field)
            body = data[i + 60:i + 60 + size]
            i += 60 + size + (size % 2)
            if not name.startswith("data.tar"):
                continue
            try:
                with tarfile.open(fileobj=io.BytesIO(body), mode="r:*") as tf:
                    for member in tf.getmembers():
                        sub = label + "!" + member.name
                        yield sub, member.name.encode("utf-8", "replace")
                        if member.isfile() and member.size <= 20_000_000:
                            f = tf.extractfile(member)
                            if f:
                                yield from nested_payloads(sub, f.read(), depth + 1)
            except (OSError, tarfile.TarError):
                pass


# 非文本扫描的**精确**例外：fuzz 语料是自动生成的随机输入，`a@b.c` / `10.x.y.z`
# 这类 Generic 命中是合成噪声；但本地字面串仍照扫（真泄密不许借语料躲过去）。
BINARY_GENERIC_SKIP = {
    "docs/swsml/ref/baseline-2026-11-07/fuzz-corpus.zip": "fuzz 合成语料，Generic 邮箱/内网形状是随机字节",
}


def looks_utf16le(data):
    """只有**真的**像 UTF-16LE（奇数位几乎全 NUL、偶数位多可打印）才走隔字节路径。

    不加这个判据时，任意二进制（甚至 zip 里的普通文本）按奇偶拆开都会得到一串
    伪随机字符，恰好拼出 `a@b.c` 这类形状 ⇒ 假阳性刷屏。
    """
    odd = data[1::2]
    even = data[0::2]
    if len(odd) < 16:
        return False
    return (odd.count(0) / len(odd) > 0.6 and
            sum(0x20 <= b <= 0x7E for b in even) / len(even) > 0.5)


# 本地字面串里 ≤5 字符的条目在二进制中会必然误报（3 位字母数字在 5 MB 里成百上千次），
# 它们仍照常被文本那道扫描；二进制这道只收 ≥6 字符的，保证报警可复查。
local_binary = [
    (re.compile(re.escape(s), re.IGNORECASE), "本地敏感串")
    for s in load_local_patterns() if len(s) >= 6
]
binary_count = 0
payload_count = 0
for path in indexed:
    try:
        with open(os.path.join(ROOT, path), "rb") as f:
            data = f.read()
    except OSError:
        continue
    if not is_binary(data):
        continue
    binary_count += 1
    for where, payload in nested_payloads(path, data):
        payload_count += 1
        binary_rules = local_binary if path in BINARY_GENERIC_SKIP else generic_res() + local_binary
        for shift in (0, 1) if looks_utf16le(payload) else (0,):
            stream = payload if shift == 0 else payload[1::2]
            for off, text in printable_runs(stream):
                scan_text("非文本", path, where, off * 2 + shift, text, binary_rules)
print("非文本全量扫描：%d 个文件 / %d 层（含容器成员；fuzz 语料只扫本地精确串）"
      % (binary_count, payload_count))

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
print("    图片 %d 张都已登记且指纹一致（含元数据扫描）" % len(tracked_images))
print("    正主在仓库之外的私有库（地址见本地 .git/info/exclude）")
