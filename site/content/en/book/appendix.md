---
title: "Appendix: Comparison and Investigation"
translationKey: "book-appendix"
---
# Appendix: Comparison and Investigation

This appendix compares SML with common formats and lists the most common errors and solutions encountered by beginners. Can be used as a reference book to flip through at any time.

## A.1 Comparison with JSON/YAML/TOML

|Ability | SML | JSON | YAML | TOML|
|------|-----|------|------|------|
|Quotation marks optional| ✅  Naked words are strings| ❌  Must be| ✅ (But there are type inference pits)| ✅ |
|Comma optional| ✅ | ❌ | ✅ | ❌ |
|Block colon can save| ✅  `a { }` ≡ `a: { }` | — |  ✅ | — |
|Indent sensitive| ❌  Relying on `{}`| ❌  Relying on `{}`| ✅  Easy to make mistakes | Partial|
|Fragment reuse| ✅  `@base`/`&base` |  ❌ | ❌ (Complex anchor points)| ❌ |
|Namespace include| ✅  `as ns` |  ❌ | ❌ | ❌ |
|Contract/Schema| ✅  Built in| ❌ | ❌ | ❌ |
|Environment variable injection| ✅  `$env` |  ❌ | ❌ | ❌ |

## A.2 Common Error Troubleshooting

|Phenomenon | Reason | Solution|
|------|------|------|
|String truncated | Value containing spaces but using bare words | Quoted `"..."`|
|Fragment not unfolded | Block naked writing `&base` | Write as `key: &base` (value position)|
|Contract report 'undeclared field' | Default strict mode | Add `loose` after the contract name, or remove unnecessary fields|
|`port` out of range error | `min/max` constraint effective | Correction value, or interval relaxation|
|`$env.X` empty string | Variable not set (normal) | Confirm that environment variables have been exported during runtime|
|Include file not found | Path relative to included file directory | Check relative path; Confirm that the file exists|
|Loop include error | A contains B, and B contains A in turn | Break loop dependency|
|`@is` report contract undefined | Contract written after `@is` | Move `@contract` to before `@is`|
|Namespace conflict | Same ns duplicate definition `@name`/`@contract` | Rename, or use different `as ns`|

## A.3 Grammar Quick Check

```
Key-Value:key: value
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
escape:"line1\nline2 \u{2744}"
```

## A.4 version and features

The beginning of the file can declare the version:

```sml
@version v1
```

Complex abilities can be activated as needed:

```sml
@feature enable glob regex
include "widgets/*.sml"
```

By default, `include`+`namespace`+`implicit-ns` (minimalist three piece set) is enabled; `multi`/`glob`/`regex`/`ext-rewrite` are turned off by default and require explicit opt in.

---

---

## Feedback and Version

This book is released with `swsml 0.4.0`. Source code and updates: [/book/](/en/book/). Please go to the issue area of SML warehouse for feedback.

The whole book is finished. Return to the homepage of the textbook (/book/).

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "appendix" >}}

{{< sml-quiz "appendix" >}}
