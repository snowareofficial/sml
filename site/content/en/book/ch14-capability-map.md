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
| Fragment (`@base {}` + `&base`) | on | ✅ expanded | ✅ | ⚠️ not expanded | ❌ | ❌ | [Ch 3](/en/book/ch03-fragments) |
| `namespace` (`include … as ns`) | on | ✅ | ✅ | ✅ | ✅ | ❌ | [Ch 4](/en/book/ch04-include) |
| `implicit-ns` | on | ✅ | ✅ | ? | ? | ? | [Ch 4](/en/book/ch04-include) |
| `top-level-array` (array as document root) | on | ✅ | ❌ | ✅ | ❌ | ❌ | [Ch 10](/en/book/ch10-features) |
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
3. **Silent or degrading — be careful across ends**: C does not expand `fragment`; C / C++ / Lua degrade `typed-block` to a plain bare block; Lua's `for` is pending re-check — these ends either report a code explicitly or leave a tree different from what you expect.

→ Back to the [textbook index](/en/book/)
