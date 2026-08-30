#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
机翻英文章节后处理：修复百度机翻引入的 Markdown 缺陷。

修复项：
  1. 标题丢失空格: `#Chapter 1` -> `# Chapter 1`
  2. 加粗被拆开:  `* * keys * *` -> `**keys**`
  3. 行内代码占位符残留: `[[C0]` / `[C2]` -> 按段落配对还原为原行内代码
  4. 内链未跟随语言: `(/book/xxx)` -> `(/en/book/xxx)`
  5. front matter title 仍为中文 -> 调百度翻译

用法：
  python site/tools/postprocess_en.py
  python site/tools/postprocess_en.py --no-translate-code
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

SRC_DIR = os.path.join(SITE, "content", "book")
DST_DIR = os.path.join(SITE, "content", "en", "book")

CODE_BLOCK = re.compile(r"```.*?```", re.DOTALL)
INLINE_CODE = re.compile(r"`[^`\n]*`")
PH_RE = re.compile(r"\[{1,2}C(\d+)\]{0,2}")
CJK = re.compile(r"[\u4e00-\u9fff]")
CODE_HINT = re.compile(r"[{}()\[\]\\=]|\.\w|\w:")
FRONT_MATTER = re.compile(r"^---\n(.*?)\n---\n", re.DOTALL)


def baidu_translate(q, retry=4):
    if not APPID or not KEY:
        return None
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


def inline_codes_outside_blocks(text):
    out = []
    pos = 0
    for m in CODE_BLOCK.finditer(text):
        out.extend(mm.group(0) for mm in INLINE_CODE.finditer(text[pos:m.start()]))
        pos = m.end()
    out.extend(mm.group(0) for mm in INLINE_CODE.finditer(text[pos:]))
    return out


def split_paras(text):
    return [p for p in re.split(r"\n\s*\n", text) if p.strip()]


def fix_headings(s):
    return re.sub(r"(?m)^(#{1,6})(\S)", r"\1 \2", s)


def fix_bold(s):
    # `* * text * *` -> `**text**`
    s = re.sub(r"\*\s\*\s*(.+?)\s\*\s\*", r"**\1**", s)
    # `** text **` -> `**text**`
    s = re.sub(r"\*\*\s+(.+?)\s+\*\*", r"**\1**", s)
    # `**text * *` （百度只拆开后半部分）
    s = re.sub(r"\*\*([^*\n]+?)\s\*\s\*", r"**\1**", s)
    # `** text**` / `**text **` （单侧多余空格）
    s = re.sub(r"\*\*\s+([^*\n]+?)\*\*", r"**\1**", s)
    s = re.sub(r"\*\*([^*\n]+?)\s+\*\*", r"**\1**", s)
    # `**text**word` 缺空格
    s = re.sub(r"\*\*([^*\n]+?)\*\*(?=[\u4e00-\u9fffA-Za-z])", r"**\1** ", s)
    return s


def fix_links(s):
    # 百度会在 "](" 之间插入空格
    s = re.sub(r"\]\s+\(", "](", s)
    return re.sub(r"\]\(/book/", "](/en/book/", s)


# 百度常在文件名/术语的句点后误加空格（仅修已知术语，避免误伤句末句点）
TERM_FIXES = [
    (r"\bnginx\.\s+conf\b", "nginx.conf"),
    (r"\bdocker[-\s]compose\b", "docker-compose"),
    (r"\bCaddy\s+file\b", "Caddyfile"),
    (r"\bLua\s*\.\s*dll\b", "Lua.dll"),
]


def fix_terms(s):
    for pat, rep in TERM_FIXES:
        s = re.sub(pat, rep, s)
    return s


def resolve_code(orig, cache, translate_code):
    """决定占位符还原成什么：原样保留，或翻译其中的中文词。"""
    inner = orig.strip("`")
    if not translate_code:
        return orig
    if not CJK.search(inner) or CODE_HINT.search(inner):
        return orig
    if inner in cache:
        return cache[inner]
    tr = baidu_translate(inner)
    cache[inner] = f"`{tr}`" if tr else orig
    time.sleep(1.2)
    return cache[inner]


def restore_placeholders(en_body, zh_body, translate_code=True):
    """按段落配对还原占位符：优先用同段落的中文行内代码池。"""
    zh_codes = inline_codes_outside_blocks(zh_body)
    if not zh_codes:
        return en_body, 0
    zh_paras = split_paras(zh_body)
    idx = 0
    zh_para_codes = []
    for p in zh_paras:
        cnt = len(INLINE_CODE.findall(p))
        zh_para_codes.append(list(range(idx, idx + cnt)))
        idx += cnt

    en_paras = split_paras(en_body)
    cache = {}
    restored = 0
    out = []
    same = len(en_paras) == len(zh_paras)
    # 段落数一致时按段配对；否则用全局游标跨段落递增
    # （百度会拆分/合并段落，导致占位符编号在每段重置，必须在段间累加）
    cursor = 0
    for i, para in enumerate(en_paras):
        if not PH_RE.search(para):
            out.append(para)
            continue
        use_para = same and i < len(zh_para_codes) and zh_para_codes[i]
        pool = zh_para_codes[i] if use_para else list(range(len(zh_codes)))
        take = 0 if use_para else cursor
        for _ in range(30):
            phs = list(PH_RE.finditer(para))
            if not phs or take >= len(pool):
                break
            gi = pool[take]
            if gi >= len(zh_codes):
                break
            repl = resolve_code(zh_codes[gi], cache, translate_code)
            para = para[:m.start()] + repl + para[m.end():]
            take += 1
            restored += 1
        if not use_para:
            cursor = take
        out.append(para)
    return "\n\n".join(out), restored


def split_blocks(text):
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


def translate_leftover_cjk(en_body):
    """翻译行内代码中残留的中文词（代码块内不动）。"""
    cache = {}
    n = 0
    out = []
    for is_code, seg in split_blocks(en_body):
        if is_code:
            out.append(seg)
            continue

        def _repl(m):
            nonlocal n
            inner = m.group(0).strip("`")
            if not CJK.search(inner) or CODE_HINT.search(inner):
                return m.group(0)
            if inner not in cache:
                tr = baidu_translate(inner)
                cache[inner] = f"`{tr}`" if tr else None
                time.sleep(1.2)
            if cache.get(inner):
                n += 1
                return cache[inner]
            return m.group(0)

        out.append(INLINE_CODE.sub(_repl, seg))
    return "".join(out), n


def main():
    no_tr = "--no-translate-code" in sys.argv
    files = [f for f in sorted(os.listdir(DST_DIR)) if f.endswith(".md")]
    total = 0
    for fn in files:
        dst = os.path.join(DST_DIR, fn)
        src = os.path.join(SRC_DIR, fn)
        if not os.path.exists(src):
            print(f"[跳过] {fn}（无中文源）")
            continue
        raw = open(dst, "r", encoding="utf-8").read()
        m = FRONT_MATTER.match(raw)
        if not m:
            print(f"[跳过] {fn}（无 front matter）")
            continue
        fm_text, body = m.group(1), raw[m.end():]
        zh_raw = open(src, "r", encoding="utf-8").read()
        zm = FRONT_MATTER.match(zh_raw)
        zh_body = zh_raw[zm.end():] if zm else zh_raw

        new_body = fix_terms(fix_links(fix_bold(fix_headings(body))))
        new_body, n = restore_placeholders(new_body, zh_body, not no_tr)
        # 翻译行内代码里残留的中文词
        if not no_tr:
            new_body, n_cjk = translate_leftover_cjk(new_body)
        else:
            n_cjk = 0

        # 翻译仍是中文的 title
        tm = re.search(r'title:\s*"([^"]*)"', fm_text)
        new_fm = fm_text
        if tm and CJK.search(tm.group(1)) and APPID:
            tr = baidu_translate(tm.group(1))
            if tr:
                new_fm = new_fm.replace(tm.group(0), f'title: "{tr}"')
                print(f"[title] {fn}: {tm.group(1)} -> {tr}")
                time.sleep(1.2)

        if new_body != body or new_fm != fm_text:
            with open(dst, "w", encoding="utf-8") as f:
                f.write("---\n" + new_fm + "\n---\n" + new_body)
            print(f"[修复] {fn} 占位符{n} 中文残留{n_cjk}")
            sys.stdout.flush()
        else:
            print(f"[无需修复] {fn}")
        total += n
    print(f"完成，共还原占位符 {total} 处")


if __name__ == "__main__":
    main()
