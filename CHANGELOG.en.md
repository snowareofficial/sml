# Changelog (English)

> **This file starts at 0.6.3.** Entries for **0.6.2 and earlier** live in the Chinese
> [`CHANGELOG.md`](CHANGELOG.md), which was the only source of truth up to that point.
> From **0.6.3** onward the two files are written together: the same sections, in the
> same order, carrying the same evidence (the English side may **condense** long notes to
> one line per change). If they ever disagree, the Chinese file is the one to trust (it is
> the one the release notes are cut from) — and the English file should be corrected to match.

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

## [Unreleased]

> Condensed mirror of the Chinese `[未发布]` section: same categories, one line per change.
> The Chinese file remains authoritative for detail and evidence.

### 🚨 Breaking — version naming: `Version` enum → `Edition` lookup

An edition is just a **name for a set of default feature bits** (`docs/version-naming.md` v1.0).
Editions are **not ordered**, so `Ord` is gone and the criterion is always "is this feature in that set".

- `enum Version { V1..V4 }` is **deleted**; new `struct Edition { major, minor }` plus
  `pub use Edition as Version` (same type under the old name). `Version::V1..V4` survive as
  **associated constants**, so existing literals still compile — but comparisons (`v3 < v4`) no longer do.
- `FeatureSet::for_version(Version)` → **`FeatureSet::for_edition(Edition)`**, answering from the
  per-row `features` field of the `EDITIONS` registry instead of computing from a version number.
- `Edition::name()` / `Display` echo the canonical `1.0`..`4.0` (was `v1`..`v4`), so error text changes.
  `Edition::from_word` is gone — use `Edition::parse` (accepts `10` / `1.0` / `v1` / `50` / `5.0` / `v5`).
- `parse_versioned` / `parse_file_versioned` return `(Value, Edition)`; `parse_allowed(text, &[Edition])`
  (`&[Version::V1]` still compiles — same type).
- `smltools --feature` accepts the same literals but **only published editions**; the invalid-value
  hint is now `(1.0..4.0, i.e. v1..v4)`.
- The C-ABI `sml_loads` `opts.allow` version strings also go through `Edition::parse` (legacy forms still accepted).

### Added — error code `E-FEATURE-010` (edition removed)

- A **removed development codename** now has its own code, kept distinct from
  `E-FEATURE-004` ("invalid version declaration") so users can tell "I typed it wrong" from
  "I should migrate". The message always carries the codename, a migration rule id (`v50-version`)
  and `smltools --migrate scan`.
- The decision lives in a **single entry point** `scan::resolve_edition` shared by `@version` and
  `@feature base` (which identifiers are accepted, whether `dev` is required, which code `Removed` uses).
  Side effect: `@feature base 50` now demands the `dev` marker, exactly like `@version 50`.
- ⚠️ The registry has no `Removed` row yet, so the branch is **not reachable** today (code and wiring
  are in place; adding a row activates it). `smltools --migrate` (V11) is not implemented yet.

### Added — featag S3: `__featags` as an ordered array

- With `@feature enable type.featag` the block head's `!` region now **becomes data**:
  `wea !critical obey[1, 2], name { a: 1 }` →
  `{ wea: { __featags: [ { name: "critical" }, { name: "obey", args: [1, 2] } ], __name: "name", __type: "wea", a: 1 } }`
  - an **ordered array**, not a map: duplicates are allowed (`!a !a !b` ⇒ three items in order);
  - a no-arg item carries only `name`; an explicit `[]` keeps an empty `args`
    (`obey[]` and "no argument slot" are different things);
  - arguments are real values (reusing the existing `parse_array` path) — no second, hand-written
    token→Value mini-parser.
- **`__args` is not reused**: that key means "barewords after the block name" (spec §3).
- `__featags` is written **after** contract application, like `__type` / `__name` — it is syntax
  metadata and must not be rejected by a strict contract as an undeclared field
  (pinned by `featags_survive_a_strict_contract`).
- ⚠️ **No declaration checking yet** (undeclared featags and wrong argument types pass) — that is S4/S5.
- ⚠️ **Hand-written `__*` keys are still accepted**: rejecting them is a separate v5 item (§4.5 #3).

### Added — featag S2: block-head `!` region recognition

- With `@feature enable type.featag`, the `!` region after the block head's first word is recognized:
  `wea !critical obey[1, 2], name { }`, `wea !critical, { }` (name optional); no-arg items are
  `!name`, items with args are `name[ … ]`; **`,` terminates** the region (no commas between items).
- **Three ambiguities, three codes** (`docs/featag-spec-draft.md` §2.3):
  - `E-PARSE-026` **empty featag region** (`wea ! , name { }`);
  - `E-PARSE-027` **`!` bareword in a value position** (`a: !b`) — **`bareword-string` does not exempt it**;
    quote it if you meant a string;
  - `E-PARSE-028` **`!`-prefixed name in a block head** (block-name position after `,`, or the type word).
- **S2 recognizes only, it emits no data**: `__featags` is S3, so the tree after that round gained
  **no key** (a test pins this so S2/S3 cannot silently merge). **S3 now emits the data — see the
  section above.**
- Look-ahead updated: with featag on, `bare_block_ahead` steps over `!name` / `name[…]` and the
  terminating comma — otherwise `wea !critical, name { }` reads as "key `wea` + value `!critical`"
  and misfires on B2 (first attempt did exactly that).
- Gated entirely behind `type.featag`; **without it v1–v4 is unchanged** (`a: !b` stays a bareword,
  `server web prod { }` keeps its `__type`/`__name`/`__args`).
- Cross-end: the three codes list `impls: [ rust ]` — C/C++/Lua have no feature set, JS has no featag,
  and they keep the old rules on the same input.

### Added — feature namespaces + the `type.featag` placeholder bit

- `@feature enable <domain>.<name>` now works (`docs/feature-namespace-plan.md` N4):
  `Feature::parse_name` validates the shape before the lookup — **split at the first dot only**,
  three segments (`a.b.c`) are rejected outright; domains are a **closed whitelist**
  `FEATURE_DOMAINS = ["lex","mod","type","flow"]` (N1), and an unknown domain is reported with it.
- **Old names stay flat and get no aliases** (N2): `typed-block` still works, `type.typed-block`
  does **not** (reported as "not registered") — two names for one feature would show up as two
  spellings in diagnostics, `disable`, and the C-ABI echo.
- All three failures share `E-FEATURE-003` and are told apart **by the detail text**
  (three segments / unknown domain / not registered + the available list).
- The rule now lives in **one place**: `@feature enable/disable/whitelist/blacklist`, the contract
  `@requires`, and the C-ABI `opts.features` all go through it (was five copies).
- **`type.featag` is reserved** (bit 18, opt-in, not in `baseline()`, not in any `Edition`'s feature
  set): the bit and name are claimed now so S2 has a stable switch to attach to (bit order only grows).
  ⚠️ In that placeholder round, enabling it **changed nothing**; **S2 now recognizes the block-head
  grammar** (next section), but still **emits no data** (`__featags` is S3) — "switch first, grammar next".
- ⚠️ Cross-end: `type.featag` is **Rust-only**; JS / Lua still report `E-FEATURE-003`
  (the code's note says so).

### Added — value model: shared path syntax (featag S1a)

- New module `sml_value::path`: `Path::parse` / `Seg` / `PathError` plus **`Value::get_path`**.
  One grammar only — key / `.` / `[n]` (0-based) — so `a.b[0].c`, `[0][1]` and `a.[0]` (empty key =
  "current value") all parse.
- **Failures say why**: `NotFound` / `OutOfRange` / `NotAnObject` / `NotAnArray`, each carrying the
  1-based segment number; syntax errors (empty path, empty segment, bad/unclosed index, illegal
  character) are rejected at `parse` time, and `PathError::is_syntax()` tells the two phases apart.
  Out-of-range messages include the real length (`that array has 2 elements`).
- **`Value::get` is untouched**: still `Option`, still "literal key first" (documents whose key
  literally contains `[` keep working; regression pin `bracket_in_key_name_is_not_rewritten`).
  The two are deliberately **not** merged — turning `get` into `Result` would silently change every
  caller, and the compat rule only lives on `get`.
- Facade: `sml::{Path, PathError, Seg}`. Passes under both the default and `preserve_order` profiles.
- The same `Path` is the **write** entry for S6 (featag targets): `segments()` / `last()` /
  `Path::new()` let code assemble targets without going back through a string.
- ⚠️ **No error code yet**: `PathError` is type + message only. S6 maps target errors onto
  `E-CONTRACT-0xx` per `featag-spec-draft.md`, which is when attaching a code becomes decidable.

### Added — parse entry: file + options

- **`parse_file_with(path, ParseOptions)`** (exported from both `sml_parse` and the `sml` facade):
  the first file entry that both **expands includes** and **accepts caller options**. Previously the
  `parse_file*` family hard-coded `ParseOptions::new()` (contract opt-out unreachable from a file),
  while `parse_with` accepts options but only runs `tokenize_spans` and **does not expand includes** —
  so "read a file (with includes) and disable in-document contracts" was **impossible**.
  Returns `ParseOutput` (`value` / `diagnostics` / `contract_map` / `errors`), same shape as
  `parse_with`; with `ParseOptions::new()` the `value` and the error code match `parse_file`
  exactly. Existing entry points are unchanged (pure addition).

### Fixed — `--no-default-features` failed to compile

- `sml::MAX_VALUE_DEPTH` used to be re-exported from `crate::emit`, but `pub mod emit` is gated by
  the seven `emit-*` features ⇒ `cargo check -p swsml --no-default-features` died with
  `error[E0432]: unresolved import crate::emit`, contradicting the documented promise that
  `default-features = false` returns to a fully dependency-free build. It now comes from
  `sml_value` (compiled unconditionally), and `emit::MAX_VALUE_DEPTH` itself is defined as
  `sml_value::MAX_VALUE_DEPTH` so the two can no longer drift. Behaviour and value are unchanged
  (still 128); what changed is that the zero-feature build finally compiles.

### 🚨 Breaking

- `Value::Float` gains a raw-spelling field (`Float(f64, Option<String>)`): pattern matches and
  struct literals need updating. `parse → to_sml` now preserves `1e10` exactly as written.
- Contract symbolic modifiers `?` / `=` are deprecated (use `optional` / `default`); they now raise
  `E-PARSE-006` on every implementation.
- `sml::serde::to_string_direct` **rejects integers beyond `i64`** (`[E-DERIVE-002]`) instead of
  writing bare words that read back as `Str`.

### Added

- Contract strictness three-state: `strict` (default) / `warn` (allow + warn) / `loose` (silent),
  with block-level override (`@is X warn|loose`) and "strictest wins" when mixing contracts.
- External modifiers `one_of` / `exactly_one` / `any_of` / `items_min` / `items_max` / `depends_on`
  (Rust extension point; register via `ParseOptions::with_modifier`).
- `x_only "<prefix>"` (`sml_contract::XOnly`): under `loose` / `warn`, only extra keys with that
  prefix are allowed; mutually exclusive with `strict`; unregistered means disabled.
- `W-PARSE-001` (duplicate key promoted to array) / `W-PARSE-002` (fragment expands over a parent
  key), exposed via `Diagnostic::code()`.
- `ParseOptions::collect_errors(true)` + `*_with_warnings`: several errors per document, including
  `finish_block` and the `@for` sub-parser.
- File entry points **without** `include` now carry line / column in errors.

### Fixed

- Iterative (explicit-stack) parser is on trunk: default-stack threads parse 128 levels and reject
  129+ with `E-LIMIT-001` instead of aborting.
- `svg` / `slint`: nested arrays no longer recurse without increasing depth (was `0xC00000FD`);
  every emit backend now returns `E-LIMIT-004` on over-deep input.
- Lua: integers beyond f64 precision stay strings (no more `3.3010620150307e+17`).
- `include "a.sml" junk` now errors (`E-INCLUDE-012`) instead of silently dropping the tail.
- `smltools`: no hardcoded Chinese error text remains in Rust; messages come from the code table.

### Docs

- Top-level `README.md` / `README.en.md` / `llms.txt` (+ the `site/static/` copy) and book chapters
  5 & 13 synced with the contract modifiers; chapter 13 no longer cites `ext.rs` line numbers.

## [0.6.4] · swsml — 2026-10-15

> 📌 A **docs / packaging revision** right after 0.6.3: parser and API **unchanged**,
> binary identical to 0.6.3. Shipped as its own version so the crates.io README facade
> covers both English and Chinese readers.

### Docs
- The crate README (`rust/README.md`) is now **bilingual** (each passage in English then
  Chinese). crates.io renders only the single file named by Cargo.toml `readme`, and
  versions are immutable — so a bilingual facade requires a new publish.

### Chore
- Repo trim: `git rm --cached` + `.gitignore` about 9 MB of **generated** `.sml`
  (`story/novel.sml` + 7×`story/novel.out.*`, `examples/micro/CH32V103xx.sml`); the
  sources (`story/gen_novel.py`, `CH32V103xx.svd`) stay and can regenerate them — no
  effect on the crate package.
- `llms.txt` (and its `site/static/` copy) release status updated to 0.6.4.

### Dependency sub-crates
- Unchanged: still the already-published alpha versions of `sml-parse` / `sml-codes` /
  `sml-feature` / `sml-contract` / `sml-lex` (shipped alongside 0.6.3).

### Debian packages
- Unchanged: the cdylib binary is byte-identical to 0.6.3, so `libswsml*_0.6.3` stays
  valid; not relabeled for a docs-only bump.

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
