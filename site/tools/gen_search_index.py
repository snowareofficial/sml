"""site/content/**/*.md -> site/static/search-index.json

给官网的教科书搜索生成索引。**构建期生成、运行期只 fetch 一次**：
不在浏览器里抓页面、不引检索库、不联网、不追踪。

索引项：{ lang, url, title, headings[], text }
  - `url` 是**相对站点根**的路径（如 `book/ch01-basics/`、`en/book/ch01-basics/`），
    由页面里的 `data-root` 拼上前缀 —— 这样多语言与 relativeURLs 都不用特殊处理。

正文会被粗洗：去掉代码围栏的标记但**保留代码内容**（码表/命令也是要搜的），
去掉 Markdown 记号、链接只留文字、合并空白。宁多勿少 —— 索引是给人搜的，
不是给人读的，多留点文本只会让召回更好。

输出**不含时间戳**：同样输入必须产出逐字节相同的文件，否则每次构建都脏一个文件。
"""

import json
import os
import re
import sys

SITE = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CONTENT = os.path.join(SITE, "content")
OUT = os.path.join(SITE, "static", "search-index.json")

# 只索引阅读型内容；下载页/跳转页/动态页没有可搜的正文
SKIP_STEMS = {"gitee", "downloads"}


def split_front_matter(text):
    """切掉 TOML/YAML front matter，返回 (meta_lines, body)。"""
    if not text.startswith("---") and not text.startswith("+++"):
        return [], text
    delim = text[:3]
    end = text.find("\n" + delim, 3)
    if end < 0:
        return [], text
    return text[3:end].splitlines(), text[end + 4 :]


def pick_title(meta, body, fallback):
    for line in meta:
        m = re.match(r'\s*title\s*[:=]\s*"?([^"]+?)"?\s*$', line)
        if m:
            return m.group(1).strip()
    for line in body.splitlines():
        if line.startswith("# "):
            return line[2:].strip()
    return fallback


def clean_body(body):
    """Markdown -> 纯文本（粗洗，够搜索用）。"""
    out = []
    in_fence = False
    for line in body.splitlines():
        if line.startswith("```"):
            in_fence = not in_fence
            continue
        if re.match(r"^\s*\{\{<.*>\}\}\s*$", line) or re.match(r"^\s*\{\{%.*%\}\}\s*$", line):
            continue
        out.append(line)
    text = "\n".join(out)
    # 表格分隔行 |---|---| 没有信息量
    text = re.sub(r"^\s*\|[\s:|-]+\|\s*$", "", text, flags=re.M)
    # 链接/图片：保留可见文字
    text = re.sub(r"!\[([^\]]*)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text)
    # 行内代码记号、强调记号
    text = text.replace("`", "")
    text = re.sub(r"\*\*|__", "", text)
    text = re.sub(r"^\s{0,3}#{1,6}\s*", "", text, flags=re.M)  # 标题记号（标题另存）
    text = re.sub(r"^\s*[-*+]\s+", "", text, flags=re.M)  # 列表记号
    text = re.sub(r"^\s*>\s?", "", text, flags=re.M)  # 引用记号
    text = re.sub(r"[ \t]+", " ", text)
    text = re.sub(r"\n{2,}", "\n", text)
    return text.strip()


def url_of(lang, relpath):
    """content/<lang>/<x>.md -> 相对站点根的 URL。"""
    stem = relpath[:-3]  # 去掉 .md
    if os.path.basename(stem) == "_index":
        stem = os.path.dirname(stem)
    parts = [p for p in stem.split(os.sep) if p]
    prefix = [] if lang == "zh" else [lang]
    return "/".join(prefix + parts) + "/"


def main():
    entries = []
    if not os.path.isdir(CONTENT):
        print("!! 找不到 site/content")
        return 1
    for lang in sorted(os.listdir(CONTENT)):
        d = os.path.join(CONTENT, lang)
        if not os.path.isdir(d):
            continue
        for root, _dirs, files in os.walk(d):
            for fn in sorted(files):
                if not fn.endswith(".md"):
                    continue
                stem = fn[:-3]
                if stem in SKIP_STEMS:
                    continue
                path = os.path.join(root, fn)
                raw = open(path, encoding="utf-8").read()
                meta, body = split_front_matter(raw)
                rel = os.path.relpath(path, d)
                title = pick_title(meta, body, stem)
                headings = [
                    re.sub(r"^\s{0,3}#{1,6}\s*", "", ln).strip()
                    for ln in body.splitlines()
                    if re.match(r"^\s{0,3}#{1,6}\s+\S", ln)
                ]
                text = clean_body(body)
                if not text and not headings:
                    continue
                entries.append(
                    {
                        "lang": lang,
                        "url": url_of(lang, rel),
                        "title": title,
                        "headings": headings,
                        "text": text,
                    }
                )
    out = {"count": len(entries), "entries": entries}
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        json.dump(out, f, ensure_ascii=False, indent=1)
        f.write("\n")
    print("已生成 %s（%d 页，%.0f KB）" % (
        os.path.relpath(OUT, SITE), len(entries), os.path.getsize(OUT) / 1024.0))
    return 0


if __name__ == "__main__":
    sys.exit(main())
