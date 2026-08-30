---
title: "Chapter 7: Multilingual Use"
translationKey: "book-ch07"
---
# Chapter 7: Multilingual Use

SML is a format that needs to be parsed by the host language to be "usable". Each language implementation is decoupled from each other and can be embedded separately (WASM/sandbox/editor are all acceptable). Below are the four most commonly used integration methods.

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

The contract system has been 100% aligned with Rust, and the behavior of the four ends of `CONFIG_CONTRACT` is consistent.

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

## 7.6 Which one to choose?

|You are writing | using|
|--------|----|
|Rust programs/command-line tools | `swsml`|
|Embedded/System Layer | C/C++|
|Front end/Node Services | `sml.mjs`|
|Soup Ecology/Script | `lib/sml.soup`|

Chapter 8: Practical Projects (/book/ch08 project)

## Hands on practice

After reading this chapter, directly modify SML in the editor below and click "Run" to immediately see the parsing results or validation errors - having output is necessary for efficient learning.

{{< sml-playground "ch07" >}}

{{< sml-quiz "ch07" >}}
