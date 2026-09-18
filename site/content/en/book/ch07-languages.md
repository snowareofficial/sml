---
title: "Chapter 7: Multilingual Use"
translationKey: "book-ch07"
---
# Chapter 7: Multilingual Use

SML is a format that needs to be parsed by the host language to be "usable". Each language implementation is decoupled from each other and can be embedded separately (WASM/sandbox/editor are all acceptable). Below are the most commonly used integration methods.

> **Scope note first**: **only the Rust implementation (`swsml`) is the reference and is
> continuously maintained** — grammar, contracts, vulnerability scanning and regression tests
> are all defined against it. Use Rust in production.
> **C / JavaScript / Lua (Soup) / C++ / Python are experimental and not guaranteed**: they may
> diverge from Rust behaviour, have no API stability promise, and are not covered by the
> routine vulnerability scan or regression tests. Statements like "aligned with Rust" below are
> **historical results** and are for reference only.

## 7.1 Rust(`swsml`)

```rust
use sml::parse;
let v = parse("name: John\nage: 27").unwrap();
assert_eq!(v["name"], "John");
```

Include file:

```rust
use sml::parse_file;
let v = parse_file("app.sml")?;
```

Serde bridging (optional feature):

```toml
# Cargo.toml
sml-rs = { version = "0.2", features = ["serde"] }
```

```rust
use sml::{parse, Value};
let v = parse("name: John\nage: 27")?;
let json = serde_json::to_string(&v)?;   // {"name":"John","age":27}
```

>`Value` handwriting implements `Serialize`/`Deserialize`, serialized as natural `27` instead of `{"Int":27}`. When serde is not enabled, crate has zero dependencies.

## 7.2 C(`sml.c`)

```c
# include "sml.h"
char err[256] = {0};
sml_value *v = sml_parse("name: John\nage: 27", err, sizeof(err));
/* v->type == SML_STR ("John") ... free it with sml_free(v) */
```

The contract system was once 100% aligned with Rust (the `CONFIG_CONTRACT` behaved consistently
on all ends); note the C implementation is now **experimental and not guaranteed**.

## 7.3 JavaScript(`sml.mjs`)

Zero dependency ESM, browser/node compatible, including contracts and Playground:

```js
import { parse, stringify } from "./sml.mjs";
const v = parse('name: John\nage: 27');
console.log(stringify(v));
```

SML  ↔  JSON conversion (isomorphic):

```js
const obj = parse(smlText);              // Ordinary JS object
const json = JSON.stringify(obj);
const sml = stringify(JSON.parse(json));
```

## 7.4 Lua / Soup(`lib/sml.soup`)

```lua
local sml = require("lib.sml")
local v, err = sml.load(text)   -- Parse
print(sml.dump(v))              -- Serialization
```

```bash
soupx lua/sml.sar config.sml     # Parse and print
```

## 7.5 Other

-* * C++* * (`cpp/`): header file+single compilation unit, zero third-party dependencies, parsing failure throws `sml::ParseError` (including row and column positions).

-Python: See py binding outside `rust/`.

## Error codes: the same code on all five implementations

The same class of error raises **the same code** (of the form `E-CONTRACT-002`) in
**Rust / C / C++ / JS / Lua**, so there is no need to remember each implementation's
wording — remember the code: the [error code reference](/en/errors/) is the single
entry point. The single source of truth is `errors/codes.sml` in the repository
(**141 codes** today), distributed to per-implementation constants by a generator.
How you **extract** the code differs: Rust appends it (`message [E-XXX-NNN]`), C / C++ /
Lua put it at the **front** of the message, and JS exposes it as the `e.code` field.

One consequence worth knowing up front: **inputs that used to "parse" now fail with a
code.** They only parsed because the resulting tree was wrong anyway (unterminated
strings, a stray `}` inside an array, undefined fragment references, top-level scalars
all used to yield a silently wrong tree). A loud rejection beats silent bad data.

Recent per-implementation progress (several faces of the same effort):

- **Lua**: gained contracts (`@contract` / `@is` plus default fill-in) and `include`
  (sandbox root, cycle detection with a chain stack, nesting limit 32 / global expansion
  limit 10000). It used to treat `include` as an ordinary key and swallow everything up
  to the next `{` into the fragment body — no error, just a wrong tree.
- **C++**: `@include` works again — it now expands **before parsing**, with chain-stack
  cycle detection plus depth / expansion caps. Previously every field of the included
  file was lost, along with the includer's own trailing fields.
- **C**: nested arrays used to lose data and even **invent keys**, because the inner `]`
  was read as the outer terminator (`m: [ 1, [2, 3], 4 ]` yielded `{"m":[1,2,3],"4":4}`).
  It now matches Rust / JS byte for byte, and `sml_dump` output is aligned with Rust's
  `to_sml` (no trailing space after `key:` when an object body follows).

## 7.6 Which one to choose?

|You are writing | using| Guarantee|
|--------|----|----|
|Rust programs/command-line tools | `swsml`| ✅ Reference, production-ready|
|Embedded/System Layer | C/C++| ⚠️ Experimental, not guaranteed|
|Front end/Node Services | `sml.mjs`| ⚠️ Experimental, not guaranteed|
|Soup Ecology/Script | `lib/sml.soup`| ⚠️ Experimental, not guaranteed|

Chapter 8: Practical Projects (/book/ch08 project)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch07" >}}

{{< sml-quiz "ch07" >}}
