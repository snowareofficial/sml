---
title: "Chapter 14: Capability Map"
translationKey: "book-ch14"
---

# Chapter 14: Capability Map

**Why this chapter**: advanced capabilities like `typed-block` / `@when` / `@for` only appear in Chapter 10, with no entry point anywhere in Chapters 1–9 — readers easily conclude "SML is just what the first nine chapters show". This chapter lays out **every capability** in one table: **opt-in or not**, **five-end support**, and **which chapter explains it**. Check here first before writing a cross-end document.

> Data source: Chapter 10's "implementation coverage table" (measured 2026-09-19) + contract mixin (2026-10-12, tasks C/D/F). The criterion is **observed behavior**, not source-reading; `?` = not measured (does not mean absent).

| Capability | Default | Rust | JS | C | C++ | Lua | Chapter |
|---|---|---|---|---|---|---|---|
| Key-value / blocks / arrays / scalars | on | ✅ | ✅ | ✅ | ✅ | ✅ | [Ch 1–2](/en/book/ch01-basics) |
| Comments (`#` `--` `//` `/* */` `_* *_`) | on | ✅ | ✅ | ✅ | ✅ | ✅ | [Ch 1](/en/book/ch01-basics) |
| Bareword-as-string (`bareword-string`) | on | ✅ | ✅ | ✅ | ✅ | ✅ | [Ch 1](/en/book/ch01-basics) |
| `include` (single file) | on | ✅ | ✅ ※ | ✅ | ✅ | ✅ | [Ch 4](/en/book/ch04-include) |
| `env` (`$env.X`) | on | ✅ | ✅ | ✅ | ✅ | ✅ | [Ch 6](/en/book/ch06-env-escape) |
| Contract (`@contract` / `@is`, single) | on | ✅ | ✅ | ✅ | ✅ | ✅ | [Ch 5](/en/book/ch05-contract) |
| Fragment (`@base {}` + `&base`) | on | ✅ expanded | ✅ | ✅ expanded | ⚠️ not expanded | ❌ | [Ch 3](/en/book/ch03-fragments) |
| `namespace` (`include … as ns`) | on | ✅ | ✅ | ✅ | ✅ | ❌ | [Ch 4](/en/book/ch04-include) |
| `implicit-ns` | on | ✅ | ✅ | ? | ? | ? | [Ch 4](/en/book/ch04-include) |
| `top-level-array` (array as document root) | on | ✅ | ✅ | ✅ | ✅ | ✅ | [Ch 10](/en/book/ch10-features) |
| **Contract mixin (multiple contracts per block)** | on※ | ✅ | ❌ `017` | ❌ `017` | ❌ `017` | ❌ `017` | [§5.4.1](/en/book/ch05-contract) |
| `typed-block` (`Contract block { }`) | **off** | ✅ | ✅ | ❌ silent degrade | ❌ | ❌ | [§5.2.2](/en/book/ch05-contract) |
| `@type` patterns (Loom-in-SML) | on | ✅ | ✅ | ❌ explicit reject | ❌ explicit reject | ❌ explicit reject | [§5.2.2](/en/book/ch05-contract) |
| `multi-include` (`include "a", "b"`) | **off** | ✅ | ✅ | ✅ | ✅ | ❌ | [Ch 10](/en/book/ch10-features) |
| `glob-include` (`include "inc/*.sml"`) | **off** | ✅ | ✅ | ❌ | ? | ❌ | [Ch 10](/en/book/ch10-features) |
| `regex-include` (`include "re:…"`) | **off** | ✅ | ✅ (restricted subset) | ? | ❌ | ❌ | [Ch 10](/en/book/ch10-features) |
| `ext-rewrite` | **off** | ✅ | ? | ? | ? | ? | [Ch 10](/en/book/ch10-features) |
| `@when` | **off** | ✅ | ❌ | ❌ | ❌ | ❌ | [Ch 10](/en/book/ch10-features) |
| `@for` | **off** | ✅ | ❌ | ❌ | ❌ | ⚠️ to re-check | [Ch 10](/en/book/ch10-features) |
| The `@feature` gating mechanism itself | — | ✅ | ✅ | ❌ | ❌ | ❌ | [Ch 10](/en/book/ch10-features) |

※ **JS resolves `include` from a host-supplied file map** (`parse(text, { files })`); JS itself never touches the filesystem (designed for editor / server embedding).
※ **Contract mixin** needs no extra `@feature` (on with `contract`), but is currently **implemented only in Rust**; the other four ends **explicitly report `E-CONTRACT-017`** (never silent).

**Three quick takeaways**:

1. **Stable on all five ends** (prefer for cross-end documents): key-value / blocks / arrays, comments, bareword strings, `include`, `$env`, single-contract `@contract`/`@is`, `namespace`.
2. **Advanced, Rust (±JS) only**: `@when` / `@for` are Rust-only; `typed-block` / `@type` / `glob`·`regex-include` are Rust + JS.
3. **Silent or degrading — be careful across ends**: line-level `&base` expansion is **implemented everywhere except C++** (C now matches Rust); C / C++ / Lua degrade `typed-block` to a plain bare block; Lua's `for` is pending re-check — these ends either report a code explicitly or leave a tree different from what you expect.

## Cross-implementation consistency suite (five-end consistency)

SML ships a **shared test cases × five ends** consistency suite: Rust / C / C++ / JS / Lua each run the
same cases, and what is compared is the **value tree**, not bytes. How "all five pass" is defined, how
the divergence ledger is graded, and which "AGREE" results are actually carrier blind spots — the
authoritative source is `tests/conformance/known_divergences.md` (**the current ledger**; its count and
each end's AGREE/DIVERGE tally change as merges land, so this page does not keep a copy).

Key points:

- **It compares value trees, not bytes**: each end's dumped text is fed back to `smltools` for
  normalization into a JSON value tree before comparison — so purely cosmetic differences like
  "C keeps source order, Rust sorts" never cause a false divergence.
- **The divergence ledger has two tiers**: **defect** (implementation disagrees with settled behavior,
  should be fixed) and **known trade-off** (a settled decision, not to be changed). See the current
  ledger for the exact count.
- **"Both ends fail" is not consistency**: scoring first checks "is there a value tree" (a parse
  failure or a dump that won't round-trip has no tree), then compares groups; having no tree on both
  sides is just "wrong together", not "aligned".
- **Carrier blind spots (B-01 / B-02)**: the suite reconstructs the value tree through a text carrier,
  which cannot express two things — `B-01`: a "number-looking string" reads back as a number (an AGREE
  on some ends that is actually not aligned); `B-02`: the `\0` cell cannot be carried. An AGREE on
  these cells **must not be read as "aligned"**.

→ Back to the [textbook index](/en/book/)
