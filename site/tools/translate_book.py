#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
把 SML 教科书中文 12 章机翻成英文（百度通用翻译 API）。

用法：
  1. 复制 site/.baidu.env.example 为 site/.baidu.env，填入真实 APPID/KEY
  2. python site/tools/translate_book.py
     （可选参数: --src content/book --dst content/en/book --dry 仅打印不写）

特性：
  - 跳过 ```代码块``` 与行内 `code` 不翻译
  - 按段落分块，单块 < 1800 字符（百度标准版上限 2000，留余量）
  - MD5 签名: sign = md5(appid + q + salt + key)
  - 生成英文文件 front matter 含 translationKey（与中文对应，便于语言切换联动）
  - 给中文原文件补 translationKey（若缺失）
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

# 加载 .baidu.env（优先 site/.baidu.env，再根目录）
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

SRC_DIR = os.path.join(SITE, "content", "book")
DST_DIR = os.path.join(SITE, "content", "en", "book")

# 中文章节 -> translationKey 映射（无 front matter 时回退用文件名）
SLUG_TK = {
    "_index.md": "book-index",
    "intro.md": "book-intro",
    "ch01-basics.md": "book-ch01",
    "ch02-blocks.md": "book-ch02",
    "ch03-fragments.md": "book-ch03",
    "ch04-include.md": "book-ch04",
    "ch05-contract.md": "book-ch05",
    "ch06-env-escape.md": "book-ch06",
    "ch07-languages.md": "book-ch07",
    "ch08-project.md": "book-ch08",
    "ch09-advanced.md": "book-ch09",
    "ch10-features.md": "book-ch10",
    "ch11-challenges.md": "book-ch11",
    "appendix.md": "book-appendix",
}

CODE_BLOCK = re.compile(r"```.*?```", re.DOTALL)
INLINE_CODE = re.compile(r"`[^`\n]*`")
FRONT_MATTER = re.compile(r"^---\n(.*?)\n---\n", re.DOTALL)


def split_blocks(text):
    """把正文切成 (is_code, content) 列表，代码块原样保留。"""
    parts = []
    pos = 0
    for m in CODE_BLOCK.finditer(text):
        if m.start() > pos:
            parts.append((False, text[pos:m.start()]))
        parts.append((True, m.group(0)))
        pos = m.end()
    if pos < len(text):
        parts.append((False, text[pos:]))
    return parts


def chunk_text(s, limit=1800):
    """把非代码文本按段落切块，单块不超 limit。
    百度 API 对含换行/特殊字符的长 query 容易报 20005，因此优先逐段落翻译，
    仅当单段超长时才在段内按句子切分。"""
    paras = [p.strip() for p in re.split(r"\n{1,}", s) if p.strip()]
    out = []
    for para in paras:
        if len(para) <= limit:
            out.append(para)
            continue
        # 超长段落：按句子切分
        buf = ""
        for sent in re.split(r"(?<=[。！？.!?])", para):
            if not sent.strip():
                continue
            if len(buf) + len(sent) > limit and buf:
                out.append(buf.strip())
                buf = sent
            else:
                buf = (buf + sent).strip() if buf else sent
        if buf.strip():
            out.append(buf.strip())
    return out


def translate(q, retry=6):
    last = None
    for attempt in range(retry):
        salt = str(int(time.time() * 1000))
        sign = hashlib.md5((APPID + q + salt + KEY).encode("utf-8")).hexdigest()
        data = urllib.parse.urlencode({
            "q": q,
            "from": "zh",
            "to": "en",
            "appid": APPID,
            "salt": salt,
            "sign": sign,
        }).encode("utf-8")
        req = urllib.request.Request(API, data=data, method="POST")
        req.add_header("Content-Type", "application/x-www-form-urlencoded")
        try:
            with urllib.request.urlopen(req, timeout=15) as resp:
                j = json.loads(resp.read().decode("utf-8"))
            if "error_code" in j:
                if j["error_code"] in ("54003", "52003", "52004") and attempt < retry - 1:
                    # 限流或频率错误：指数退避后重试（每次换新 salt）
                    wait = 10 * (attempt + 1)
                    print(f"  [限流] {j['error_code']} 等待 {wait}s 后重试 ({attempt + 1}/{retry})")
                    sys.stdout.flush()
                    time.sleep(wait)
                    last = j
                    continue
                raise RuntimeError(f"百度翻译错误 {j['error_code']}: {j.get('error_msg')} | q前100={q[:100]!r}")
            return "".join(t["dst"] for t in j["trans_result"])
        except urllib.error.URLError as e:
            last = e
            time.sleep(3 * (attempt + 1))
    raise RuntimeError(f"百度翻译重试失败: {last}")


def translate_noncode(text):
    """翻译非代码块文本。

    行内代码用**全局唯一**的纯字母数字占位符 XQZ{n}XQZ 保护：
    - 纯字母数字避免百度把 `[[C0]]` 这类带括号的占位符改写得无法还原
    - 全局唯一避免不同段落出现同名占位符导致还原歧义
    """
    blocks = split_blocks(text)
    out = []
    ph_map = {}
    counter = [0]

    def _mask(m):
        ph = f"XQZ{counter[0]}XQZ"
        counter[0] += 1
        ph_map[ph] = m.group(0)
        return ph

    for is_code, content in blocks:
        if is_code:
            out.append(content)
            continue
        tr_parts = []
        for chunk in chunk_text(content):
            if not chunk.strip():
                continue
            masked = INLINE_CODE.sub(_mask, chunk)
            tr = translate(masked)
            tr_parts.append(tr)
            time.sleep(2.0)  # 限流：百度标准版约 1 QPS
        out.append("\n\n".join(tr_parts))
    # 全局统一还原（占位符唯一，按映射精确替换）
    result = "\n\n".join(out)
    for ph, orig in ph_map.items():
        result = result.replace(ph, orig)
    left = re.findall(r"XQZ\d+XQZ", result)
    if left:
        print(f"  [警告] {len(left)} 个占位符未还原（百度改写了占位符）")
    return result


def get_fm(path):
    with open(path, "r", encoding="utf-8") as f:
        raw = f.read()
    m = FRONT_MATTER.match(raw)
    if not m:
        return {}, raw, None
    fm = {}
    for line in m.group(1).split("\n"):
        if ":" in line:
            k, v = line.split(":", 1)
            fm[k.strip()] = v.strip().strip('"')
    return fm, raw, m


def main():
    if not APPID or not KEY:
        print("错误：未找到 BAIDU_TRANSLATE_APPID / BAIDU_TRANSLATE_KEY，请配置 .baidu.env")
        sys.exit(1)
    os.makedirs(DST_DIR, exist_ok=True)
    files = [f for f in os.listdir(SRC_DIR) if f.endswith(".md")]
    for fn in sorted(files):
        tk = SLUG_TK.get(fn, "book-" + fn.replace(".md", ""))
        src_path = os.path.join(SRC_DIR, fn)
        dst_path = os.path.join(DST_DIR, fn)
        # 断点续传：已生成且非空的英文文件直接跳过（--force 时覆盖重译）
        if os.path.exists(dst_path) and os.path.getsize(dst_path) > 200 and "--force" not in sys.argv:
            print(f"[跳过] {fn} 已存在")
            sys.stdout.flush()
            continue
        fm, raw, m = get_fm(src_path)
        body = raw[m.end():] if m else raw
        # 给中文补 translationKey
        if m and "translationKey" not in fm:
            new_fm = m.group(1).rstrip() + f'\ntranslationKey: "{tk}"\n'
            with open(src_path, "w", encoding="utf-8") as f:
                f.write("---\n" + new_fm + "---\n" + body)
            print(f"[中文] {fn} 补 translationKey={tk}")
        print(f"[翻译] {fn} -> {tk}")
        sys.stdout.flush()
        en_body = translate_noncode(body)
        # 英文 front matter
        title_en = fm.get("title", tk)
        en_fm = f'---\ntitle: "{title_en}"\ntranslationKey: "{tk}"\n---\n'
        with open(dst_path, "w", encoding="utf-8") as f:
            f.write(en_fm + en_body + "\n")
        print(f"  已写 {dst_path}")
        sys.stdout.flush()
        time.sleep(1.0)


if __name__ == "__main__":
    main()
