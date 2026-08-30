#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
翻译交互部件的数据文件（中 -> 英）：
  data/sml-quizzes.json    -> data/sml-quizzes_en.json
  data/sml-lessons.json    -> data/sml-lessons_en.json
  data/sml-challenges.json -> data/sml-challenges_en.json

注意：这里用下划线命名（*_en.json）而非 Hugo 的语言后缀（*.en.json）。
因为 *.en.json 会被 Hugo 解析成与中文版同名的 key（sml-quizzes），
两者冲突时取到的是中文版，导致英文站题库不生效。
shortcode 里用 site.Language / RelPermalink 显式选择 key。

翻译字段：
  quizzes:    title / q / options[] / explain
  lessons:    task / hint            （main 是 SML 代码，不翻）
  challenges: title / prompt / hint  （source 是原始配置代码，difficulty 是星号，不翻）

保护策略（占位符 XQZ{n}XQZ，译后原样还原）：
  - HTML 标签 <b> / <code> / <br> 等
  - 反引号包裹的行内代码
  - SML/配置代码片段（@contract / $env.X / key: value / {…} / […]）
"""
import os
import re
import sys
import json
import time
import hashlib
import urllib.parse
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")

HERE = os.path.dirname(os.path.abspath(__file__))          # site/tools
SITE = os.path.dirname(HERE)                                # site
ROOT = os.path.dirname(SITE)                                # 仓库根（.baidu.env 回退）

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
DATA = os.path.join(SITE, "data")

PH_RE = re.compile(r"XQZ\d+XQZ")
CJK = re.compile(r"[\u4e00-\u9fff]")

# 保护模式（顺序敏感：先长后短、先结构后片段）
PROTECT_PATS = [
    re.compile(r"<[^<>]+>"),                       # HTML 标签
    re.compile(r"`[^`\n]*`"),                      # 行内代码
    re.compile(r"@feature\s+enable\s+\w+"),
    re.compile(r"@contract\s+\w+(?:\s+\w+)?"),
    re.compile(r"@is\s+\w+"),
    re.compile(r"@base\s*\{[^}]*\}"),
    re.compile(r"@base|@is|@name|@contract|@feature"),
    re.compile(r"\$env\.\w+"),
    re.compile(r"\$\{[^}]*\}"),
    re.compile(r"\b\w+\s*:\s*\{[^}]*\}"),
    re.compile(r"\{[^{}]*\}"),
    re.compile(r"\[[^\[\]]*\]"),
    re.compile(r"\b\w+\s*:\s*[\"']?[\w./-]+[\"']?"),
    re.compile(r"\\[ntrux\"\\]|\bu\{[0-9A-Fa-f]+\}"),
    re.compile(r"\b\w+\s*\([^)]*\)"),
    re.compile(r"\b\w+\.\w+(?:\.\w+)?"),
]


def baidu(q, retry=5):
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
                    time.sleep(8 * (attempt + 1))
                    last = j
                    continue
                print(f"    [百度错误] {j['error_code']}: {j.get('error_msg')}")
                return None
            return "".join(t["dst"] for t in j["trans_result"])
        except Exception as e:  # noqa: BLE001
            last = e
            time.sleep(8 * (attempt + 1))
    print(f"    [翻译失败] {last}")
    return None


def translate_text(text, cache):
    if not text or not isinstance(text, str) or not text.strip():
        return text
    if not CJK.search(text):
        return text
    ph_map = {}
    counter = [0]

    def _sub(m):
        ph = f"XQZ{counter[0]}XQZ"
        counter[0] += 1
        ph_map[ph] = m.group(0)
        return ph

    masked = text
    for pat in PROTECT_PATS:
        masked = pat.sub(_sub, masked)

    if not CJK.search(masked):
        return text  # 全是代码/标签，无需翻译

    if masked in cache:
        tr = cache[masked]
    else:
        tr = baidu(masked)
        cache[masked] = tr
        time.sleep(1.2)

    if not tr:
        return text
    for ph, orig in ph_map.items():
        tr = tr.replace(ph, orig)
    if PH_RE.search(tr):
        print(f"    [警告] 占位符残留: {text[:50]!r}")
    return tr


def translate_quizzes(cache):
    src = json.load(open(os.path.join(DATA, "sml-quizzes.json"), encoding="utf-8"))
    out = {}
    for key, block in src.items():
        print(f"[quiz] {key}: {block.get('title','')}")
        sys.stdout.flush()
        items = []
        for it in block.get("items", []):
            items.append({
                "q": translate_text(it.get("q", ""), cache),
                "type": it.get("type"),
                "options": [translate_text(o, cache) for o in it.get("options", [])],
                "answer": it.get("answer"),
                "explain": translate_text(it.get("explain", ""), cache),
            })
        out[key] = {"title": translate_text(block.get("title", ""), cache), "items": items}
    return out


def translate_lessons(cache):
    src = json.load(open(os.path.join(DATA, "sml-lessons.json"), encoding="utf-8"))
    out = {}
    for key, cfg in src.items():
        print(f"[lesson] {key}")
        sys.stdout.flush()
        item = dict(cfg)  # 保留 main / files 等原样字段
        if "task" in cfg:
            item["task"] = translate_text(cfg["task"], cache)
        if "hint" in cfg:
            item["hint"] = translate_text(cfg["hint"], cache)
        out[key] = item
    return out


def translate_challenges(cache):
    src = json.load(open(os.path.join(DATA, "sml-challenges.json"), encoding="utf-8"))
    out = {}
    for key, cfg in src.items():
        print(f"[challenge] {key}: {cfg.get('title','')}")
        sys.stdout.flush()
        item = dict(cfg)  # 保留 source / difficulty 原样
        for f in ("title", "prompt", "hint"):
            if f in cfg:
                item[f] = translate_text(cfg[f], cache)
        out[key] = item
    return out


def main():
    if not APPID or not KEY:
        print("缺少百度翻译凭据（site/.baidu.env）")
        sys.exit(1)
    cache = {}
    jobs = [
        ("sml-quizzes", translate_quizzes),
        ("sml-lessons", translate_lessons),
        ("sml-challenges", translate_challenges),
    ]
    for name, fn in jobs:
        dst = os.path.join(DATA, f"{name}_en.json")
        if os.path.exists(dst) and os.path.getsize(dst) > 200 and "--force" not in sys.argv:
            print(f"[跳过] {name}_en.json 已存在（用 --force 覆盖）")
            continue
        print(f"\n===== 翻译 {name} =====")
        data = fn(cache)
        with open(dst, "w", encoding="utf-8") as f:
            json.dump(data, f, ensure_ascii=False, indent=2)
        print(f"[完成] {dst}")
        sys.stdout.flush()
    print("\n全部完成")


if __name__ == "__main__":
    main()
