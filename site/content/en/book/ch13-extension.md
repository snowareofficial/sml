---
title: "Chapter 13: External Extensions (custom directives and types)"
translationKey: "book-ch13"
---

# Chapter 13: External Extensions (custom directives and types)

This chapter covers an uncommon but decisive capability: when SML lacks the `@directive`,
field type, or modifier you need, you **do not have to patch SML's source** — register an extension.

> In one line: keep the core grammar small and stable; put domain differences in
> **extension points registered by downstream projects**.

## 13.1 Why it exists

A real scenario: a project wants to attach a "form description" metadata block to a document —

```sml
@contract BugReport {
    title: str
}

@form BugReport {
    title: { label: "Title", widget: text, required: true }
}
```

SML knows `@contract` but not `@form`. **So the project forks its own parser** and writes a
subset implementation. The result: the same `.sml` document parses there and fails entirely in SML —
the two sides no longer interoperate, and **a dialect is born**.

Yet this need ("attach a typed metadata block that must not enter the main data tree") is
**generic**, not project-specific. The extension point fills that gap:
**dialect definitions stay in your repository, and SML's core stays generic.**

## 13.2 Three extension points

| Point | Solves | Example |
|---|---|---|
| `Directive` | custom `@directive` | `@form` / `@policy` / `@flow` |
| `TypeCheck` | custom contract type | `image` / `link` / `time` |
| `Modifier` | custom field modifier | `items_max` (array length cap) |

All three obey the same rule: **with no extension registered, parsing behaves exactly as before**
(pinned by unit tests).

## 13.3 Custom directive (Rust)

```rust
use sml::{ext::Outcome, parse_with, ParseOptions, Value};

struct Form;

impl sml::ext::Directive for Form {
    fn name(&self) -> &str { "form" }

    // Accept the positional form `@form Name { }`? Default false (prefer `@form name: Name { }`).
    // `true` keeps old documents working, but emits a deprecation diagnostic.
    fn positional(&self) -> bool { true }

    // arg: the name; body: the `{ ... }` block (Value::Null when absent)
    fn call(&self, _arg: Option<&str>, _body: Value) -> Result<Outcome, String> {
        // Metadata block: written in the document, absent from the parse result
        Ok(Outcome::Discard)
    }
}

let opts = ParseOptions::new().directive(Form)?;
let out = parse_with(text, opts)?;
let value = out.value;         // main data tree
let warns = out.diagnostics;   // deprecation notes and other non-fatal diagnostics
```

Return `Outcome::Emit(object)` to expand a directive into fields — the object's fields are merged
into the block containing the directive.

**Both argument forms are accepted**:

```sml
@form name: BugReport { ... }   # preferred: same shape as fragment parameters, unambiguous
@form BugReport { ... }         # positional: deprecated since v4, only with positional() == true
```

**Built-in directive names cannot be taken over**: registering `contract` / `is` / `type` /
`version` / `feature` / `when` / `for` fails immediately, so a dialect can never rewrite core
semantics. Duplicate registration fails too — no silent override.

## 13.4 Custom contract type (Rust)

```rust
use sml::contract_ext::TypeCheck;
use sml::Value;

#[derive(Debug)]
struct Image;

impl TypeCheck for Image {
    fn name(&self) -> &str { "image" }
    fn check(&self, v: &Value) -> Result<(), String> {
        match v {
            Value::Str(s) if s.starts_with("ev-") => Ok(()),
            _ => Err("must be an evidence id starting with ev-".into()),
        }
    }
}

let opts = ParseOptions::new().with_type(Image)?;
```

Then use it in contracts, including arrays:

```sml
@contract Report { images: [image] }
```

Errors carry the **field path** plus your reason, e.g.
`images[0] does not match extension type image: must be an evidence id starting with ev-`.

Why bother: if domain types like `image` / `link` / `time` were added directly to SML's type system,
the specification layer would have to answer "what is an `image`?" — that is a business question,
not one a data format should answer.

## 13.5 Custom field modifier (Rust)

```rust
use sml::contract_ext::Modifier;

#[derive(Debug)]
struct ItemsMax;

impl Modifier for ItemsMax {
    fn name(&self) -> &str { "items_max" }

    // Parse time: validate the argument, and may rewrite the field spec
    fn apply(&self, _spec: &mut sml::FieldSpec, v: &Value) -> Result<(), String> {
        match v { Value::Int(n) if *n >= 0 => Ok(()), _ => Err("must be a non-negative integer".into()) }
    }

    // Check time: called only after built-in checks (type / enum / range) pass
    fn check(&self, spec: &sml::FieldSpec, v: &Value) -> Result<(), String> {
        let limit = match spec.ext_data.get("items_max") { Some(Value::Int(n)) => *n, _ => return Ok(()) };
        match v {
            Value::Array(items) if items.len() as i64 > limit =>
                Err(format!("at most {limit} items, got {}", items.len())),
            _ => Ok(()),
        }
    }
}
```

```sml
@contract Report { tags: [str] items_max 2 }
```

> ⚠️ **Do not reuse `max`**: in SML contracts `max` is a **numeric upper bound**, and arrays do not
> participate in min/max checks. Using `max` as a length cap makes the same syntax mean two
> different things. Give your extension **a new name**.

## 13.6 JavaScript

The same three capabilities, passed as options:

```js
import { parse } from "./sml.mjs";

const warnings = [];
const value = parse(text, {
  directives: {
    // discard = metadata block; emit = expand (must be an object)
    form: { positional: true, call: (arg, body) => ({ discard: true }) },
  },
  types: {
    // return true (pass) / false (fail) / a string (the reason)
    image: (v) => (typeof v === "string" && v.startsWith("ev-"))
      ? true
      : "must be an evidence id starting with ev-",
  },
  warnings,   // pass an array; deprecation notes are pushed into it
});
```

## 13.7 Boundaries and current status (important)

- **Zero extensions = zero impact**: passing no `directives` / `types` yields byte-for-byte the
  same behaviour as before.
- **Not available over C-ABI**: extensions are trait objects and cannot cross the C boundary.
  C / C++ callers can only use the extension-free path; note on dialect documents that they
  "require Rust / JS with the extension registered".
- **Lua**: neither contracts nor extensions are implemented yet.
- **Document portability drops**: a `.sml` file containing `@form` cannot be read where that
  directive is not registered (Rust reports an explicit error). This is an **inherent trade-off** —
  the more convenient dialects get, the weaker interoperability becomes.

## 13.8 When not to use it

- You just want to express data → plain blocks and arrays suffice; do not invent directives.
- You want **everyone** to read your document → stay on core syntax.
- Your "type" is actually a generic concept (time, URL, email) → prefer `@type` custom patterns,
  which need no registration on the reading side.
