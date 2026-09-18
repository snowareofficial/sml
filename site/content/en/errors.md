---
title: "Error code reference"
translationKey: "errors"
---

# Error code reference

Every class of SML error has a **stable, machine-readable code** such as `E-CONTRACT-002`:

- segment 1 is the **level**: `E` error / `W` warning / `I` info
- segment 2 is the **domain** — 14 of them, grouped by layer: language (lexer, parser, contract,
  include, limits, features, extensions), host binding (IO, internal, derive bridge), tooling
  (migration formats, CLI, lint), editor (editor host)
- segment 3 is a **sequence number**, three digits, append-only (retired codes are never reused)

**The code is the stable contract; the wording is not.** Implementations may phrase the message
differently (even in a different language) — the same code means the same condition. Match on
codes, document on codes, report bugs with codes.

> The authoritative source is [`errors/codes.sml`](https://gitee.com/snoware/sml/blob/master/errors/codes.sml)
> in the repository; this table is generated from it by `python errors/gen_json.py`, so the page and
> the code cite the same facts — there is no "documented here but missing in the code".

The table answers "which codes exist"; whether an implementation **actually raises** a given code is
a separate matter — see the "code landing status" section of
[`errors/README.md`](https://gitee.com/snoware/sml/blob/master/errors/README.md) in the repository.
Today **Rust, JS, the C-ABI and the native C implementation carry codes**; the native C++ implementation
and Lua are still catching up.

<div id="errors-app">
  <p>Loading the code table… (if this never fills in, <code>errors.json</code> was not generated:
  run <code>python errors/gen_json.py</code> first)</p>
</div>

## How to use it

```text
E-CONTRACT-002   field type mismatch
E-PARSE-008      top-level scalar cannot round-trip
E-LIMIT-001      nesting too deep
```

- When branching in scripts, matching the first two segments is usually better than the full code
  (all contract errors are `E-CONTRACT-*`)
- Report bugs with the code plus a minimal reproduction — far more useful than pasting the wording
- Need a new code? Edit `errors/codes.sml`, append a number, translate the message freely, never
  reuse or repurpose a code. Then re-run both generators: `python errors/gen_json.py` (this page)
  and `python errors/gen_codes.py` (per-implementation constants; it also verifies that hand-written
  code literals in the sources exist in the registry)
