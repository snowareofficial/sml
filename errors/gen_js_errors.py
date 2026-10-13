#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""从 site/static/errors.json（中文，唯一事实来源）+ errors/msg.en.json（英文层）
生成「按错误码取本地化信息」的模块（i18n）。**多端产物**：JS / Rust / C / Lua。

    python errors/gen_js_errors.py            # 生成全部目标
    python errors/gen_js_errors.py --check    # 只校验不写（给 CI）：产物与将生成内容逐字节一致

i18n 约定（与仓库一致）：**中文是源语言（默认）**，英文是翻译层；某条码/字段缺英文时
回退中文，永不返回空。错误码本身是语言中立的稳定契约（见 errors/README.md）。

数据流：errors/codes.sml --(gen_json.py)--> site/static/errors.json（中文）
        errors/msg.en.json（英文，手维护） --(本脚本)--> 各端模块（双语）

产物（全部是生成物，**不要手改**）：
    js/sml-errors.mjs              JS 模块（ESM）
    rust/sml-codes/src/msgs.rs     Rust 模块（查表 + 归一 + 回退）
    c/sml_codes_msg.h              C / C++ 头（静态表 + 线性查找；header-only）
    lua/sml_errors.lua             Lua 模块（由 lib/sml.soup 的 Sml.error_info 复用）

五端对同一组向量必须给出相同结果（见 js/test-sml-errors.mjs）。文案从数据来，
不许手写/改写任何 title/msg。
"""
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ERRORS_JSON = os.path.join(ROOT, "site", "static", "errors.json")
MSG_EN = os.path.join(ROOT, "errors", "msg.en.json")
JS_OUT = os.path.join(ROOT, "js", "sml-errors.mjs")
RUST_OUT = os.path.join(ROOT, "rust", "sml-codes", "src", "msgs.rs")
C_OUT = os.path.join(ROOT, "c", "sml_codes_msg.h")
LUA_OUT = os.path.join(ROOT, "lua", "sml_errors.lua")

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


def js(obj):
    return json.dumps(obj, ensure_ascii=False, separators=(",", ":"))


def c_escape(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def rust_escape(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def lua_escape(s):
    return s.replace("\\", "\\\\").replace('"', '\\"')


def build_data():
    """读两份数据，返回 (zh, enmap, dom, en_all)。

    zh:     {code: [severity, domain, zh_title, zh_msg]}
    enmap:  {code: [en_title, en_msg]}  （仅 msg.en.json 中存在的码）
    dom:    {domain: [zh_name, en_name]}
    en_all: {code: [en_title, en_msg]}  （全部码；缺英文为 ["", ""]）
    """
    with open(ERRORS_JSON, encoding="utf-8") as fh:
        data = json.load(fh)
    with open(MSG_EN, encoding="utf-8") as fh:
        en = json.load(fh)
    en.pop("_comment", None)

    domains_zh = data.get("domains", {})
    zh = {}
    enmap = {}
    en_all = {}
    for c in data["codes"]:
        cid = c["id"]
        zh[cid] = [c.get("severity", ""), c.get("domain", ""),
                   c.get("title", ""), c.get("msg", "")]
        e = en.get(cid)
        if e:
            enmap[cid] = [e.get("title", ""), e.get("msg", "")]
            en_all[cid] = [e.get("title", ""), e.get("msg", "")]
        else:
            en_all[cid] = ["", ""]

    dom = {}
    for did, zname in domains_zh.items():
        dom[did] = [zname, DOMAIN_EN.get(did, zname)]

    return zh, enmap, dom, en_all


def js_src(zh, enmap, dom):
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
    return "\n".join(lines)


def rust_src(zh, en_all):
    codes = sorted(zh.keys())
    lines = [
        "// SPDX-License-Identifier: MulanPSL-2.0",
        "//! SML 错误码本地化信息查找表（由生成器产出，请勿手改）。",
        "//!",
        "//! 中文（默认/源语言）来自 `site/static/errors.json`；英文来自 `errors/msg.en.json`。",
        "//! 某条码/字段缺英文时回退中文，永不返回空。",
        "//!",
        "//! 与 `js/sml-errors.mjs` 逐字对齐（五端同向量同结果）。",
        "",
        "/// 一条错误码的完整本地化信息。",
        "pub struct ErrorInfo {",
        "    pub id: &'static str,",
        "    pub severity: &'static str,",
        "    pub domain: &'static str,",
        "    pub title: String,",
        "    pub msg: String,",
        "}",
        "",
        "struct Entry {",
        "    id: &'static str,",
        "    severity: &'static str,",
        "    domain: &'static str,",
        "    zh_title: &'static str,",
        "    zh_msg: &'static str,",
        "    en_title: &'static str, // 缺英文为 \"\"",
        "    en_msg: &'static str,",
        "}",
        "",
        "static ENTRIES: &[Entry] = &[",
    ]
    for cid in codes:
        sev, dom, zt, zm = zh[cid]
        et, em = en_all[cid]
        lines.append(
            '    Entry { id: "%s", severity: "%s", domain: "%s",'
            ' zh_title: "%s", zh_msg: "%s", en_title: "%s", en_msg: "%s" },'
            % (cid, rust_escape(sev), rust_escape(dom),
               rust_escape(zt), rust_escape(zm),
               rust_escape(et), rust_escape(em))
        )
    lines += [
        "];",
        "",
        "/// 全部错误码 id（按升序）。",
        "pub const ALL: &[&str] = &[",
    ]
    for cid in codes:
        lines.append('    "%s",' % cid)
    lines += [
        "];",
        "",
        "/// 默认 locale（中文/源语言）。",
        'pub const DEFAULT_LOCALE: &str = "zh";',
        "",
        "fn norm_locale(locale: &str) -> &str {",
        '    if locale.is_empty() { return DEFAULT_LOCALE; }',
        "    let l = locale.to_lowercase();",
        '    if l.starts_with("en") { "en" } else { "zh" }',
        "}",
        "",
        "/// 取一条错误码的完整本地化信息；未知码返回 `None`。",
        "///",
        "/// - `code`：错误码字符串（空/非字符串按未知码处理）。",
        "/// - `locale`：可选；缺省 / 空 = `zh`；以 `en` 开头 = 英文；其它回落中文。",
        "/// - 缺英文：该字段回退中文，永不返回空串。",
        "pub fn error_info(code: &str, locale: &str) -> Option<ErrorInfo> {",
        "    if code.is_empty() { return None; }",
        "    let e = ENTRIES.iter().find(|e| e.id == code)?;",
        "    let mut title = e.zh_title.to_string();",
        "    let mut msg = e.zh_msg.to_string();",
        '    if norm_locale(locale) == "en" {',
        "        if !e.en_title.is_empty() { title = e.en_title.to_string(); }",
        "        if !e.en_msg.is_empty() { msg = e.en_msg.to_string(); }",
        "    }",
        "    Some(ErrorInfo {",
        "        id: e.id,",
        "        severity: e.severity,",
        "        domain: e.domain,",
        "        title,",
        "        msg,",
        "    })",
        "}",
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    use super::*;",
        "",
        "    #[test]",
        "    fn zh_default_title() {",
        "        let i = error_info(\"E-PARSE-006\", \"\").unwrap();",
        "        assert_eq!(i.title, \"期望键或标识符\");",
        "        assert_eq!(i.domain, \"PARSE\");",
        "        assert_eq!(i.severity, \"error\");",
        "    }",
        "",
        "    #[test]",
        "    fn en_title() {",
        "        let i = error_info(\"E-PARSE-006\", \"en\").unwrap();",
        "        assert_eq!(i.title, \"Expected key or identifier\");",
        "    }",
        "",
        "    #[test]",
        "    fn en_us_normalize() {",
        "        let i = error_info(\"E-LEX-001\", \"en-US\").unwrap();",
        "        assert_eq!(i.title, \"Unterminated string\");",
        "    }",
        "",
        "    #[test]",
        "    fn locale_fallback_zh() {",
        "        assert_eq!(error_info(\"E-LEX-001\", \"zh-CN\").unwrap().title, \"字符串未闭合\");",
        "        assert_eq!(error_info(\"E-LEX-001\", \"fr\").unwrap().title, \"字符串未闭合\");",
        "    }",
        "",
        "    #[test]",
        "    fn unknown_is_none() {",
        "        assert!(error_info(\"E-NOPE-999\", \"zh\").is_none());",
        "        assert!(error_info(\"E-NOPE-999\", \"\").is_none());",
        "        assert!(error_info(\"\", \"zh\").is_none());",
        "    }",
        "",
        "    #[test]",
        "    fn all_nonempty_and_parse_count() {",
        "        let mut parse = 0usize;",
        "        let mut empty = 0usize;",
        "        for code in ALL {",
        "            let i = error_info(code, \"zh\").expect(\"known code\");",
        "            if i.title.is_empty() || i.msg.is_empty() { empty += 1; }",
        "            if i.domain == \"PARSE\" { parse += 1; }",
        "        }",
        "        assert_eq!(empty, 0, \"所有码 title/msg 必须非空\");",
        "        assert_eq!(parse, 25, \"PARSE 领域应为 25 条\");",
        "    }",
        "}",
        "",
    ]
    return "\n".join(lines)


def c_src(zh, en_all):
    codes = sorted(zh.keys())
    lines = [
        "/* SPDX-License-Identifier: MulanPSL-2.0 */",
        "/* 本文件由 errors/gen_js_errors.py 生成，请勿手改。",
        " * 中文（默认/源语言）来自 site/static/errors.json；英文来自 errors/msg.en.json。",
        " * 某条码/字段缺英文时回退中文，永不返回空。",
        " *",
        " * 用法：",
        ' *   #include "sml_codes_msg.h"',
        ' *   const char *t = sml_error_title("E-PARSE-006", "en");  // "Expected key or identifier"',
        ' *   const char *m = sml_error_msg("E-PARSE-006", NULL);   // 中文 msg（locale 为 NULL/"" = 中文）',
        " *",
        " * 未知码 / NULL 入参 -> 返回 NULL。locale 以 \"en\" 开头 -> 英文，否则中文。",
        " * C++ 直接 include 本头即可（header-only，不另造）。",
        " */",
        "#ifndef SML_CODES_MSG_H",
        "#define SML_CODES_MSG_H",
        "",
        "#include <string.h>",
        "",
        "struct SmlErrorMsg {",
        "    const char *code;",
        "    const char *severity;",
        "    const char *domain;",
        "    const char *zh_title;",
        "    const char *zh_msg;",
        "    const char *en_title; /* 缺英文为 NULL */",
        "    const char *en_msg;",
        "};",
        "",
        "static const struct SmlErrorMsg SML_ERROR_MSGS[] = {",
    ]
    for cid in codes:
        sev, dom, zt, zm = zh[cid]
        et, em = en_all[cid]
        en_t = ("\"%s\"" % c_escape(et)) if et else "NULL"
        en_m = ("\"%s\"" % c_escape(em)) if em else "NULL"
        lines.append(
            "    { \"%s\", \"%s\", \"%s\", \"%s\", \"%s\", %s, %s },"
            % (cid, c_escape(sev), c_escape(dom),
               c_escape(zt), c_escape(zm), en_t, en_m)
        )
    lines += [
        "    { NULL, NULL, NULL, NULL, NULL, NULL, NULL } /* 哨兵 */",
        "};",
        "",
        "static int sml_locale_is_en(const char *locale) {",
        "    if (!locale || !*locale) return 0;",
        "    char c0 = locale[0], c1 = locale[1];",
        "    if (c0 >= 'A' && c0 <= 'Z') c0 = (char)(c0 + 32);",
        "    if (c1 >= 'A' && c1 <= 'Z') c1 = (char)(c1 + 32);",
        "    return (c0 == 'e' && c1 == 'n');",
        "}",
        "",
        "static const struct SmlErrorMsg *sml_find_error(const char *code) {",
        "    if (!code || !*code) return NULL;",
        "    for (int i = 0; SML_ERROR_MSGS[i].code != NULL; i++) {",
        "        if (strcmp(SML_ERROR_MSGS[i].code, code) == 0) return &SML_ERROR_MSGS[i];",
        "    }",
        "    return NULL;",
        "}",
        "",
        "/* 取标题（本地化）；未知码 / NULL 入参 -> NULL */",
        "static const char *sml_error_title(const char *code, const char *locale) {",
        "    const struct SmlErrorMsg *e = sml_find_error(code);",
        "    if (!e) return NULL;",
        "    if (sml_locale_is_en(locale) && e->en_title && e->en_title[0]) return e->en_title;",
        "    return e->zh_title;",
        "}",
        "",
        "/* 取文案（本地化）；未知码 / NULL 入参 -> NULL */",
        "static const char *sml_error_msg(const char *code, const char *locale) {",
        "    const struct SmlErrorMsg *e = sml_find_error(code);",
        "    if (!e) return NULL;",
        "    if (sml_locale_is_en(locale) && e->en_msg && e->en_msg[0]) return e->en_msg;",
        "    return e->zh_msg;",
        "}",
        "",
        "#endif /* SML_CODES_MSG_H */",
        "",
    ]
    return "\n".join(lines)


def lua_src(zh, en_all):
    codes = sorted(zh.keys())
    lines = [
        "-- SPDX-License-Identifier: MulanPSL-2.0",
        "-- 本文件由 errors/gen_js_errors.py 生成，请勿手改。",
        "-- 中文（默认/源语言）来自 site/static/errors.json；英文来自 errors/msg.en.json。",
        "-- 某条码/字段缺英文时回退中文，永不返回空。",
        "-- 由 lua/lib/sml.soup 的 Sml.error_info 复用（五端同向量同结果）。",
        "local M = {}",
        "",
        "M.ZH = {",
    ]
    for cid in codes:
        sev, dom, zt, zm = zh[cid]
        lines.append(
            '  ["%s"] = { severity = "%s", domain = "%s", title = "%s", msg = "%s" },'
            % (cid, lua_escape(sev), lua_escape(dom),
               lua_escape(zt), lua_escape(zm))
        )
    lines += [
        "}",
        "",
        "M.EN = {",
    ]
    for cid in codes:
        et, em = en_all[cid]
        if et or em:
            lines.append(
                '  ["%s"] = { title = "%s", msg = "%s" },'
                % (cid, lua_escape(et), lua_escape(em))
            )
    lines += [
        "}",
        "",
        "function M.norm_locale(locale)",
        '  if not locale or locale == "" then return "zh" end',
        '  local l = string.lower(tostring(locale))',
        '  if l:sub(1, 2) == "en" then return "en" end',
        '  return "zh"',
        "end",
        "",
        "-- 取一条错误码的完整本地化信息；未知码返回 nil。",
        "--   code   错误码字符串（空/非字符串按未知码处理）",
        '--   locale 可选；缺省/空 = "zh"；以 "en" 开头 = 英文；其它回落中文',
        "--   缺英文：该字段回退中文，永不返回空串",
        "function M.error_info(code, locale)",
        '  if type(code) ~= "string" or code == "" then return nil end',
        "  local z = M.ZH[code]",
        "  if not z then return nil end",
        "  local title, msg = z.title, z.msg",
        '  if M.norm_locale(locale) == "en" then',
        "    local e = M.EN[code]",
        "    if e then",
        '      if e.title and e.title ~= "" then title = e.title end',
        '      if e.msg and e.msg ~= "" then msg = e.msg end',
        "    end",
        "  end",
        '  return { title = title, msg = msg }',
        "end",
        "",
        "return M",
        "",
    ]
    return "\n".join(lines)


def main():
    check = "--check" in sys.argv
    zh, enmap, dom, en_all = build_data()

    targets = [
        (JS_OUT, js_src(zh, enmap, dom), "js/sml-errors.mjs"),
        (RUST_OUT, rust_src(zh, en_all), "rust/sml-codes/src/msgs.rs"),
        (C_OUT, c_src(zh, en_all), "c/sml_codes_msg.h"),
        (LUA_OUT, lua_src(zh, en_all), "lua/sml_errors.lua"),
    ]

    if check:
        bad = []
        for path, want, rel in targets:
            if not os.path.isfile(path):
                bad.append("%s：缺失（请跑 python errors/gen_js_errors.py）" % rel)
                continue
            with open(path, encoding="utf-8") as fh:
                cur = fh.read()
            if cur != want:
                bad.append("%s：与数据不一致（请跑 python errors/gen_js_errors.py）" % rel)
        if bad:
            print("CHECK FAIL: 以下产物漂移：")
            for b in bad:
                print("   -", b)
            return 1
        print("CHECK OK: 4 个产物与 errors.json + msg.en.json 一致（%d 条码）" % len(zh))
        for _, _, rel in targets:
            print("CHECK OK: %s 一致" % rel)
        return 0

    for path, want, rel in targets:
        os.makedirs(os.path.dirname(path), exist_ok=True)
        # 产物一律 LF（newline="" + 显式 \n），避免 Windows 检出把生成物写成 CRLF
        with open(path, "w", encoding="utf-8", newline="") as fh:
            fh.write(want)
        print("已生成 %s（%d 条码）" % (rel, len(zh)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
