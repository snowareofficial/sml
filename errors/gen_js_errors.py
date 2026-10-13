#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""从 site/static/errors.json（中文，唯一事实来源）+ errors/msg.en.json（英文层）
生成 js/sml-errors.mjs —— 按错误码取本地化信息的模块（i18n）。

    python errors/gen_js_errors.py            # 生成 js/sml-errors.mjs
    python errors/gen_js_errors.py --check    # 只校验不写（给 CI）：产物与将生成内容一致

i18n 约定（与仓库一致）：**中文是源语言（默认）**，英文是翻译层；某条码/字段缺英文时
回退中文，永不返回空。错误码本身是语言中立的稳定契约（见 errors/README.md）。

数据流：errors/codes.sml --(gen_json.py)--> site/static/errors.json（中文）
        errors/msg.en.json（英文，手维护） --(本脚本)--> js/sml-errors.mjs（双语）
"""
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ERRORS_JSON = os.path.join(ROOT, "site", "static", "errors.json")
MSG_EN = os.path.join(ROOT, "errors", "msg.en.json")
OUT = os.path.join(ROOT, "js", "sml-errors.mjs")

# 领域 ID 的语言中立显示名：领域 ID 是码的一部分（不翻译），这里补**英文显示名**，
# 缺的回落中文名（DOMAINS 里仍带中文）。加新领域时在此补一行英文。
DOMAIN_EN = {
    "LEX": "Lexical",
    "PARSE": "Syntax",
    "CONTRACT": "Contract",
    "INCLUDE": "Fragments & include",
    "LIMIT": "Depth & budget limits",
    "FEATURE": "Features, version & env",
    "EXT": "Extension points",
    "IO": "Input/output",
    "INTERNAL": "Internal errors",
    "DERIVE": "Rust derive & serde bridge",
    "MIGRATE": "Import formats",
    "CLI": "Command line & tool usage",
    "LINT": "lint diagnostics",
    "EDITOR": "Editor extension host",
}

TAIL = r'''
const DEFAULT_LOCALE = "zh";

// 归一化 locale：'zh-CN'/'zh'/'cn' -> 'zh'；'en'/'en-US'/... -> 'en'；其它 -> 'zh'（回退源语言）
function normLocale(locale) {
  if (!locale) return DEFAULT_LOCALE;
  const l = String(locale).toLowerCase();
  if (l.indexOf("en") === 0) return "en";
  return "zh";
}

function pick(code, locale) {
  const z = ZH[code];
  if (!z) return null;
  const severity = z[0], domain = z[1];
  let title = z[2], msg = z[3];
  const dz = DOMAINS[domain];
  let domainName = dz ? dz[0] : domain;
  if (locale === "en") {
    const e = EN[code];
    if (e) {
      if (e[0]) title = e[0];
      if (e[1]) msg = e[1];
    }
    if (dz && dz[1]) domainName = dz[1];
  }
  return { id: code, severity: severity, domain: domain, domainName: domainName, title: title, msg: msg };
}

// 取一条错误码的完整信息（本地化）。未知码返回 null。
//   errorInfo("E-PARSE-006")                 -> 中文（默认）
//   errorInfo("E-PARSE-006", {locale:"en"})  -> 英文
export function errorInfo(code, opts) {
  opts = opts || {};
  if (!code || typeof code !== "string") return null;
  return pick(code, normLocale(opts.locale || opts.lang));
}

// 只要 msg（本地化）；未知码返回 null
export function errorMessage(code, opts) {
  const info = errorInfo(code, opts);
  return info ? info.msg : null;
}

// 只要 title（本地化）；未知码返回 null
export function errorTitle(code, opts) {
  const info = errorInfo(code, opts);
  return info ? info.title : null;
}

// 列出所有错误码，可按 domain / severity 过滤；返回 [{id, severity, domain}]
export function listErrorCodes(opts) {
  opts = opts || {};
  const out = [];
  for (const code of Object.keys(ZH)) {
    const severity = ZH[code][0], domain = ZH[code][1];
    if (opts.domain && domain !== opts.domain) continue;
    if (opts.severity && severity !== opts.severity) continue;
    out.push({ id: code, severity: severity, domain: domain });
  }
  return out;
}

export { DEFAULT_LOCALE };
'''


def main():
    check = "--check" in sys.argv
    with open(ERRORS_JSON, encoding="utf-8") as fh:
        data = json.load(fh)
    with open(MSG_EN, encoding="utf-8") as fh:
        en = json.load(fh)
    en.pop("_comment", None)

    domains_zh = data.get("domains", {})
    zh = {}
    enmap = {}
    for c in data["codes"]:
        cid = c["id"]
        zh[cid] = [c.get("severity", ""), c.get("domain", ""), c.get("title", ""), c.get("msg", "")]
        e = en.get(cid)
        if e:
            enmap[cid] = [e.get("title", ""), e.get("msg", "")]

    missing = [c["id"] for c in data["codes"] if c["id"] not in enmap]
    if missing:
        print("警告：%d 条码缺英文（将回退中文）：%s" % (len(missing), ", ".join(missing)))

    dom = {}
    for did, zname in domains_zh.items():
        dom[did] = [zname, DOMAIN_EN.get(did, zname)]

    def js(obj):
        return json.dumps(obj, ensure_ascii=False, separators=(",", ":"))

    lines = [
        "// SPDX-License-Identifier: MulanPSL-2.0",
        "// 本文件由 errors/gen_js_errors.py 生成，请勿手改。",
        "// 中文（默认/源语言）来自 site/static/errors.json（唯一事实来源 errors/codes.sml）；",
        "// 英文来自 errors/msg.en.json。某条码/字段缺英文时回退中文，永不返回空。",
        "//",
        "// 用法：",
        '//   import { errorInfo, errorMessage, errorTitle, listErrorCodes } from "./sml-errors.mjs";',
        '//   errorInfo("E-PARSE-006")                 // 中文（默认）',
        '//   errorInfo("E-PARSE-006", {locale:"en"})  // 英文',
        '//   errorMessage("E-PARSE-006", {locale:"en"})',
        "",
        "// [severity, domain, title, msg] —— 中文（源语言）",
        "const ZH = " + js(zh) + ";",
        "// [title, msg] —— 英文（缺失则回退中文）",
        "const EN = " + js(enmap) + ";",
        "// 领域显示名 [zh, en]",
        "const DOMAINS = " + js(dom) + ";",
        TAIL,
    ]
    content = "\n".join(lines)

    if check:
        if not os.path.isfile(OUT):
            print("CHECK FAIL: %s 不存在（跑 python errors/gen_js_errors.py 生成）" % OUT)
            return 1
        with open(OUT, encoding="utf-8") as fh:
            cur = fh.read()
        if cur != content:
            print("CHECK FAIL: js/sml-errors.mjs 与将生成内容不一致（重跑生成器）")
            return 1
        print("CHECK OK: js/sml-errors.mjs 与 errors.json + msg.en.json 一致（%d 条码）" % len(zh))
        return 0

    with open(OUT, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(content)
    print("已生成 %s（%d 条码，英文覆盖 %d）" % (os.path.relpath(OUT, ROOT).replace("\\", "/"), len(zh), len(enmap)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
