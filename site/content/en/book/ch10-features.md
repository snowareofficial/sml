---
title: "Chapter 10: Complete Reference to Features"
translationKey: "book-ch10"
---
# Chapter 10: Complete Reference to Features

The design principle of SML is "from minimalism to richness, with customizable functions" - the basic seven piece set is enabled by default; Complex abilities are disabled by default and can be explicitly enabled using `@feature enable` when needed.

This chapter is an authoritative reference for each feature: opening methods, syntax, error messages, and relationships with other features.

## 10.1 How to turn on/off features

At the beginning of the file, use the `@feature` command:

```sml
@version v1
@feature enable glob regex
@feature disable ext-rewrite
```

-`enable` name list: separated by spaces.

-`disable` name list: Same as above.

-It can appear multiple times and take effect by overlapping in the order of appearance.

-Write after `@version` and before other content.

After activation, the corresponding capabilities can be used within this file (or included subfiles); Different files can be declared independently.

## 10.2 Built in feature list

### `include` (**default enabled**)

The most basic "file insertion" ability.

|Item | Value|
|----|----|
|Status | Default enabled, unable to disable|
|Grammar | `include "path.sml"`|
|Function | Insert the content of the target file as it is|
|Path benchmark | The directory containing the file itself|
|Nesting limit | 32 layers (over limit error)|
|Circular reference | Error (not silent)|

Error codes: `include.circular`, `include.depth-exceeded`, `include.not-found`.

### `namespace` (**default enabled**)

`as` namespace, pointwise path, macro/contract isolation.

|Item | Value|
|----|----|
|Status | Default On|
|Grammar | `include "x" as a.b.c`|
|Function | Package content into `a { b { c { ... } } }` nested blocks|
|Macro Isolation | Yes - `@name`/`@contract` is also isolated by namespace|
|External reference syntax | `a.b.c.MacroName` (qualified name)|
|Internal reference syntax | `MacroName` (local name, parser automatically adds prefix)|

Error codes: `ns.invalid-path`, `ns.duplicate-symbol`, `ns.unresolved-prefix`.

### `implicit-ns` (**default enabled**)

`include` without extension is automatically used as a namespace.

|Item | Value|
|----|----|
|Status | Default On|
|Grammar | `include "ui"` Equivalent `include "ui.sml" as ui`|
|Implicit namespace name | file name (without extension)|
|Explicit `as` Coverage | Explicit `as foo` takes precedence over implicit|

>Close it: `@feature disable implicit-ns` - Afterwards, `include "ui"` must have an extension.

### `multi` (**default off**)

Include multiple targets and `import` aliases at once.

|Item | Value|
|----|----|
|Status | Default Off|
|Open | `@feature enable multi`|
|Grammar | `include "a.sml", "b.sml" as y, "c"`|
|Alias format | `import ui.buttons, admin.panel`|
|Same name conflict | Error (not silent)|

Error codes: `multi.dup-name`, `multi.empty`.

### `glob` (**default off**)

`*` is compatible with multiple files in the matching directory.

|Item | Value|
|----|----|
|Status | Default Off|
|Open | `@feature enable glob`|
|Grammar | `include "widgets/*.sml"`|
|Wildcard | `*` (one character, not crossing `/`), `?` (single character)|
|Sort | Dictionary order determined, consistent across platforms|
|Implicit `as` | Applicable - `include "widgets/*"` ⇒ `as widgets`|

Error code: `glob.not-found` (if 0 matches) `glob.malformed`。

### `regex` (**default off**)

`re:`/`/.../` prefix triggers regular matching.

|Item | Value|
|----|----|
|Status | Default Off|
|Open | `@feature enable regex`|
|Grammar | `include "re:^v[0-9]+\\.sml$" as versions`|
|Regular subset| `.` `*` `+` `?` `^` `$` `[a-z]`、`\\.` `\\d` `\\w` |
|Performance | Handwritten recursive backtracking, O (n · m) sufficient file name short string|
|Attention | Use `\\.` as the path delimiter (`\` also needs to be double written in md)|

>Complete regularization features (lookahead, backref) **not supported** - deliberately kept simple, for complex matching, please use shell universal+glob.

### `ext-rewrite` (**default off**)

Parse files with any suffix as `.sml`.

|Item | Value|
|----|----|
|Status | Default Off|
|Open | `@feature enable ext-rewrite`|
|Grammar | `include "*.json" -> .sml`, `include "conf" -> .sml`|
|Typical usage | Rewrite `.json`/`.yaml`/`.conf` and process with SML parser|
|Risk | Incorrect parsing of binary files as SMLs can cause stack explosion; Suggest adding globe restrictions|

### `contract` (**default enabled**)

`@contract`/`@is` verification and backfilling.

|Item | Value|
|----|----|
|Status | Default On|
|Grammar | `@contract Name [loose] { field: type [default v] [min n] [max n] [enum(a,b)] [?] [required] }`|
|Reference type | `str` `int` `num` `bool` XQZ `array[T]` `enum(...)` or another contract name|
|Strictness | default strictness; `loose` allows undeclared fields|
|Nested | Infinite; Recursive contract detects loops when referenced|
|Wrong position | Accurate to rows and columns|

Error codes: `contract.required-missing`, `contract.type-mismatch`, `contract.enum-invalid`, `contract.out-of-range`, `contract.unknown-field`, `contract.recursive`.

### `env` (**default enabled**)

`$env.VAR` environment variable injection.

|Item | Value|
|----|----|
|Status | Default On|
|Grammar | `$env.VAR_NAME`|
|Missing behavior | Replace with empty string (no error reported)|
|Type | Always a string (numbers should also be quoted)|
|Escape | `_` `.` `-` is available in the name; The first character must be a letter or `_`|
|Nested | `$env` cannot write `$env` anymore|

Error code: `env.bad-name`.

### `escape` (**default enabled**)

Escaping within a quoted string.

|Item | Value|
|----|----|
|Status | Default On|
|Support| `\n` `\t` `\r` `\\` `\"` `\'` `\0` `\u{XXXX}` `\uXXXX` |
|Not supported | Octal `\077`, hexadecimal naked `\x41` (to avoid ambiguity)|
|Scope | Quotation string only; Naked words do not escape|

### `fragment` (**default enabled**)

`@name`/`&name` fragment inheritance.

|Item | Value|
|----|----|
|Status | Default On|
|Grammar | Define `@name { ... }`, Reference `key: &name`|
|Scope | Follow namespace (when `@feature enable namespace`)|
|Conflict | Same Scope Duplicate `@name` Error|

### `top-array` (**default enabled**)

The top level of the file allows for arrays (not objects).

|Item | Value|
|----|----|
|Status | Default On|
|Grammar | `[{...} {...} ...]` (top layer is array)|
|Typical scenario | Configuration items are sequential tables (such as monitoring rules, routing tables)|

### `bareword-str` (**default enabled**)

Naked words are automatically recognized as strings.

|Item | Value|
|----|----|
|Status | Default On|
|Close | `@feature disable bareword-str` - all strings must be quoted afterwards|
|Trigger | When version `v.strict-strings()` is set|

## 10.3 Compatibility Matrix

|Features ↓→| with namespace | with multi | with globe | with regex|
|---|---|---|---|---|
| `include` |  ✅  Direct combination| ✅ | ✅ | ✅ |
| `namespace` | — |  ✅ | ✅ | ✅ |
| `multi` |  ✅ | — | ⚠️  See Note 1| ⚠️ |
| `glob` |  ✅ (Implicit `as`)| ⚠️ | — | ❌  Exclusive see Note 2|
| `regex` |  ✅ | ⚠️ | ❌  Mutual exclusion | -|
| `ext-rewrite` |  ✅ | ✅ | ✅ | ✅ |
| `contract` |  ✅ (Reference contract with restricted name) | --- | --- | ---|

>**Note 1**: The "comma separated" syntax of `include "a, b" as y` is used together with `multi`, and a single target in the list cannot contain `,` (even inside quotation marks - commas inside quotation marks are considered literal).

>

>**Note 2**: `include "re:^.*\\.sml$"` has already covered all. sml, there is no need to use `include "*.sml"` again. Simultaneously using what may be interpreted as "glob priority" or "regex priority" may result in different cross implementation behaviors - SML specifies **explicit prefix priority**: `re:` follows regex; `*.sml` takes the globe.

## 10.4 Implementation Layer of Features (Architecture Tips)

SML parser runs according to "feature bitmask":

```text
FeatureSet = (include | namespace | implicit-ns | contract | env | escape
              | fragment | top-array | bareword-str
              | multi | glob | regex | ext-rewrite)
```

-Core layer (default enabled) 7 bits default=1.

-Entering the hierarchy (default off) 4 bits default=0.

-The parser directly rejects the corresponding syntax for unopened features (parsing error, **not silently skipping**).

This is the implementation foundation of 'customizable functionality, don't fall into the same path as YAML'.

## 10.5 Performance and Portability

|Features | Impact on parsing time | Cross platform differences|
|---------|----------------|------------|
|`include` | O (total file size) | Path delimiter normalization (`/`) ↔  `\`） |
|`namespace` | Extremely small (path compiled once) | None|
|`multi` | Linear superposition | None|
|`glob` | O (n) file enumeration | hidden files (`.foo`) default **not** included|
|`regex` | O (n · m) short string | None|
|`ext-rewrite` | Size of file to be rewritten | Content encoding assumed UTF-8|
|`contract` | Linear with field number | Recursive contract needs to be memoize|

## 10.6 Future Features (Roadmap)

The following **has not yet been implemented** and is only a roadmap preview to avoid readers' misuse:

-`@import once` (remove duplicate include to avoid the same file being included multiple times)

-`feature from "another.sml"` (Inherit feature settings from another file)

-`with contract=loose` (block level relaxation, override file level settings)

-`?` ternary abbreviation (semantics of `a ? b : c` at value position) - Currently, `?` is an optional tag

## 10.7 Give it a try with your hands

Write an `features.sml` for your project:

```sml
@version v1
@feature enable glob multi
@feature disable ext-rewrite

include "modules/*.sml" as modules
import modules.auth, modules.billing
```

Use commands such as `sml check features.sml` (refer to [ch07 Multilingual](/en/book/ch07-languages)) to run parsing and contract verification.

→ [Appendix: Comparison and Investigation](/en/book/appendix)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch10" >}}

{{< sml-quiz "ch10" >}}
