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
> the code cite the same facts — there is no "documented here but missing in the code". There are
> **141 codes** today.

The table answers "which codes exist"; whether an implementation **actually raises** a given code is
a separate matter — see the "code landing status" section of
[`errors/README.md`](https://gitee.com/snoware/sml/blob/master/errors/README.md) in the repository.
Today **all five implementations (Rust / JS / C / C++ / Lua) plus the `smltools` CLI carry codes
in full**, each with a "condition → expected code" suite, so **the same condition raises the same
code on every implementation**.

A visible consequence: inputs that used to pass silently now **fail with a code** instead of quietly
yielding a wrong tree — unterminated string `E-LEX-001`, unterminated block comment `E-LEX-002` /
`E-LEX-003`, unknown escape `E-LEX-004`, invalid `\u` `E-LEX-005`, stray `}` inside an array
`E-PARSE-003`, mismatched closing bracket `E-PARSE-002`, unregistered directive `E-PARSE-005`,
undefined fragment reference `E-INCLUDE-006`, top-level scalar `E-PARSE-008`. Newly added codes
include `E-INCLUDE-012` (malformed include path) and `E-CLI-008` (smltools CLI usage error).
(⚠️ 2026-09-19: `E-INCLUDE-012` became a **language-level rule** (the check lives in
`sml-include`), so **both Rust — including `smltools` — and Lua raise it**; the table below follows
the code registry.)

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

- The search box supports **wildcards**: `E-CONTRACT-*` or `E-*-008` (matched against the code
  itself only); plain keywords ("contract", "nesting") still search every field
- Every card is **deep-linkable**: opening `/errors/#E-PARSE-008` fills the box, highlights that
  card and scrolls it into view (wildcard hashes such as `#E-*-008` work too); clicking the code
  itself gives you a shareable link
- The textbook [`/search`](/en/search/) understands codes too: when a query looks like a code it
  also lists **code hits**, linking back to `/errors/#<code>`
- When branching in scripts, matching the first two segments is usually better than the full code
  (all contract errors are `E-CONTRACT-*`)
- Report bugs with the code plus a minimal reproduction — far more useful than pasting the wording
- Need a new code? Edit `errors/codes.sml`, append a number, translate the message freely, never
  reuse or repurpose a code. Then re-run both generators: `python errors/gen_json.py` (this page)
  and `python errors/gen_codes.py` (per-implementation constants; it also verifies that hand-written
  code literals in the sources exist in the registry)
