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

> Repo: [snoware/sml](https://gitee.com/snoware/sml) ｜ Bindings: Rust (`swsml`) · C (`sml.c`) · JavaScript (`sml.mjs`) · Lua (`lib/sml.soup`) · C++ · Python

## Features

- **Optional quotes**: bare words are strings; quotes only needed for spaces / special chars
- **Optional block colon**: `address { }` ≡ `address: { }`
- **Flexible array separators**: commas optional `[ a b c ]` ≡ `[ a, b, c ]`
- **Fragment inheritance**: `@base { }` defines, `&base` references — config reuse
- **include inline / namespaces**: `include "x.sml"` expands recursively; `include "x.sml" as a.b` isolates into a dotted scope (macros & contracts included), conflicts error out
- **Environment variables**: `$env.HOME` inlined at parse time
- **Contract system**: `@contract` / `@is` validate config type & structure (strict / loose modes)
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

| Lang | Repo / file | Status |
|------|------------|--------|
| Rust | `rust/` (`swsml`) | ✅ usable (full contract system, serde bridge) |
| C | `c/sml.c` | ✅ usable (contracts aligned 100% with Rust) |
| JavaScript | `js/sml.mjs` | ✅ usable (zero-dep ESM, browser / Node, with contracts & playground) |
| Lua | `lua/lib/sml.lua` | ✅ usable (same source as Soup `lib/sml.soup`) |
| C++ | `cpp/` | ✅ usable |
| Python | see `py` bindings | ✅ usable |

> The contract system is aligned across **Rust / C / JavaScript / C++**: the same
> `CONFIG_CONTRACT` definition parses identically on all four.

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
