#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
翻译英文章节代码块内的中文注释（只译注释，不动代码）。

安全策略：
  - 只处理 ```代码块``` 内部
  - 只处理「注释行」：剥离前导注释符（# / -- / //）后，剩余部分是纯中文的才翻译
  - 代码行（含 : { } = 等）一律不动

用法：
  python site/tools/translate_code_comments.py
"""
import os
import re
import sys
import time
import json
import hashlib
import urllib.parse
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SITE = os.path.join(ROOT, "site")

for cand in (os.path.join(SITE, ".baidu.env"), os.path.join(ROOT, ".baidu.env")):
    if os.path.exists(cand):
        with open(cand, "r", encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line or line.startswith("#") or "=" not in line:
                    continue
                k, v = line.split("=", 1)
                os.environ.setdefault(k.strip(), v.strip())
        break

APPID = os.environ.get("BAIDU_TRANSLATE_APPID", "")
KEY = os.environ.get("BAIDU_TRANSLATE_KEY", "")
API = "https://fanyi-api.baidu.com/api/trans/vip/translate"

DST = os.path.join(SITE, "content", "en", "book")
CODE_BLOCK = re.compile(r"(```[a-z]*\n)(.*?)(```)", re.DOTALL)
CJK = re.compile(r"[\u4e00-\u9fff]")
# 代码特征：出现这些符号视为代码行，不翻译
CODE_CHAR = re.compile(r"[:{}\[\]=<>|&$@]")
# 行内注释：`code  # 中文注释` 形式
INLINE_NOTE = re.compile(r"^(.*?)(#\s*)(.+)$")


def baidu_translate(q, retry=4):
    last = None
    for attempt in range(retry):
        salt = str(int(time.time() * 1000))
        sign = hashlib.md5((APPID + q + salt + KEY).encode("utf-8")).hexdigest()
        data = urllib.parse.urlencode({
            "q": q, "from": "zh", "to": "en", "appid": APPID, "salt": salt, "sign": sign,
        }).encode("utf-8")
        req = urllib.request.Request(API, data=data, method="POST")
        req.add_header("Content-Type", "application/x-www-form-urlencoded")
        try:
            with urllib.request.urlopen(req, timeout=15) as resp:
                j = json.loads(resp.read().decode("utf-8"))
            if "error_code" in j:
                if j["error_code"] in ("54003", "52003", "52004") and attempt < retry - 1:
                    time.sleep(5 * (attempt + 1))
                    last = j
                    continue
                return None
            return "".join(t["dst"] for t in j["trans_result"])
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(5 * (attempt + 1))
    return None


def is_comment_line(line):
    """返回 (前导空白, 注释符, 注释正文) 或 None。

    严格判据（防止误伤 Markdown 标题与代码）：
      1. 注释正文必须「以中文为主」：中文字符数 >= 2 且中文占比 > 30%
      2. 若整行去掉注释符后仍是 Markdown 标题形态（如 `# 1.1 xxx`），跳过
    """
    s = line.rstrip()
    if not s.strip():
        return None
    # 排除 Markdown 标题（行首 # 后紧跟标题层级，如 `## 1.1`、`# Title`）
    if re.match(r"^\s*#{1,6}\s", s):
        # 形如 `## 1.1 xxx` 是标题；真正的代码注释形如 `# 中文注释`
        # 判据：标题的 # 数量 >=2，或 # 后紧跟数字/大写字母标题形态
        if re.match(r"^\s*#{2,6}\s", s):
            return None
        # 单个 # 时，若其后是 ASCII 标题文本（非中文），也视为标题
        rest = re.sub(r"^\s*#\s*", "", s)
        if not CJK.search(rest):
            return None
    stripped = s.strip()
    lead_ws = s[:len(s) - len(stripped)]
    for prefix in ("# ", "#", "-- ", "--", "// ", "//"):
        if stripped.startswith(prefix):
            body = stripped[len(prefix):].strip()
            if not body or not CJK.search(body):
                return None
            # 注释里若含代码符号则不翻译（避免破坏伪代码）
            if CODE_CHAR.search(body):
                return None
            # 中文为主判定
            cjk_n = len(CJK.findall(body))
            if cjk_n < 2 or cjk_n / max(len(body), 1) < 0.3:
                return None
            return lead_ws, prefix, body
    return None


def main():
    if not APPID or not KEY:
        print("缺少凭据")
        sys.exit(1)
    cache = {}
    total = 0
    for fn in sorted(os.listdir(DST)):
        if not fn.endswith(".md"):
            continue
        path = os.path.join(DST, fn)
        txt = open(path, encoding="utf-8").read()
        changed = [0]

        def _block(m):
            head, body, tail = m.group(1), m.group(2), m.group(3)
            new_lines = []
            for line in body.split("\n"):
                info = is_comment_line(line)
                if not info:
                    new_lines.append(line)
                    continue
                lead, prefix, zh = info
                if zh in cache:
                    en = cache[zh]
                else:
                    en = baidu_translate(zh)
                    cache[zh] = en
                    time.sleep(1.2)
                if not en:
                    new_lines.append(line)
                    continue
                # 首字母大写，保持注释风格
                en = en[0].upper() + en[1:] if en[:1].islower() else en
                new_lines.append(f"{lead}{prefix}{en}")
                changed[0] += 1
            return head + "\n".join(new_lines) + tail

        new_txt = CODE_BLOCK.sub(_block, txt)
        if changed[0]:
            open(path, "w", encoding="utf-8").write(new_txt)
            print(f"[注释翻译] {fn}: {changed[0]} 行")
            sys.stdout.flush()
        total += changed[0]
    print(f"完成，共翻译代码块注释 {total} 行")


if __name__ == "__main__":
    main()
