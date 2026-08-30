# -*- coding: utf-8 -*-
"""修复 en/book 机器翻译引入的代码语义破坏与排版问题。"""
import os
import sys

# Windows 控制台默认 GBK，emoji 会导致 print 崩溃
try:
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass

BASE = "site/content/en/book"

REPLACEMENTS = {
    "ch09-advanced.md": [
        # array[...] 是 SML 关键字，被误译为 Arrangement 且加了空格
        ('Using `Arrangement [contract name]` to express "contract array" - suitable for list type data:',
         'Using `array[ContractName]` to express a "contract array" — suitable for list data:'),
        ("password: str  ?               # Optional: Local/Test Empty",
         "password: str  ?               # Optional: may be empty for local/test"),
        ("webhook: $env.OPTIONAL_WEBHOOK   # Not set ->empty string",
         "webhook: $env.OPTIONAL_WEBHOOK   # Unset -> empty string"),
    ],
    "ch05-contract.md": [
        # @is 是关键字，被拆成 "@ is"
        ("# Writing Method 1: Anonymous Block Top Level Direct @ is",
         "# Style 1: @is at the top level of an anonymous block"),
        ("# Writing 2: Block level @ is",
         "# Style 2: block-level @is"),
        ("`contract: Service — field main.port Greater than the maximum value 65535`",
         "`contract: Service — field main.port is greater than the max 65535`"),
    ],
    "ch04-include.md": [
        # 路径被加了空格
        ('include "ui"          # Equivalent to include "ui. sml" as ui',
         'include "ui"          # Equivalent to include "ui.sml" as ui'),
        ('include "ui.sml" as ui.form.widgets   # Explicitly specified, priority given',
         'include "ui.sml" as ui.form.widgets   # Explicit namespace, takes priority'),
    ],
    "ch01-basics.md": [
        ("_* This is also a comment，Soup Habitual writing style *_",
         "_* This is also a block comment, the Soup-family convention *_"),
        ("firstName: \"John Doe\"     # Containing spaces ->must be quoted",
         "firstName: \"John Doe\"     # Contains spaces -> must be quoted"),
        ("state: NY                 # Single word ->Naked word is sufficient",
         "state: NY                 # Single word -> a bare word is enough"),
        ("from: \"SML Team <dev@mail.swebase.cn>\"   # Containing spaces ->quotation marks",
         "from: \"SML Team <dev@mail.swebase.cn>\"   # Contains spaces -> needs quotes"),
    ],
    "ch03-fragments.md": [
        ("    &base            # ❌  This way of writing,&base is treated as a 'key name' and will not expand fields",
         "    &base            # ❌  Wrong: &base is parsed as a \"key name\"; fields are NOT expanded"),
        ("    net: &base       # ✅  The. net key obtains all fields of the base",
         "    net: &base       # ✅  Key \"net\" receives all fields of base"),
    ],
    "ch06-env-escape.md": [
        ("optionalWebhook: $env.UNSET_WEBHOOK   # Not set ->empty string, no error reported",
         "optionalWebhook: $env.UNSET_WEBHOOK   # Unset -> empty string, no error"),
    ],
    "ch07-languages.md": [
        ("soupx lua/sml.sar config.sml     # Analyze and print",
         "soupx lua/sml.sar config.sml     # Parse and print"),
    ],
    "ch02-blocks.md": [
        ("ports: [ 80, 443, 8080 ]               # Even with commas",
         "ports: [ 80, 443, 8080 ]               # Commas are also fine"),
    ],
}

# appendix.md 的 A.3 速查表整体重建：恢复对齐、统一大小写、清掉中文标点
OLD_TABLE = """Key-Value:key: value
Naked word string:state: NY
Quotation string:name: "John Doe"
integer:age: 27
floating point:ratio: 0.75
Boolean:on: true
null value:x: null
Object Block:a { b: 1 }    ≡   a: { b: 1 }
array:list: [ a b c ]
Line comments:#  --  //
Block annotation:/* ... */     _* ... *_
Fragment definition:@name { ... }
Fragment reference:key: &name
Contract Definition:@contract Name loose { field: type ... }
Contract application:@is Name
include：   include "x.sml"        (inline)
            include "ui" as ui     (namespace)
            include "a", "b" as y  （多目标，需 feature）
environment variable:secret: $env.API_KEY
escape:"line1\\nline2 \\u{2744}\""""

NEW_TABLE = """Key-value:          key: value
Bare word string:   state: NY
Quoted string:      name: "John Doe"
Integer:            age: 27
Float:              ratio: 0.75
Boolean:            on: true
Null:               x: null
Object block:       a { b: 1 }    ≡   a: { b: 1 }
Array:              list: [ a b c ]
Line comments:      #  --  //
Block comments:     /* ... */     _* ... *_
Fragment define:    @name { ... }
Fragment ref:       key: &name
Contract define:    @contract Name loose { field: type ... }
Contract apply:     @is Name
include:            include "x.sml"        (inline)
                    include "ui" as ui     (namespace)
                    include "a", "b" as y  (multi-target, needs feature)
Env variable:       secret: $env.API_KEY
Escape:             "line1\\nline2 \\u{2744}\""""


def main():
    total = 0
    for fname, pairs in REPLACEMENTS.items():
        path = os.path.join(BASE, fname)
        with open(path, encoding="utf-8", newline="") as f:
            text = f.read()
        for old, new in pairs:
            if old in text:
                text = text.replace(old, new)
                total += 1
                print("  fixed  %s :: %s" % (fname, old.strip()[:52]))
            else:
                print("  MISS   %s :: %s" % (fname, old.strip()[:52]))
        with open(path, "w", encoding="utf-8", newline="") as f:
            f.write(text)

    ap = os.path.join(BASE, "appendix.md")
    with open(ap, encoding="utf-8", newline="") as f:
        t = f.read()
    # 文件可能是 CRLF，按实际行尾重建待匹配文本
    nl = "\r\n" if "\r\n" in t else "\n"
    old_tbl = OLD_TABLE.replace("\n", nl)
    new_tbl = NEW_TABLE.replace("\n", nl)
    if old_tbl in t:
        t = t.replace(old_tbl, new_tbl)
        total += 1
        print("  fixed  appendix.md :: A.3 table rebuilt (eol=%r)" % nl)
    else:
        print("  MISS   appendix.md :: A.3 table pattern not found (eol=%r)" % nl)
    with open(ap, "w", encoding="utf-8", newline="") as f:
        f.write(t)

    print("\napplied %d edits" % total)


if __name__ == "__main__":
    main()
