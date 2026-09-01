---
title: "SML { ❄ } — SNOWARE Markup Language"
translationKey: "en-home"
---

# SML { ❄ }

**SML (SNOWARE Markup Language)** is a declarative data / configuration format,
positioned as a lightweight alternative to JSON / YAML / TOML. It emphasizes
**readability** and **low ceremony**: quotes are optional, block colons can be
omitted, commas are optional, and it supports fragment inheritance and contract
validation.

> Repo: [snoware/sml](https://gitee.com/snoware/sml) ｜ **Reference implementation: Rust (`swsml`)** ｜ Experimental (not guaranteed): C (`sml.c`) · JavaScript (`sml.mjs`) · Lua (`lib/sml.soup`) · C++ · Python

## Features

- **Optional quotes**: bare words are strings; quotes only needed for spaces / special chars
- **Optional block colon**: `address { }` ≡ `address: { }`
- **Flexible array separators**: commas optional `[ a b c ]` ≡ `[ a, b, c ]`
- **Fragment inheritance**: `@base { }` defines, `&base` references — config reuse
- **include inline / namespaces**: `include "x.sml"` expands recursively; `include "x.sml" as a.b` isolates into a dotted scope (macros & contracts included), conflicts error out
- **Environment variables**: `$env.HOME` inlined at parse time
- **Contract system**: `@contract` / `@is` validate config type & structure (strict / loose modes)
- **Multi-target emit**: once parsed to `Value`, compile to Markdown / LaTeX / XML / SVG / Slint UI / custom formats (emit backends)
- **Zero dependencies**: each implementation is decoupled, embeddable individually (WASM / sandbox / editor)

## 📖 SML Textbook

Start from zero and progress step by step, or use it as a reference anytime.
→ **[Read online](/en/book/)**

- [Preface: Why SML](/en/book/intro)
- [Ch1: Your first SML file](/en/book/ch01-basics) · [Ch2: Blocks & nesting](/en/book/ch02-blocks) · [Ch3: Fragment inheritance](/en/book/ch03-fragments)
- [Ch4: include & namespaces](/en/book/ch04-include) · [Ch5: Contract system](/en/book/ch05-contract) · [Ch6: Env vars & escaping](/en/book/ch06-env-escape)
- [Ch7: Multi-language usage](/en/book/ch07-languages) · [Ch8: Real project](/en/book/ch08-project) · [Ch9: Feature composition](/en/book/ch09-advanced) · [Ch10: feature reference](/en/book/ch10-features) · [Ch11: Translation challenges](/en/book/ch11-challenges) · [Appendix: Cheatsheet & troubleshooting](/en/book/appendix)
- Offline: **[Download EPUB](/sml-book.epub)**

## Implementations

> **Implementation status (important)**
>
> - **Rust (`rust/`, crate `swsml`) is the reference implementation.** Grammar, contract
>   system, emit backends, tests and security scanning (OSV dependency audit + full
>   regression suite) are all defined against it. It is currently the **only continuously
>   maintained and version-guaranteed** implementation — use Rust for production.
> - **All non-Rust implementations (C / JavaScript / Lua / C++ / Python) are marked
>   "experimental".** They ship with the repo and do run and embed, but they are **not
>   guaranteed** to match Rust behaviour, have **no API stability promise**, and are **not
>   covered** by the routine vulnerability scan or regression tests. Evaluate before use;
>   issues are welcome.

| Lang | Repo / file | Status |
|------|------------|--------|
| Rust | `rust/` (`swsml`) | ✅ **Reference · recommended** (full contract system, serde bridge, routine vuln scan) |
| C | `c/sml.c` | ⚠️ Experimental (not guaranteed) |
| JavaScript | `js/sml.mjs` | ⚠️ Experimental (not guaranteed, zero-dep ESM, browser / Node, contracts & playground) |
| Lua | `lua/lib/sml.lua` | ⚠️ Experimental (not guaranteed, same source as Soup `lib/sml.soup`) |
| C++ | `cpp/` | ⚠️ Experimental (not guaranteed) |
| Python | see `py` bindings | ⚠️ Experimental (not guaranteed) |

> The Rust implementation defines the contract-system spec and its verdicts. C / JavaScript /
> C++ were once verified against it, but since non-Rust ports are now "not guaranteed",
> cross-language consistency is no longer a version promise.

## Quick start

```sml
# basic key/value
firstName: John
age: 27
address:
{
    streetAddress: "21 2nd Street"
    state: NY
}

# arrays (commas optional)
phoneNumbers: [ { type: home } { type: office } ]

# fragment inheritance (&name is a value reference, write as key: &name)
@base { region: cn-north-1 }
region: &base
```

### Rust

```rust
use sml::parse;
let v = parse("name: John\nage: 27").unwrap();
assert_eq!(v["name"], "John");
```

### C

```c
#include "sml.h"
char err[256] = {0};
sml_value *v = sml_parse("name: John\nage: 27", err, sizeof(err));
/* v->type == SML_STR ("John") ... free with sml_free(v) */
```

### JavaScript

```js
import { parse, stringify } from "./sml.mjs";
const v = parse('name: John\nage: 27');
console.log(stringify(v));
```

### C++

```cpp
#include "sml.hpp"
sml::Value v = sml::parse("name: John\nage: 27");
// v["name"].as_str() == "John" ｜ v["age"].as_int() == 27
// throws sml::ParseError (with line/col) on failure
```

## Versioning (`@version`)

SML declares the syntax version a document follows via `@version`, so the parser
can still read old documents if a future incompatible syntax is introduced. The
current implementation supports **v1 / v2 / v3** (`@version` accepts `v1`/`1`,
`v2`/`2`, `v3`/`3`; anything outside `v1..v3` errors outright), with **v3** as
the latest baseline.

| Version | Semantics | String syntax |
|---------|-----------|---------------|
| **v1** (default) | initial public release; barewords are strings, types auto-detected | `name: John` ✅ |
| **v2** | draft; introduced the incompatible "strings must be quoted" rule | `name: "John"` required |
| **v3** (CURRENT) | finalized; same semantics as v2; free text must be `"..."` | `name: "John"` required |

> v2 and v3 share **identical string semantics** — v2 is the draft codename, v3
> the finalized one. Numbers / `bool` / `null` / fragment refs `&x` / env vars
> `$env.X` remain **barewords** under v2 / v3 — only free strings need quotes.

```sml
# default v1: barewords are strings
name: John
age: 27
tags: [ a b c ]          # bareword array elements OK

# explicit v3: strings must be quoted, scalars stay bare
@version v3
name: "John"
age: 27
active: true
tags: [ "a" "b" "c" ]    # array elements also quoted
ref: &frag               # fragment ref still bareword
```

Undefined fragment refs are **hard errors** under v3 (no silent downgrade to a
string). Callers can also restrict accepted versions via
`parse_allowed(docs, &[Version::V1, Version::V2, Version::V3])` — out-of-range
documents are rejected, so `@version` cannot become a backdoor around capability
limits.

## Contract system

A contract is SML's "config Schema": it defines field types, required flags,
defaults and ranges, validated at every `@is` application. Ideal for
"configuration as SML" scenarios.

### Define a contract

```sml
@contract ResenderConfig loose {
    api_key:     str
    port:        int  default 8080 min 1 max 65535
    debug:       bool default false
    mode:        enum(active, disabled) default active
    tags:        array[str] ?
}
```

Field modifiers:

| Modifier | Meaning |
|----------|---------|
| `str` / `int` / `num` / `bool` | field type |
| `enum(a, b, c)` | enum, value must be one of them |
| `array[T]` | array of element type `T` |
| `?` or `optional` | optional field |
| `required` | explicit required (default) |
| `default <val>` | fill default when missing (also implicit optional) |
| `min <n>` / `max <n>` | numeric range (inclusive) |

### Apply a contract

```sml
@contract Cfg loose { api_key: str port: int default 8080 }
@is Cfg
api_key: re_abc
port: 8080
```

### Loose vs strict

- **`loose`**: allows fields not declared in the contract (tolerant, good for evolving configs)
- **`strict`**: forbids any undeclared field, otherwise validation fails

### Composed contracts (recursive refs)

```sml
@contract Endpoint { host: str port: int }
@contract Service {
    name:  str
    main:  Endpoint
    peers: array[Endpoint]
}
@is Service
name: gateway
main: { host: localhost port: 8080 }
peers: [ { host: a port: 1 } { host: b port: 2 } ]
```

> Contract validation failure returns a precise, located error (e.g.
> `contract: Service — field main.port greater than max 65535`), so editors / CLIs
> can point directly at the problem.

## Advanced features

### include & namespaces

> Design philosophy: **from minimal to rich, features are opt-in** — basic
> abilities are on by default; complex ones (multi-target / glob / regex /
> ext-rewrite) require an explicit `@feature enable`, avoiding YAML's over-complexity trap.

**Basic form**

```sml
# with extension ⇒ plain inline (content merges into current scope)
include "common.sml"
app: myapp

# without extension ⇒ default namespace = filename (zero-boilerplate isolation)
include "ui"          # equivalent to include "ui.sml" as ui
title: ui.title       # access via prefix

# explicit namespace (overrides default)
include "ui.sml" as ui.form.widgets
```

Rule: **with extension = inline; without extension = namespace (filename as ns)**.
Explicit `as` always wins.

**Dotted paths (nested namespaces)**

`ns` supports `a.b.c` form, equivalent to Rust module paths, expanded to nested
blocks `a { b { c { ... } } }`.

**Macros & contracts are also namespace-isolated**

A namespace isolates not just data keys, but also `@contract` / `@name` / `@base`
definitions — they must be referenced with the `ns.` prefix from outside:

```sml
# inside widgets.sml
@contract Button { label: str }
@name primary = { label: "OK" }

# caller must use the prefix
@is ui.form.widgets.Button
button: &ui.form.widgets.primary
```

**Conflict = error (never silent)**

Namespaces are exclusive scopes, never silently overwritten:

- macros/contracts register as `ns.Name` after `as ns`; duplicate definitions in same ns → error
- missing file / circular include / >32 nesting depth → error

**Multi-target & `import` alias**

```sml
include "a.sml", "b.sml" as y, "c"
import ui.buttons, admin.panel
```

**Glob & regex (feature-gated)**

```sml
@feature enable glob
include "widgets/*.sml"

@feature enable regex
include /plugins/.*\.sml/
```

**Zero-copy slicing (performance)**

`include` does not deep-copy file contents into one big string. Each included
file is read once, held as a string slice, and the parser consumes a "slice
stream": on `include "x" as a.b` it inserts a zero-copy open literal `a { b {`,
feeds x's slices, then closes `} }`. Memory = sum of all file slices.

### \u escaping

Strings support `\u{XXXX}` and `\uXXXX` Unicode escapes, converted to UTF-8 at parse time:

```sml
label: "snowflake \u{2744} snow"
```

### JSON bidirectional bridge

SML and JSON are isomorphic (both tree-shaped key/value / arrays), convertible losslessly:

```js
import { parse, stringify } from "./sml.mjs";
const obj = parse(smlText);
const json = JSON.stringify(obj);
const sml = stringify(JSON.parse(json));
```

Rust provides `sml::serde::from_str` / `to_string` via the `serde` feature — any
`#[derive(Deserialize)]` struct deserializes in one step.

### Multi-target emit

SML is not just a config format — once parsed to a `Value`, it can be **compiled /
translated into other host formats** via built-in backends, feeding the same data
into different ecosystems:

| feature | target | entry function |
|---------|--------|---------------|
| `emit-markdown` | Markdown / GFM | `to_markdown` |
| `emit-latex` | LaTeX document | `to_latex` |
| `emit-xml` | XML / LVGL UI | `to_xml` / `to_lvgl` |
| `emit-svg` | SVG graphics | `to_svg` |
| `emit-slint` | Slint DSL (Rust/Qt GUI) | `to_slint` |
| `emit-custom` | user-defined SML template generator | `to_custom` |

All are on by default; if you only need parse / serialize-back-to-SML, set
`default-features = false` to drop every `emit-*`, and this module is excluded
from compilation entirely.

Conventions: objects / blocks generally map to a host "container / element /
environment", arrays to a "list / sequence", and string scalars are **escaped by
default** to prevent injecting host reserved characters (e.g. XML `<`, `&`). Bare
block metadata `__type` / `__name` selects backend semantics rather than being
emitted as ordinary fields.

```rust
use sml::{parse, emit::to_markdown, emit::MarkdownOptions};

let v = parse("# title\nbody: content\nitems: [ a b c ]").unwrap();
// SML -> Markdown
let md = to_markdown(&v, &MarkdownOptions::new()).unwrap();
// SML -> Slint GUI description
use sml::emit::{to_slint, SlintOptions};
let slint = to_slint(&v, &SlintOptions::new()).unwrap();
```

> Emit backends cap recursion depth (`MAX_VALUE_DEPTH = 128`) against untrusted
> input: over-deep nesting returns `Err` instead of stack-overflowing the host,
> avoiding DoS.

## Real-world usage

SML is adopted by several real projects:

- **BamZap** (package manager): `bamzap.sml` manifest describes deps, sources, build scripts
- **soupmake** (Soup build system): SML describes artifacts & dependency graph
- **resender** (Resend mailer): uses SML contracts for `AppConfig` persistence

## Full example

```sml
@version v1

@contract Service loose {
    name:    str
    port:    int  default 8080 min 1 max 65535
    debug:   bool default false
    peers:   array[str] ?
}

@base {
    region: cn-north-1
    timeout: 30
}

@is Service
name: gateway
port: 9090
debug: true
peers: [ auth billing ]

service auth { &base port: 7100 name: auth-svc }
service billing { &base port: 7200 name: billing-svc }

database: {
    url: "postgres://localhost:5432/app"
    pool: { min: 2 max: 16 }
}
features: [ logging metrics tracing ]
```

## Local Playground

Don't want to install anything? Try it in the browser:
**[SML Playground →](/en/playground/)**

Write SML (with contracts) on the left, see parsed result or precise error
location on the right.
