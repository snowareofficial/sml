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

### Added — featag S8: the `!any` contract-level switch

- `any` means giving up type validation ⇒ **disabled by default**: a contract using `any` (or `[any]`)
  without `!any` reports `E-FEATURE-001` (same cause: the capability was not allowed), naming the
  contract and field and telling the user what to write.
- Writing **`!any`** in the contract body allows `any` **for that contract** (a **contract-level**
  switch, **not a feature bit**, §10 Q7); `null` inside `any` passes.
- **Both application paths share one rule**: the typed-block head and the in-block `@is` are both
  checked — otherwise the same contract would behave differently depending on how it is applied.
- Recursive contracts (A↔B) are deduplicated with `seen`, so they cannot loop.
- Gated by `type.featag`: with it off nothing is checked and v4 `any` keeps working.
- ⚠️ `!any` used to be collected as "a featag named `any`"; inside a contract body it is now a switch
  (and no longer lands in `featags`).

### Added — featag S7: `from-featag` source rule

- Field modifier **`from-featag`** (`FieldSpec.from_featag`): the key **may only be written by a
  featag target**; assigning it in the document is `E-CONTRACT-024`, reported with the **full field
  path** (e.g. `mid.inner.secret`).
- **The check runs before the featag fill**: afterwards, featag-written and document-written keys
  look identical, so "legal source" and "forged" could no longer be told apart — and the source is
  the whole point of the rule.
- **Nested contracts are checked** (recursing through `ContractRef`); ⚠️ **contract references
  inside arrays are not** (`[T]` needs per-element ownership info — deferred until there is a real need).
- **Two source tiers only**: default (anyone) / `from-featag` (featag only). There is deliberately
  **no `none` / `readonly` tier** — "nobody may write it" is expressed as "required but nobody
  provides it" or `optional` (§6.2 decision).
- Gated by `type.featag`: with the feature off the check does not run and documents may write such
  fields (v4 behaviour unchanged).

### Added — featag S6: target filling

- Arguments are written into the block along the declared `-> target` (§6.1 rules 1–5):
  ```sml
  @contract api { !cert: str -> tls.cert  !key: str -> tls.key }
  api cert["c.pem"] key["k.pem"], web { name: api }
  ;; ⇒ api: { tls: { cert: "c.pem", key: "k.pem" }, name: "api" }
  ```
- **An existing target is an error** (`E-CONTRACT-022`), **never silently skipped**: skipping would
  make "I thought it was written" the hardest bug to find. This is the language-level form of
  "a featag must not overwrite".
- **Parent objects are created automatically** (that is "insert", not "overwrite"); the contract-level
  switch **`!strict-paths`** requires them to exist and reports `E-CONTRACT-023` otherwise. A
  non-object in the middle of the path reports the same code.
- **`auto`** takes the declared `default`; without a `default` it is `E-CONTRACT-022`. ⚠️ Known
  limitation: arguments are already values by the time we see them, so a bareword `auto` and the
  quoted `"auto"` **cannot be told apart** (both are treated as `auto`). Fixing that requires
  `parse_array` to preserve "was this a bareword", which is a separate change.
- **No target ⇒ pure tag**: only `__featags` gets it, no key is written.
- **Ordering**: validation (S5) and filling (S6) now both run **before** contract application —
  otherwise fields that only a featag may supply would be reported as missing first (S7 relies on this).
- ⚠️ **Targets do not traverse arrays yet**: rejected at declaration time (`E-CONTRACT-020`).
  The spec contradicts itself here — §6.1 rule 6 (2026-11-04) allows `[n]`, §10 Q12 forbids it.
  **This needs a decision from the user**; this round follows the more conservative Q12, and the
  check lives in exactly two places.

### Added — featag S5: validation opens (undeclared ⇒ reject, argument checks)

- Every featag used at a block head must be declared in a contract **applied to that block**,
  otherwise `E-CONTRACT-021` (**new code**, per spec §10 Q3 — not `E-CONTRACT-012`). Diagnostics
  spell out `<contract>.<featag>` and list what the contract does declare.
- **No contract applied** is also an error: featag scope is contract-private (§5), so
  `server !critical, api { }` has nobody declaring it — the message says "declare it with `@contract`".
- Usage that contradicts the declaration is the same `E-CONTRACT-021` (different detail):
  arguments on a tag-only featag, a typed featag without `[…]`, or a non-array declaration given
  more than one argument.
- **Argument types reuse the field-type chain** (new public `sml_contract::check_value`): an array
  type (`!obey: [int]`) validates the **whole argument list**; any other type (`!cert: str`) takes
  exactly one argument and validates it. Type mismatches report the **existing** type codes
  (same cause as field type errors, so the code must not fork).
- **Callers can opt out**: `ParseOptions::apply_contracts(false)` skips featag validation too —
  the **same switch** as contract validation (otherwise "contracts off but featag still blocks"
  would be a sandbox hole). `__featags` is still emitted: it is syntax-layer output.
- ⚠️ **Behaviour change**: featags can no longer hang off a bare block with **no contract**
  (the S2/S3 tests were updated to declare one).
- ⚠️ **`!any` (S8) does not exist yet**: "any-shaped arguments" must be typed concretely or the
  opt-out switch must be used.

### Added — featag S4: declaration collection + duplicate check

- `sml_contract::Contract` gains **`featags: Vec<FeatagDecl>`** (**declaration order**), filled by the
  `@contract` body parser: `!critical` (plain tag) / `!obey: [int]` (typed) / `!tls: [str] -> tls`
  (typed with target) / `!cert: str -> tls.cert` (dotted target).
- Parameter types **reuse the field-spec parser** (`parse_field_spec`), so `[T]`, enums, contract
  references, modifiers and external types all work — S5 needs no second type parser.
- Targets are stored as **`sml_value::Path`** (reusing S1a): **object keys only, never arrays** —
  rejected at declaration time with `E-CONTRACT-020`, covering both bare `node[0]` (the lexer breaks
  at `[`) and quoted `"node[0]"`.
- **Duplicate declaration** ⇒ `E-CONTRACT-019`; **duplicates at the use site stay legal**
  (`!a !a` is kept in `__featags`, B4).
- Identifier rules (`lex.ident`) now cover **declarations**: featag names and field names are
  validated, plus **one script per contract** — Han + Kana count as one family (`設定ファイル` works),
  digits / `_` / `-` are ignored, mixing scripts is `E-LEX-007`.
- When contracts are stacked (`@is A B`), featag declarations are **unioned in declaration order**:
  the same name in two contracts is two different featags (scope is contract-private).
- ⚠️ Still **not** done: "undeclared ⇒ reject" and argument type checking (S5), target filling (S6).
- ⚠️ Collection has **no external observation point** yet (`Contract.featags` is a library type):
  this slice's tests pin "valid declarations are accepted" and "each invalid form reports its code".
  It only becomes visible when S5 consumes it — noted here so nobody assumes it was untested.

### Added — S1b: identifier whitelist, gated by `lex.ident`

- New module `sml_lex::identifiers`: `check_ident` / `IdentError`; entry point
  `@feature enable lex.ident` (**off by default**).
- **Allowed**: ASCII letters/digits, `_` `-`, CJK Han (U+4E00–U+9FFF), Kana (U+3040–U+30FF),
  Hangul syllables (U+AC00–U+D7AF).
- **Rejected** (six classes, all sharing `E-LEX-007`, told apart by the detail text):
  `.` `[` `]` (reserved by path syntax), whitespace / control chars (Cc), zero-width and
  bidi controls (Cf), **combining marks (Mn/Mc)**, Cyrillic / Greek / Armenian homoglyphs,
  fullwidth and compatibility characters.
- **NFC for free, zero dependencies**: only **precomposed** code points are accepted and all
  combining marks are rejected (`é` = U+00E9 is fine, `e`+U+0301 is not) — no normalisation
  library, no Unicode tables.
- Positions are reported in **characters**, not bytes (`端口.x` reports the dot at char 3).
- `lex.ident` is a **new bit (19th, second dotted feature)**; off ⇒ v4 behaviour, `a.b: 1` is
  still one key. Fragment-merge keys (`&base`) are not identifiers and are not validated.
- ⚠️ **Deliberate deviation** from the cross-slice rule "every slice lives behind `type.featag`":
  identifier rules have nothing to do with the `!` marker, and sharing the bit would mean
  "enabling featag silently changes every key name" with coarser rollback. Logged in the plan.
- ⚠️ **Per-contract single-script rule is not here**: it compares several identifiers of one
  contract, so it ships with S4.
- ⚠️ The "generate both range tables to five ends" item **only covers Rust this round**
  (other ends have no identifier validation yet; `impls: [ rust ]`). The generator waits until
  cross-end consistency is actually needed — five tables nobody reads are pure overhead.

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
