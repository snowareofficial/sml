# Changelog (English)

> **This file starts at 0.6.3.** Entries for **0.6.2 and earlier** live in the Chinese
> [`CHANGELOG.md`](CHANGELOG.md), which was the only source of truth up to that point.
> From **0.6.3** onward the two files are written together: the same sections, in the
> same order, carrying the same evidence. If they ever disagree, the Chinese file is
> the one to trust (it is the one the release notes are cut from) — and the English
> file should be corrected to match.

Conventions carried over from `CHANGELOG.md`:

- Version numbers follow Cargo's reading of semver: in the **0.x** stage a **MINOR**
  bump counts as **incompatible** (0.6 → 0.7 would stop downstream from upgrading
  automatically), while PATCH is for compatible additions. So "a new backend / a new
  API" goes to PATCH (0.6.0 → 0.6.1 → 0.6.2 → 0.6.3); only genuinely breaking changes
  move MINOR.
- `swsml` (the library) and `smltools` (the CLI) are versioned independently; a release
  names both in its section header.
- Sections are prefixed with what they are — `Added` / `Changed` / `Fixed` / `Docs` —
  and every behavioural change says so explicitly instead of hiding in a feature note.

---

## [0.6.3] · swsml (shipped together with `smltools`) — 2026-10-15

> 📌 This round (TASK-agent-w24): **contracts move from "one contract per block" to
> "several contracts per block"**, programs can finally ask *"which contracts apply to
> this entity?"*, and both contract inference and error positions land. Rust goes first;
> the other implementations hold the same line by **failing loudly** (see below).

### Added · Contract mixin: one block, several contracts

Three spellings, **exactly equivalent** (pinned case by case in
`rust/tests/contract_mixin.rs`, 11 tests):

```sml
api {
    @is HasHost, HasPort        # 1: comma separated
    host: example.com
}
api { @is HasHost HasPort … }   # 2: commas omitted (same as above)
HasHost HasPort api { … }       # 3: block-level (needs `@feature enable typed-block`)
```

- **Merge semantics (D3)**: `strict` wins (if any participating contract is strict, the
  whole block is validated as strict); **order-independent** (fields are unioned by
  name); **a conflict is an error** — the same field name with a different `ty` /
  `default` / `min` / `max` in two contracts raises
  `E-CONTRACT-016 contract mixin conflict`, naming the field and both contracts.
- **The single-contract path is byte-for-byte unchanged**: when only one contract name
  is collected, the old `apply_contract` runs (same error wording included). The P1-3
  red line `server web prod { }` produces exactly what it produced before.
- ⚠️ **Implementation status**: Rust only for now. JS / C / C++ / Lua — see
  "failing loudly" below.

### Changed · Several `@is` in one block: from "last one wins" to "all of them apply"

**Behavioural change** (D4). Previously a second `@is` in the same block silently
overrode the first; it is now synonymous with `@is A B` — the fields of both contracts
are unioned and validated together. Default fill-in, type checks and recursive
composition all run the old code path; only the input changed from one name to a list.

### Added · Contract map API: a program can ask "which contracts apply to this entity?"

- Enabled with `ParseOptions::trace_contracts(true)`. **When it is off, `ParseOutput`
  is byte-for-byte what it was** (`contract_map: None`) — a test pins that guarantee.
- `ContractMap::names_at(&[PathSeg]) -> &[String]`, with
  `PathSeg::Key(String) | Index(usize)` (an `Index` segment is an array position, a
  `Key` segment is a key; SML's own type inference tells them apart, so no new DSL).
- **The interchange form is SML itself** (`ContractMap::to_sml()`):
  `{ path: [ database primary ] contracts: [ Server ] }`; a top-level entity has
  `path: []`; entities without a contract do not appear; output is sorted by path and
  carries **no timestamp** (same input ⇒ byte-identical output).
- Blocks pulled in by `include` are recorded at **their position in the result tree**;
  fields spliced in by a `&base` fragment are not recorded separately.
- ⚠️ The public struct `ParseOutput` **gained a field** (`contract_map`) — see
  "Upgrading" at the end of this section.

### Added · Contract inference: `smltools --infer-contract`

- New `rust/sml-contract/src/infer.rs` (`infer_from_value`), re-exported from the main
  crate as `sml::infer`; `smltools` gained `--infer-contract` (SML `@contract` output by
  default, `--json` for a JSON field table).
- Type mapping (TASK-agent-w24 §7): `Int→int` / `Float→num` / `Bool→bool` / `Str→str` /
  `Null→any` / `Object→sub-contract` (UpperCamelCase from the field name, e.g.
  `address` → `Address`, same-named fields reuse one sub-contract) / `Array→[T]`
  (all-int ⇒ `[int]`, uniform objects ⇒ `[Sub]`, mixed ⇒ `[any]` with a warning).
- `optional` is inferred **only** when merging several instances (a field missing from
  some of them); a single instance is all-`required`, with a note saying so. It
  **never guesses** `default` / `min` / `max` / `enum` / custom `@type`.
- Generated contracts default to `loose`, so adding a field later is not rejected by
  strict mode.
- Deterministic: fields sorted by name, no timestamp, byte-identical for the same
  input; 9 golden unit tests inside `infer.rs` assert the exact output.

### Added · Errors carry a position: `[line N, column M]`

- `Tok` is **untouched** (smallest possible blast radius): the lexer gained a
  `tokenize_spans` side channel (byte offsets), `SmlError` gained
  `pos: Option<usize>`, and `sml-parse` converts the offset into a line/column pair
  that is **appended** to the message at the API boundary. `Display` stays plain text,
  so every old construction point that passes no position keeps its exact wording.
- Known edges: **file entry points (`parse_file` etc.) carry no position** — an
  `include` merges token streams from several files, so there is no single source text
  to map to (better missing than wrong); `E-LEX-*` lexer errors **have no position yet**
  (this round only wired up the parser layer).
- One old behaviour fixed along the way: `scan.rs`'s `strip_version` / `strip_features`
  used to **delete lines**, which is shorter than the original and therefore shifted
  every subsequent line/column. They now do an **equal-length replacement** (directive
  lines become the same number of spaces; CRLF and BOM are replaced with equal-width
  spaces), so offsets map 1:1 onto the source — the same fix JS already had in
  `stripFeatureLines`.

### Added · Other implementations fail loudly on multi-contract blocks: `E-CONTRACT-017`

- JS / C / C++ / Lua: a second bare word after `@is`, or a second registered contract
  in block-level args, raises **`E-CONTRACT-017 contract mixin not implemented`** —
  **never a silent tree**. The single-contract path is byte-for-byte unchanged on all
  four.
- **Why this had to be done**: a mixin **cannot be opt-in** (some of those
  implementations do not even have a `@feature` directive), so shipping Rust alone
  would replay the `typed-block` trap where the other ends silently degrade — a lesson
  already written up in the textbook, §5.2.2.
- JS also lost a **misleading error**: `@is tls, retry` used to produce a **wrong tree
  plus `E-CONTRACT-003`** (which says nothing about contract names); it now correctly
  reports `017`.

### Added · JS contract map API + editor reads the map (hover / go-to-definition)

- `js/sml.mjs` gained `opts.traceContracts` and `contractMap`, aligned with Rust:
  opt-in, `{ key }` / `{ index }` segments, sorted by path, no timestamp, and
  `contractMapToSml` is **byte-identical** to Rust's `to_sml`; **when off, the return
  value is byte-for-byte the old one**. JS still rejects multi-contract blocks loudly
  (untouched).
- The VS Code extension now reads the map: hover lists **all** contract names with
  their declaration, go-to-definition supports several targets, and the previous
  regex-based logic is kept as a fallback.

### Fixed (five, ordered by blast radius)

1. **Quantifier ceiling + program-size gate for patterns** (`sml-pattern`): a single
   quantifier is capped at `QUANT_MAX = 1000`, the compiled program at
   `MAX_PROG = 100_000` instructions, and the run at a `10_000_000` step budget;
   JS is aligned (compile-time estimate, `PATTERN_STEP_BUDGET = 100000`). A rule like
   `次: 999999999` is now **rejected at compile time** instead of being expanded in
   memory (OOM / hang, i.e. DoS).
2. **`required` is OR-ed when merging contracts**: it used to be left out of the merged
   view, which made the result depend on the order the contracts were written in —
   violating D3's "order-independent". It is now a logical **or**: if any contract
   requires a field, it is required.
3. **`smltools` main moved onto a 64 MB stack thread**: the parser promises 128 levels,
   but the Windows main-thread stack is 1 MB by default — it overflowed at **70 levels**
   (`lint_deep_nesting_is_wlint_005`, measured; the previous code merely got lucky).
4. **Include base directory falls back to the current directory when empty**:
   `-i bare-filename.sml` (`parent()` is empty) no longer reports `E-INCLUDE-010`
   (three base computations in `rust/sml-parse/src/api.rs` now `.filter` it).
5. **Not fixed, recorded only**: the library entry points `parse` / `parse_with` can
   still overflow on a 1 MB-stack thread — call the library from a thread with a large
   stack when parsing deeply nested documents.

### Docs / tooling

- README gained a "30 seconds" section in both languages: the same config in JSON /
  YAML / SML, side by side (**pure addition**, no existing paragraph touched; all three
  samples were actually parsed while writing them).
- Textbook: `ch05` gained §5.4.1 "Mixin", cross-referenced against §5.4 "Composition" to
  explain **why one cannot replace the other**; `ch10` documents the block-level
  multi-contract spelling and marks roadmap items; new `ch14 Capability map` (zh + en).
- `rust/fuzz/`: a `cargo-fuzz` scaffold with a `parse` target (coverage-guided fuzzing;
  the ASan runtime is still pending on this machine).
- **VS Code extension 0.4.3**: `@type` gained **go-to-definition and hover** (highlighting and
  completion were already there). Pressing F12 on a contract field's type position
  (`编号: 规则编号`) now jumps to `@type name: 规则编号`; hover shows a **signature**
  (e.g. `DS- <层: NET|IDC|APP|DAT|EGR> - <序号: 数字×4>`) plus the raw pattern, and works both
  on the type name in `@type` and on the contract field that uses it. When a signature cannot be
  rendered (nested `组` / `序列`), the hover **falls back to the raw source** rather than showing
  a wrong one. Assertions added in `scripts/_verify_ext.mjs`.

### Upgrading (this round)

- **Behavioural change**: several `@is` in one block now **all apply** instead of the
  last one winning (see above).
- **Public struct gained a field**: `ParseOutput` has a new
  `contract_map: Option<ContractMap>`. Code that only **reads** the result is
  unaffected; **only code that builds a `ParseOutput` with a struct literal** is
  affected (same rule as 0.6.2: adding a field is an API change, so it is called out
  here).
- Everything else (mixin, contract map, inference, error positions) is **pure
  addition** — existing paths are byte-for-byte unchanged.
