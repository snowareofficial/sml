#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""gen_epub.py — 把 site/content/book/ 下的教科书 Markdown 打包成 EPUB。

纯标准库实现（zipfile + 手写轻量 Markdown->XHTML 转换），零第三方依赖。

用法:
    python gen_epub.py                  # 输出到 site/public/sml-book.epub
    python gen_epub.py --out book.epub  # 指定输出路径
"""
import os
import re
import sys
import zipfile
import html as _html

HERE = os.path.dirname(os.path.abspath(__file__))
BOOK_DIR = os.path.join(HERE, "content", "book")
OUT_DEFAULT = os.path.join(HERE, "public", "sml-book.epub")

# 章节顺序（相对 book 目录的文件名，不含 .md）。顺序即书的阅读顺序。
CHAPTERS = [
    ("_index", "SML 教科书"),
    ("intro", "序章：为什么是 SML"),
    ("ch01-basics", "第 1 章：第一个 SML 文件"),
    ("ch02-blocks", "第 2 章：块与嵌套"),
    ("ch03-fragments", "第 3 章：片段继承"),
    ("ch04-include", "第 4 章：include 与命名空间"),
    ("ch05-contract", "第 5 章：契约系统"),
    ("ch06-env-escape", "第 6 章：环境变量与转义"),
    ("ch07-languages", "第 7 章：多语言使用"),
    ("ch08-project", "第 8 章：实战——完整项目配置"),
    ("ch09-advanced", "第 9 章：进阶——功能组合与设计模式"),
    ("ch10-features", "第 10 章：feature 完整参考"),
    ("appendix", "附录：对照与排查"),
]

BOOK_ID = "sml-book-2026"
BOOK_TITLE = "SML 教科书 { ❄ }"
BOOK_AUTHOR = "SNOWARE"
LANG = "zh-CN"


def read_chapter(fname):
    """读取 md 文件，返回 (title, body_md)。优先用 frontmatter 的 title。"""
    path = os.path.join(BOOK_DIR, fname + ".md")
    if not os.path.isfile(path):
        # _index 用目录页；若缺失则跳过
        return None, None
    with open(path, "r", encoding="utf-8") as f:
        text = f.read()
    title = None
    m = re.match(r"^---\s*\n(.*?)\n---\s*\n", text, re.S)
    if m:
        for line in m.group(1).splitlines():
            if line.startswith("title:"):
                title = line[len("title:"):].strip().strip('"').strip("'")
        text = text[m.end():]
    if title is None:
        # 取首个 # 标题
        for line in text.splitlines():
            if line.startswith("# "):
                title = line[2:].strip()
                break
    return title, text


def inline_md(s):
    """行内 Markdown -> XHTML（code / bold / italic / link）。"""
    # 保护行内代码
    codes = []

    def stash(m):
        codes.append(m.group(1))
        return "\x00CODE%d\x00" % (len(codes) - 1)

    s = re.sub(r"`([^`]+)`", stash, s)
    # 链接 [text](url)
    s = re.sub(r"\[([^\]]+)\]\(([^)]+)\)",
               lambda m: '<a href="%s">%s</a>' % (_html.escape(m.group(2), quote=True),
                                                   _html.escape(m.group(1))),
               s)
    # 粗体 **x**
    s = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", s)
    # 斜体 *x* / _x_
    s = re.sub(r"(?<!\*)\*([^*]+)\*(?!\*)", r"<em>\1</em>", s)
    s = re.sub(r"_([^_]+)_", r"<em>\1</em>", s)
    # 还原代码
    def restore(m):
        return "<code>%s</code>" % _html.escape(codes[int(m.group(1))])
    s = re.sub(r"\x00CODE(\d+)\x00", restore, s)
    return s


def md_to_xhtml(md):
    """极简块级 Markdown -> XHTML 片段。覆盖教科书所用语法。"""
    lines = md.splitlines()
    out = []
    i = 0
    n = len(lines)
    in_list = None  # 'ul' or 'ol'
    list_buf = []

    def flush_list():
        nonlocal in_list, list_buf
        if in_list:
            out.append("<%s>" % in_list)
            for item in list_buf:
                out.append("  <li>%s</li>" % inline_md(item))
            out.append("</%s>" % in_list)
            in_list = None
            list_buf = []

    while i < n:
        line = lines[i].rstrip()
        # 跳过空行
        if line.strip() == "":
            flush_list()
            i += 1
            continue
        # 分隔符
        if re.match(r"^---+$", line.strip()):
            flush_list()
            out.append("<hr/>")
            i += 1
            continue
        # Hugo shortcode（EPUB 无法交互，转成提示框）
        if line.strip().startswith("{{<") or line.strip().startswith("{{%"):
            flush_list()
            m = re.search(r'sml-playground\s+"([^"]+)"', line)
            key = m.group(1) if m else ""
            out.append(
                '<div class="exercise"><strong>动手练习</strong>：本章交互式练习请在 '
                'SML 官网 <a href="https://sml.snoware.org/book/">在线教科书</a> 中运行'
                '（浏览器内嵌可编辑 SML 解析器）。</div>'
            )
            i += 1
            continue
        # 标题
        hm = re.match(r"^(#{1,4})\s+(.*)$", line)
        if hm:
            flush_list()
            level = len(hm.group(1))
            txt = inline_md(hm.group(2).strip())
            out.append("<h%d>%s</h%d>" % (level, txt, level))
            i += 1
            continue
        # 代码块 ```lang
        if line.strip().startswith("```"):
            flush_list()
            lang = line.strip()[3:].strip()
            i += 1
            code = []
            while i < n and not lines[i].strip().startswith("```"):
                code.append(lines[i])
                i += 1
            i += 1  # 跳过结束 ```
            cls = ' class="%s"' % _html.escape(lang) if lang else ""
            out.append("<pre><code%s>%s</code></pre>" %
                       (cls, _html.escape("\n".join(code))))
            continue
        # 引用
        if line.lstrip().startswith(">"):
            flush_list()
            quote = []
            while i < n and lines[i].lstrip().startswith(">"):
                quote.append(lines[i].lstrip()[1:].strip())
                i += 1
            out.append("<blockquote>%s</blockquote>" % inline_md(" ".join(quote)))
            continue
        # 表格（当前行含 | 且下一行是分隔）
        if "|" in line and i + 1 < n and re.match(r"^\s*\|?[\s:-]+\|[\s:|-]+\|?\s*$", lines[i + 1]):
            flush_list()
            header = [c.strip() for c in line.strip().strip("|").split("|")]
            i += 2  # 跳过表头与分隔行
            rows = []
            while i < n and "|" in lines[i] and lines[i].strip():
                rows.append([c.strip() for c in lines[i].strip().strip("|").split("|")])
                i += 1
            out.append("<table>")
            out.append("<thead><tr>" + "".join("<th>%s</th>" % inline_md(c) for c in header) + "</tr></thead>")
            out.append("<tbody>")
            for r in rows:
                out.append("<tr>" + "".join("<td>%s</td>" % inline_md(c) for c in r) + "</tr>")
            out.append("</tbody></table>")
            continue
        # 列表
        lim = re.match(r"^(\s*)[-*]\s+(.*)$", line)
        olm = re.match(r"^(\s*)\d+\.\s+(.*)$", line)
        if lim:
            if in_list != "ul":
                flush_list()
                in_list = "ul"
            list_buf.append(lim.group(2))
            i += 1
            continue
        if olm:
            if in_list != "ol":
                flush_list()
                in_list = "ol"
            list_buf.append(olm.group(2))
            i += 1
            continue
        # 普通段落
        flush_list()
        out.append("<p>%s</p>" % inline_md(line.strip()))
        i += 1
    flush_list()
    return "\n".join(out)


def build_epub(out_path):
    os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
    items = []  # (id, title, xhtml)
    for fname, default_title in CHAPTERS:
        title, md = read_chapter(fname)
        if md is None:
            print("!! 跳过缺失章节:", fname)
            continue
        title = title or default_title
        body = md_to_xhtml(md)
        xhtml = (
            '<?xml version="1.0" encoding="utf-8"?>\n'
            '<!DOCTYPE html>\n'
            '<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="%s" lang="%s">\n'
            '<head><meta charset="utf-8"/><title>%s</title></head>\n'
            '<body>\n<h1>%s</h1>\n%s\n</body>\n</html>\n'
            % (LANG, LANG, _html.escape(title), _html.escape(title), body)
        )
        items.append((fname, title, xhtml))

    # OPF manifest items
    manifest = []
    spine = []
    for idx, (fname, title, xhtml) in enumerate(items):
        fid = "chap%d" % idx
        manifest.append('<item id="%s" href="%s.xhtml" media-type="application/xhtml+xml"/>' %
                        (fid, _html.escape(fname)))
        spine.append('<itemref idref="%s"/>' % fid)

    # NCX navPoints
    nav = []
    for idx, (fname, title, xhtml) in enumerate(items):
        nav.append(
            '    <navPoint id="nav%d" playOrder="%d">\n'
            '      <navLabel><text>%s</text></navLabel>\n'
            '      <content src="%s.xhtml"/>\n'
            '    </navPoint>' % (idx, idx + 1, _html.escape(title), _html.escape(fname))
        )

    content_opf = (
        '<?xml version="1.0" encoding="utf-8"?>\n'
        '<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="bookid">\n'
        '  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">\n'
        '    <dc:identifier id="bookid">urn:uuid:%s</dc:identifier>\n'
        '    <dc:title>%s</dc:title>\n'
        '    <dc:language>%s</dc:language>\n'
        '    <dc:creator>%s</dc:creator>\n'
        '    <meta property="dcterms:modified">2026-08-30T00:00:00Z</meta>\n'
        '  </metadata>\n'
        '  <manifest>\n'
        '    <item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>\n'
        '    %s\n'
        '  </manifest>\n'
        '  <spine toc="ncx">\n'
        '    %s\n'
        '  </spine>\n'
        '</package>\n'
        % (BOOK_ID, _html.escape(BOOK_TITLE), LANG, _html.escape(BOOK_AUTHOR),
           "\n    ".join(manifest), "\n    ".join(spine))
    )

    toc_ncx = (
        '<?xml version="1.0" encoding="utf-8"?>\n'
        '<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1">\n'
        '  <head>\n'
        '    <meta name="dtb:uid" content="urn:uuid:%s"/>\n'
        '    <meta name="dtb:depth" content="1"/>\n'
        '    <meta name="dtb:totalPageCount" content="0"/>\n'
        '    <meta name="dtb:maxPageNumber" content="0"/>\n'
        '  </head>\n'
        '  <docTitle><text>%s</text></docTitle>\n'
        '  <navMap>\n'
        '%s\n'
        '  </navMap>\n'
        '</ncx>\n'
        % (BOOK_ID, _html.escape(BOOK_TITLE), "\n".join(nav))
    )

    container = (
        '<?xml version="1.0" encoding="utf-8"?>\n'
        '<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">\n'
        '  <rootfiles>\n'
        '    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>\n'
        '  </rootfiles>\n'
        '</container>\n'
    )

    with zipfile.ZipFile(out_path, "w", zipfile.ZIP_DEFLATED) as z:
        # mimetype 必须第一个且无压缩
        z.writestr("mimetype", "application/epub+zip",
                   compress_type=zipfile.ZIP_STORED)
        z.writestr("META-INF/container.xml", container)
        z.writestr("OEBPS/content.opf", content_opf)
        z.writestr("OEBPS/toc.ncx", toc_ncx)
        for fname, title, xhtml in items:
            z.writestr("OEBPS/%s.xhtml" % fname, xhtml)
    print("EPUB -> %s (%d 章)" % (out_path, len(items)))


def main():
    out = OUT_DEFAULT
    if "--out" in sys.argv:
        idx = sys.argv.index("--out")
        if idx + 1 < len(sys.argv):
            out = sys.argv[idx + 1]
    build_epub(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
